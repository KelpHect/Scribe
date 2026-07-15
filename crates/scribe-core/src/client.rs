use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_io::Timer;
use event_listener::Event;
use futures::{AsyncReadExt, FutureExt, pin_mut, select_biased};
use http_client::{AsyncBody, HttpClient, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

use crate::{ApiFeeds, CacheLoad, Catalog, Category, GameConfig, GameVersion, RemoteAddon};
use crate::{RemoteAddonDetails, SaveOutcome, Storage, storage::StorageError};

pub const BOOTSTRAP_URL: &str = "https://api.mmoui.com/v3/globalconfig.json";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const DETAILS_CACHE_CAPACITY: usize = 24;
const DETAILS_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<CancellationState>);

#[derive(Default)]
struct CancellationState {
    cancelled: AtomicBool,
    event: Event,
}

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        self.0.event.notify(usize::MAX);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }

    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        let listener = self.0.event.listen();
        if self.is_cancelled() {
            return;
        }
        listener.await;
    }
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("request cancelled")]
    Cancelled,
    #[error("GET {url} timed out after 30 seconds")]
    Timeout { url: String },
    #[error("GET {url} failed: {message}")]
    Transport { url: String, message: String },
    #[error("GET {url} returned {status}")]
    Status { url: String, status: u16 },
    #[error("response from {url} exceeded 64 MiB")]
    Oversized { url: String },
    #[error("decode JSON from {url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("MMOUI service is inactive (status={0:?})")]
    InactiveService(String),
    #[error("ESO game is inactive on MMOUI (status={0:?})")]
    InactiveGame(String),
    #[error("ESO game config URL not found in globalconfig")]
    MissingGame,
    #[error("client not initialized; call init first")]
    NotInitialized,
}

#[derive(Debug, Error)]
pub enum CatalogServiceError {
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

pub struct EsouiClient {
    http: Arc<dyn HttpClient>,
    bootstrap_url: String,
    feeds: RwLock<Option<ApiFeeds>>,
    retry_delays: [Duration; 3],
}

impl EsouiClient {
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self::with_bootstrap(http, BOOTSTRAP_URL)
    }

    pub fn with_bootstrap(http: Arc<dyn HttpClient>, bootstrap_url: impl Into<String>) -> Self {
        Self {
            http,
            bootstrap_url: bootstrap_url.into(),
            feeds: RwLock::new(None),
            retry_delays: RETRY_DELAYS,
        }
    }

    pub async fn init(&self, cancel: &CancellationToken) -> Result<ApiFeeds, ClientError> {
        let global: ApiGlobalConfig = self.get_json(&self.bootstrap_url, cancel).await?;
        if !global.active.status.is_empty() && global.active.status != "1" {
            return Err(ClientError::InactiveService(global.active.status));
        }
        let game = global
            .games
            .into_iter()
            .find(|game| game.game_id == "ESO")
            .ok_or(ClientError::MissingGame)?;
        if !game.active.status.is_empty() && game.active.status != "1" {
            return Err(ClientError::InactiveGame(game.active.status));
        }

        let config: GameConfig = self.get_json(&game.game_config, cancel).await?;
        let feeds = config.api_feeds;
        *self.feeds.write().expect("feed lock poisoned") = Some(feeds.clone());
        Ok(feeds)
    }

    pub fn feed_urls(&self) -> Option<ApiFeeds> {
        self.feeds.read().expect("feed lock poisoned").clone()
    }

    pub async fn fetch_addon_list(
        &self,
        cancel: &CancellationToken,
    ) -> Result<Vec<RemoteAddon>, ClientError> {
        let url = self.feeds()?.file_list;
        let addons: Vec<ApiRemoteAddon> = self.get_json(&url, cancel).await?;
        Ok(addons.into_iter().map(RemoteAddon::from).collect())
    }

    pub async fn fetch_categories(
        &self,
        cancel: &CancellationToken,
    ) -> Result<Vec<Category>, ClientError> {
        let url = self.feeds()?.category_list;
        let categories: Vec<ApiCategory> = self.get_json(&url, cancel).await?;
        Ok(categories.into_iter().map(Category::from).collect())
    }

    pub async fn fetch_addon_details(
        &self,
        uids: &[String],
        cancel: &CancellationToken,
    ) -> Result<Vec<RemoteAddonDetails>, ClientError> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{}{}.json", self.feeds()?.file_details, uids.join(","));
        let details: Vec<ApiRemoteAddonDetails> = self.get_json(&url, cancel).await?;
        Ok(details.into_iter().map(RemoteAddonDetails::from).collect())
    }

    fn feeds(&self) -> Result<ApiFeeds, ClientError> {
        self.feed_urls().ok_or(ClientError::NotInitialized)
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        cancel: &CancellationToken,
    ) -> Result<T, ClientError> {
        let mut last_error = None;
        for attempt in 0..=self.retry_delays.len() {
            if attempt > 0 {
                wait_or_cancel(self.retry_delays[attempt - 1], cancel).await?;
            }

            let response = match with_cancel_and_timeout(
                self.http.get(url, AsyncBody::empty(), true),
                url,
                cancel,
            )
            .await
            {
                Ok(response) => response,
                Err(error @ ClientError::Cancelled) => return Err(error),
                Err(error) => {
                    last_error = Some(error);
                    continue;
                }
            };

            let status = response.status();
            if status.is_server_error() {
                last_error = Some(ClientError::Status {
                    url: url.into(),
                    status: status.as_u16(),
                });
                continue;
            }
            if status != StatusCode::OK {
                return Err(ClientError::Status {
                    url: url.into(),
                    status: status.as_u16(),
                });
            }

            let bytes =
                match read_response_body(response.into_body(), url, cancel, MAX_RESPONSE_BYTES)
                    .await
                {
                    Ok(bytes) => bytes,
                    Err(error @ (ClientError::Cancelled | ClientError::Oversized { .. })) => {
                        return Err(error);
                    }
                    Err(error) => {
                        last_error = Some(error);
                        continue;
                    }
                };
            return serde_json::from_slice(&bytes).map_err(|source| ClientError::Decode {
                url: url.into(),
                source,
            });
        }
        Err(last_error.unwrap_or_else(|| ClientError::Transport {
            url: url.into(),
            message: "request failed without an error".into(),
        }))
    }
}

async fn read_response_body<R>(
    mut body: R,
    url: &str,
    cancel: &CancellationToken,
    limit: usize,
) -> Result<Vec<u8>, ClientError>
where
    R: futures::io::AsyncRead + Unpin,
{
    let url_for_read = url.to_owned();
    let operation = async move {
        let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
        let mut chunk = [0_u8; 16 * 1024];
        loop {
            let count = body
                .read(&mut chunk)
                .await
                .map_err(|error| ClientError::Transport {
                    url: url_for_read.clone(),
                    message: error.to_string(),
                })?;
            if count == 0 {
                return Ok(bytes);
            }
            if bytes.len().saturating_add(count) > limit {
                return Err(ClientError::Oversized {
                    url: url_for_read.clone(),
                });
            }
            bytes.extend_from_slice(&chunk[..count]);
        }
    }
    .fuse();
    let timeout = Timer::after(REQUEST_TIMEOUT).fuse();
    let cancelled = cancel.cancelled().fuse();
    pin_mut!(operation, timeout, cancelled);
    select_biased! {
        _ = cancelled => Err(ClientError::Cancelled),
        _ = timeout => Err(ClientError::Timeout { url: url.into() }),
        result = operation => result,
    }
}

pub struct CatalogService {
    storage: Arc<Storage>,
    client: Arc<EsouiClient>,
    details_cache: Mutex<DetailsCache>,
}

impl CatalogService {
    pub fn new(storage: Arc<Storage>, client: Arc<EsouiClient>) -> Self {
        Self {
            storage,
            client,
            details_cache: Mutex::new(DetailsCache::default()),
        }
    }

    pub fn load_cached(&self) -> Result<Option<CacheLoad>, CatalogServiceError> {
        Ok(self.storage.load_catalog(now_unix())?)
    }

    pub async fn refresh(
        &self,
        cancel: &CancellationToken,
    ) -> Result<(Arc<Catalog>, SaveOutcome), CatalogServiceError> {
        let feeds = self.client.init(cancel).await?;
        let addons = self.client.fetch_addon_list(cancel).await?;
        let categories = self.client.fetch_categories(cancel).await?;
        let catalog = Arc::new(Catalog { addons, categories });
        let outcome = self
            .storage
            .save_catalog_and_feeds(&catalog, Some(&feeds), now_unix())?;
        Ok((catalog, outcome))
    }

    pub async fn details(
        &self,
        uids: &[String],
        cancel: &CancellationToken,
    ) -> Result<Vec<RemoteAddonDetails>, CatalogServiceError> {
        let now = Instant::now();
        let mut by_uid = HashMap::with_capacity(uids.len());
        let mut missing = Vec::new();
        {
            let mut cache = self.details_cache.lock().expect("details cache poisoned");
            for uid in uids {
                if let Some(details) = cache.get(uid, now) {
                    by_uid.insert(uid.clone(), details);
                } else if !missing.contains(uid) {
                    missing.push(uid.clone());
                }
            }
        }
        if missing.is_empty() {
            return Ok(uids
                .iter()
                .filter_map(|uid| by_uid.get(uid).cloned())
                .collect());
        }
        if self.client.feed_urls().is_none() {
            self.client.init(cancel).await?;
        }
        let fetched = self.client.fetch_addon_details(&missing, cancel).await?;
        {
            let mut cache = self.details_cache.lock().expect("details cache poisoned");
            for details in fetched {
                by_uid.insert(details.addon.uid.to_string(), details.clone());
                cache.insert(details, now);
            }
        }
        Ok(uids
            .iter()
            .filter_map(|uid| by_uid.get(uid).cloned())
            .collect())
    }
}

#[derive(Default)]
struct DetailsCache {
    entries: HashMap<String, (Instant, RemoteAddonDetails)>,
    recency: VecDeque<String>,
}

impl DetailsCache {
    fn get(&mut self, uid: &str, now: Instant) -> Option<RemoteAddonDetails> {
        let (stored_at, details) = self.entries.get(uid)?;
        if now.duration_since(*stored_at) >= DETAILS_CACHE_TTL {
            self.entries.remove(uid);
            self.recency.retain(|key| key != uid);
            return None;
        }
        let details = details.clone();
        self.touch(uid);
        Some(details)
    }

    fn insert(&mut self, details: RemoteAddonDetails, now: Instant) {
        let uid = details.addon.uid.to_string();
        self.entries.insert(uid.clone(), (now, details));
        self.touch(&uid);
        while self.entries.len() > DETAILS_CACHE_CAPACITY {
            if let Some(oldest) = self.recency.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    fn touch(&mut self, uid: &str) {
        self.recency.retain(|key| key != uid);
        self.recency.push_back(uid.to_owned());
    }
}

async fn with_cancel_and_timeout<T, F>(
    operation: F,
    url: &str,
    cancel: &CancellationToken,
) -> Result<T, ClientError>
where
    F: Future<Output = http_client::Result<T>>,
{
    let operation = operation.fuse();
    let timeout = Timer::after(REQUEST_TIMEOUT).fuse();
    let cancelled = cancel.cancelled().fuse();
    pin_mut!(operation, timeout, cancelled);
    select_biased! {
        _ = cancelled => Err(ClientError::Cancelled),
        _ = timeout => Err(ClientError::Timeout { url: url.into() }),
        result = operation => result.map_err(|error| ClientError::Transport {
            url: url.into(),
            message: error.to_string(),
        }),
    }
}

async fn wait_or_cancel(duration: Duration, cancel: &CancellationToken) -> Result<(), ClientError> {
    let timer = Timer::after(duration).fuse();
    let cancelled = cancel.cancelled().fuse();
    pin_mut!(timer, cancelled);
    select_biased! {
        _ = cancelled => Err(ClientError::Cancelled),
        _ = timer => Ok(()),
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[derive(Default, Deserialize)]
#[serde(default)]
#[serde(rename_all = "PascalCase")]
struct ApiActive {
    #[serde(default)]
    status: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct ApiGlobalGame {
    #[serde(rename = "GameID")]
    game_id: String,
    #[serde(rename = "GameConfig")]
    game_config: String,
    #[serde(rename = "Active")]
    active: ApiActive,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct ApiGlobalConfig {
    #[serde(rename = "Active")]
    active: ApiActive,
    #[serde(rename = "GAMES")]
    games: Vec<ApiGlobalGame>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct ApiRemoteAddon {
    #[serde(rename = "UID")]
    uid: String,
    #[serde(rename = "UICATID")]
    category_id: String,
    #[serde(rename = "UIName")]
    ui_name: String,
    #[serde(rename = "UIAuthorName")]
    ui_author_name: String,
    #[serde(rename = "UIDate")]
    ui_date: i64,
    #[serde(rename = "UIVersion")]
    ui_version: String,
    #[serde(rename = "UIDir", default, deserialize_with = "null_default")]
    ui_dirs: Vec<String>,
    #[serde(rename = "UIFileInfoURL")]
    ui_file_info_url: String,
    #[serde(rename = "UIDownloadTotal")]
    ui_download_total: String,
    #[serde(rename = "UIDownloadMonthly")]
    ui_download_monthly: String,
    #[serde(rename = "UIFavoriteTotal")]
    ui_favorite_total: String,
    #[serde(rename = "UIIMG_Thumbs", default, deserialize_with = "null_default")]
    ui_img_thumbs: Vec<String>,
    #[serde(rename = "UIIMGs", default, deserialize_with = "null_default")]
    ui_imgs: Vec<String>,
    #[serde(rename = "UICompatibility", default, deserialize_with = "null_default")]
    compatabilities: Vec<GameVersion>,
    #[serde(rename = "UISiblings", default, deserialize_with = "null_default")]
    siblings: Vec<String>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct ApiRemoteAddonDetails {
    #[serde(flatten)]
    addon: ApiRemoteAddon,
    #[serde(rename = "UIMD5")]
    ui_md5: String,
    #[serde(rename = "UIFileName")]
    ui_file_name: String,
    #[serde(rename = "UIDownload")]
    ui_download: String,
    #[serde(rename = "UIDescription")]
    ui_description: String,
    #[serde(rename = "UIChangeLog")]
    ui_change_log: String,
    #[serde(rename = "UIHitCount")]
    ui_hit_count: String,
    #[serde(rename = "UIHitCountMonthly")]
    ui_hit_count_monthly: String,
    #[serde(rename = "UIDonationLink")]
    ui_donation: String,
    #[serde(rename = "UIPending")]
    ui_pending: String,
    #[serde(rename = "UICATID")]
    ui_cat_id: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct ApiCategory {
    #[serde(rename = "UICATID")]
    id: String,
    #[serde(rename = "UICATTitle")]
    title: String,
    #[serde(rename = "UICATICON")]
    icon: String,
    #[serde(rename = "UICATFileCount")]
    count: String,
    #[serde(rename = "UICATParentIDs", default, deserialize_with = "null_default")]
    parent_ids: Vec<String>,
}

fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Option::<T>::deserialize(deserializer).map(Option::unwrap_or_default)
}

impl From<ApiRemoteAddon> for RemoteAddon {
    fn from(addon: ApiRemoteAddon) -> Self {
        let ui_date = if addon.ui_date > 0 {
            OffsetDateTime::from_unix_timestamp_nanos(i128::from(addon.ui_date) * 1_000_000)
                .map(|date| date.date().to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };
        Self {
            uid: addon.uid.into(),
            category_id: addon.category_id.into(),
            ui_name: addon.ui_name.into(),
            ui_author_name: addon.ui_author_name.into(),
            ui_date: ui_date.into(),
            ui_version: addon.ui_version.into(),
            ui_dirs: addon.ui_dirs.into_iter().map(Into::into).collect(),
            ui_file_info_url: addon.ui_file_info_url.into(),
            ui_download_total: parse_i64(&addon.ui_download_total),
            ui_download_monthly: parse_i64(&addon.ui_download_monthly),
            ui_favorite_total: parse_i64(&addon.ui_favorite_total),
            ui_img_thumbs: addon.ui_img_thumbs.into_iter().map(Into::into).collect(),
            ui_imgs: addon.ui_imgs.into_iter().map(Into::into).collect(),
            compatabilities: addon.compatabilities,
            siblings: addon.siblings.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ApiRemoteAddonDetails> for RemoteAddonDetails {
    fn from(details: ApiRemoteAddonDetails) -> Self {
        Self {
            addon: details.addon.into(),
            ui_md5: details.ui_md5,
            ui_file_name: details.ui_file_name,
            ui_download: details.ui_download,
            ui_description: details.ui_description,
            ui_change_log: details.ui_change_log,
            ui_hit_count: parse_i64(&details.ui_hit_count),
            ui_hit_count_monthly: parse_i64(&details.ui_hit_count_monthly),
            ui_donation: details.ui_donation,
            ui_pending: details.ui_pending == "1",
            ui_cat_id: details.ui_cat_id,
        }
    }
}

impl From<ApiCategory> for Category {
    fn from(category: ApiCategory) -> Self {
        Self {
            id: category.id.into(),
            name: category.title.into(),
            icon_url: category.icon.into(),
            parent_id: category
                .parent_ids
                .first()
                .cloned()
                .unwrap_or_default()
                .into(),
            parent_ids: category.parent_ids.into_iter().map(Into::into).collect(),
            count: category.count.trim().parse().unwrap_or_default(),
        }
    }
}

fn parse_i64(value: &str) -> i64 {
    value.trim().parse().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;
    use http_client::{FakeHttpClient, Response};

    #[test]
    fn converts_api_counts_dates_and_categories() {
        let addon = RemoteAddon::from(ApiRemoteAddon {
            uid: "1".into(),
            ui_date: 1_710_000_000_000,
            ui_download_total: "123".into(),
            ui_download_monthly: "45".into(),
            ui_favorite_total: "6".into(),
            ..ApiRemoteAddon::default()
        });
        assert_eq!(addon.ui_date, "2024-03-09");
        assert_eq!(addon.ui_download_total, 123);

        let category = Category::from(ApiCategory {
            id: "cat".into(),
            title: "Tools".into(),
            icon: "icon".into(),
            count: "99".into(),
            parent_ids: vec!["root".into(), "parent".into()],
        });
        assert_eq!(category.parent_id, "root");
        assert_eq!(category.count, 99);
    }

    #[test]
    fn accepts_nullable_lists_from_live_mmoui_payloads() {
        let addon: ApiRemoteAddon = serde_json::from_str(
            r#"{
                "UID":"7",
                "UIDir":["LibAddonMenu-2.0"],
                "UIIMG_Thumbs":null,
                "UIIMGs":null,
                "UICompatibility":null,
                "UISiblings":null
            }"#,
        )
        .unwrap();
        assert_eq!(addon.ui_dirs, ["LibAddonMenu-2.0"]);
        assert!(addon.ui_img_thumbs.is_empty());
        assert!(addon.ui_imgs.is_empty());
        assert!(addon.compatabilities.is_empty());
        assert!(addon.siblings.is_empty());

        let category: ApiCategory =
            serde_json::from_str(r#"{"UICATID":"1","UICATParentIDs":null}"#).unwrap();
        assert!(category.parent_ids.is_empty());
    }

    #[test]
    fn cancellation_interrupts_retry_wait() {
        futures::executor::block_on(async {
            let token = CancellationToken::default();
            token.cancel();
            assert!(matches!(
                wait_or_cancel(Duration::from_secs(30), &token).await,
                Err(ClientError::Cancelled)
            ));
        });
    }

    #[test]
    fn response_body_limit_is_enforced_while_streaming() {
        futures::executor::block_on(async {
            let body = AsyncBody::from(vec![7_u8; 17]);
            assert!(matches!(
                read_response_body(
                    body,
                    "http://test.example/oversized",
                    &CancellationToken::default(),
                    16,
                )
                .await,
                Err(ClientError::Oversized { .. })
            ));
        });
    }

    #[test]
    fn discovers_feeds_and_fetches_catalog_from_mocked_mmoui() {
        futures::executor::block_on(async {
            let http = FakeHttpClient::create(|request| async move {
                let body = match request.uri().path() {
                    "/global" => {
                        r#"{
                        "Active":{"Status":"1"},
                        "GAMES":[{"GameID":"ESO","GameConfig":"http://test.example/game","Active":{"Status":"1"}}]
                    }"#
                    }
                    "/game" => {
                        r#"{
                        "APIFeeds":{
                            "FileList":"http://test.example/files",
                            "FileDetails":"http://test.example/details/",
                            "CategoryList":"http://test.example/categories",
                            "ListFiles":"http://test.example/list"
                        }
                    }"#
                    }
                    "/files" => {
                        r#"[{
                        "UID":"7","UIName":"Fixture","UIVersion":"1.2.3",
                        "UIDir":["Fixture"],"UIDownloadTotal":"123","UIDate":1710000000000,
                        "UIIMG_Thumbs":null,"UIIMGs":null,"UICompatibility":null,"UISiblings":null
                    }]"#
                    }
                    "/categories" => {
                        r#"[{
                        "UICATID":"1","UICATTitle":"Tools","UICATFileCount":"1"
                    }]"#
                    }
                    path => panic!("unexpected fixture request: {path}"),
                };
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .body(AsyncBody::from(body))?)
            });
            let client = EsouiClient::with_bootstrap(http, "http://test.example/global");
            let cancel = CancellationToken::default();
            let feeds = client.init(&cancel).await.unwrap();
            assert_eq!(feeds.file_list, "http://test.example/files");
            let addons = client.fetch_addon_list(&cancel).await.unwrap();
            assert_eq!(addons.len(), 1);
            assert_eq!(addons[0].uid, "7");
            assert_eq!(addons[0].ui_download_total, 123);
            let categories = client.fetch_categories(&cancel).await.unwrap();
            assert_eq!(categories[0].name, "Tools");
        });
    }

    #[test]
    fn retries_server_errors_but_not_client_errors() {
        futures::executor::block_on(async {
            let attempts = Arc::new(AtomicUsize::new(0));
            let handler_attempts = attempts.clone();
            let http = FakeHttpClient::create(move |_| {
                let attempt = handler_attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        Ok(Response::builder()
                            .status(StatusCode::BAD_GATEWAY)
                            .body(AsyncBody::empty())?)
                    } else {
                        Ok(Response::builder()
                            .status(StatusCode::OK)
                            .body(AsyncBody::from(r#"{"ok":true}"#))?)
                    }
                }
            });
            let mut client = EsouiClient::with_bootstrap(http, "unused");
            client.retry_delays = [Duration::ZERO; 3];
            let value: serde_json::Value = client
                .get_json("http://test.example/retry", &CancellationToken::default())
                .await
                .unwrap();
            assert_eq!(value["ok"], true);
            assert_eq!(attempts.load(Ordering::SeqCst), 2);

            let attempts = Arc::new(AtomicUsize::new(0));
            let handler_attempts = attempts.clone();
            let http = FakeHttpClient::create(move |_| {
                handler_attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    Ok(Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(AsyncBody::empty())?)
                }
            });
            let client = EsouiClient::with_bootstrap(http, "unused");
            assert!(matches!(
                client
                    .get_json::<serde_json::Value>(
                        "http://test.example/missing",
                        &CancellationToken::default()
                    )
                    .await,
                Err(ClientError::Status { status: 404, .. })
            ));
            assert_eq!(attempts.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn details_cache_is_bounded_and_expires_entries() {
        let now = Instant::now();
        let mut cache = DetailsCache::default();
        for index in 0..=DETAILS_CACHE_CAPACITY {
            cache.insert(
                RemoteAddonDetails {
                    addon: RemoteAddon {
                        uid: index.to_string().into(),
                        ..RemoteAddon::default()
                    },
                    ..RemoteAddonDetails::default()
                },
                now,
            );
        }
        assert_eq!(cache.entries.len(), DETAILS_CACHE_CAPACITY);
        assert!(cache.get("0", now).is_none());
        assert!(cache.get("1", now).is_some());
        assert!(
            cache
                .get("1", now + DETAILS_CACHE_TTL + Duration::from_millis(1))
                .is_none()
        );
    }
}
