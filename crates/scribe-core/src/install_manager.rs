use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{Read as _, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use async_executor::Executor;
use async_io::Timer;
use async_lock::Semaphore;
use futures::{AsyncReadExt, FutureExt, pin_mut, select_biased};
use http_client::{AsyncBody, HttpClient, StatusCode};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::{CancellationToken, InstallPlanEntry, InstallRecord, Installer, Storage};

const DEFAULT_CONCURRENCY: usize = 3;
const SINGLE_PROGRESS_INTERVAL: Duration = Duration::from_millis(200);
const CONCURRENT_PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskState {
    #[default]
    Queued,
    Planning,
    Downloading,
    Extracting,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub uid: String,
    pub name: String,
    pub state: TaskState,
    pub percent: f64,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub speed: f64,
    pub error: String,
    pub files_extracted: usize,
    pub total_files: usize,
    pub queue_position: usize,
    pub install_plan: Vec<InstallPlanEntry>,
}

#[derive(Clone, Debug)]
pub struct InstallRequest {
    pub uid: String,
    pub name: String,
    pub download_url: String,
    pub md5: String,
    pub addon_path: std::path::PathBuf,
    pub expected_directories: Vec<String>,
}

type ProgressEmitter = dyn Fn(TaskProgress) + Send + Sync + 'static;

pub struct InstallManager {
    http: Arc<dyn HttpClient>,
    storage: Arc<Storage>,
    executor: Arc<Executor<'static>>,
    slots: Arc<Semaphore>,
    emitter: Option<Arc<ProgressEmitter>>,
    tasks: Mutex<HashMap<String, CancellationToken>>,
    statuses: Mutex<HashMap<String, TaskProgress>>,
    pending: Mutex<usize>,
    pending_changed: Condvar,
    active: AtomicUsize,
    stop: CancellationToken,
    workers: Mutex<Vec<std::thread::JoinHandle<()>>>,
    shutdown_started: AtomicBool,
}

impl InstallManager {
    pub fn new(
        concurrency: usize,
        http: Arc<dyn HttpClient>,
        storage: Arc<Storage>,
        emitter: Option<Arc<ProgressEmitter>>,
    ) -> Arc<Self> {
        let concurrency = if concurrency == 0 {
            DEFAULT_CONCURRENCY
        } else {
            concurrency
        };
        let manager = Arc::new(Self {
            http,
            storage,
            executor: Arc::new(Executor::new()),
            slots: Arc::new(Semaphore::new(concurrency)),
            emitter,
            tasks: Mutex::new(HashMap::new()),
            statuses: Mutex::new(HashMap::new()),
            pending: Mutex::new(0),
            pending_changed: Condvar::new(),
            active: AtomicUsize::new(0),
            stop: CancellationToken::default(),
            workers: Mutex::new(Vec::new()),
            shutdown_started: AtomicBool::new(false),
        });
        for index in 0..concurrency {
            let executor = manager.executor.clone();
            let stop = manager.stop.clone();
            let worker = std::thread::Builder::new()
                .name(format!("scribe-install-{index}"))
                .spawn(move || futures::executor::block_on(executor.run(stop.cancelled())))
                .expect("start install worker");
            manager
                .workers
                .lock()
                .expect("worker lock poisoned")
                .push(worker);
        }
        manager
    }

    pub fn enqueue(self: &Arc<Self>, request: InstallRequest) -> bool {
        if self.shutdown_started.load(Ordering::Acquire) || request.uid.trim().is_empty() {
            return false;
        }
        let token = CancellationToken::default();
        {
            let mut tasks = self.tasks.lock().expect("task lock poisoned");
            if tasks.contains_key(&request.uid) {
                return false;
            }
            tasks.insert(request.uid.clone(), token.clone());
        }
        *self.pending.lock().expect("pending lock poisoned") += 1;
        let queue_position = self
            .statuses
            .lock()
            .expect("status lock poisoned")
            .values()
            .filter(|status| status.state == TaskState::Queued)
            .count()
            + 1;
        self.publish(TaskProgress {
            uid: request.uid.clone(),
            name: request.name.clone(),
            state: TaskState::Queued,
            queue_position,
            ..TaskProgress::default()
        });

        let manager = self.clone();
        self.executor
            .spawn(async move {
                manager.run(request, token).await;
                manager.finish_pending();
            })
            .detach();
        true
    }

    pub fn cancel(&self, uid: &str) -> bool {
        let token = self
            .tasks
            .lock()
            .expect("task lock poisoned")
            .get(uid)
            .cloned();
        if let Some(token) = token {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub fn cancel_all(&self) {
        for token in self.tasks.lock().expect("task lock poisoned").values() {
            token.cancel();
        }
    }

    pub fn statuses(&self) -> Vec<TaskProgress> {
        let mut statuses: Vec<_> = self
            .statuses
            .lock()
            .expect("status lock poisoned")
            .values()
            .cloned()
            .collect();
        statuses.sort_by(|left, right| left.name.cmp(&right.name));
        statuses
    }

    pub fn shutdown(&self) {
        if self.shutdown_started.swap(true, Ordering::AcqRel) {
            return;
        }
        self.cancel_all();
        self.wait_idle();
        self.stop.cancel();
        for worker in self.workers.lock().expect("worker lock poisoned").drain(..) {
            let _ = worker.join();
        }
    }

    pub fn wait_idle(&self) {
        let mut pending = self.pending.lock().expect("pending lock poisoned");
        while *pending != 0 {
            pending = self
                .pending_changed
                .wait(pending)
                .expect("pending lock poisoned");
        }
    }

    async fn run(self: &Arc<Self>, request: InstallRequest, token: CancellationToken) {
        let acquire = self.slots.acquire_arc().fuse();
        let cancelled = token.cancelled().fuse();
        pin_mut!(acquire, cancelled);
        let permit = select_biased! {
            _ = cancelled => {
                self.finish_task(&request, TaskState::Cancelled, String::new());
                return;
            },
            permit = acquire => permit,
        };
        let _permit = permit;
        self.active.fetch_add(1, Ordering::AcqRel);
        self.update_queue_positions();
        let result = self.run_active(&request, &token).await;
        self.active.fetch_sub(1, Ordering::AcqRel);

        match result {
            Ok(()) => self.finish_task(&request, TaskState::Complete, String::new()),
            Err(_) if token.is_cancelled() => {
                self.finish_task(&request, TaskState::Cancelled, String::new())
            }
            Err(error) => self.finish_task(&request, TaskState::Failed, error),
        }
    }

    async fn run_active(
        &self,
        request: &InstallRequest,
        token: &CancellationToken,
    ) -> Result<(), String> {
        self.publish_for(request, TaskState::Downloading, Vec::new());
        let temp = self.download(request, token).await?;
        if !request.md5.trim().is_empty() {
            verify_md5(temp.path(), &request.md5)?;
        }
        let plan = Installer::plan_archive(
            temp.path(),
            &request.addon_path,
            &request.expected_directories,
        )
        .map_err(|error| error.to_string())?;
        self.publish_for(request, TaskState::Planning, plan.clone());
        self.publish_for(request, TaskState::Extracting, plan.clone());

        let interval = self.progress_interval();
        let mut last_emit = None;
        Installer::install_archive(
            temp.path(),
            &request.addon_path,
            &request.expected_directories,
            token,
            |done, total| {
                let now = Instant::now();
                if done != total
                    && last_emit.is_some_and(|last: Instant| now.duration_since(last) < interval)
                {
                    return;
                }
                last_emit = Some(now);
                self.publish(TaskProgress {
                    uid: request.uid.clone(),
                    name: request.name.clone(),
                    state: TaskState::Extracting,
                    percent: percent(done as u64, total as u64),
                    files_extracted: done,
                    total_files: total,
                    install_plan: plan.clone(),
                    ..TaskProgress::default()
                });
            },
        )
        .map_err(|error| error.to_string())?;
        if !request.md5.is_empty() {
            self.storage
                .put_install_record(&InstallRecord {
                    uid: request.uid.clone(),
                    md5: request.md5.clone(),
                })
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    async fn download(
        &self,
        request: &InstallRequest,
        token: &CancellationToken,
    ) -> Result<tempfile::NamedTempFile, String> {
        let deadline_started = Instant::now();
        let operation = self
            .http
            .get(&request.download_url, AsyncBody::empty(), true)
            .fuse();
        let timeout = Timer::after(DOWNLOAD_TIMEOUT).fuse();
        let cancelled = token.cancelled().fuse();
        pin_mut!(operation, timeout, cancelled);
        let response = select_biased! {
            _ = cancelled => return Err("download cancelled".into()),
            _ = timeout => return Err("download timed out after 5 minutes".into()),
            response = operation => response.map_err(|error| format!("download failed: {error}"))?,
        };
        if response.status() != StatusCode::OK {
            return Err(format!("download returned {}", response.status()));
        }
        let total = response
            .headers()
            .get(http_client::http::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
            .unwrap_or_default();
        let started = Instant::now();
        let interval = self.progress_interval();
        let mut last_emit = Instant::now() - interval;
        let mut downloaded = 0_u64;
        let mut body = response.into_body();
        let mut temp = tempfile::Builder::new()
            .prefix("scribeeso_")
            .suffix(".zip")
            .tempfile()
            .map_err(|error| format!("create download file: {error}"))?;
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let remaining = DOWNLOAD_TIMEOUT.saturating_sub(deadline_started.elapsed());
            if remaining.is_zero() {
                return Err("download timed out after 5 minutes".into());
            }
            let read_operation = body.read(&mut buffer).fuse();
            let timeout = Timer::after(remaining).fuse();
            let cancelled = token.cancelled().fuse();
            pin_mut!(read_operation, timeout, cancelled);
            let read = select_biased! {
                _ = cancelled => return Err("download cancelled".into()),
                _ = timeout => return Err("download timed out after 5 minutes".into()),
                read = read_operation => read.map_err(|error| format!("read download: {error}"))?,
            };
            if read == 0 {
                break;
            }
            temp.write_all(&buffer[..read])
                .map_err(|error| format!("write download: {error}"))?;
            downloaded += read as u64;
            let now = Instant::now();
            if now.duration_since(last_emit) >= interval {
                last_emit = now;
                let elapsed = started.elapsed().as_secs_f64();
                self.publish(TaskProgress {
                    uid: request.uid.clone(),
                    name: request.name.clone(),
                    state: TaskState::Downloading,
                    percent: percent(downloaded, total),
                    bytes_downloaded: downloaded,
                    total_bytes: total,
                    speed: if elapsed > 0.0 {
                        downloaded as f64 / elapsed
                    } else {
                        0.0
                    },
                    ..TaskProgress::default()
                });
            }
        }
        temp.flush()
            .map_err(|error| format!("flush download: {error}"))?;
        Ok(temp)
    }

    fn publish_for(
        &self,
        request: &InstallRequest,
        state: TaskState,
        install_plan: Vec<InstallPlanEntry>,
    ) {
        self.publish(TaskProgress {
            uid: request.uid.clone(),
            name: request.name.clone(),
            state,
            install_plan,
            ..TaskProgress::default()
        });
    }

    fn finish_task(&self, request: &InstallRequest, state: TaskState, error: String) {
        self.publish(TaskProgress {
            uid: request.uid.clone(),
            name: request.name.clone(),
            state,
            percent: if state == TaskState::Complete {
                100.0
            } else {
                0.0
            },
            error,
            ..TaskProgress::default()
        });
        self.tasks
            .lock()
            .expect("task lock poisoned")
            .remove(&request.uid);
    }

    fn publish(&self, progress: TaskProgress) {
        self.statuses
            .lock()
            .expect("status lock poisoned")
            .insert(progress.uid.clone(), progress.clone());
        if let Some(emitter) = &self.emitter {
            emitter(progress);
        }
    }

    fn update_queue_positions(&self) {
        let mut statuses = self.statuses.lock().expect("status lock poisoned");
        let mut position = 1;
        for status in statuses.values_mut() {
            if status.state == TaskState::Queued {
                status.queue_position = position;
                position += 1;
            }
        }
    }

    fn progress_interval(&self) -> Duration {
        if self.active.load(Ordering::Acquire) > 1 {
            CONCURRENT_PROGRESS_INTERVAL
        } else {
            SINGLE_PROGRESS_INTERVAL
        }
    }

    fn finish_pending(&self) {
        let mut pending = self.pending.lock().expect("pending lock poisoned");
        *pending = pending.saturating_sub(1);
        self.pending_changed.notify_all();
    }
}

fn verify_md5(path: &std::path::Path, expected: &str) -> Result<(), String> {
    let mut file = File::open(path).map_err(|error| format!("open download for MD5: {error}"))?;
    let mut digest = Md5::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("compute MD5: {error}"))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let actual = lowercase_hex(digest.finalize().as_ref());
    if actual.eq_ignore_ascii_case(expected.trim()) {
        Ok(())
    } else {
        Err(format!("MD5 mismatch: expected {expected}, got {actual}"))
    }
}

fn lowercase_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn percent(done: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        done as f64 / total as f64 * 100.0
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use http_client::{FakeHttpClient, Response};
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use super::*;

    struct PendingReader;

    impl futures::AsyncRead for PendingReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buffer: &mut [u8],
        ) -> Poll<std::io::Result<usize>> {
            Poll::Pending
        }
    }

    fn archive() -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut bytes);
            writer
                .start_file("Addon/Addon.txt", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"## Title: Addon").unwrap();
            writer
                .start_file("Addon/main.lua", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"installed").unwrap();
            writer.finish().unwrap();
        }
        bytes.into_inner()
    }

    #[test]
    fn downloads_checks_integrity_and_installs() {
        let archive = archive();
        let md5 = lowercase_hex(Md5::digest(&archive).as_ref());
        let http = FakeHttpClient::create(move |_| {
            let archive = archive.clone();
            async move {
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .body(AsyncBody::from(archive))?)
            }
        });
        let temp = tempfile::tempdir().unwrap();
        let storage = Arc::new(Storage::open(temp.path().join("scribe.redb")).unwrap());
        let addon_path = temp.path().join("AddOns");
        std::fs::create_dir(&addon_path).unwrap();
        let manager = InstallManager::new(1, http, storage.clone(), None);
        assert!(manager.enqueue(InstallRequest {
            uid: "7".into(),
            name: "Addon".into(),
            download_url: "http://test.example/addon.zip".into(),
            md5: md5.clone(),
            addon_path: addon_path.clone(),
            expected_directories: vec!["Addon".into()],
        }));
        assert!(!manager.enqueue(InstallRequest {
            uid: "7".into(),
            name: "duplicate".into(),
            download_url: String::new(),
            md5: String::new(),
            addon_path: addon_path.clone(),
            expected_directories: Vec::new(),
        }));
        manager.wait_idle();
        let statuses = manager.statuses();
        assert_eq!(statuses[0].state, TaskState::Complete, "{statuses:#?}");
        assert_eq!(
            std::fs::read_to_string(addon_path.join("Addon/main.lua")).unwrap(),
            "installed"
        );
        assert_eq!(storage.install_record("7").unwrap().unwrap().md5, md5);
        manager.shutdown();
    }

    #[test]
    fn queued_cancellation_finishes_without_starting_download() {
        let http = FakeHttpClient::create(|_| async move {
            futures::future::pending::<http_client::Result<Response<AsyncBody>>>().await
        });
        let temp = tempfile::tempdir().unwrap();
        let storage = Arc::new(Storage::open(temp.path().join("scribe.redb")).unwrap());
        let addon_path = temp.path().join("AddOns");
        std::fs::create_dir(&addon_path).unwrap();
        let manager = InstallManager::new(1, http, storage, None);
        for uid in ["first", "second"] {
            assert!(manager.enqueue(InstallRequest {
                uid: uid.into(),
                name: uid.into(),
                download_url: "http://test.example/pending".into(),
                md5: String::new(),
                addon_path: addon_path.clone(),
                expected_directories: Vec::new(),
            }));
        }
        assert!(manager.cancel("second"));
        assert!(manager.cancel("first"));
        manager.shutdown();
        assert!(
            manager
                .statuses()
                .iter()
                .all(|status| status.state == TaskState::Cancelled)
        );
    }

    #[test]
    fn cancellation_interrupts_a_stalled_response_body() {
        let http = FakeHttpClient::create(|_| async move {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(AsyncBody::from_reader(PendingReader))?)
        });
        let temp = tempfile::tempdir().unwrap();
        let storage = Arc::new(Storage::open(temp.path().join("scribe.redb")).unwrap());
        let addon_path = temp.path().join("AddOns");
        std::fs::create_dir(&addon_path).unwrap();
        let manager = InstallManager::new(1, http, storage, None);
        assert!(manager.enqueue(InstallRequest {
            uid: "stalled".into(),
            name: "Stalled".into(),
            download_url: "http://test.example/stalled".into(),
            md5: String::new(),
            addon_path,
            expected_directories: Vec::new(),
        }));
        let started = Instant::now();
        while manager
            .statuses()
            .iter()
            .all(|status| status.state != TaskState::Downloading)
        {
            assert!(started.elapsed() < Duration::from_secs(2));
            std::thread::yield_now();
        }
        assert!(manager.cancel("stalled"));
        manager.wait_idle();
        assert_eq!(manager.statuses()[0].state, TaskState::Cancelled);
        manager.shutdown();
    }

    #[test]
    fn progress_intervals_match_desktop_contract() {
        assert_eq!(SINGLE_PROGRESS_INTERVAL, Duration::from_millis(200));
        assert_eq!(CONCURRENT_PROGRESS_INTERVAL, Duration::from_millis(250));
    }
}
