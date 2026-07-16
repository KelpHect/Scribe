use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::UNIX_EPOCH;

use regex::Regex;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{Addon, ScannerCacheRecord, Storage};

const MAX_SCAN_WORKERS: usize = 8;

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("addon path is not configured")]
    PathNotConfigured,
    #[error("failed to access addon files: {0}")]
    Io(#[from] std::io::Error),
}

pub struct Scanner {
    addon_path: RwLock<PathBuf>,
    addons: RwLock<HashMap<String, Addon>>,
    storage: Option<Arc<Storage>>,
}

impl Scanner {
    pub fn new(addon_path: impl Into<PathBuf>) -> Self {
        Self {
            addon_path: RwLock::new(addon_path.into()),
            addons: RwLock::new(HashMap::new()),
            storage: None,
        }
    }

    pub fn with_storage(mut self, storage: Arc<Storage>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub fn set_addon_path(&self, path: impl Into<PathBuf>) {
        *self.addon_path.write().expect("addon path lock poisoned") = path.into();
    }

    pub fn addon_path(&self) -> PathBuf {
        self.addon_path
            .read()
            .expect("addon path lock poisoned")
            .clone()
    }

    pub fn addons(&self) -> Vec<Addon> {
        let mut addons: Vec<_> = self
            .addons
            .read()
            .expect("addons lock poisoned")
            .values()
            .cloned()
            .collect();
        addons.sort_by(|a, b| a.folder_name.cmp(&b.folder_name));
        addons
    }

    pub fn scan(&self) -> Result<Vec<Addon>, ScanError> {
        let addon_path = self.addon_path();
        if addon_path.as_os_str().is_empty() {
            return Err(ScanError::PathNotConfigured);
        }

        let mut directories: Vec<_> = fs::read_dir(&addon_path)?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                entry
                    .file_type()
                    .ok()
                    .filter(|kind| kind.is_dir())
                    .map(|_| entry.path())
            })
            .collect();
        directories.sort();

        let worker_count = scan_worker_count(directories.len());
        if worker_count == 0 {
            self.addons.write().expect("addons lock poisoned").clear();
            return Ok(Vec::new());
        }

        let addon_path_text = addon_path.to_string_lossy();
        let cache_keys: Vec<_> = directories
            .iter()
            .map(|directory| {
                let folder = directory
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                format!("{addon_path_text}\u{0}{folder}")
            })
            .collect();
        let cached = self
            .storage
            .as_ref()
            .and_then(|storage| storage.scanner_records(&cache_keys).ok())
            .unwrap_or_default();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let results = std::sync::Mutex::new(Vec::with_capacity(directories.len()));
        let pending_records = std::sync::Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                scope.spawn(|| {
                    loop {
                        let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(directory) = directories.get(index) else {
                            break;
                        };
                        let Some(cache_key) = cache_keys.get(index) else {
                            continue;
                        };
                        let Ok(fingerprint) = directory_fingerprint(directory) else {
                            continue;
                        };
                        if let Some(record) = cached
                            .get(cache_key)
                            .filter(|record| record.fingerprint == fingerprint)
                        {
                            results
                                .lock()
                                .expect("scan results lock poisoned")
                                .push(record.addon.clone());
                            continue;
                        }
                        let Ok(Some(addon)) = scan_addon_directory(directory) else {
                            continue;
                        };
                        pending_records
                            .lock()
                            .expect("scanner cache records lock poisoned")
                            .push((
                                cache_key.clone(),
                                ScannerCacheRecord {
                                    fingerprint,
                                    addon: addon.clone(),
                                    ..ScannerCacheRecord::default()
                                },
                            ));
                        results
                            .lock()
                            .expect("scan results lock poisoned")
                            .push(addon);
                    }
                });
            }
        });
        if let Some(storage) = &self.storage {
            let pending_records = pending_records
                .into_inner()
                .expect("scanner cache records lock poisoned");
            let _ = storage.put_scanner_records(&pending_records);
        }

        let mut addons = results.into_inner().expect("scan results lock poisoned");
        addons.sort_by(|a, b| a.folder_name.cmp(&b.folder_name));
        let map = addons
            .iter()
            .cloned()
            .map(|addon| (addon.id.clone(), addon))
            .collect();
        *self.addons.write().expect("addons lock poisoned") = map;
        Ok(addons)
    }

    pub fn detect_path() -> Option<PathBuf> {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)?;
        detect_path_for(
            &home,
            std::env::consts::OS,
            |path| path.is_dir(),
            glob_one_drive,
        )
    }
}

pub fn parse_addon_file(path: &Path) -> Result<Addon, ScanError> {
    let folder_path = path.parent().unwrap_or_else(|| Path::new(""));
    let folder_name = folder_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    let file = fs::File::open(path)?;
    let mut addon = Addon {
        id: folder_name.clone(),
        folder_name: folder_name.clone(),
        path: folder_path.to_string_lossy().into_owned(),
        enabled: true,
        ..Addon::default()
    };
    let color = color_code_regex();

    for line in BufReader::new(file).lines() {
        let line = line?;
        let trimmed = line.trim();
        if !trimmed.starts_with("##") {
            continue;
        }
        let header = trimmed
            .strip_prefix("## ")
            .or_else(|| trimmed.strip_prefix("##"))
            .unwrap_or(trimmed);
        let Some((key, value)) = header.split_once(':') else {
            continue;
        };
        let value = color.replace_all(value.trim(), "$1").trim().to_owned();
        match key.trim().to_ascii_lowercase().as_str() {
            "title" => addon.title = value,
            "version" => addon.version = value,
            "author" => addon.author = value,
            "description" => addon.description = value,
            "dependson" => addon.depends_on = parse_list(&value),
            "pcdependson" => addon.depends_on.extend(parse_list(&value)),
            "consoledependson" => {}
            "optionaldependson" => addon.optional_depends_on = parse_list(&value),
            "savedvariables" => addon.saved_variables = parse_list(&value),
            "apiversion" => addon.api_version = value,
            "addonversion" => addon.add_on_version = value,
            "islibrary" => addon.is_library = value == "1" || value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }
    if addon.title.is_empty() {
        addon.title = folder_name;
    }
    Ok(addon)
}

fn color_code_regex() -> &'static Regex {
    static COLOR_CODE: OnceLock<Regex> = OnceLock::new();
    COLOR_CODE
        .get_or_init(|| Regex::new(r"\|c[0-9A-Fa-f]{6}([^|]*)\|r").expect("valid color-code regex"))
}

fn parse_list(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .take_while(|part| !part.starts_with(';'))
        .map(str::to_owned)
        .collect()
}

fn scan_addon_directory(directory: &Path) -> Result<Option<Addon>, ScanError> {
    let folder = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let mut entries: Vec<_> = fs::read_dir(directory)?.filter_map(Result::ok).collect();
    entries.sort_by_key(fs::DirEntry::file_name);

    for extension in ["addon", "txt"] {
        let canonical = format!("{folder}.{extension}");
        if let Some(path) = entries.iter().find_map(|entry| {
            entry
                .file_name()
                .to_str()
                .filter(|name| name.eq_ignore_ascii_case(&canonical))
                .map(|_| entry.path())
        }) && let Ok(addon) = parse_addon_file(&path)
        {
            return Ok(Some(addon));
        }
    }

    for entry in entries {
        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(true) {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let lower = name.to_ascii_lowercase();
        if name.starts_with('.') || (!lower.ends_with(".txt") && !lower.ends_with(".addon")) {
            continue;
        }
        return parse_addon_file(&entry.path()).map(Some);
    }
    Ok(None)
}

fn directory_fingerprint(directory: &Path) -> Result<String, ScanError> {
    let mut parts = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_ascii_lowercase();
        if name.starts_with('.') || (!lower.ends_with(".txt") && !lower.ends_with(".addon")) {
            continue;
        }
        let metadata = entry.metadata()?;
        let modified = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        parts.push((name, metadata.len(), modified));
    }
    parts.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Sha256::new();
    hasher.update(b"scribe-directory-fingerprint-v2\0");
    for (name, len, modified) in parts {
        hasher.update((name.len() as u64).to_le_bytes());
        hasher.update(name.as_bytes());
        hasher.update(len.to_le_bytes());
        hasher.update(modified.to_le_bytes());
    }
    Ok(hex_digest(hasher.finalize().as_slice()))
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

fn scan_worker_count(directory_count: usize) -> usize {
    if directory_count == 0 {
        return 0;
    }
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(2)
        .clamp(2, MAX_SCAN_WORKERS)
        .min(directory_count)
}

fn detect_path_for(
    home: &Path,
    os: &str,
    exists: impl Fn(&Path) -> bool,
    glob: impl Fn(&Path) -> Vec<PathBuf>,
) -> Option<PathBuf> {
    let eso = ["Documents", "Elder Scrolls Online"];
    let mut candidates = Vec::new();
    match os {
        "windows" => {
            for realm in ["live", "liveeu"] {
                candidates.push(home.join(eso[0]).join(eso[1]).join(realm).join("AddOns"));
            }
            for realm in ["live", "liveeu"] {
                candidates.push(
                    home.join("OneDrive")
                        .join(eso[0])
                        .join(eso[1])
                        .join(realm)
                        .join("AddOns"),
                );
            }
            for realm in ["live", "liveeu"] {
                candidates.extend(glob(
                    &home
                        .join("OneDrive*")
                        .join(eso[0])
                        .join(eso[1])
                        .join(realm)
                        .join("AddOns"),
                ));
            }
        }
        "macos" => candidates.push(home.join(eso[0]).join(eso[1]).join("live/AddOns")),
        "linux" => {
            candidates.push(home.join(".steam/steam/steamapps/compatdata/306130/pfx/drive_c/users/steamuser/Documents/Elder Scrolls Online/live/AddOns"));
            candidates.push(home.join(eso[0]).join(eso[1]).join("live/AddOns"));
        }
        _ => return None,
    }
    candidates.into_iter().find(|path| exists(path))
}

fn glob_one_drive(pattern: &Path) -> Vec<PathBuf> {
    let Some(home) = pattern.parent().and_then(|path| {
        path.ancestors()
            .find(|candidate| candidate.join("OneDrive").parent() == Some(*candidate))
    }) else {
        return Vec::new();
    };
    let Some(suffix) = pattern
        .components()
        .skip(home.components().count() + 1)
        .collect::<PathBuf>()
        .to_str()
        .map(str::to_owned)
    else {
        return Vec::new();
    };
    fs::read_dir(home)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("OneDrive"))
        .map(|entry| entry.path().join(&suffix))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_folder_named_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("LibLazyCrafting");
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("LLC.txt"), "## Version: 2.3\n").unwrap();
        fs::write(
            directory.join("LibLazyCrafting.addon"),
            "## Title: LibLazyCrafting\n## Version: 4.035\n## Author: Dolgubon\n",
        )
        .unwrap();
        let addons = Scanner::new(temp.path()).scan().unwrap();
        assert_eq!(addons[0].version, "4.035");
        assert_eq!(addons[0].author, "Dolgubon");
    }

    #[test]
    fn parses_metadata_dependencies_and_color_codes() {
        // Golden parity fixture from old_app/internal/scanner/scanner_test.go:
        // TestParseAddonFile_MetadataEdgeCases.
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("EdgeAddon");
        fs::create_dir(&directory).unwrap();
        let path = directory.join("EdgeAddon.txt");
        fs::write(
            &path,
            "## Title: |cFFAA00Colored Title|r\n\
             ## Version: |cFFFFFF1.2.3|r\n\
             ## Author: |c00FF00Author Name|r\n\
             ## Description: |c123456A description|r\n\
             ## DependsOn: LibRequired>=1.0 LibAnother<=2\n\
             ## PCDependsOn: LibPC LibPCVersion>=3\n\
             ## ConsoleDependsOn: ConsoleOnly\n\
             ## OptionalDependsOn: LibOptional LibOptionalVersion>=4\n\
             ## SavedVariables: EdgeSaved AccountWideSaved\n\
             ## APIVersion: 101046 101047\n\
             ## AddOnVersion: 42\n\
             ## IsLibrary: 1\n\n\
             EdgeAddon.lua\n",
        )
        .unwrap();
        let addon = parse_addon_file(&path).unwrap();
        assert_eq!(addon.title, "Colored Title");
        assert_eq!(addon.version, "1.2.3");
        assert_eq!(addon.author, "Author Name");
        assert_eq!(addon.description, "A description");
        assert_eq!(
            addon.depends_on,
            [
                "LibRequired>=1.0",
                "LibAnother<=2",
                "LibPC",
                "LibPCVersion>=3"
            ]
        );
        assert_eq!(
            addon.optional_depends_on,
            ["LibOptional", "LibOptionalVersion>=4"]
        );
        assert_eq!(addon.saved_variables, ["EdgeSaved", "AccountWideSaved"]);
        assert_eq!(addon.api_version, "101046 101047");
        assert_eq!(addon.add_on_version, "42");
        assert!(addon.is_library);
    }

    #[test]
    fn path_detection_preserves_candidate_precedence() {
        let home = PathBuf::from("C:/Users/Tester");
        let live = home.join("Documents/Elder Scrolls Online/live/AddOns");
        let live_eu = home.join("Documents/Elder Scrolls Online/liveeu/AddOns");
        let detected = detect_path_for(
            &home,
            "windows",
            |path| path == live || path == live_eu,
            |_| Vec::new(),
        );
        assert_eq!(detected, Some(live));
    }

    #[test]
    fn worker_count_is_bounded() {
        assert_eq!(scan_worker_count(0), 0);
        assert_eq!(scan_worker_count(1), 1);
        assert!(scan_worker_count(1_000) <= MAX_SCAN_WORKERS);
    }

    #[test]
    fn scanner_cache_batches_round_trip_and_invalidates_changed_manifests() {
        let temp = tempfile::tempdir().unwrap();
        let addons_path = temp.path().join("AddOns");
        let directory = addons_path.join("CachedAddon");
        fs::create_dir_all(&directory).unwrap();
        let manifest = directory.join("CachedAddon.txt");
        fs::write(&manifest, "## Title: Cached\n## Version: 1\n").unwrap();
        let storage = Arc::new(Storage::open(temp.path().join("cache.redb")).unwrap());
        let scanner = Scanner::new(&addons_path).with_storage(storage.clone());

        assert_eq!(scanner.scan().unwrap()[0].version, "1");
        let cache_key = format!("{}\u{0}CachedAddon", addons_path.to_string_lossy());
        assert!(storage.scanner_record(&cache_key).unwrap().is_some());

        fs::write(&manifest, "## Title: Cached\n## Version: 2\n").unwrap();
        assert_eq!(scanner.scan().unwrap()[0].version, "2");
    }
}
