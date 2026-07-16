#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bbcode;
mod components;
mod flows;
mod model;
mod overlays;
mod rows;
mod theme;
mod window;

#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::future::Future;
use std::io::Write as _;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use gpui::{
    App, AppContext, AssetSource, Bounds, KeyBinding, SharedString, Window,
    WindowBackgroundAppearance, WindowBounds, WindowOptions, actions, px, size,
};
use gpui_component::{Root, TitleBar};
use http_client::HttpClient;
use reqwest_client::ReqwestClient;

use crate::model::AppModel;
use crate::theme::apply_scribe_theme;
use crate::window::ScribeWindow;

static STARTUP_TRACE: OnceLock<Option<StartupTrace>> = OnceLock::new();
static UI_METRICS: OnceLock<UiMetrics> = OnceLock::new();

const UI_SAMPLE_LIMIT: usize = 512;

#[derive(Default)]
struct UiMetricsData {
    last_scroll_input: Option<Instant>,
    last_keyboard_input: Option<Instant>,
    scroll_events: u64,
    keyboard_events: u64,
    scroll_frame_latency_us: VecDeque<u64>,
    keyboard_frame_latency_us: VecDeque<u64>,
    render_build_us: VecDeque<u64>,
    resize_render_build_us: VecDeque<u64>,
    overlay_render_build_us: VecDeque<u64>,
    page_render_build_us: VecDeque<u64>,
}

struct UiMetrics {
    data: Mutex<UiMetricsData>,
    scroll_frame_pending: AtomicBool,
    keyboard_frame_pending: AtomicBool,
}

#[derive(Clone, Copy, Default)]
struct UiMetricsSnapshot {
    scroll_events: u64,
    scroll_samples: usize,
    scroll_p50_us: u64,
    scroll_p95_us: u64,
    scroll_p99_us: u64,
    slow_scroll_frames: usize,
    keyboard_events: u64,
    keyboard_samples: usize,
    keyboard_p95_us: u64,
    slow_keyboard_frames: usize,
    render_p95_us: u64,
    resize_render_p95_us: u64,
    overlay_render_p95_us: u64,
    page_render_p95_us: u64,
}

fn ui_metrics() -> &'static UiMetrics {
    UI_METRICS.get_or_init(|| UiMetrics {
        data: Mutex::new(UiMetricsData::default()),
        scroll_frame_pending: AtomicBool::new(false),
        keyboard_frame_pending: AtomicBool::new(false),
    })
}

fn push_bounded(samples: &mut VecDeque<u64>, value: u64) {
    if samples.len() == UI_SAMPLE_LIMIT {
        samples.pop_front();
    }
    samples.push_back(value);
}

fn record_render_build(
    elapsed: std::time::Duration,
    resized: bool,
    overlay_changed: bool,
    page_changed: bool,
) {
    if let Ok(mut metrics) = ui_metrics().data.lock() {
        let elapsed = elapsed.as_micros() as u64;
        push_bounded(&mut metrics.render_build_us, elapsed);
        if resized {
            push_bounded(&mut metrics.resize_render_build_us, elapsed);
        }
        if overlay_changed {
            push_bounded(&mut metrics.overlay_render_build_us, elapsed);
        }
        if page_changed {
            push_bounded(&mut metrics.page_render_build_us, elapsed);
        }
    }
}

fn record_scroll_input(window: &mut Window) {
    let metrics = ui_metrics();
    if let Ok(mut data) = metrics.data.lock() {
        data.scroll_events = data.scroll_events.saturating_add(1);
        data.last_scroll_input = Some(Instant::now());
    }
    if metrics.scroll_frame_pending.swap(true, Ordering::AcqRel) {
        return;
    }
    window.on_next_frame(|_, _| {
        let metrics = ui_metrics();
        metrics.scroll_frame_pending.store(false, Ordering::Release);
        if let Ok(mut data) = metrics.data.lock()
            && let Some(input) = data.last_scroll_input.take()
        {
            push_bounded(
                &mut data.scroll_frame_latency_us,
                input.elapsed().as_micros() as u64,
            );
        }
    });
}

fn record_keyboard_input(window: &mut Window) {
    let metrics = ui_metrics();
    if let Ok(mut data) = metrics.data.lock() {
        data.keyboard_events = data.keyboard_events.saturating_add(1);
        data.last_keyboard_input = Some(Instant::now());
    }
    if metrics.keyboard_frame_pending.swap(true, Ordering::AcqRel) {
        return;
    }
    window.on_next_frame(|_, _| {
        let metrics = ui_metrics();
        metrics
            .keyboard_frame_pending
            .store(false, Ordering::Release);
        if let Ok(mut data) = metrics.data.lock()
            && let Some(input) = data.last_keyboard_input.take()
        {
            push_bounded(
                &mut data.keyboard_frame_latency_us,
                input.elapsed().as_micros() as u64,
            );
        }
    });
}

fn percentile(samples: &VecDeque<u64>, percentile: f32) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted: Vec<_> = samples.iter().copied().collect();
    sorted.sort_unstable();
    let index = (((sorted.len() - 1) as f32) * percentile).round() as usize;
    sorted[index]
}

fn ui_metrics_snapshot() -> UiMetricsSnapshot {
    let Ok(metrics) = ui_metrics().data.lock() else {
        return UiMetricsSnapshot::default();
    };
    UiMetricsSnapshot {
        scroll_events: metrics.scroll_events,
        scroll_samples: metrics.scroll_frame_latency_us.len(),
        scroll_p50_us: percentile(&metrics.scroll_frame_latency_us, 0.50),
        scroll_p95_us: percentile(&metrics.scroll_frame_latency_us, 0.95),
        scroll_p99_us: percentile(&metrics.scroll_frame_latency_us, 0.99),
        slow_scroll_frames: metrics
            .scroll_frame_latency_us
            .iter()
            .filter(|sample| **sample > 16_667)
            .count(),
        keyboard_events: metrics.keyboard_events,
        keyboard_samples: metrics.keyboard_frame_latency_us.len(),
        keyboard_p95_us: percentile(&metrics.keyboard_frame_latency_us, 0.95),
        slow_keyboard_frames: metrics
            .keyboard_frame_latency_us
            .iter()
            .filter(|sample| **sample > 16_667)
            .count(),
        render_p95_us: percentile(&metrics.render_build_us, 0.95),
        resize_render_p95_us: percentile(&metrics.resize_render_build_us, 0.95),
        overlay_render_p95_us: percentile(&metrics.overlay_render_build_us, 0.95),
        page_render_p95_us: percentile(&metrics.page_render_build_us, 0.95),
    }
}

fn duration_label(microseconds: u64) -> String {
    if microseconds == 0 {
        "No samples".into()
    } else {
        format!("{:.2} ms", microseconds as f64 / 1_000.0)
    }
}

fn performance_report(snapshot: UiMetricsSnapshot) -> String {
    format!(
        "Scribe UI performance diagnostics\nscroll events: {}\nscroll frame-response samples: {}\nscroll response p50: {}\nscroll response p95: {}\nscroll response p99: {}\nscroll responses over 16.67 ms: {}\nkeyboard events: {}\nkeyboard frame-response samples: {}\nkeyboard response p95: {}\nkeyboard responses over 16.67 ms: {}\nrender-build p95: {}\nresize render-build p95: {}\noverlay render-build p95: {}\npage render-build p95: {}\n",
        snapshot.scroll_events,
        snapshot.scroll_samples,
        duration_label(snapshot.scroll_p50_us),
        duration_label(snapshot.scroll_p95_us),
        duration_label(snapshot.scroll_p99_us),
        snapshot.slow_scroll_frames,
        snapshot.keyboard_events,
        snapshot.keyboard_samples,
        duration_label(snapshot.keyboard_p95_us),
        snapshot.slow_keyboard_frames,
        duration_label(snapshot.render_p95_us),
        duration_label(snapshot.resize_render_p95_us),
        duration_label(snapshot.overlay_render_p95_us),
        duration_label(snapshot.page_render_p95_us),
    )
}

struct ScribeAssets;

impl AssetSource for ScribeAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path == "scribe-logo-v2.png" {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/scribe-logo-v2.png"
            ))));
        }
        if path == "scribe-trash.svg" {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/scribe-trash.svg"
            ))));
        }
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut assets = gpui_component_assets::Assets.list(path)?;
        if "scribe-logo-v2.png".starts_with(path) {
            assets.push("scribe-logo-v2.png".into());
        }
        if "scribe-trash.svg".starts_with(path) {
            assets.push("scribe-trash.svg".into());
        }
        Ok(assets)
    }
}

struct StartupTrace {
    started: Instant,
    output: Mutex<File>,
}

fn trace_startup(event: &'static str) {
    let trace = STARTUP_TRACE.get_or_init(|| {
        let started = Instant::now();
        let path = std::env::var_os("SCRIBE_STARTUP_TRACE")?;
        let output = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .ok()?;
        Some(StartupTrace {
            started,
            output: Mutex::new(output),
        })
    });
    let Some(trace) = trace else {
        return;
    };
    if let Ok(mut output) = trace.output.lock() {
        let _ = writeln!(output, "{event} {}", trace.started.elapsed().as_micros());
    }
}

fn embedded_assets_ready() -> bool {
    matches!(
        ScribeAssets.load("scribe-logo-v2.png"),
        Ok(Some(asset)) if !asset.is_empty()
    ) && matches!(
        ScribeAssets.load("scribe-trash.svg"),
        Ok(Some(asset)) if !asset.is_empty()
    )
}

struct LazyHttpClient {
    inner: OnceLock<ReqwestClient>,
    user_agent: http_client::http::HeaderValue,
}

impl LazyHttpClient {
    fn new(user_agent: &'static str) -> Self {
        Self {
            inner: OnceLock::new(),
            user_agent: http_client::http::HeaderValue::from_static(user_agent),
        }
    }

    fn get(&self) -> &ReqwestClient {
        self.inner.get_or_init(|| {
            ReqwestClient::user_agent(self.user_agent.to_str().expect("static user agent"))
                .expect("initialize native HTTP client")
        })
    }
}

impl HttpClient for LazyHttpClient {
    fn user_agent(&self) -> Option<&http_client::http::HeaderValue> {
        Some(&self.user_agent)
    }

    fn proxy(&self) -> Option<&http_client::Url> {
        None
    }

    fn send(
        &self,
        request: http_client::http::Request<http_client::AsyncBody>,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = http_client::Result<
                        http_client::http::Response<http_client::AsyncBody>,
                    >,
                > + Send
                + 'static,
        >,
    > {
        self.get().send(request)
    }
}

actions!(
    scribe,
    [
        ShowInstalled,
        ShowFindMore,
        ShowUpdates,
        FocusSearch,
        OpenSettings
    ]
);

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(target_os = "windows")]
fn system_prefers_reduced_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };

    let mut animations_enabled = 1_i32;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes a BOOL to the valid pointer supplied here.
    let succeeded = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            (&mut animations_enabled as *mut i32).cast(),
            0,
        )
    };
    succeeded != 0 && animations_enabled == 0
}

#[cfg(not(target_os = "windows"))]
fn system_prefers_reduced_motion() -> bool {
    false
}

fn main() {
    trace_startup("main_enter");
    assert!(
        embedded_assets_ready(),
        "the embedded Scribe logo asset is unavailable"
    );
    trace_startup("embedded_assets_ready");
    gpui_platform::application()
        .with_assets(ScribeAssets)
        .run(|cx: &mut App| {
            trace_startup("gpui_run");
            gpui_component::init(cx);
            cx.set_reduce_motion(system_prefers_reduced_motion());
            trace_startup("component_init");
            apply_scribe_theme(cx);
            let http_client: Arc<dyn HttpClient> = Arc::new(LazyHttpClient::new(concat!(
                "Scribe/",
                env!("CARGO_PKG_VERSION")
            )));
            cx.set_http_client(http_client);
            cx.bind_keys([
                KeyBinding::new("ctrl-1", ShowInstalled, None),
                KeyBinding::new("ctrl-2", ShowFindMore, None),
                KeyBinding::new("ctrl-u", ShowUpdates, None),
                KeyBinding::new("ctrl-f", FocusSearch, None),
                KeyBinding::new("ctrl-,", OpenSettings, None),
            ]);

            let model = cx.new(AppModel::new);
            trace_startup("model_spawned");
            let bounds = Bounds::centered(None, size(px(1120.0), px(768.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(1024.0), px(640.0))),
                    titlebar: Some(TitleBar::title_bar_options()),
                    window_background: WindowBackgroundAppearance::Blurred,
                    ..Default::default()
                },
                {
                    let model = model.clone();
                    move |window, cx| {
                        window.set_window_title("Scribe");
                        let view = cx.new(|cx| ScribeWindow::new(model, window, cx));
                        let root = cx.new(|cx| Root::new(view, window, cx));
                        trace_startup("window_root");
                        window.on_next_frame(|_, _| trace_startup("first_frame"));
                        root
                    }
                },
            )
            .expect("open Scribe window");
            trace_startup("window_opened");

            cx.activate(true);
        });
}
