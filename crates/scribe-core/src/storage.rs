use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use redb::{Builder, Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(feature = "rkyv-catalog")]
use crate::CatalogArchive;
use crate::{ApiFeeds, Catalog, CatalogIndex, InstallRecord, ScannerCacheRecord};

const SCHEMA_VERSION: u32 = 1;
pub const CACHE_TTL_SECONDS: i64 = 4 * 60 * 60;

const META: TableDefinition<&str, &[u8]> = TableDefinition::new("meta_v1");
const CATALOG: TableDefinition<u8, &[u8]> = TableDefinition::new("catalog_v1");
const SCANNER: TableDefinition<&str, &[u8]> = TableDefinition::new("scanner_v1");
const INSTALLS: TableDefinition<&str, &[u8]> = TableDefinition::new("installs_v1");

const CATALOG_KEY: u8 = 0;
const KEY_SCHEMA: &str = "schema_version";
const KEY_FETCHED_AT: &str = "catalog_fetched_at";
const KEY_HASH: &str = "catalog_hash";
const KEY_FEEDS: &str = "feed_urls";
const KEY_CODEC: &str = "catalog_codec";
#[cfg(feature = "rkyv-catalog")]
const CATALOG_CODEC: &str = "rkyv-validated-v1";
#[cfg(not(feature = "rkyv-catalog"))]
const CATALOG_CODEC: &str = "postcard-owned-v1";
const PAGE_CACHE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage directory is unavailable")]
    MissingParent,
    #[error("storage I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("redb failed: {0}")]
    Database(#[from] redb::Error),
    #[error("redb database failed to open: {0}")]
    Open(#[from] redb::DatabaseError),
    #[error("redb transaction failed: {0}")]
    Transaction(#[from] redb::TransactionError),
    #[error("redb table operation failed: {0}")]
    Table(#[from] redb::TableError),
    #[error("redb storage operation failed: {0}")]
    Storage(#[from] redb::StorageError),
    #[error("redb commit failed: {0}")]
    Commit(#[from] redb::CommitError),
    #[error("unsupported storage schema {found}; expected {expected}")]
    SchemaMismatch { found: u32, expected: u32 },
    #[error("cached data is invalid: {0}")]
    Decode(#[from] postcard::Error),
    #[error("cached catalog archive is invalid: {0}")]
    Archive(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    Written,
    Unchanged,
}

#[derive(Clone)]
pub struct CacheLoad {
    pub catalog: Arc<CatalogIndex>,
    pub feed_urls: Option<ApiFeeds>,
    pub fetched_at: i64,
    pub stale: bool,
    pub hash: String,
}

pub struct Storage {
    database: Database,
    path: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RebuildOutcome {
    pub retained_database: Option<PathBuf>,
}

impl Storage {
    pub fn open_default() -> Result<Self, StorageError> {
        let directory =
            crate::settings::app_config_directory().ok_or(StorageError::MissingParent)?;
        Self::open(directory.join("scribe.redb"))
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref();
        let parent = path.parent().ok_or(StorageError::MissingParent)?;
        fs::create_dir_all(parent)?;
        let mut builder = Builder::new();
        builder.set_cache_size(PAGE_CACHE_BYTES);
        let database = if path.exists() {
            builder.open(path)?
        } else {
            builder.create(path)?
        };
        let storage = Self {
            database,
            path: path.to_owned(),
        };
        storage.initialize()?;
        Ok(storage)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn rebuild_default_reconstructible() -> Result<RebuildOutcome, StorageError> {
        let directory =
            crate::settings::app_config_directory().ok_or(StorageError::MissingParent)?;
        Self::rebuild_reconstructible(directory.join("scribe.redb"))
    }

    pub fn rebuild_reconstructible(path: impl AsRef<Path>) -> Result<RebuildOutcome, StorageError> {
        let path = path.as_ref();
        match Database::create(path) {
            Ok(database) => {
                let write = database.begin_write()?;
                write.delete_table(CATALOG)?;
                write.delete_table(SCANNER)?;
                write.delete_table(META)?;
                {
                    let mut meta = write.open_table(META)?;
                    let schema = encode(&SCHEMA_VERSION)?;
                    meta.insert(KEY_SCHEMA, schema.as_slice())?;
                    write.open_table(CATALOG)?;
                    write.open_table(SCANNER)?;
                    write.open_table(INSTALLS)?;
                }
                write.commit()?;
                Ok(RebuildOutcome::default())
            }
            Err(error) => {
                if !path.exists() {
                    return Err(error.into());
                }
                let file_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("scribe.redb");
                let retained = path.with_file_name(format!(
                    "{file_name}.failed-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                ));
                fs::rename(path, &retained)?;
                if let Err(create_error) = Self::open(path) {
                    let _ = fs::rename(&retained, path);
                    return Err(create_error);
                }
                Ok(RebuildOutcome {
                    retained_database: Some(retained),
                })
            }
        }
    }

    fn initialize(&self) -> Result<(), StorageError> {
        if self.has_complete_schema()? {
            return Ok(());
        }

        let write = self.database.begin_write()?;
        {
            let mut meta = write.open_table(META)?;
            let found = meta
                .get(KEY_SCHEMA)?
                .map(|value| decode::<u32>(value.value()))
                .transpose()?;
            match found {
                Some(found) if found != SCHEMA_VERSION => {
                    return Err(StorageError::SchemaMismatch {
                        found,
                        expected: SCHEMA_VERSION,
                    });
                }
                Some(_) => {}
                None => {
                    let bytes = encode(&SCHEMA_VERSION)?;
                    meta.insert(KEY_SCHEMA, bytes.as_slice())?;
                }
            }
            write.open_table(CATALOG)?;
            write.open_table(SCANNER)?;
            write.open_table(INSTALLS)?;
        }
        write.commit()?;
        Ok(())
    }

    fn has_complete_schema(&self) -> Result<bool, StorageError> {
        let read = self.database.begin_read()?;
        let meta = match read.open_table(META) {
            Ok(table) => table,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let found = meta
            .get(KEY_SCHEMA)?
            .map(|value| decode::<u32>(value.value()))
            .transpose()?;
        match found {
            Some(found) if found != SCHEMA_VERSION => {
                return Err(StorageError::SchemaMismatch {
                    found,
                    expected: SCHEMA_VERSION,
                });
            }
            None => return Ok(false),
            Some(_) => {}
        }

        Ok(true)
    }

    pub fn load_catalog(&self, now_unix: i64) -> Result<Option<CacheLoad>, StorageError> {
        let read = self.database.begin_read()?;
        let catalog_table = read.open_table(CATALOG)?;
        let Some(snapshot) = catalog_table.get(CATALOG_KEY)? else {
            return Ok(None);
        };
        let meta = read.open_table(META)?;
        if read_meta::<String>(&meta, KEY_CODEC)?.as_deref() != Some(CATALOG_CODEC) {
            return Ok(None);
        }
        #[cfg(feature = "rkyv-catalog")]
        let catalog = {
            let archive = CatalogArchive::from_bytes(snapshot.value())
                .map_err(|error| StorageError::Archive(error.to_string()))?;
            Arc::new(CatalogIndex::from_archive(Arc::new(archive)))
        };
        #[cfg(not(feature = "rkyv-catalog"))]
        let catalog = Arc::new(CatalogIndex::new(Arc::new(decode(snapshot.value())?)));
        let fetched_at = read_meta::<i64>(&meta, KEY_FETCHED_AT)?.unwrap_or_default();
        let hash = read_meta::<String>(&meta, KEY_HASH)?.unwrap_or_default();
        let feed_urls = read_meta::<ApiFeeds>(&meta, KEY_FEEDS)?;
        Ok(Some(CacheLoad {
            catalog,
            feed_urls,
            fetched_at,
            stale: now_unix.saturating_sub(fetched_at) >= CACHE_TTL_SECONDS,
            hash,
        }))
    }

    pub fn save_catalog(
        &self,
        catalog: &Catalog,
        fetched_at: i64,
    ) -> Result<SaveOutcome, StorageError> {
        self.save_catalog_and_feeds(catalog, None, fetched_at)
    }

    pub fn save_catalog_and_feeds(
        &self,
        catalog: &Catalog,
        feed_urls: Option<&ApiFeeds>,
        fetched_at: i64,
    ) -> Result<SaveOutcome, StorageError> {
        let hash = normalized_catalog_hash(catalog, feed_urls);
        let write = self.database.begin_write()?;
        let outcome;
        {
            let mut meta = write.open_table(META)?;
            let existing_hash = read_meta::<String>(&meta, KEY_HASH)?;
            let existing_codec = read_meta::<String>(&meta, KEY_CODEC)?;
            let fetched = encode(&fetched_at)?;
            meta.insert(KEY_FETCHED_AT, fetched.as_slice())?;
            if let Some(feed_urls) = feed_urls {
                let feeds = encode(feed_urls)?;
                meta.insert(KEY_FEEDS, feeds.as_slice())?;
            }
            if existing_hash.as_deref() == Some(hash.as_str())
                && existing_codec.as_deref() == Some(CATALOG_CODEC)
            {
                outcome = SaveOutcome::Unchanged;
            } else {
                let snapshot = encode_catalog(catalog)?;
                let mut table = write.open_table(CATALOG)?;
                table.insert(CATALOG_KEY, snapshot.as_slice())?;
                let hash_bytes = encode(&hash)?;
                meta.insert(KEY_HASH, hash_bytes.as_slice())?;
                let codec_bytes = encode(&CATALOG_CODEC)?;
                meta.insert(KEY_CODEC, codec_bytes.as_slice())?;
                outcome = SaveOutcome::Written;
            }
        }
        write.commit()?;
        Ok(outcome)
    }

    pub fn scanner_record(&self, key: &str) -> Result<Option<ScannerCacheRecord>, StorageError> {
        self.read_record(SCANNER, key)
    }

    pub fn put_scanner_record(
        &self,
        key: &str,
        record: &ScannerCacheRecord,
    ) -> Result<(), StorageError> {
        self.write_record(SCANNER, key, record)
    }

    pub fn scanner_records(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, ScannerCacheRecord>, StorageError> {
        let read = self.database.begin_read()?;
        let table = read.open_table(SCANNER)?;
        let mut records = HashMap::with_capacity(keys.len());
        for key in keys {
            if let Some(value) = table.get(key.as_str())? {
                records.insert(key.clone(), decode(value.value())?);
            }
        }
        Ok(records)
    }

    pub fn put_scanner_records(
        &self,
        records: &[(String, ScannerCacheRecord)],
    ) -> Result<(), StorageError> {
        if records.is_empty() {
            return Ok(());
        }
        let encoded = records
            .iter()
            .map(|(key, record)| Ok((key.as_str(), encode(record)?)))
            .collect::<Result<Vec<_>, postcard::Error>>()?;
        let write = self.database.begin_write()?;
        {
            let mut table = write.open_table(SCANNER)?;
            for (key, bytes) in encoded {
                table.insert(key, bytes.as_slice())?;
            }
        }
        write.commit()?;
        Ok(())
    }

    pub fn install_record(&self, uid: &str) -> Result<Option<InstallRecord>, StorageError> {
        self.read_record(INSTALLS, uid)
    }

    pub fn install_records(&self, uids: &[String]) -> Result<Vec<InstallRecord>, StorageError> {
        let read = self.database.begin_read()?;
        let table = read.open_table(INSTALLS)?;
        let mut records = Vec::new();
        for uid in uids {
            if let Some(value) = table.get(uid.as_str())? {
                records.push(decode(value.value())?);
            }
        }
        Ok(records)
    }

    pub fn put_install_record(&self, record: &InstallRecord) -> Result<(), StorageError> {
        self.write_record(INSTALLS, &record.uid, record)
    }

    fn read_record<T: DeserializeOwned>(
        &self,
        definition: TableDefinition<&str, &[u8]>,
        key: &str,
    ) -> Result<Option<T>, StorageError> {
        let read = self.database.begin_read()?;
        let table = read.open_table(definition)?;
        Ok(table
            .get(key)?
            .map(|value| decode(value.value()))
            .transpose()?)
    }

    fn write_record<T: Serialize>(
        &self,
        definition: TableDefinition<&str, &[u8]>,
        key: &str,
        value: &T,
    ) -> Result<(), StorageError> {
        let bytes = encode(value)?;
        let write = self.database.begin_write()?;
        {
            let mut table = write.open_table(definition)?;
            table.insert(key, bytes.as_slice())?;
        }
        write.commit()?;
        Ok(())
    }
}

fn read_meta<T: DeserializeOwned>(
    table: &impl ReadableTable<&'static str, &'static [u8]>,
    key: &str,
) -> Result<Option<T>, StorageError> {
    Ok(table
        .get(key)?
        .map(|value| decode(value.value()))
        .transpose()?)
}

fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_allocvec(value)
}

#[cfg(feature = "rkyv-catalog")]
fn encode_catalog(catalog: &Catalog) -> Result<rkyv::util::AlignedVec<16>, StorageError> {
    rkyv::to_bytes::<rkyv::rancor::Error>(catalog)
        .map_err(|error| StorageError::Archive(error.to_string()))
}

#[cfg(not(feature = "rkyv-catalog"))]
fn encode_catalog(catalog: &Catalog) -> Result<Vec<u8>, StorageError> {
    Ok(encode(catalog)?)
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, postcard::Error> {
    postcard::from_bytes(bytes)
}

fn sha256_hex(digest: impl AsRef<[u8]>) -> String {
    let digest = digest.as_ref();
    let mut result = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

fn normalized_catalog_hash(catalog: &Catalog, feed_urls: Option<&ApiFeeds>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"scribe-normalized-catalog-v2\0");
    hash_feed_urls(&mut hasher, feed_urls);

    let mut addons: Vec<_> = catalog.addons.iter().collect();
    addons.sort_unstable_by(|left, right| left.uid.cmp(&right.uid));
    hash_len(&mut hasher, addons.len());
    for addon in addons {
        hash_str(&mut hasher, &addon.uid);
        hash_str(&mut hasher, &addon.category_id);
        hash_str(&mut hasher, &addon.ui_name);
        hash_str(&mut hasher, &addon.ui_author_name);
        hash_str(&mut hasher, &addon.ui_date);
        hash_str(&mut hasher, &addon.ui_version);
        hash_strs(&mut hasher, &addon.ui_dirs);
        hash_str(&mut hasher, &addon.ui_file_info_url);
        hasher.update(addon.ui_download_total.to_le_bytes());
        hasher.update(addon.ui_download_monthly.to_le_bytes());
        hasher.update(addon.ui_favorite_total.to_le_bytes());
        hash_strs(&mut hasher, &addon.ui_img_thumbs);
        hash_strs(&mut hasher, &addon.ui_imgs);
        hash_len(&mut hasher, addon.compatabilities.len());
        for version in &addon.compatabilities {
            hash_str(&mut hasher, &version.version);
            hash_str(&mut hasher, &version.name);
        }
        hash_strs(&mut hasher, &addon.siblings);
    }

    let mut categories: Vec<_> = catalog.categories.iter().collect();
    categories.sort_unstable_by(|left, right| left.id.cmp(&right.id));
    hash_len(&mut hasher, categories.len());
    for category in categories {
        hash_str(&mut hasher, &category.id);
        hash_str(&mut hasher, &category.name);
        hash_str(&mut hasher, &category.icon_url);
        hash_str(&mut hasher, &category.parent_id);
        hash_strs(&mut hasher, &category.parent_ids);
        hasher.update(category.count.to_le_bytes());
    }
    sha256_hex(hasher.finalize())
}

fn hash_feed_urls(hasher: &mut Sha256, feeds: Option<&ApiFeeds>) {
    hasher.update([u8::from(feeds.is_some())]);
    if let Some(feeds) = feeds {
        hash_str(hasher, &feeds.file_list);
        hash_str(hasher, &feeds.file_details);
        hash_str(hasher, &feeds.category_list);
        hash_str(hasher, &feeds.list_files);
    }
}

fn hash_strs<T: AsRef<str>>(hasher: &mut Sha256, values: &[T]) {
    hash_len(hasher, values.len());
    for value in values {
        hash_str(hasher, value.as_ref());
    }
}

fn hash_str(hasher: &mut Sha256, value: &str) {
    hash_len(hasher, value.len());
    hasher.update(value.as_bytes());
}

fn hash_len(hasher: &mut Sha256, len: usize) {
    hasher.update((len as u64).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RemoteAddon;
    use serde::Serialize;

    fn catalog() -> Catalog {
        Catalog {
            addons: vec![RemoteAddon {
                uid: "7".into(),
                ui_name: "Test Addon".into(),
                ..RemoteAddon::default()
            }],
            categories: vec![],
        }
    }

    #[test]
    fn catalog_round_trip_and_unchanged_save() {
        let temp = tempfile::tempdir().unwrap();
        let storage = Storage::open(temp.path().join("scribe.redb")).unwrap();
        assert_eq!(
            storage.save_catalog(&catalog(), 100).unwrap(),
            SaveOutcome::Written
        );
        assert_eq!(
            storage.save_catalog(&catalog(), 200).unwrap(),
            SaveOutcome::Unchanged
        );
        let loaded = storage.load_catalog(201).unwrap().unwrap();
        assert_eq!(loaded.catalog.addon(0).unwrap().uid, "7");
        assert_eq!(loaded.fetched_at, 200);
        assert!(!loaded.stale);
    }

    #[test]
    fn smol_catalog_fields_decode_legacy_string_snapshots_without_a_schema_change() {
        #[derive(Serialize)]
        struct LegacyCatalog {
            addons: Vec<LegacyAddon>,
            categories: Vec<LegacyCategory>,
        }
        #[derive(Serialize)]
        struct LegacyAddon {
            uid: String,
            category_id: String,
            ui_name: String,
            ui_author_name: String,
            ui_date: String,
            ui_version: String,
            ui_dirs: Vec<String>,
            ui_file_info_url: String,
            ui_download_total: i64,
            ui_download_monthly: i64,
            ui_favorite_total: i64,
            ui_img_thumbs: Vec<String>,
            ui_imgs: Vec<String>,
            compatabilities: Vec<LegacyGameVersion>,
            siblings: Vec<String>,
        }
        #[derive(Serialize)]
        struct LegacyGameVersion {
            version: String,
            name: String,
        }
        #[derive(Serialize)]
        struct LegacyCategory {
            id: String,
            name: String,
            icon_url: String,
            parent_id: String,
            parent_ids: Vec<String>,
            count: i32,
        }

        let legacy = LegacyCatalog {
            addons: vec![LegacyAddon {
                uid: "7".into(),
                category_id: "1".into(),
                ui_name: "Legacy".into(),
                ui_author_name: "Author".into(),
                ui_date: "2026-07-14".into(),
                ui_version: "1.0".into(),
                ui_dirs: vec!["Legacy".into()],
                ui_file_info_url: "https://example.invalid/7".into(),
                ui_download_total: 1,
                ui_download_monthly: 2,
                ui_favorite_total: 3,
                ui_img_thumbs: vec!["thumb".into()],
                ui_imgs: vec!["image".into()],
                compatabilities: vec![LegacyGameVersion {
                    version: "101047".into(),
                    name: "ESO".into(),
                }],
                siblings: vec!["8".into()],
            }],
            categories: vec![LegacyCategory {
                id: "1".into(),
                name: "Category".into(),
                icon_url: "icon".into(),
                parent_id: String::new(),
                parent_ids: Vec::new(),
                count: 1,
            }],
        };
        let legacy_bytes = postcard::to_allocvec(&legacy).unwrap();
        let decoded: Catalog = postcard::from_bytes(&legacy_bytes).unwrap();
        assert_eq!(decoded.addons[0].uid, "7");
        assert_eq!(postcard::to_allocvec(&decoded).unwrap(), legacy_bytes);
    }

    #[test]
    fn catalog_hash_ignores_addon_and_category_order() {
        let temp = tempfile::tempdir().unwrap();
        let storage = Storage::open(temp.path().join("scribe.redb")).unwrap();
        let first = Catalog {
            addons: vec![
                RemoteAddon {
                    uid: "2".into(),
                    ui_name: "Second".into(),
                    ..RemoteAddon::default()
                },
                RemoteAddon {
                    uid: "1".into(),
                    ui_name: "First".into(),
                    ..RemoteAddon::default()
                },
            ],
            categories: vec![
                crate::Category {
                    id: "b".into(),
                    ..crate::Category::default()
                },
                crate::Category {
                    id: "a".into(),
                    ..crate::Category::default()
                },
            ],
        };
        let mut reordered = first.clone();
        reordered.addons.reverse();
        reordered.categories.reverse();

        assert_eq!(
            storage.save_catalog(&first, 100).unwrap(),
            SaveOutcome::Written
        );
        assert_eq!(
            storage.save_catalog(&reordered, 200).unwrap(),
            SaveOutcome::Unchanged
        );
        let loaded = storage.load_catalog(201).unwrap().unwrap();
        assert_eq!(loaded.catalog.addon(0).unwrap().uid, "2");
        assert_eq!(loaded.fetched_at, 200);
    }

    #[test]
    fn scanner_and_install_records_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let storage = Storage::open(temp.path().join("scribe.redb")).unwrap();
        let scanner = ScannerCacheRecord {
            fingerprint: "abc".into(),
            ..ScannerCacheRecord::default()
        };
        storage.put_scanner_record("path/folder", &scanner).unwrap();
        assert_eq!(
            storage
                .scanner_record("path/folder")
                .unwrap()
                .unwrap()
                .fingerprint,
            "abc"
        );
        let install = InstallRecord {
            uid: "10".into(),
            md5: "not-security".into(),
        };
        storage.put_install_record(&install).unwrap();
        assert_eq!(storage.install_record("10").unwrap().unwrap(), install);
    }

    #[test]
    fn reopens_committed_catalog() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("scribe.redb");
        {
            let storage = Storage::open(&path).unwrap();
            storage.save_catalog(&catalog(), 100).unwrap();
        }
        let reopened = Storage::open(&path).unwrap();
        assert_eq!(
            reopened
                .load_catalog(101)
                .unwrap()
                .unwrap()
                .catalog
                .addon(0)
                .unwrap()
                .uid,
            "7"
        );
    }

    #[test]
    fn aborted_transaction_does_not_publish_partial_catalog() {
        let temp = tempfile::tempdir().unwrap();
        let storage = Storage::open(temp.path().join("scribe.redb")).unwrap();
        let write = storage.database.begin_write().unwrap();
        {
            let mut table = write.open_table(CATALOG).unwrap();
            let bytes = encode(&catalog()).unwrap();
            table.insert(CATALOG_KEY, bytes.as_slice()).unwrap();
        }
        write.abort().unwrap();
        assert!(storage.load_catalog(100).unwrap().is_none());
    }

    #[test]
    fn schema_mismatch_is_retained_and_reported() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("scribe.redb");
        {
            let storage = Storage::open(&path).unwrap();
            let write = storage.database.begin_write().unwrap();
            {
                let mut meta = write.open_table(META).unwrap();
                let version = encode(&(SCHEMA_VERSION + 1)).unwrap();
                meta.insert(KEY_SCHEMA, version.as_slice()).unwrap();
            }
            write.commit().unwrap();
        }
        assert!(matches!(
            Storage::open(&path),
            Err(StorageError::SchemaMismatch { .. })
        ));
        let database = Database::create(&path).unwrap();
        let read = database.begin_read().unwrap();
        let meta = read.open_table(META).unwrap();
        assert_eq!(
            read_meta::<u32>(&meta, KEY_SCHEMA).unwrap(),
            Some(SCHEMA_VERSION + 1)
        );
    }

    #[test]
    fn explicit_rebuild_clears_reconstructible_data_but_preserves_installs() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("scribe.redb");
        {
            let storage = Storage::open(&path).unwrap();
            storage.save_catalog(&catalog(), 100).unwrap();
            storage
                .put_scanner_record(
                    "folder",
                    &ScannerCacheRecord {
                        fingerprint: "cached".into(),
                        ..ScannerCacheRecord::default()
                    },
                )
                .unwrap();
            storage
                .put_install_record(&InstallRecord {
                    uid: "7".into(),
                    md5: "integrity".into(),
                })
                .unwrap();
            let write = storage.database.begin_write().unwrap();
            {
                let mut meta = write.open_table(META).unwrap();
                let version = encode(&(SCHEMA_VERSION + 1)).unwrap();
                meta.insert(KEY_SCHEMA, version.as_slice()).unwrap();
            }
            write.commit().unwrap();
        }

        assert!(matches!(
            Storage::open(&path),
            Err(StorageError::SchemaMismatch { .. })
        ));
        let outcome = Storage::rebuild_reconstructible(&path).unwrap();
        assert!(outcome.retained_database.is_none());
        let rebuilt = Storage::open(&path).unwrap();
        assert!(rebuilt.load_catalog(101).unwrap().is_none());
        assert!(rebuilt.scanner_record("folder").unwrap().is_none());
        assert_eq!(
            rebuilt.install_record("7").unwrap().unwrap().md5,
            "integrity"
        );
    }

    #[test]
    fn invalid_database_is_never_overwritten() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("scribe.redb");
        let invalid = b"not a redb database";
        fs::write(&path, invalid).unwrap();
        assert!(Storage::open(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), invalid);
    }

    #[test]
    fn explicit_rebuild_retains_an_unreadable_database_as_a_backup() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("scribe.redb");
        let invalid = b"not a redb database";
        fs::write(&path, invalid).unwrap();
        let outcome = Storage::rebuild_reconstructible(&path).unwrap();
        let retained = outcome.retained_database.expect("retained path");
        assert_eq!(fs::read(retained).unwrap(), invalid);
        assert!(Storage::open(path).is_ok());
    }

    #[test]
    fn concurrent_reads_observe_only_committed_catalogs() {
        let temp = tempfile::tempdir().unwrap();
        let storage = Arc::new(Storage::open(temp.path().join("scribe.redb")).unwrap());
        storage.save_catalog(&catalog(), 0).unwrap();
        let readers: Vec<_> = (0..4)
            .map(|_| {
                let storage = storage.clone();
                std::thread::spawn(move || {
                    for now in 1..100 {
                        let loaded = storage.load_catalog(now).unwrap().unwrap();
                        assert_eq!(loaded.catalog.len(), 1);
                        assert_eq!(loaded.catalog.addon(0).unwrap().uid, "7");
                    }
                })
            })
            .collect();
        let writer = {
            let storage = storage.clone();
            std::thread::spawn(move || {
                for now in 1..25 {
                    storage.save_catalog(&catalog(), now).unwrap();
                }
            })
        };
        for reader in readers {
            reader.join().unwrap();
        }
        writer.join().unwrap();
    }

    #[cfg(feature = "rkyv-catalog")]
    #[test]
    fn invalid_catalog_archive_is_reported_without_replacing_the_database() {
        let temp = tempfile::tempdir().unwrap();
        let storage = Storage::open(temp.path().join("cache.redb")).unwrap();
        let write = storage.database.begin_write().unwrap();
        {
            let mut catalog = write.open_table(CATALOG).unwrap();
            catalog.insert(CATALOG_KEY, &[1_u8, 2, 3][..]).unwrap();
            let mut meta = write.open_table(META).unwrap();
            let codec = encode(&CATALOG_CODEC).unwrap();
            meta.insert(KEY_CODEC, codec.as_slice()).unwrap();
        }
        write.commit().unwrap();

        assert!(matches!(
            storage.load_catalog(0),
            Err(StorageError::Archive(_))
        ));
        assert!(storage.path().exists());
    }

    #[cfg(feature = "rkyv-catalog")]
    #[test]
    fn legacy_postcard_catalog_is_a_reconstructible_cache_miss() {
        let temp = tempfile::tempdir().unwrap();
        let storage = Storage::open(temp.path().join("cache.redb")).unwrap();
        let write = storage.database.begin_write().unwrap();
        {
            let bytes = encode(&catalog()).unwrap();
            let mut table = write.open_table(CATALOG).unwrap();
            table.insert(CATALOG_KEY, bytes.as_slice()).unwrap();
        }
        write.commit().unwrap();

        assert!(storage.load_catalog(0).unwrap().is_none());
    }
}
