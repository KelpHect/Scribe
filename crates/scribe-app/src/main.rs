#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::future::Future;
use std::io::Write as _;
use std::ops::Range;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, AssetSource, Bounds, ClipboardItem, Context, ElementId, Entity, FocusHandle,
    Focusable, KeyBinding, ObjectFit, PathPromptOptions, Role, SharedString, StyledImage,
    Subscription, Window, WindowBounds, WindowOptions, actions, div, img, px, relative, size,
    uniform_list,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, IndexPath, Root, Sizable as _, Size, StyledExt as _, Theme,
    ThemeMode, TitleBar,
    alert::Alert,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement as _,
    select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState},
};
use http_client::HttpClient;
use reqwest_client::ReqwestClient;
use scribe_core::{
    Addon, AppSettings, CancellationToken, Catalog, CatalogIndex, CatalogService, CatalogSort,
    Category, EsouiClient, InstallManager, InstallRequest, InstalledIndex, Installer, MatchedAddon,
    Matcher, MissingDependency, RemoteAddon, RemoteAddonDetails, Scanner, SettingsManager, Storage,
    TaskProgress, TaskState,
};

static STARTUP_TRACE: OnceLock<Option<StartupTrace>> = OnceLock::new();

struct ScribeAssets;

impl AssetSource for ScribeAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path == "scribe-logo.png" {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/scribe-logo.png"
            ))));
        }
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut assets = gpui_component_assets::Assets.list(path)?;
        if "scribe-logo.png".starts_with(path) {
            assets.push("scribe-logo.png".into());
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
        Tab,
        TabPrevious,
        ShowInstalled,
        ShowFindMore,
        ShowUpdates,
        FocusSearch,
        OpenSettings
    ]
);

type ActivateHandler = dyn Fn(&mut Window, &mut App) + 'static;
type CloseHandler = dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static;

#[derive(IntoElement)]
struct NativeButton {
    id: ElementId,
    label: SharedString,
    variant: NativeButtonVariant,
    icon: Option<IconName>,
    on_activate: Arc<ActivateHandler>,
}

#[derive(Clone, Copy, Default)]
enum NativeButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Danger,
}

impl NativeButton {
    fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant: NativeButtonVariant::Primary,
            icon: None,
            on_activate: Arc::new(|_, _| {}),
        }
    }

    fn secondary(mut self) -> Self {
        self.variant = NativeButtonVariant::Secondary;
        self
    }

    fn ghost(mut self) -> Self {
        self.variant = NativeButtonVariant::Ghost;
        self
    }

    fn danger(mut self) -> Self {
        self.variant = NativeButtonVariant::Danger;
        self
    }

    fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    fn on_activate(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Arc::new(handler);
        self
    }
}

impl RenderOnce for NativeButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let activate = self.on_activate;
        let button = Button::new(self.id).label(self.label);
        let button = match self.variant {
            NativeButtonVariant::Primary => button.primary(),
            NativeButtonVariant::Secondary => button.secondary(),
            NativeButtonVariant::Ghost => button.ghost(),
            NativeButtonVariant::Danger => button.danger(),
        };
        button
            .when_some(self.icon, |button, icon| button.icon(icon))
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                activate(window, cx);
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FilterOption {
    label: SharedString,
    value: SharedString,
    icon_url: Option<SharedString>,
}

impl FilterOption {
    fn new(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            icon_url: None,
        }
    }

    fn with_icon(mut self, icon_url: impl Into<SharedString>) -> Self {
        self.icon_url = Some(icon_url.into());
        self
    }
}

impl SelectItem for FilterOption {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn display_title(&self) -> Option<AnyElement> {
        self.icon_url.as_ref().map(|icon_url| {
            h_flex()
                .gap(px(7.0))
                .child(category_artwork(
                    Some(icon_url.to_string()),
                    self.label.as_ref(),
                    16.0,
                ))
                .child(self.label.clone())
                .into_any_element()
        })
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        h_flex()
            .gap(px(8.0))
            .when_some(self.icon_url.clone(), |row, icon_url| {
                row.child(category_artwork(
                    Some(icon_url.to_string()),
                    self.label.as_ref(),
                    18.0,
                ))
            })
            .child(self.label.clone())
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(IntoElement)]
struct Title {
    text: SharedString,
    order: u8,
}

impl Title {
    fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            order: 3,
        }
    }

    fn order(mut self, order: u8) -> Self {
        self.order = order;
        self
    }
}

impl RenderOnce for Title {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .font_semibold()
            .text_size(px(match self.order {
                1 | 2 => 22.0,
                3 => 18.0,
                _ => 14.0,
            }))
            .child(self.text)
    }
}

#[derive(IntoElement)]
struct Group {
    gap: Size,
    children: Vec<AnyElement>,
}

impl Group {
    fn new() -> Self {
        Self {
            gap: Size::Medium,
            children: Vec::new(),
        }
    }

    fn gap(mut self, gap: Size) -> Self {
        self.gap = gap;
        self
    }

    fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl RenderOnce for Group {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        h_flex()
            .gap(px(match self.gap {
                Size::XSmall => 4.0,
                Size::Small => 8.0,
                Size::Large => 16.0,
                _ => 12.0,
            }))
            .children(self.children)
    }
}

#[derive(IntoElement)]
struct Modal {
    title: SharedString,
    width: f32,
    child: Option<AnyElement>,
    on_close: Arc<CloseHandler>,
}

impl Modal {
    fn new() -> Self {
        Self {
            title: SharedString::default(),
            width: 560.0,
            child: None,
            on_close: Arc::new(|_, _, _| {}),
        }
    }

    fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    fn on_close(
        mut self,
        close: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Arc::new(close);
        self
    }

    fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl RenderOnce for Modal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let close = self.on_close;
        div()
            .id("modal-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black().opacity(0.72))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .w(px(self.width))
                    .max_h(px(720.0))
                    .p(px(16.0))
                    .rounded(px(10.0))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().popover)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(Title::new(self.title).order(3))
                            .child(
                                Button::new("modal-close")
                                    .ghost()
                                    .icon(IconName::Close)
                                    .on_click(move |event, window, cx| close(event, window, cx)),
                            ),
                    )
                    .children(self.child),
            )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Installed,
    FindMore,
    Updates,
    Settings,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Installed => "Installed",
            Self::FindMore => "Find More",
            Self::Updates => "Updates",
            Self::Settings => "Settings",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Installed => "Your local ESO addons, libraries, and update health",
            Self::FindMore => "Discover and install addons directly from ESOUI",
            Self::Updates => "Review newer releases before changing local files",
            Self::Settings => "Library location, appearance, storage, and diagnostics",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Installed => IconName::LayoutDashboard,
            Self::FindMore => IconName::Search,
            Self::Updates => IconName::ArrowUp,
            Self::Settings => IconName::Settings2,
        }
    }
}

struct PageState {
    query: String,
    search: Entity<InputState>,
    _subscription: Subscription,
}

impl PageState {
    fn new(placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let subscription = cx.subscribe(&search, |this, _, event, cx| {
            if matches!(event, InputEvent::Change | InputEvent::PressEnter { .. }) {
                this.query = this.search.read(cx).value().to_string();
                cx.notify();
            }
        });
        Self {
            query: String::new(),
            search,
            _subscription: subscription,
        }
    }
}

struct AppModel {
    page: Page,
    settings: AppSettings,
    catalog_index: Arc<CatalogIndex>,
    installed: Arc<Vec<Addon>>,
    installed_index: Arc<InstalledIndex>,
    matched: Arc<Vec<MatchedAddon>>,
    missing_dependencies: Arc<Vec<MissingDependency>>,
    storage: Option<Arc<Storage>>,
    catalog_service: Option<Arc<CatalogService>>,
    install_manager: Option<Arc<InstallManager>>,
    loading: bool,
    status: String,
    selected_details: Option<RemoteAddonDetails>,
    selected_local: Option<(Addon, MatchedAddon)>,
    lightbox_index: Option<usize>,
    pending_uninstall: Vec<String>,
    observed_completions: HashSet<String>,
}

struct InitialState {
    settings: AppSettings,
    catalog_index: Arc<CatalogIndex>,
    installed: Arc<Vec<Addon>>,
    installed_index: Arc<InstalledIndex>,
    matched: Arc<Vec<MatchedAddon>>,
    missing_dependencies: Arc<Vec<MissingDependency>>,
    status: String,
    storage: Option<Arc<Storage>>,
    refresh_required: bool,
}

impl AppModel {
    fn new(cx: &mut Context<Self>) -> Self {
        let http = cx.http_client();
        let load = cx.background_executor().spawn(async move {
            let settings = SettingsManager::new()
                .and_then(|manager| manager.load())
                .unwrap_or_default();
            let storage = Storage::open_default().map(Arc::new);
            let (catalog_index, mut cache_status, refresh_required) = match &storage {
                Ok(storage) => match storage.load_catalog(unix_now()) {
                    Ok(Some(cached)) => {
                        let status = if cached.stale {
                            "Cached ESOUI catalog loaded; background refresh is due."
                        } else {
                            "Cached ESOUI catalog loaded."
                        };
                        (cached.catalog, status.to_owned(), cached.stale)
                    }
                    Ok(None) => (
                        Arc::new(CatalogIndex::new(Arc::new(Catalog::default()))),
                        "Catalog needs refresh.".into(),
                        true,
                    ),
                    Err(error) => (
                        Arc::new(CatalogIndex::new(Arc::new(Catalog::default()))),
                        format!("Catalog is unavailable: {error}. The database was retained."),
                        false,
                    ),
                },
                Err(error) => (
                    Arc::new(CatalogIndex::new(Arc::new(Catalog::default()))),
                    format!("Storage is unavailable: {error}. No database was replaced."),
                    false,
                ),
            };
            let installed = if settings.addon_path.is_empty() {
                Vec::new()
            } else {
                let cleanup = Installer::clean_stale_artifacts(
                    &settings.addon_path,
                    std::time::Duration::from_secs(24 * 60 * 60),
                );
                if !cleanup.removed.is_empty() {
                    cache_status.push_str(&format!(
                        " Removed {} stale Scribe install artifacts.",
                        cleanup.removed.len()
                    ));
                }
                if !cleanup.errors.is_empty() {
                    cache_status.push_str(" Some stale Scribe artifacts could not be cleaned.");
                }
                let scanner = Scanner::new(PathBuf::from(&settings.addon_path));
                let scanner = match &storage {
                    Ok(storage) => scanner.with_storage(storage.clone()),
                    Err(_) => scanner,
                };
                match scanner.scan() {
                    Ok(installed) => {
                        cache_status
                            .push_str(&format!(" Detected {} installed addons.", installed.len()));
                        installed
                    }
                    Err(error) => {
                        cache_status.push_str(&format!(" AddOns scan failed: {error}."));
                        Vec::new()
                    }
                }
            };
            let (matched, missing_dependencies) =
                Matcher::analyze_index(&installed, &catalog_index);
            let matched = Arc::new(matched);
            InitialState {
                settings,
                catalog_index,
                installed_index: Arc::new(InstalledIndex::new(&installed, &matched)),
                matched,
                missing_dependencies: Arc::new(missing_dependencies),
                installed: Arc::new(installed),
                status: cache_status,
                storage: storage.ok(),
                refresh_required,
            }
        });
        cx.spawn(async move |this, cx| {
            let state = load.await;
            cx.update(|cx| {
                apply_scribe_theme(cx);
                cx.refresh_windows();
            });
            let service = state.storage.as_ref().map(|storage| {
                Arc::new(CatalogService::new(
                    storage.clone(),
                    Arc::new(EsouiClient::new(http.clone())),
                ))
            });
            let install_manager = state
                .storage
                .as_ref()
                .map(|storage| InstallManager::new(3, http, storage.clone(), None));
            let refresh_required = state.refresh_required && service.is_some();
            this.update(cx, |this, cx| {
                this.settings = state.settings;
                this.catalog_index = state.catalog_index;
                this.installed = state.installed;
                this.installed_index = state.installed_index;
                this.matched = state.matched;
                this.missing_dependencies = state.missing_dependencies;
                this.storage = state.storage.clone();
                this.status = state.status;
                this.catalog_service = service.clone();
                this.install_manager = install_manager.clone();
                this.loading = false;
                cx.notify();
            })
            .ok();
            trace_startup("catalog_ready");

            if refresh_required {
                this.update(cx, |this, cx| {
                    this.status = "Refreshing the ESOUI catalog in the background…".into();
                    cx.notify();
                })
                .ok();
                let service = service.clone().expect("service checked above");
                let refresh = cx
                    .background_executor()
                    .spawn(async move { service.refresh(&CancellationToken::default()).await });
                let result = refresh.await;
                this.update(cx, |this, cx| {
                    match result {
                        Ok((catalog, outcome)) => {
                            replace_catalog_state(this, catalog);
                            this.status = format!("ESOUI catalog refreshed ({outcome:?}).");
                        }
                        Err(error) => {
                            this.status = format!(
                                "ESOUI refresh failed: {error}. Cached data remains available."
                            );
                        }
                    }
                    cx.notify();
                })
                .ok();
            }

            if let (Some(storage), Some(service)) = (state.storage, service) {
                let matched = this
                    .update(cx, |app, _| app.matched.as_ref().clone())
                    .unwrap_or_default();
                let integrity = cx
                    .background_executor()
                    .spawn(async move { enrich_md5_decisions(storage, service, matched).await });
                if let Ok(decisions) = integrity.await {
                    this.update(cx, |app, cx| {
                        app.matched = Arc::new(decisions);
                        app.installed_index =
                            Arc::new(InstalledIndex::new(&app.installed, &app.matched));
                        cx.notify();
                    })
                    .ok();
                }
            }

            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let manager = match this.update(cx, |app, _| app.install_manager.clone()) {
                    Ok(Some(manager)) => manager,
                    Ok(None) => continue,
                    Err(_) => break,
                };
                let completed: Vec<String> = manager
                    .statuses()
                    .into_iter()
                    .filter(|task| task.state == TaskState::Complete)
                    .map(|task| task.uid)
                    .collect();
                let should_rescan = match this.update(cx, |app, cx| {
                    let mut changed = false;
                    for uid in completed {
                        changed |= app.observed_completions.insert(uid);
                    }
                    cx.notify();
                    changed
                }) {
                    Ok(changed) => changed,
                    Err(_) => break,
                };
                if should_rescan {
                    let snapshot = this.update(cx, |app, _| {
                        (
                            app.settings.addon_path.clone(),
                            app.storage.clone(),
                            app.catalog_index.clone(),
                        )
                    });
                    let Ok((addon_path, storage, catalog_index)) = snapshot else {
                        break;
                    };
                    if addon_path.is_empty() {
                        continue;
                    }
                    let scan = cx.background_executor().spawn(async move {
                        let scanner = Scanner::new(PathBuf::from(addon_path));
                        let scanner = match storage {
                            Some(storage) => scanner.with_storage(storage),
                            None => scanner,
                        };
                        scanner.scan()
                    });
                    match scan.await {
                        Ok(installed) => {
                            this.update(cx, |app, cx| {
                                replace_installed_state(app, installed, &catalog_index);
                                app.status = "Installation complete; AddOns rescanned.".into();
                                cx.notify();
                            })
                            .ok();
                        }
                        Err(error) => {
                            this.update(cx, |app, cx| {
                                app.status = format!(
                                    "Installation completed, but the rescan failed: {error}"
                                );
                                cx.notify();
                            })
                            .ok();
                        }
                    }
                }
            }
        })
        .detach();

        Self {
            page: Page::Installed,
            settings: AppSettings::default(),
            catalog_index: Arc::new(CatalogIndex::new(Arc::new(Catalog::default()))),
            installed: Arc::new(Vec::new()),
            installed_index: Arc::new(InstalledIndex::default()),
            matched: Arc::new(Vec::new()),
            missing_dependencies: Arc::new(Vec::new()),
            storage: None,
            catalog_service: None,
            install_manager: None,
            loading: true,
            status: "Loading local state…".into(),
            selected_details: None,
            selected_local: None,
            lightbox_index: None,
            pending_uninstall: Vec::new(),
            observed_completions: HashSet::new(),
        }
    }
}

fn replace_catalog_state(app: &mut AppModel, catalog: Arc<Catalog>) {
    let catalog_index = Arc::new(CatalogIndex::new(catalog));
    let (matched, missing_dependencies) = Matcher::analyze_index(&app.installed, &catalog_index);
    app.matched = Arc::new(matched);
    app.missing_dependencies = Arc::new(missing_dependencies);
    app.installed_index = Arc::new(InstalledIndex::new(&app.installed, &app.matched));
    app.catalog_index = catalog_index;
}

fn replace_installed_state(
    app: &mut AppModel,
    installed: Vec<Addon>,
    catalog_index: &CatalogIndex,
) {
    let (matched, missing_dependencies) = Matcher::analyze_index(&installed, catalog_index);
    let matched = Arc::new(matched);
    app.missing_dependencies = Arc::new(missing_dependencies);
    app.installed_index = Arc::new(InstalledIndex::new(&installed, &matched));
    app.matched = matched;
    app.installed = Arc::new(installed);
}

impl Drop for AppModel {
    fn drop(&mut self) {
        if let Some(manager) = self.install_manager.take() {
            manager.shutdown();
        }
    }
}

#[derive(Clone)]
struct InstalledGroup {
    id: String,
    name: String,
    icon_url: Option<String>,
    items: Vec<(Addon, MatchedAddon)>,
}

fn installed_groups(model: &AppModel, query: &str, updates_only: bool) -> Vec<InstalledGroup> {
    let indices = model.installed_index.search(query, updates_only);
    let categories: HashMap<&str, &Category> = model
        .catalog_index
        .categories()
        .iter()
        .map(|category| (category.id.as_str(), category))
        .collect();
    let mut groups: BTreeMap<String, InstalledGroup> = BTreeMap::new();
    for index in indices {
        let (Some(addon), Some(decision)) = (model.installed.get(index), model.matched.get(index))
        else {
            continue;
        };
        let remote_category = decision
            .remote
            .as_ref()
            .and_then(|remote| categories.get(remote.category_id.as_str()).copied());
        let (id, name, icon_url) = if addon.is_library {
            let category = remote_category
                .filter(|category| category.name.to_ascii_lowercase().contains("librar"));
            (
                category
                    .map(|category| category.id.to_string())
                    .unwrap_or_else(|| "libraries".into()),
                category
                    .map(|category| category.name.to_string())
                    .unwrap_or_else(|| "Libraries".into()),
                category
                    .filter(|category| !category.icon_url.is_empty())
                    .map(|category| category.icon_url.to_string()),
            )
        } else if let Some(category) = remote_category {
            (
                category.id.to_string(),
                category.name.to_string(),
                (!category.icon_url.is_empty()).then(|| category.icon_url.to_string()),
            )
        } else {
            ("other".into(), "Other Addons".into(), None)
        };
        groups
            .entry(format!("{}:{id}", name.to_ascii_lowercase()))
            .or_insert_with(|| InstalledGroup {
                id,
                name,
                icon_url,
                items: Vec::new(),
            })
            .items
            .push((addon.clone(), decision.clone()));
    }
    let mut groups: Vec<_> = groups.into_values().collect();
    for group in &mut groups {
        group
            .items
            .sort_unstable_by(|left, right| left.0.title.cmp(&right.0.title));
    }
    groups
}

struct ScribeWindow {
    model: Entity<AppModel>,
    installed: Entity<PageState>,
    find_more: Entity<PageState>,
    updates: Entity<PageState>,
    settings: Entity<PageState>,
    category_select: Entity<SelectState<SearchableVec<FilterOption>>>,
    content_select: Entity<SelectState<SearchableVec<FilterOption>>>,
    version_select: Entity<SelectState<SearchableVec<FilterOption>>>,
    sort_select: Entity<SelectState<SearchableVec<FilterOption>>>,
    category_option_count: usize,
    version_option_count: usize,
    hide_installed: bool,
    sort_ascending: bool,
    expanded_categories: HashSet<String>,
    installed_groups_initialized: bool,
    selected_folders: HashSet<String>,
    dismissed_required_dependencies: bool,
    dismissed_optional_dependencies: bool,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl ScribeWindow {
    fn new(model: Entity<AppModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let installed = cx.new(|cx| PageState::new("Search installed addons…", window, cx));
        let find_more = cx.new(|cx| PageState::new("Search the ESOUI catalog…", window, cx));
        let updates = cx.new(|cx| PageState::new("Search available updates…", window, cx));
        let settings = cx.new(|cx| PageState::new("Search settings…", window, cx));
        let category_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(vec![FilterOption::new("Category: Any", "")]),
                Some(IndexPath::default()),
                window,
                cx,
            )
            .searchable(true)
        });
        let content_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(vec![
                    FilterOption::new("All content", "all"),
                    FilterOption::new("Libraries only", "libraries"),
                ]),
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        let version_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(vec![FilterOption::new("All versions", "")]),
                Some(IndexPath::default()),
                window,
                cx,
            )
            .searchable(true)
        });
        let sort_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(vec![
                    FilterOption::new("Sort: Downloads", "downloads"),
                    FilterOption::new("Sort: Favorites", "favorites"),
                    FilterOption::new("Sort: Updated", "date"),
                    FilterOption::new("Sort: Title", "title"),
                    FilterOption::new("Sort: Author", "author"),
                    FilterOption::new("Sort: Category", "category"),
                ]),
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        let mut subscriptions: Vec<Subscription> = [&installed, &find_more, &updates, &settings]
            .into_iter()
            .map(|state| cx.observe(state, |_, _, cx| cx.notify()))
            .collect();
        subscriptions.push(cx.observe(&model, |_, _, cx| cx.notify()));
        subscriptions.push(cx.subscribe(
            &category_select,
            |_, _, _: &SelectEvent<SearchableVec<FilterOption>>, cx| cx.notify(),
        ));
        subscriptions.push(cx.subscribe(
            &content_select,
            |_, _, _: &SelectEvent<SearchableVec<FilterOption>>, cx| cx.notify(),
        ));
        subscriptions.push(cx.subscribe(
            &version_select,
            |_, _, _: &SelectEvent<SearchableVec<FilterOption>>, cx| cx.notify(),
        ));
        subscriptions.push(cx.subscribe(
            &sort_select,
            |_, _, _: &SelectEvent<SearchableVec<FilterOption>>, cx| cx.notify(),
        ));
        Self {
            model,
            installed,
            find_more,
            updates,
            settings,
            category_select,
            content_select,
            version_select,
            sort_select,
            category_option_count: 1,
            version_option_count: 1,
            hide_installed: true,
            sort_ascending: false,
            expanded_categories: HashSet::new(),
            installed_groups_initialized: false,
            selected_folders: HashSet::new(),
            dismissed_required_dependencies: false,
            dismissed_optional_dependencies: false,
            focus,
            _subscriptions: subscriptions,
        }
    }

    fn page_state(&self, page: Page) -> &Entity<PageState> {
        match page {
            Page::Installed => &self.installed,
            Page::FindMore => &self.find_more,
            Page::Updates => &self.updates,
            Page::Settings => &self.settings,
        }
    }

    fn sync_catalog_filter_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (mut categories, versions) = {
            let model = self.model.read(cx);
            (
                model.catalog_index.categories().to_vec(),
                model.catalog_index.compatibility_versions().to_vec(),
            )
        };
        categories.sort_unstable_by(|left, right| left.name.cmp(&right.name));
        if self.category_option_count != categories.len() + 1 {
            let selected = self
                .category_select
                .read(cx)
                .selected_value()
                .cloned()
                .unwrap_or_default();
            let mut options = Vec::with_capacity(categories.len() + 1);
            options.push(FilterOption::new("Category: Any", ""));
            options.extend(
                categories
                    .into_iter()
                    .filter(|category| category.count > 0)
                    .map(|category| {
                        FilterOption::new(
                            format!("{} ({})", category.name, category.count),
                            category.id.to_string(),
                        )
                        .with_icon(category.icon_url.to_string())
                    }),
            );
            self.category_option_count = options.len();
            self.category_select.update(cx, |state, cx| {
                state.set_items(SearchableVec::new(options), window, cx);
                state.set_selected_value(&selected, window, cx);
                if state.selected_value().is_none() {
                    state.set_selected_value(&SharedString::default(), window, cx);
                }
            });
        }
        let visible_versions: Vec<String> = versions.into_iter().take(64).collect();
        if self.version_option_count != visible_versions.len() + 1 {
            let selected = self
                .version_select
                .read(cx)
                .selected_value()
                .cloned()
                .unwrap_or_default();
            let mut options = Vec::with_capacity(visible_versions.len() + 1);
            options.push(FilterOption::new("All versions", ""));
            options.extend(
                visible_versions
                    .into_iter()
                    .map(|version| FilterOption::new(version.clone(), version)),
            );
            self.version_option_count = options.len();
            self.version_select.update(cx, |state, cx| {
                state.set_items(SearchableVec::new(options), window, cx);
                state.set_selected_value(&selected, window, cx);
                if state.selected_value().is_none() {
                    state.set_selected_value(&SharedString::default(), window, cx);
                }
            });
        }
    }

    fn selected_catalog_sort(&self, cx: &App) -> CatalogSort {
        match self
            .sort_select
            .read(cx)
            .selected_value()
            .map(SharedString::as_ref)
        {
            Some("title") => CatalogSort::Title,
            Some("author") => CatalogSort::Author,
            Some("category") => CatalogSort::Category,
            Some("favorites") => CatalogSort::Favorites,
            Some("date") => CatalogSort::Date,
            _ => CatalogSort::Downloads,
        }
    }

    fn render_toolbar(
        &self,
        page: Page,
        search: Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let toolbar = gpui::rgb(0x2a1f17);
        let toolbar_border = gpui::rgb(0x4b392a);
        let toolbar_input = gpui::rgb(0x3a2b20);
        let toolbar_foreground = gpui::rgb(0xf1e5d2);
        let toolbar_muted = gpui::rgb(0xc1ad91);
        let (installed_count, library_count, update_count) = {
            let model = self.model.read(cx);
            (
                model.installed.len(),
                model
                    .installed
                    .iter()
                    .filter(|addon| addon.is_library)
                    .count(),
                model
                    .matched
                    .iter()
                    .filter(|decision| decision.update_available)
                    .count(),
            )
        };
        let hide_installed = self.hide_installed;
        let direction_label = if self.sort_ascending { "Asc" } else { "Desc" };
        let toggle_hidden = cx.entity();
        let toggle_sort = cx.entity();
        let expand = cx.entity();
        let collapse = cx.entity();
        let select_visible = cx.entity();
        let clear_selection = cx.entity();
        let selected_count = self.selected_folders.len();
        let selected_for_remove: Vec<String> = self.selected_folders.iter().cloned().collect();
        let visible_folders: Vec<String> = if page == Page::Installed {
            let query = self.installed.read(cx).query.clone();
            let model = self.model.read(cx);
            installed_groups(model, &query, false)
                .into_iter()
                .flat_map(|group| group.items)
                .map(|(addon, _)| addon.folder_name)
                .collect()
        } else {
            Vec::new()
        };
        let refresh_model = self.model.clone();
        let bulk_model = self.model.clone();
        let rescan_model = self.model.clone();
        let title_row = div()
            .h(px(48.0))
            .px(px(16.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .size(px(28.0))
                            .rounded(px(5.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(gpui::rgb(0x493625))
                            .child(Icon::new(page.icon()).size(px(15.0))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().font_semibold().child(page.title()))
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(toolbar_muted)
                                    .child(page.subtitle()),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .when(page == Page::FindMore, |row| {
                        row.child(
                            div()
                                .id("hide-installed-filter")
                                .role(Role::Switch)
                                .aria_selected(hide_installed)
                                .aria_label("Hide installed addons")
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .gap(px(7.0))
                                .on_click(move |_, _, cx| {
                                    toggle_hidden.update(cx, |this, cx| {
                                        this.hide_installed = !this.hide_installed;
                                        cx.notify();
                                    });
                                })
                                .child(
                                    div()
                                        .w(px(28.0))
                                        .h(px(16.0))
                                        .p(px(2.0))
                                        .rounded(px(8.0))
                                        .bg(if hide_installed {
                                            gpui::rgb(0xa76e35)
                                        } else {
                                            toolbar_input
                                        })
                                        .flex()
                                        .justify_end()
                                        .when(!hide_installed, |track| track.justify_start())
                                        .child(
                                            div().size(px(12.0)).rounded(px(6.0)).bg(gpui::white()),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(toolbar_muted)
                                        .child("Hide installed"),
                                ),
                        )
                        .child(
                            NativeButton::new("refresh-catalog", "Refresh")
                                .secondary()
                                .icon(IconName::LoaderCircle)
                                .on_activate(move |window, cx| {
                                    refresh_catalog(refresh_model.clone(), window, cx)
                                }),
                        )
                        .child(
                            NativeButton::new("sort-direction", direction_label)
                                .secondary()
                                .icon(IconName::ChevronsUpDown)
                                .on_activate(move |_, cx| {
                                    toggle_sort.update(cx, |this, cx| {
                                        this.sort_ascending = !this.sort_ascending;
                                        cx.notify();
                                    });
                                }),
                        )
                    })
                    .when(page == Page::Installed, |row| {
                        row.child(
                            div()
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded(px(5.0))
                                .bg(gpui::rgb(0xf5ece0))
                                .text_color(gpui::rgb(0x2c1f14))
                                .text_size(px(11.0))
                                .font_semibold()
                                .child(format!("{installed_count} addons • {library_count} libs")),
                        )
                        .when(selected_count > 0, |row| {
                            row.child(
                                NativeButton::new(
                                    "remove-selected",
                                    format!("Remove {selected_count} selected"),
                                )
                                .danger()
                                .icon(IconName::CircleX)
                                .on_activate(move |_, cx| {
                                    bulk_model.update(cx, |app, cx| {
                                        app.pending_uninstall = selected_for_remove.clone();
                                        cx.notify();
                                    });
                                }),
                            )
                        })
                        .child(
                            NativeButton::new("rescan-toolbar", "Rescan")
                                .ghost()
                                .icon(IconName::LoaderCircle)
                                .on_activate(move |window, cx| {
                                    rescan_configured_addons(rescan_model.clone(), window, cx)
                                }),
                        )
                    })
                    .when(page == Page::Updates, |row| {
                        row.child(
                            div()
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded(px(5.0))
                                .bg(gpui::rgb(0xf5ece0))
                                .text_color(gpui::rgb(0x2c1f14))
                                .text_size(px(11.0))
                                .font_semibold()
                                .child(format!("{update_count} updates")),
                        )
                    }),
            );

        let filter_row = div()
            .h(px(45.0))
            .px(px(16.0))
            .pb(px(10.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(div().w(px(180.0)).flex_none().child(Input::new(&search)))
            .when(page == Page::FindMore, |row| {
                row.child(
                    div()
                        .id("category-filter-control")
                        .role(Role::ComboBox)
                        .aria_label("Category filter")
                        .child(
                            Select::new(&self.category_select)
                                .small()
                                .w(px(175.0))
                                .menu_width(px(280.0))
                                .search_placeholder("Search categories"),
                        ),
                )
                .child(
                    div()
                        .id("content-filter-control")
                        .role(Role::ComboBox)
                        .aria_label("Content filter")
                        .child(Select::new(&self.content_select).small().w(px(120.0))),
                )
                .child(
                    div()
                        .id("version-filter-control")
                        .role(Role::ComboBox)
                        .aria_label("Game version filter")
                        .child(
                            Select::new(&self.version_select)
                                .small()
                                .w(px(130.0))
                                .menu_width(px(220.0))
                                .search_placeholder("Search versions"),
                        ),
                )
                .child(
                    div()
                        .id("sort-filter-control")
                        .role(Role::ComboBox)
                        .aria_label("Catalog sort")
                        .child(Select::new(&self.sort_select).small().w(px(145.0))),
                )
            })
            .when(page == Page::Installed, |row| {
                row.child(
                    NativeButton::new("expand-installed", "Expand all")
                        .secondary()
                        .on_activate(move |_, cx| {
                            expand.update(cx, |this, cx| {
                                let model = this.model.read(cx);
                                this.expanded_categories = installed_groups(model, "", false)
                                    .into_iter()
                                    .map(|group| group.id)
                                    .collect();
                                cx.notify();
                            });
                        }),
                )
                .child(
                    NativeButton::new("select-visible-installed", "Select visible")
                        .secondary()
                        .on_activate(move |_, cx| {
                            select_visible.update(cx, |this, cx| {
                                this.selected_folders.extend(visible_folders.clone());
                                cx.notify();
                            });
                        }),
                )
                .child(
                    NativeButton::new("clear-installed-selection", "Clear selection")
                        .secondary()
                        .on_activate(move |_, cx| {
                            clear_selection.update(cx, |this, cx| {
                                this.selected_folders.clear();
                                cx.notify();
                            });
                        }),
                )
                .child(
                    NativeButton::new("collapse-installed", "Collapse all")
                        .secondary()
                        .on_activate(move |_, cx| {
                            collapse.update(cx, |this, cx| {
                                this.expanded_categories.clear();
                                cx.notify();
                            });
                        }),
                )
            });

        div()
            .w_full()
            .flex()
            .flex_col()
            .bg(toolbar)
            .border_b_1()
            .border_color(toolbar_border)
            .text_color(toolbar_foreground)
            .child(title_row)
            .when(page != Page::Settings, |toolbar| toolbar.child(filter_row))
            .into_any_element()
    }

    fn render_nav_button(
        &self,
        page: Page,
        active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let count = {
            let model = self.model.read(cx);
            match page {
                Page::Installed => model.installed.len(),
                Page::FindMore => model.catalog_index.len(),
                Page::Updates => model
                    .matched
                    .iter()
                    .filter(|item| item.update_available)
                    .count(),
                Page::Settings => 0,
            }
        };
        let model = self.model.clone();
        let keyboard_model = self.model.clone();
        div()
            .id(SharedString::from(format!("nav-{}", page.title())))
            .focusable()
            .tab_stop(true)
            .role(Role::Button)
            .aria_label(page.title())
            .w_full()
            .h(px(42.0))
            .px(px(11.0))
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(7.0))
            .cursor_pointer()
            .bg(if active {
                theme.sidebar_accent
            } else {
                theme.sidebar
            })
            .text_color(if active {
                theme.sidebar_accent_foreground
            } else {
                theme.sidebar_foreground.opacity(0.78)
            })
            .on_click(move |_, _, cx| {
                model.update(cx, |model, cx| {
                    model.page = page;
                    cx.notify();
                });
            })
            .on_key_down(move |event, _, cx| {
                if !event.is_held && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    keyboard_model.update(cx, |model, cx| {
                        model.page = page;
                        cx.notify();
                    });
                }
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(Icon::new(page.icon()).size(px(17.0)))
                    .child(div().font_medium().child(page.title())),
            )
            .when(count > 0, |button| {
                button.child(
                    div()
                        .min_w(px(24.0))
                        .h(px(20.0))
                        .px(px(6.0))
                        .rounded(px(10.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(11.0))
                        .bg(if active {
                            theme.sidebar_primary.opacity(0.18)
                        } else {
                            theme.sidebar_accent.opacity(0.55)
                        })
                        .child(count.to_string()),
                )
            })
    }

    fn render_installed(
        &mut self,
        query: &str,
        updates_only: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let model = self.model.read(cx);
        self.selected_folders.retain(|folder| {
            model
                .installed
                .iter()
                .any(|addon| addon.folder_name == *folder)
        });
        let missing = model.missing_dependencies.clone();
        let groups = installed_groups(model, query, updates_only);
        if !self.installed_groups_initialized && !groups.is_empty() {
            self.expanded_categories = groups.iter().map(|group| group.id.clone()).collect();
            self.installed_groups_initialized = true;
        }
        if groups.is_empty() {
            let (title, message) = if updates_only {
                (
                    "Everything is up to date",
                    "No installed addons currently have a newer ESOUI release.",
                )
            } else if query.trim().is_empty() {
                (
                    "No addons detected",
                    "Choose the ESO AddOns folder in Settings, then run a rescan.",
                )
            } else {
                (
                    "No matching addons",
                    "Try a different name, author, folder, or version.",
                )
            };
            return empty_state(IconName::Inbox, title, message);
        }
        let category_map: HashMap<String, Category> = model
            .catalog_index
            .categories()
            .iter()
            .cloned()
            .map(|category| (category.id.to_string(), category))
            .collect();
        let mut rows = Vec::with_capacity(groups.len());
        let selection_owner = cx.entity();
        for group in groups {
            let expanded = self.expanded_categories.contains(&group.id);
            let group_id = group.id.clone();
            let toggle = cx.entity();
            let icon = group.icon_url.clone();
            let count = group.items.len();
            let name = group.name.clone();
            let mut section = div().w_full().flex().flex_col().gap(px(4.0)).child(
                div()
                    .id(SharedString::from(format!(
                        "installed-category-{}",
                        group.id
                    )))
                    .role(Role::Button)
                    .aria_expanded(expanded)
                    .aria_label(format!("{name}, {count} addons"))
                    .cursor_pointer()
                    .h(px(39.0))
                    .px(px(12.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(gpui::rgb(0xc9b89e))
                    .bg(gpui::rgb(0xf8f0e5))
                    .hover(|header| header.bg(gpui::rgb(0xeadcc8)))
                    .flex()
                    .items_center()
                    .justify_between()
                    .on_click(move |_, _, cx| {
                        toggle.update(cx, |this, cx| {
                            if !this.expanded_categories.remove(&group_id) {
                                this.expanded_categories.insert(group_id.clone());
                            }
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(9.0))
                            .child(Icon::new(if expanded {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            }))
                            .child(category_artwork(icon, &name, 22.0))
                            .child(div().font_medium().child(name)),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .opacity(0.7)
                            .child(count.to_string()),
                    ),
            );
            if expanded {
                for (addon, decision) in group.items {
                    let category = decision
                        .remote
                        .as_ref()
                        .and_then(|remote| category_map.get(remote.category_id.as_str()))
                        .cloned();
                    let selected =
                        (!updates_only).then(|| self.selected_folders.contains(&addon.folder_name));
                    section = section.child(matched_row(
                        addon,
                        decision,
                        category,
                        selected,
                        selection_owner.clone(),
                        self.model.clone(),
                    ));
                }
            }
            rows.push(section.into_any_element());
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .when(!updates_only && !missing.is_empty(), |layout| {
                layout.child(render_missing_dependencies(
                    missing,
                    self.model.clone(),
                    cx.entity(),
                    self.dismissed_required_dependencies,
                    self.dismissed_optional_dependencies,
                ))
            })
            .child(
                div()
                    .min_h_0()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .px(px(16.0))
                    .py(px(10.0))
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_catalog(&self, query: &str, cx: &mut Context<Self>) -> gpui::AnyElement {
        let model = self.model.read(cx);
        let catalog_index = model.catalog_index.clone();
        let category_id = self
            .category_select
            .read(cx)
            .selected_value()
            .filter(|value| !value.is_empty())
            .map(SharedString::to_string);
        let libraries_only = self
            .content_select
            .read(cx)
            .selected_value()
            .is_some_and(|value| value.as_ref() == "libraries");
        let compatibility = self
            .version_select
            .read(cx)
            .selected_value()
            .filter(|value| !value.is_empty())
            .map(SharedString::to_string);
        let hidden_uids: HashSet<String> = if self.hide_installed {
            model
                .matched
                .iter()
                .filter_map(|decision| {
                    decision
                        .remote
                        .as_ref()
                        .map(|remote| remote.uid.to_string())
                })
                .collect()
        } else {
            HashSet::new()
        };
        let indices: Arc<Vec<usize>> = Arc::new(catalog_index.filter_sort(
            query,
            category_id.as_deref(),
            libraries_only,
            compatibility.as_deref(),
            &hidden_uids,
            self.selected_catalog_sort(cx),
            self.sort_ascending,
        ));
        let count = indices.len();
        if count == 0 {
            let (title, message) = if query.trim().is_empty() {
                (
                    "Catalog is not ready",
                    "Scribe will show ESOUI addons here after the catalog refresh completes.",
                )
            } else {
                (
                    "No catalog matches",
                    "Try a broader addon name, author, category, or folder search.",
                )
            };
            return empty_state(IconName::Search, title, message);
        }
        let app_model = self.model.clone();
        let category_index = catalog_index.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(33.0))
                    .px(px(18.0))
                    .flex()
                    .items_center()
                    .text_size(px(11.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{count} results")),
            )
            .child(
                div().min_h_0().flex_1().child(
                    uniform_list("catalog-list", count, move |range: Range<usize>, _, _| {
                        range
                            .filter_map(|index| indices.get(index))
                            .filter_map(|index| category_index.addon(*index))
                            .map(|addon| {
                                let category = category_index.category(&addon.category_id).cloned();
                                catalog_row(addon, category, app_model.clone())
                            })
                            .collect()
                    })
                    .size_full(),
                ),
            )
            .into_any_element()
    }

    fn render_settings_page(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (path, storage_degraded, installed_count, catalog_count, memory_limit) = {
            let model = self.model.read(cx);
            (
                if model.settings.addon_path.is_empty() {
                    "No AddOns folder selected".to_owned()
                } else {
                    model.settings.addon_path.clone()
                },
                model.storage.is_none(),
                model.installed.len(),
                model.catalog_index.len(),
                model.settings.memory_limit_mb,
            )
        };
        let browse_model = self.model.clone();
        let rescan_model = self.model.clone();
        let theme_model = self.model.clone();
        let rebuild_model = self.model.clone();
        let reveal_path = path.clone();
        let copy_path = path.clone();
        div()
            .size_full()
            .overflow_y_scrollbar()
            .px(px(24.0))
            .py(px(20.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                settings_card(
                    IconName::FolderOpen,
                    "Addon library",
                    "Scribe scans and safely changes addons only inside this folder.",
                )
                .child(
                    div()
                        .mt(px(15.0))
                        .p(px(12.0))
                        .rounded(px(8.0))
                        .bg(cx.theme().muted.opacity(0.55))
                        .text_size(px(12.0))
                        .font_family("Consolas")
                        .child(path),
                )
                .child(
                    div()
                        .mt(px(12.0))
                        .flex()
                        .flex_wrap()
                        .gap(px(8.0))
                        .child(
                            NativeButton::new("browse-addons", "Choose folder")
                                .icon(IconName::FolderOpen)
                                .on_activate(move |window, cx| {
                                    browse_for_addons(browse_model.clone(), window, cx);
                                }),
                        )
                        .child(
                            NativeButton::new("rescan-addons", "Rescan now")
                                .secondary()
                                .icon(IconName::Search)
                                .on_activate(move |window, cx| {
                                    rescan_configured_addons(rescan_model.clone(), window, cx);
                                }),
                        )
                        .child(
                            NativeButton::new("open-addons", "Open folder")
                                .ghost()
                                .icon(IconName::ExternalLink)
                                .on_activate(move |_, cx| {
                                    let path = PathBuf::from(&reveal_path);
                                    if path.is_absolute() {
                                        cx.open_with_system(&path);
                                    }
                                }),
                        )
                        .child(
                            NativeButton::new("copy-addons", "Copy path")
                                .ghost()
                                .icon(IconName::Copy)
                                .on_activate(move |_, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        copy_path.clone(),
                                    ));
                                }),
                        ),
                ),
            )
            .child(
                settings_card(
                    IconName::Palette,
                    "Appearance",
                    "Scribe uses its original high-contrast parchment palette across the whole app.",
                )
                .child(
                    div()
                        .mt(px(14.0))
                        .flex()
                        .gap(px(8.0))
                        .child(
                            NativeButton::new("theme-scribe", "Scribe parchment  ✓")
                            .secondary()
                            .icon(IconName::Palette)
                            .on_activate(move |_, cx| {
                                set_app_theme("scribe", theme_model.clone(), cx)
                            }),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .text_size(px(11.0))
                                .text_color(cx.theme().muted_foreground)
                                .child("Warm surface, dark chrome, accessible controls"),
                        ),
                ),
            )
            .child(
                settings_card(
                    IconName::HardDrive,
                    "Local data & diagnostics",
                    "Catalog data is cached for fast offline startup; install records remain versioned separately.",
                )
                .child(
                    div()
                        .mt(px(14.0))
                        .flex()
                        .gap(px(10.0))
                        .child(metric_pill(installed_count.to_string(), "installed"))
                        .child(metric_pill(catalog_count.to_string(), "catalog"))
                        .child(metric_pill(format!("{memory_limit} MB"), "warning threshold"))
                        .child(metric_pill(
                            if storage_degraded { "Degraded" } else { "Healthy" },
                            "storage",
                        )),
                )
                .when(storage_degraded, |card| {
                    card.child(
                        div().mt(px(12.0)).child(
                            NativeButton::new("rebuild-storage-page", "Rebuild local cache")
                                .danger()
                                .on_activate(move |window, cx| {
                                    rebuild_local_storage(rebuild_model.clone(), window, cx);
                                }),
                        ),
                    )
                }),
            )
            .child(Alert::info(
                "auto-update-inactive",
                "Automatic updates stay off. Scribe always lets you review an update before changing addon files.",
            ))
            .child(
                settings_card(
                    IconName::Info,
                    "About Scribe",
                    "A fast, native ESO addon manager powered by GPUI. Addon metadata and downloads are provided by ESOUI/MMOUI.",
                )
                .child(
                    div().mt(px(12.0)).child(
                        NativeButton::new("open-esoui", "Visit ESOUI")
                            .ghost()
                            .icon(IconName::ExternalLink)
                            .on_activate(|_, cx| cx.open_url("https://www.esoui.com/")),
                    ),
                ),
            )
            .into_any_element()
    }
}

impl Render for ScribeWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_catalog_filter_options(window, cx);
        let theme = cx.theme().clone();
        let body_color = theme.background;
        let text_color = theme.foreground;
        let border_color = theme.border;
        let (
            page,
            status,
            loading,
            tasks,
            selected_details,
            selected_local,
            lightbox_index,
            pending_uninstall,
        ) = {
            let model = self.model.read(cx);
            (
                model.page,
                model.status.clone(),
                model.loading,
                model
                    .install_manager
                    .as_ref()
                    .map(|manager| manager.statuses())
                    .unwrap_or_default(),
                model.selected_details.clone(),
                model.selected_local.clone(),
                model.lightbox_index,
                model.pending_uninstall.clone(),
            )
        };
        let query = self.page_state(page).read(cx).query.clone();
        let search = self.page_state(page).read(cx).search.clone();
        let selected_category = selected_details.as_ref().and_then(|details| {
            self.model
                .read(cx)
                .catalog_index
                .category(&details.addon.category_id)
                .cloned()
        });
        let has_selected_local = selected_local.is_some();
        let toolbar = self.render_toolbar(page, search.clone(), cx);
        let escape_model = self.model.clone();
        let status_is_error = {
            let normalized = status.to_ascii_lowercase();
            normalized.contains("failed")
                || normalized.contains("could not")
                || normalized.contains("unavailable")
                || normalized.contains("error")
        };

        let content = match page {
            Page::Installed => self.render_installed(&query, false, cx),
            Page::Updates => self.render_installed(&query, true, cx),
            Page::FindMore => self.render_catalog(&query, cx),
            Page::Settings => self.render_settings_page(cx),
        };

        let body = div()
            .id("scribe-root")
            .role(Role::Application)
            .aria_label("Scribe ESO addon manager")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &Tab, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &TabPrevious, window, cx| window.focus_prev(cx)))
            .on_action(cx.listener(|this, _: &ShowInstalled, _, cx| {
                this.model.update(cx, |app, cx| {
                    app.page = Page::Installed;
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &ShowFindMore, _, cx| {
                this.model.update(cx, |app, cx| {
                    app.page = Page::FindMore;
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &ShowUpdates, _, cx| {
                this.model.update(cx, |app, cx| {
                    app.page = Page::Updates;
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                let page = this.model.read(cx).page;
                let input = this.page_state(page).read(cx).search.clone();
                window.focus(&input.read(cx).focus_handle(cx), cx);
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, _, cx| {
                this.model.update(cx, |app, cx| {
                    app.page = Page::Settings;
                    cx.notify();
                });
            }))
            .on_key_down(move |event, _, cx| {
                if event.is_held {
                    return;
                }
                if event.keystroke.key == "escape" {
                    escape_model.update(cx, |app, cx| {
                        if app.lightbox_index.take().is_none() {
                            app.selected_details = None;
                            app.selected_local = None;
                            app.pending_uninstall.clear();
                        }
                        cx.notify();
                    });
                } else if matches!(event.keystroke.key.as_str(), "left" | "right") {
                    escape_model.update(cx, |app, cx| {
                        let Some(index) = app.lightbox_index else {
                            return;
                        };
                        let screenshot_count = app
                            .selected_details
                            .as_ref()
                            .map(|details| details.addon.ui_imgs.len().min(12))
                            .unwrap_or_default();
                        if screenshot_count == 0 {
                            return;
                        }
                        app.lightbox_index = Some(if event.keystroke.key == "left" {
                            index.checked_sub(1).unwrap_or(screenshot_count - 1)
                        } else {
                            (index + 1) % screenshot_count
                        });
                        cx.notify();
                    });
                }
            })
            .size_full()
            .flex()
            .bg(body_color)
            .text_color(text_color)
            .child(
                div()
                    .w(px(208.0))
                    .h_full()
                    .px(px(14.0))
                    .py(px(18.0))
                    .border_r_1()
                    .border_color(theme.sidebar_border)
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .bg(theme.sidebar)
                    .text_color(theme.sidebar_foreground)
                    .child(
                        div()
                            .px(px(10.0))
                            .pb(px(7.0))
                            .text_size(px(10.0))
                            .font_semibold()
                            .opacity(0.48)
                            .child("LIBRARY"),
                    )
                    .child(self.render_nav_button(Page::Installed, page == Page::Installed, cx))
                    .child(self.render_nav_button(Page::FindMore, page == Page::FindMore, cx))
                    .child(self.render_nav_button(Page::Updates, page == Page::Updates, cx))
                    .child(div().flex_1())
                    .child(
                        div()
                            .mx(px(10.0))
                            .mb(px(8.0))
                            .pt(px(12.0))
                            .border_t_1()
                            .border_color(theme.sidebar_border)
                            .text_size(px(11.0))
                            .opacity(0.56)
                            .child(if status_is_error {
                                "Action needed - see banner"
                            } else if loading {
                                "Loading local library…"
                            } else {
                                "Local library ready"
                            }),
                    )
                    .child(self.render_nav_button(Page::Settings, page == Page::Settings, cx)),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(toolbar)
                    .when(status_is_error, |layout| {
                        layout.child(
                            div()
                                .min_h(px(36.0))
                                .px(px(16.0))
                                .py(px(8.0))
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .border_b_1()
                                .border_color(border_color)
                                .bg(theme.danger.opacity(0.09))
                                .text_color(theme.danger)
                                .text_size(px(12.0))
                                .child(Icon::new(IconName::TriangleAlert))
                                .child(status),
                        )
                    })
                    .child(div().min_h_0().flex_1().child(content))
                    .when(!tasks.is_empty(), |layout| {
                        layout.child(render_task_center(tasks, border_color, self.model.clone()))
                    }),
            );

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(body_color)
            .text_color(text_color)
            .child(
                TitleBar::new().child(
                    h_flex().w_full().child(
                        div()
                            .pl(px(11.0))
                            .flex()
                            .items_center()
                            .gap(px(7.0))
                            .child(
                                img("scribe-logo.png")
                                    .size(px(18.0))
                                    .object_fit(ObjectFit::Contain),
                            )
                            .child(
                                div()
                                    .font_medium()
                                    .text_size(px(12.0))
                                    .text_color(gpui::rgb(0xd9c4a5))
                                    .child("Scribe"),
                            ),
                    ),
                ),
            )
            .child(div().min_h_0().flex_1().child(body))
            .when_some(selected_local, |root, local| {
                root.child(render_local_details_modal(
                    local,
                    selected_details.clone(),
                    selected_category.clone(),
                    self.model.clone(),
                ))
            })
            .when(!has_selected_local && selected_details.is_some(), |root| {
                root.child(render_details_modal(
                    selected_details.clone().expect("checked above"),
                    selected_category.clone(),
                    self.model.clone(),
                ))
            })
            .when(!pending_uninstall.is_empty(), |root| {
                root.child(render_uninstall_modal(
                    pending_uninstall.clone(),
                    self.model.clone(),
                ))
            })
            .when_some(lightbox_index, |root, index| {
                root.when_some(selected_details, |root, details| {
                    root.child(render_lightbox(details, index, self.model.clone()))
                })
            })
    }
}

fn empty_state(icon: IconName, title: &'static str, message: &'static str) -> gpui::AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .max_w(px(420.0))
                .px(px(28.0))
                .py(px(36.0))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(9.0))
                .text_center()
                .child(
                    div()
                        .size(px(48.0))
                        .rounded(px(15.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui::hsla(0.58, 0.72, 0.5, 0.12))
                        .text_color(gpui::hsla(0.58, 0.72, 0.5, 1.0))
                        .child(Icon::new(icon).size(px(23.0))),
                )
                .child(div().font_semibold().text_size(px(16.0)).child(title))
                .child(div().opacity(0.66).text_size(px(13.0)).child(message)),
        )
        .into_any_element()
}

fn settings_card(icon: IconName, title: &'static str, description: &'static str) -> gpui::Div {
    div()
        .w_full()
        .p(px(18.0))
        .rounded(px(12.0))
        .border_1()
        .border_color(gpui::hsla(0.0, 0.0, 0.5, 0.14))
        .bg(gpui::black().opacity(0.015))
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(12.0))
                .child(
                    div()
                        .size(px(34.0))
                        .rounded(px(9.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui::hsla(0.58, 0.72, 0.5, 0.12))
                        .text_color(gpui::hsla(0.58, 0.72, 0.5, 1.0))
                        .child(Icon::new(icon).size(px(18.0))),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(div().font_semibold().child(title))
                        .child(div().text_size(px(12.0)).opacity(0.64).child(description)),
                ),
        )
}

fn metric_pill(value: impl Into<SharedString>, label: &'static str) -> gpui::Div {
    div()
        .px(px(11.0))
        .py(px(8.0))
        .rounded(px(8.0))
        .bg(gpui::black().opacity(0.045))
        .flex()
        .flex_col()
        .gap(px(1.0))
        .child(
            div()
                .font_semibold()
                .text_size(px(12.0))
                .child(value.into()),
        )
        .child(div().text_size(px(10.0)).opacity(0.58).child(label))
}

fn addon_artwork(source: Option<String>, title: &str) -> gpui::AnyElement {
    let frame = div()
        .size(px(48.0))
        .flex_none()
        .rounded(px(10.0))
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::hsla(0.58, 0.55, 0.5, 0.12));
    match source {
        Some(source) if !source.is_empty() => {
            frame.child(img(source).size_full()).into_any_element()
        }
        _ => frame
            .text_color(gpui::hsla(0.58, 0.68, 0.5, 1.0))
            .font_semibold()
            .text_size(px(16.0))
            .child(
                title
                    .chars()
                    .find(|character| character.is_alphanumeric())
                    .map(|character| character.to_uppercase().to_string())
                    .unwrap_or_else(|| "S".into()),
            )
            .into_any_element(),
    }
}

fn category_artwork(source: Option<String>, title: &str, extent: f32) -> gpui::AnyElement {
    let frame = div()
        .size(px(extent))
        .flex_none()
        .rounded(px((extent * 0.18).max(3.0)))
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::rgb(0xe0d0b7));
    match source {
        Some(source) if !source.is_empty() => {
            frame.child(img(source).size_full()).into_any_element()
        }
        _ => frame
            .text_color(gpui::rgb(0x7a4f2e))
            .font_semibold()
            .text_size(px((extent * 0.55).max(9.0)))
            .child(
                title
                    .chars()
                    .find(|character| character.is_alphanumeric())
                    .map(|character| character.to_uppercase().to_string())
                    .unwrap_or_else(|| "?".into()),
            )
            .into_any_element(),
    }
}

fn format_count(value: i64) -> String {
    let value = value.max(0) as f64;
    if value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}

fn matched_row(
    addon: Addon,
    decision: MatchedAddon,
    category: Option<Category>,
    selected: Option<bool>,
    selection_owner: Entity<ScribeWindow>,
    model: Entity<AppModel>,
) -> gpui::AnyElement {
    let thumbnail = decision
        .remote
        .as_ref()
        .and_then(|remote| remote.ui_img_thumbs.first())
        .map(ToString::to_string);
    let update = decision
        .remote
        .clone()
        .filter(|_| decision.update_available);
    let folder_name = addon.folder_name.clone();
    let selection_folder = addon.folder_name.clone();
    let details_model = model.clone();
    let update_model = model.clone();
    let uninstall_model = model.clone();
    let detail_addon = addon.clone();
    let detail_decision = decision.clone();
    let keyboard_addon = addon.clone();
    let keyboard_decision = decision.clone();
    let row_id = addon.folder_name.clone();
    let category_name = category
        .as_ref()
        .map(|category| category.name.to_string())
        .unwrap_or_else(|| {
            if addon.is_library {
                "Library".into()
            } else {
                "Addon".into()
            }
        });
    div()
        .id(SharedString::from(format!("installed-addon-{row_id}")))
        .role(Role::Button)
        .aria_label(format!("Open details for {}", addon.title))
        .cursor_pointer()
        .h(px(67.0))
        .w_full()
        .px(px(12.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(gpui::rgb(0xc9b89e))
        .bg(gpui::rgb(0xf5ece0))
        .flex()
        .items_center()
        .justify_between()
        .hover(|row| row.bg(gpui::rgb(0xeadcc8)))
        .on_click(move |_, window, cx| {
            show_installed_details(
                detail_addon.clone(),
                detail_decision.clone(),
                details_model.clone(),
                window,
                cx,
            );
        })
        .on_key_down(move |event, window, cx| {
            if !event.is_held && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                cx.stop_propagation();
                show_installed_details(
                    keyboard_addon.clone(),
                    keyboard_decision.clone(),
                    model.clone(),
                    window,
                    cx,
                );
            }
        })
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(12.0))
                .when_some(selected, |row, selected| {
                    row.child(
                        Checkbox::new(format!("select-{selection_folder}"))
                            .small()
                            .aria_label(format!("Select {}", addon.title))
                            .tooltip(format!("Select {}", addon.title))
                            .checked(selected)
                            .on_click(move |checked, _, cx| {
                                cx.stop_propagation();
                                selection_owner.update(cx, |window, cx| {
                                    if *checked {
                                        window.selected_folders.insert(selection_folder.clone());
                                    } else {
                                        window.selected_folders.remove(&selection_folder);
                                    }
                                    cx.notify();
                                });
                            }),
                    )
                })
                .child(addon_artwork(thumbnail, &addon.title))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(div().font_semibold().child(addon.title.clone()))
                                .child(
                                    div()
                                        .px(px(7.0))
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .bg(gpui::rgb(0xeadcc8))
                                        .text_size(px(10.0))
                                        .child(format!("v{}", addon.version)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .text_size(px(11.0))
                                .opacity(0.67)
                                .child(if addon.author.is_empty() {
                                    addon.folder_name.clone()
                                } else {
                                    format!("{}  •  {}", addon.author, addon.folder_name)
                                })
                                .child(category_name),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .opacity(0.58)
                                .child(decision.update_reason.clone()),
                        ),
                ),
        )
        .child(
            Group::new()
                .gap(Size::XSmall)
                .child(
                    div()
                        .mr(px(5.0))
                        .flex()
                        .flex_col()
                        .items_end()
                        .text_size(px(11.0))
                        .child(format!("Installed v{}", addon.version))
                        .child(div().opacity(0.62).child(decision.update_state.clone())),
                )
                .when_some(update, move |group, remote| {
                    group.child(
                        NativeButton::new(format!("update-{}", remote.uid), "Update").on_activate(
                            move |window, cx| {
                                enqueue_remote(remote.clone(), update_model.clone(), window, cx);
                            },
                        ),
                    )
                })
                .child(
                    NativeButton::new(format!("uninstall-{folder_name}"), "Remove")
                        .ghost()
                        .on_activate(move |_, cx| {
                            uninstall_model.update(cx, |app, cx| {
                                app.pending_uninstall = vec![folder_name.clone()];
                                cx.notify();
                            });
                        }),
                ),
        )
        .into_any_element()
}

fn catalog_row(
    addon: RemoteAddon,
    category: Option<Category>,
    model: Entity<AppModel>,
) -> gpui::AnyElement {
    let title = addon.ui_name.clone();
    let author = addon.ui_author_name.clone();
    let version = addon.ui_version.clone();
    let uid = addon.uid.clone();
    let thumbnail = addon.ui_img_thumbs.first().map(ToString::to_string);
    let downloads = format_count(addon.ui_download_total);
    let favorites = format_count(addon.ui_favorite_total);
    let category_name = category
        .as_ref()
        .map(|category| category.name.to_string())
        .unwrap_or_else(|| "Other".into());
    let category_icon = category
        .filter(|category| !category.icon_url.is_empty())
        .map(|category| category.icon_url.to_string());
    let compatibility = addon
        .compatabilities
        .first()
        .map(|version| {
            if version.name.is_empty() {
                version.version.to_string()
            } else {
                version.name.to_string()
            }
        })
        .unwrap_or_default();
    let row_remote = addon.clone();
    let keyboard_remote = addon.clone();
    let details_model = model.clone();
    let keyboard_model = model.clone();
    let install_model = model.clone();
    div()
        .id(SharedString::from(format!("catalog-addon-{uid}")))
        .role(Role::Button)
        .aria_label(format!("Open details for {title}"))
        .cursor_pointer()
        .h(px(68.0))
        .w_full()
        .mx(px(16.0))
        .px(px(12.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(gpui::rgb(0xc9b89e))
        .bg(gpui::rgb(0xf5ece0))
        .flex()
        .items_center()
        .justify_between()
        .hover(|row| row.bg(gpui::rgb(0xeadcc8)))
        .on_click(move |_, window, cx| {
            show_addon_details(row_remote.clone(), details_model.clone(), window, cx);
        })
        .on_key_down(move |event, window, cx| {
            if !event.is_held && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                cx.stop_propagation();
                show_addon_details(keyboard_remote.clone(), keyboard_model.clone(), window, cx);
            }
        })
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(addon_artwork(thumbnail, &title))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(div().font_semibold().child(title.to_string()))
                                .child(
                                    div()
                                        .px(px(7.0))
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .bg(gpui::rgb(0xeadcc8))
                                        .text_size(px(10.0))
                                        .child(version.to_string()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .gap(px(8.0))
                                .text_size(px(11.0))
                                .opacity(0.68)
                                .child(author.to_string())
                                .child(format!("{downloads} downloads"))
                                .child(format!("{favorites} favorites"))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.0))
                                        .child(category_artwork(
                                            category_icon,
                                            &category_name,
                                            14.0,
                                        ))
                                        .child(category_name),
                                )
                                .when(!compatibility.is_empty(), |row| {
                                    row.child(format!("API {compatibility}"))
                                })
                                .when(!addon.ui_date.is_empty(), |row| {
                                    row.child(addon.ui_date.to_string())
                                }),
                        ),
                ),
        )
        .child(
            Group::new().gap(Size::XSmall).child(
                NativeButton::new(format!("install-{uid}"), "Install")
                    .icon(IconName::ArrowDown)
                    .on_activate(move |window, cx| {
                        enqueue_remote(addon.clone(), install_model.clone(), window, cx);
                    }),
            ),
        )
        .into_any_element()
}

fn enqueue_remote(remote: RemoteAddon, model: Entity<AppModel>, window: &mut Window, cx: &mut App) {
    let prepared = {
        let app = model.read(cx);
        match (
            app.catalog_service.clone(),
            app.install_manager.clone(),
            app.settings.addon_path.is_empty(),
        ) {
            (Some(service), Some(manager), false) => {
                Some((service, manager, PathBuf::from(&app.settings.addon_path)))
            }
            _ => None,
        }
    };
    let Some((service, manager, addon_path)) = prepared else {
        model.update(cx, |app, cx| {
            app.status = "Select an AddOns folder before installing an addon.".into();
            cx.notify();
        });
        return;
    };
    let request_uid = remote.uid.to_string();
    let details = cx.background_executor().spawn(async move {
        service
            .details(&[request_uid], &CancellationToken::default())
            .await
    });
    window
        .spawn(cx, async move |cx| match details.await {
            Ok(details) => {
                let Some(details) = details.into_iter().next() else {
                    model.update(cx, |app, cx| {
                        app.status =
                            format!("ESOUI returned no download details for {}.", remote.ui_name);
                        cx.notify();
                    });
                    return;
                };
                let enqueued = manager.enqueue(InstallRequest {
                    uid: remote.uid.to_string(),
                    name: remote.ui_name.to_string(),
                    download_url: details.ui_download,
                    md5: details.ui_md5,
                    addon_path,
                    expected_directories: remote
                        .ui_dirs
                        .into_iter()
                        .map(|directory| directory.to_string())
                        .collect(),
                });
                model.update(cx, |app, cx| {
                    app.status = if enqueued {
                        format!("Queued {} for installation.", remote.ui_name)
                    } else {
                        format!("{} is already queued or installing.", remote.ui_name)
                    };
                    cx.notify();
                });
            }
            Err(error) => {
                model.update(cx, |app, cx| {
                    app.status = format!(
                        "Could not load ESOUI details for {}: {error}",
                        remote.ui_name
                    );
                    cx.notify();
                });
            }
        })
        .detach();
}

fn show_addon_details(
    remote: RemoteAddon,
    model: Entity<AppModel>,
    window: &mut Window,
    cx: &mut App,
) {
    model.update(cx, |app, cx| {
        app.selected_local = None;
        app.lightbox_index = None;
        cx.notify();
    });
    load_remote_details(remote, model, window, cx);
}

fn show_installed_details(
    addon: Addon,
    decision: MatchedAddon,
    model: Entity<AppModel>,
    window: &mut Window,
    cx: &mut App,
) {
    let remote = decision.remote.clone();
    model.update(cx, |app, cx| {
        app.selected_local = Some((addon.clone(), decision));
        app.selected_details = None;
        app.lightbox_index = None;
        app.status = format!("Viewing {}.", addon.title);
        cx.notify();
    });
    if let Some(remote) = remote {
        load_remote_details(remote, model, window, cx);
    }
}

fn load_remote_details(
    remote: RemoteAddon,
    model: Entity<AppModel>,
    window: &mut Window,
    cx: &mut App,
) {
    let service = model.read(cx).catalog_service.clone();
    let Some(service) = service else {
        model.update(cx, |app, cx| {
            app.status = "ESOUI details are unavailable while storage is degraded.".into();
            cx.notify();
        });
        return;
    };
    model.update(cx, |app, cx| {
        app.status = format!("Loading details for {}…", remote.ui_name);
        cx.notify();
    });
    let uid = remote.uid.to_string();
    let details = cx
        .background_executor()
        .spawn(async move { service.details(&[uid], &CancellationToken::default()).await });
    window
        .spawn(cx, async move |cx| match details.await {
            Ok(mut details) => {
                if let Some(details) = details.first_mut() {
                    details.addon = remote.clone();
                }
                model.update(cx, |app, cx| {
                    app.selected_details = details.into_iter().next();
                    app.status = if app.selected_details.is_some() {
                        format!("Loaded details for {}.", remote.ui_name)
                    } else {
                        format!("ESOUI returned no details for {}.", remote.ui_name)
                    };
                    cx.notify();
                });
            }
            Err(error) => {
                model.update(cx, |app, cx| {
                    app.status = format!("Could not load {} details: {error}", remote.ui_name);
                    cx.notify();
                });
            }
        })
        .detach();
}

fn render_missing_dependencies(
    dependencies: Arc<Vec<MissingDependency>>,
    model: Entity<AppModel>,
    window: Entity<ScribeWindow>,
    dismissed_required: bool,
    dismissed_optional: bool,
) -> gpui::AnyElement {
    let required: Vec<_> = dependencies
        .iter()
        .filter(|dependency| !dependency.optional)
        .cloned()
        .collect();
    let optional: Vec<_> = dependencies
        .iter()
        .filter(|dependency| dependency.optional)
        .cloned()
        .collect();
    div()
        .px(px(16.0))
        .pt(px(10.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .when(!dismissed_required && !required.is_empty(), |column| {
            column.child(dependency_banner(
                required,
                false,
                model.clone(),
                window.clone(),
            ))
        })
        .when(!dismissed_optional && !optional.is_empty(), |column| {
            column.child(dependency_banner(
                optional,
                true,
                model.clone(),
                window.clone(),
            ))
        })
        .into_any_element()
}

fn dependency_banner(
    dependencies: Vec<MissingDependency>,
    optional: bool,
    model: Entity<AppModel>,
    window: Entity<ScribeWindow>,
) -> gpui::AnyElement {
    let installable: Vec<String> = dependencies
        .iter()
        .filter(|dependency| dependency.can_install)
        .map(|dependency| dependency.remote_uid.clone())
        .collect();
    let names = dependencies
        .iter()
        .take(5)
        .map(|dependency| dependency.dep_folder_name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let overflow = dependencies.len().saturating_sub(5);
    let count = dependencies.len();
    div()
        .w_full()
        .px(px(13.0))
        .py(px(10.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(gpui::rgb(0xd7b77e))
        .bg(gpui::rgb(0xf4ead9))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(Icon::new(IconName::TriangleAlert).size(px(16.0)))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(div().font_semibold().text_size(px(12.0)).child(format!(
                            "{count} missing {} dependencies detected",
                            if optional { "optional" } else { "required" }
                        )))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(gpui::rgb(0x6d5a3f))
                                .child(if overflow > 0 {
                                    format!("{names} +{overflow} more")
                                } else {
                                    names
                                }),
                        ),
                ),
        )
        .child(
            h_flex()
                .gap(px(6.0))
                .when(!installable.is_empty(), |actions| {
                    actions.child(
                        NativeButton::new(
                            if optional {
                                "install-optional-dependencies"
                            } else {
                                "install-required-dependencies"
                            },
                            if optional {
                                "Install optional"
                            } else {
                                "Install required"
                            },
                        )
                        .icon(IconName::ArrowDown)
                        .on_activate(move |host, cx| {
                            enqueue_dependency_uids(installable.clone(), model.clone(), host, cx)
                        }),
                    )
                })
                .child(
                    NativeButton::new(
                        if optional {
                            "dismiss-optional-dependencies"
                        } else {
                            "dismiss-required-dependencies"
                        },
                        "Dismiss",
                    )
                    .ghost()
                    .on_activate(move |_, cx| {
                        window.update(cx, |view, cx| {
                            if optional {
                                view.dismissed_optional_dependencies = true;
                            } else {
                                view.dismissed_required_dependencies = true;
                            }
                            cx.notify();
                        });
                    }),
                ),
        )
        .into_any_element()
}

fn enqueue_dependency_uids(
    uids: Vec<String>,
    model: Entity<AppModel>,
    window: &mut Window,
    cx: &mut App,
) {
    let remotes: Vec<RemoteAddon> = {
        let app = model.read(cx);
        uids.iter()
            .filter_map(|uid| app.catalog_index.by_uid(uid))
            .collect()
    };
    for remote in remotes {
        enqueue_remote(remote, model.clone(), window, cx);
    }
}

fn details_screenshot_rail(screenshots: &[String], model: Entity<AppModel>) -> gpui::AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(div().font_semibold().child("Screenshots"))
        .child(
            div().w_full().flex().gap(px(10.0)).children(
                screenshots
                    .iter()
                    .take(4)
                    .enumerate()
                    .map(|(index, source)| {
                        let lightbox_model = model.clone();
                        div()
                            .id(("detail-screenshot", index))
                            .role(Role::Button)
                            .aria_label(format!(
                                "Open screenshot {} of {}",
                                index + 1,
                                screenshots.len()
                            ))
                            .cursor_pointer()
                            .flex_1()
                            .h(px(142.0))
                            .rounded(px(7.0))
                            .overflow_hidden()
                            .border_1()
                            .border_color(gpui::rgb(0xb9a487))
                            .bg(gpui::rgb(0xe4d6c1))
                            .hover(|image| image.border_color(gpui::rgb(0x7a4f2e)))
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                lightbox_model.update(cx, |app, cx| {
                                    app.lightbox_index = Some(index);
                                    cx.notify();
                                });
                            })
                            .child(img(source.clone()).size_full().object_fit(ObjectFit::Cover))
                    }),
            ),
        )
        .into_any_element()
}

fn render_details_modal(
    details: RemoteAddonDetails,
    category: Option<Category>,
    model: Entity<AppModel>,
) -> gpui::AnyElement {
    let close_model = model.clone();
    let install_model = model.clone();
    let website = details.addon.ui_file_info_url.to_string();
    let remote_for_install = details.addon.clone();
    let screenshots: Vec<_> = details
        .addon
        .ui_imgs
        .iter()
        .take(12)
        .map(ToString::to_string)
        .collect();
    let description = plain_text(&details.ui_description);
    let change_log = plain_text(&details.ui_change_log);
    let compatibility = details
        .addon
        .compatabilities
        .iter()
        .take(4)
        .map(|version| version.name.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let thumbnail = details.addon.ui_img_thumbs.first().map(ToString::to_string);
    let category_name = category
        .as_ref()
        .map(|category| category.name.to_string())
        .unwrap_or_else(|| "ESOUI addon".into());
    let category_icon = category
        .filter(|category| !category.icon_url.is_empty())
        .map(|category| category.icon_url.to_string());
    let title = details.addon.ui_name.to_string();
    let author = details.addon.ui_author_name.to_string();
    let version = details.addon.ui_version.to_string();
    let updated = details.addon.ui_date.to_string();
    let downloads = format_count(details.addon.ui_download_total);
    let favorites = format_count(details.addon.ui_favorite_total);
    let views = format_count(details.ui_hit_count);
    let title_for_aria = title.clone();
    Modal::new()
        .title(title.clone())
        .width(980.0)
        .on_close(move |_, _, cx| {
            close_model.update(cx, |app, cx| {
                app.selected_details = None;
                app.selected_local = None;
                app.lightbox_index = None;
                cx.notify();
            });
        })
        .child(
            div()
                .id("addon-details-dialog")
                .role(Role::Dialog)
                .aria_label(format!("{title_for_aria} addon details"))
                .max_h(px(650.0))
                .overflow_y_scrollbar()
                .pr(px(5.0))
                .flex()
                .flex_col()
                .gap(px(18.0))
                .child(
                    div()
                        .p(px(15.0))
                        .rounded(px(9.0))
                        .border_1()
                        .border_color(gpui::rgb(0xc9b89e))
                        .bg(gpui::rgb(0xf1e6d5))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(18.0))
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(13.0))
                                .child(addon_artwork(thumbnail, &title))
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(5.0))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(7.0))
                                                .child(category_artwork(category_icon, &category_name, 18.0))
                                                .child(
                                                    div()
                                                        .px(px(7.0))
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .bg(gpui::rgb(0xe2d0b5))
                                                        .text_size(px(10.0))
                                                        .child(category_name),
                                                )
                                                .child(
                                                    div()
                                                        .px(px(7.0))
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .bg(gpui::rgb(0xe2d0b5))
                                                        .text_size(px(10.0))
                                                        .child(format!("v{version}")),
                                                ),
                                        )
                                        .child(div().font_semibold().text_size(px(20.0)).child(title))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(gpui::rgb(0x5f4c34))
                                                .child(format!("{author}  •  {compatibility}  •  Updated {updated}")),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap(px(12.0))
                                                .text_size(px(11.0))
                                                .text_color(gpui::rgb(0x5f4c34))
                                                .child(format!("{downloads} downloads"))
                                                .child(format!("{favorites} favorites"))
                                                .child(format!("{views} views")),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_end()
                                .gap(px(7.0))
                                .child(
                                    NativeButton::new("install-from-details", "Install")
                                        .icon(IconName::ArrowDown)
                                        .on_activate(move |window, cx| {
                                            enqueue_remote(
                                                remote_for_install.clone(),
                                                install_model.clone(),
                                                window,
                                                cx,
                                            );
                                        }),
                                )
                                .when(!website.is_empty(), |column| {
                                    column.child(
                                        NativeButton::new("open-detail-website", "ESOUI page")
                                            .ghost()
                                            .icon(IconName::ExternalLink)
                                            .on_activate(move |_, cx| cx.open_url(&website)),
                                    )
                                }),
                        ),
                )
                .when(!screenshots.is_empty(), |column| {
                    column.child(details_screenshot_rail(&screenshots, model.clone()))
                })
                .when(!description.is_empty(), |column| {
                    column.child(detail_text_section("Description", description))
                })
                .when(!change_log.is_empty(), |column| {
                    column.child(detail_text_section("Latest changes", change_log))
                }),
        )
        .into_any_element()
}

fn render_local_details_modal(
    local: (Addon, MatchedAddon),
    details: Option<RemoteAddonDetails>,
    category: Option<Category>,
    model: Entity<AppModel>,
) -> gpui::AnyElement {
    let (addon, decision) = local;
    let close_model = model.clone();
    let remove_model = model.clone();
    let update_model = model.clone();
    let folder = addon.folder_name.clone();
    let update = decision
        .remote
        .clone()
        .filter(|_| decision.update_available);
    let remote = decision.remote.clone();
    let screenshots: Vec<String> = details
        .as_ref()
        .map(|details| {
            details
                .addon
                .ui_imgs
                .iter()
                .take(12)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    let description = details
        .as_ref()
        .map(|details| plain_text(&details.ui_description))
        .filter(|description| !description.is_empty())
        .unwrap_or_else(|| addon.description.clone());
    let thumbnail = remote
        .as_ref()
        .and_then(|remote| remote.ui_img_thumbs.first())
        .map(ToString::to_string);
    let category_name = category
        .as_ref()
        .map(|category| category.name.to_string())
        .unwrap_or_else(|| {
            if addon.is_library {
                "Library".into()
            } else {
                "Installed addon".into()
            }
        });
    let category_icon = category
        .filter(|category| !category.icon_url.is_empty())
        .map(|category| category.icon_url.to_string());
    let downloads = remote
        .as_ref()
        .map(|remote| format_count(remote.ui_download_total))
        .unwrap_or_else(|| "Local".into());
    let favorites = remote
        .as_ref()
        .map(|remote| format_count(remote.ui_favorite_total))
        .unwrap_or_else(|| "-".into());
    let website = remote
        .as_ref()
        .map(|remote| remote.ui_file_info_url.to_string())
        .unwrap_or_default();
    let aria_title = addon.title.clone();
    Modal::new()
        .title(addon.title.clone())
        .width(650.0)
        .on_close(move |_, _, cx| {
            close_model.update(cx, |app, cx| {
                app.selected_local = None;
                app.selected_details = None;
                app.lightbox_index = None;
                cx.notify();
            });
        })
        .child(
            div()
                .id("installed-addon-dialog")
                .role(Role::Dialog)
                .aria_label(format!("{aria_title} installed addon details"))
                .max_h(px(650.0))
                .overflow_y_scrollbar()
                .pr(px(5.0))
                .flex()
                .flex_col()
                .gap(px(16.0))
                .when(!screenshots.is_empty(), |column| {
                    column.child(details_screenshot_rail(&screenshots, model.clone()))
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(11.0))
                                .child(addon_artwork(thumbnail, &addon.title))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.0))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(7.0))
                                                .child(
                                                    div()
                                                        .font_semibold()
                                                        .text_size(px(16.0))
                                                        .child(addon.title.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .px(px(7.0))
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .bg(gpui::rgb(0xe2d0b5))
                                                        .text_size(px(10.0))
                                                        .child(format!("v{}", addon.version)),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(5.0))
                                                .text_size(px(11.0))
                                                .text_color(gpui::rgb(0x5f4c34))
                                                .child(category_artwork(
                                                    category_icon,
                                                    &category_name,
                                                    15.0,
                                                ))
                                                .child(category_name)
                                                .child(format!("• {}", decision.update_state)),
                                        ),
                                ),
                        )
                        .child(
                            Group::new()
                                .gap(Size::XSmall)
                                .when_some(update, move |group, remote| {
                                    group.child(
                                        NativeButton::new("update-from-local-detail", "Update")
                                            .icon(IconName::ArrowDown)
                                            .on_activate(move |window, cx| {
                                                enqueue_remote(
                                                    remote.clone(),
                                                    update_model.clone(),
                                                    window,
                                                    cx,
                                                )
                                            }),
                                    )
                                })
                                .child(
                                    NativeButton::new("remove-from-local-detail", "Uninstall")
                                        .danger()
                                        .on_activate(move |_, cx| {
                                            remove_model.update(cx, |app, cx| {
                                                app.pending_uninstall = vec![folder.clone()];
                                                cx.notify();
                                            });
                                        }),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .child(metric_pill(downloads, "downloads"))
                        .child(metric_pill(favorites, "favorites"))
                        .when(!website.is_empty(), |row| {
                            row.child(
                                NativeButton::new("open-local-esoui", "ESOUI")
                                    .ghost()
                                    .icon(IconName::ExternalLink)
                                    .on_activate(move |_, cx| cx.open_url(&website)),
                            )
                        }),
                )
                .when(!description.is_empty(), |column| {
                    column.child(detail_text_section("Description", description))
                })
                .child(
                    div()
                        .pt(px(12.0))
                        .border_t_1()
                        .border_color(gpui::rgb(0xc9b89e))
                        .grid()
                        .grid_cols(2)
                        .gap(px(12.0))
                        .child(detail_fact(
                            "Author",
                            if addon.author.is_empty() {
                                "Unknown".into()
                            } else {
                                addon.author.clone()
                            },
                        ))
                        .child(detail_fact("Version", addon.version.clone()))
                        .child(detail_fact("API version", addon.api_version.clone()))
                        .child(detail_fact("Folder", addon.folder_name.clone())),
                )
                .when(!addon.depends_on.is_empty(), |column| {
                    column.child(detail_text_section(
                        "Required dependencies",
                        addon.depends_on.join(", "),
                    ))
                })
                .when(!addon.optional_depends_on.is_empty(), |column| {
                    column.child(detail_text_section(
                        "Optional dependencies",
                        addon.optional_depends_on.join(", "),
                    ))
                })
                .when(!addon.saved_variables.is_empty(), |column| {
                    column.child(detail_text_section(
                        "Saved variables",
                        addon.saved_variables.join(", "),
                    ))
                }),
        )
        .into_any_element()
}

fn detail_text_section(title: &'static str, text: String) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(7.0))
        .child(
            div()
                .pb(px(6.0))
                .border_b_1()
                .border_color(gpui::rgb(0xc9b89e))
                .font_semibold()
                .child(title),
        )
        .child(
            div()
                .text_size(px(13.0))
                .line_height(relative(1.45))
                .text_color(gpui::rgb(0x4e3c29))
                .child(text),
        )
        .into_any_element()
}

fn detail_fact(label: &'static str, value: String) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .text_size(px(10.0))
                .text_color(gpui::rgb(0x6d5a3f))
                .child(label),
        )
        .child(div().font_medium().text_size(px(12.0)).child(value))
        .into_any_element()
}

fn render_lightbox(
    details: RemoteAddonDetails,
    index: usize,
    model: Entity<AppModel>,
) -> gpui::AnyElement {
    let screenshots: Vec<String> = details
        .addon
        .ui_imgs
        .iter()
        .take(12)
        .map(ToString::to_string)
        .collect();
    let Some(source) = screenshots.get(index).cloned() else {
        return div().into_any_element();
    };
    let count = screenshots.len();
    let previous_model = model.clone();
    let next_model = model.clone();
    let close_model = model.clone();
    div()
        .id("screenshot-lightbox")
        .role(Role::Dialog)
        .aria_label(format!("Screenshot {} of {count}", index + 1))
        .absolute()
        .inset_0()
        .bg(gpui::black().opacity(0.94))
        .flex()
        .items_center()
        .justify_center()
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(
            div().absolute().top(px(16.0)).right(px(16.0)).child(
                NativeButton::new("close-lightbox", "Close")
                    .secondary()
                    .icon(IconName::Close)
                    .on_activate(move |_, cx| {
                        close_model.update(cx, |app, cx| {
                            app.lightbox_index = None;
                            cx.notify();
                        });
                    }),
            ),
        )
        .child(
            div().absolute().left(px(18.0)).child(
                NativeButton::new("lightbox-previous", "Previous")
                    .secondary()
                    .icon(IconName::ChevronLeft)
                    .on_activate(move |_, cx| {
                        previous_model.update(cx, |app, cx| {
                            app.lightbox_index = Some(index.checked_sub(1).unwrap_or(count - 1));
                            cx.notify();
                        });
                    }),
            ),
        )
        .child(
            div()
                .w(relative(0.82))
                .h(relative(0.82))
                .flex()
                .items_center()
                .justify_center()
                .child(img(source).size_full().object_fit(ObjectFit::Contain)),
        )
        .child(
            div().absolute().right(px(18.0)).child(
                NativeButton::new("lightbox-next", "Next")
                    .secondary()
                    .icon(IconName::ChevronRight)
                    .on_activate(move |_, cx| {
                        next_model.update(cx, |app, cx| {
                            app.lightbox_index = Some((index + 1) % count);
                            cx.notify();
                        });
                    }),
            ),
        )
        .child(
            div()
                .absolute()
                .bottom(px(18.0))
                .px(px(10.0))
                .py(px(5.0))
                .rounded(px(12.0))
                .bg(gpui::white().opacity(0.12))
                .text_color(gpui::white())
                .text_size(px(12.0))
                .child(format!("{} / {count}", index + 1)),
        )
        .into_any_element()
}

fn render_uninstall_modal(folders: Vec<String>, model: Entity<AppModel>) -> gpui::AnyElement {
    let count = folders.len();
    let title = if count == 1 {
        "Confirm uninstall".to_owned()
    } else {
        format!("Confirm {count} uninstalls")
    };
    let description = if count == 1 {
        format!(
            "Remove only the named addon folder {}? This cannot be undone.",
            folders[0]
        )
    } else {
        format!(
            "Remove these {count} selected addon folders? Scribe validates every folder name and never removes folders outside the configured AddOns directory. This cannot be undone."
        )
    };
    let cancel_model = model.clone();
    let confirm_model = model.clone();
    let close_model = model.clone();
    Modal::new()
        .title(title)
        .width(500.0)
        .on_close(move |_, _, cx| {
            close_model.update(cx, |app, cx| {
                app.pending_uninstall.clear();
                cx.notify();
            });
        })
        .child(
            div()
                .id("uninstall-dialog")
                .role(Role::Dialog)
                .aria_label(format!("Confirm uninstall of {count} addon folders"))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(description)
                .when(count > 1, |dialog| {
                    dialog.child(
                        div()
                            .max_h(px(150.0))
                            .overflow_y_scrollbar()
                            .px(px(10.0))
                            .py(px(8.0))
                            .rounded(px(6.0))
                            .bg(gpui::black().opacity(0.04))
                            .text_size(px(11.0))
                            .children(folders.iter().cloned().map(|folder| div().child(folder))),
                    )
                })
                .child(
                    Group::new()
                        .gap(Size::Small)
                        .child(NativeButton::new("cancel-uninstall", "Cancel").on_activate(
                            move |_, cx| {
                                cancel_model.update(cx, |app, cx| {
                                    app.pending_uninstall.clear();
                                    cx.notify();
                                });
                            },
                        ))
                        .child(
                            NativeButton::new("confirm-uninstall", "Uninstall").on_activate(
                                move |window, cx| {
                                    uninstall_named_folders(
                                        folders.clone(),
                                        confirm_model.clone(),
                                        window,
                                        cx,
                                    );
                                },
                            ),
                        ),
                ),
        )
        .into_any_element()
}

fn uninstall_named_folders(
    folders: Vec<String>,
    model: Entity<AppModel>,
    window: &mut Window,
    cx: &mut App,
) {
    let (addon_path, storage, catalog_index) = {
        let app = model.read(cx);
        (
            app.settings.addon_path.clone(),
            app.storage.clone(),
            app.catalog_index.clone(),
        )
    };
    if addon_path.is_empty() {
        return;
    }
    model.update(cx, |app, cx| {
        app.pending_uninstall.clear();
        app.status = if folders.len() == 1 {
            format!("Uninstalling {}…", folders[0])
        } else {
            format!("Uninstalling {} selected addons…", folders.len())
        };
        cx.notify();
    });
    let folders_for_task = folders.clone();
    let task = cx.background_executor().spawn(async move {
        for folder in &folders_for_task {
            Installer::uninstall(&addon_path, folder).map_err(|error| error.to_string())?;
        }
        let scanner = Scanner::new(PathBuf::from(addon_path));
        let scanner = match storage {
            Some(storage) => scanner.with_storage(storage),
            None => scanner,
        };
        scanner.scan().map_err(|error| error.to_string())
    });
    window
        .spawn(cx, async move |cx| match task.await {
            Ok(installed) => {
                model.update(cx, |app, cx| {
                    replace_installed_state(app, installed, &catalog_index);
                    app.status = if folders.len() == 1 {
                        format!("Uninstalled {} and rescanned AddOns.", folders[0])
                    } else {
                        format!(
                            "Uninstalled {} selected addons and rescanned AddOns.",
                            folders.len()
                        )
                    };
                    cx.notify();
                });
            }
            Err(error) => {
                model.update(cx, |app, cx| {
                    app.status = if folders.len() == 1 {
                        format!("Could not uninstall {}: {error}", folders[0])
                    } else {
                        format!(
                            "Bulk uninstall stopped: {error}. Scribe rescans on the next action."
                        )
                    };
                    cx.notify();
                });
            }
        })
        .detach();
}

fn plain_text(html: &str) -> String {
    let mut output = String::with_capacity(html.len().min(2_000));
    let characters: Vec<char> = html.chars().take(12_000).collect();
    let mut index = 0;
    while index < characters.len() && output.len() < 2_000 {
        if characters[index] == '<' {
            while index < characters.len() && characters[index] != '>' {
                index += 1;
            }
        } else if characters[index] == '['
            && let Some(end) = characters[index + 1..]
                .iter()
                .position(|character| *character == ']')
                .map(|offset| index + 1 + offset)
        {
            let token: String = characters[index + 1..end]
                .iter()
                .collect::<String>()
                .to_ascii_uppercase();
            let name = token
                .trim_start_matches('/')
                .split(['=', ' '])
                .next()
                .unwrap_or_default();
            if matches!(
                name,
                "B" | "I"
                    | "U"
                    | "S"
                    | "SIZE"
                    | "COLOR"
                    | "FONT"
                    | "URL"
                    | "QUOTE"
                    | "CODE"
                    | "LIST"
                    | "CENTER"
                    | "LEFT"
                    | "RIGHT"
                    | "IMG"
                    | "BR"
                    | "*"
            ) {
                index = end;
            } else {
                output.push(characters[index]);
            }
        } else {
            output.push(characters[index]);
        }
        index += 1;
    }
    output
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_task_center(
    tasks: Vec<TaskProgress>,
    border_color: gpui::Hsla,
    model: Entity<AppModel>,
) -> gpui::AnyElement {
    div()
        .max_h(px(150.0))
        .px(px(18.0))
        .py(px(8.0))
        .border_t_1()
        .border_color(border_color)
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(Title::new("Task center").order(5))
        .children(tasks.into_iter().rev().take(4).map(|task| {
            let uid = task.uid.clone();
            let cancel_model = model.clone();
            let can_cancel = matches!(
                task.state,
                TaskState::Queued
                    | TaskState::Planning
                    | TaskState::Downloading
                    | TaskState::Extracting
            );
            div()
                .h(px(24.0))
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(12.0))
                .child(task.name)
                .child(
                    Group::new()
                        .gap(Size::XSmall)
                        .child(format!("{:?} - {:.0}%", task.state, task.percent))
                        .when(can_cancel, |group| {
                            group.child(
                                NativeButton::new(format!("cancel-task-{uid}"), "Cancel")
                                    .on_activate(move |_, cx| {
                                        if let Some(manager) =
                                            &cancel_model.read(cx).install_manager
                                        {
                                            manager.cancel(&uid);
                                        }
                                    }),
                            )
                        }),
                )
        }))
        .into_any_element()
}

fn rebuild_local_storage(model: Entity<AppModel>, window: &mut Window, cx: &mut App) {
    model.update(cx, |app, cx| {
        app.status = "Rebuilding reconstructible local cache data…".into();
        cx.notify();
    });
    let http = cx.http_client();
    let rebuild = cx.background_executor().spawn(async move {
        let outcome = Storage::rebuild_default_reconstructible()?;
        let storage = Arc::new(Storage::open_default()?);
        Ok::<_, scribe_core::storage::StorageError>((outcome, storage))
    });
    window
        .spawn(cx, async move |cx| match rebuild.await {
            Ok((outcome, storage)) => {
                let service = Arc::new(CatalogService::new(
                    storage.clone(),
                    Arc::new(EsouiClient::new(http.clone())),
                ));
                let manager = InstallManager::new(3, http, storage.clone(), None);
                model.update(cx, |app, cx| {
                    app.storage = Some(storage);
                    app.catalog_service = Some(service.clone());
                    app.install_manager = Some(manager);
                    replace_catalog_state(app, Arc::new(Catalog::default()));
                    app.status = if outcome.retained_database.is_some() {
                        "Local cache rebuilt; the unreadable database was retained as a backup. Refreshing ESOUI…".into()
                    } else {
                        "Reconstructible cache data rebuilt; install records were preserved. Refreshing ESOUI…".into()
                    };
                    cx.notify();
                });
                let refresh = cx.background_executor().spawn(async move {
                    service.refresh(&CancellationToken::default()).await
                });
                match refresh.await {
                    Ok((catalog, outcome)) => {
                        model.update(cx, |app, cx| {
                            replace_catalog_state(app, catalog);
                            app.status = format!("Local cache rebuilt and refreshed ({outcome:?}).");
                            cx.notify();
                        });
                    }
                    Err(error) => {
                        model.update(cx, |app, cx| {
                            app.status =
                                format!("Local cache rebuilt, but ESOUI refresh failed: {error}");
                            cx.notify();
                        });
                    }
                }
            }
            Err(error) => {
                model.update(cx, |app, cx| {
                    app.status = format!("Local cache rebuild failed: {error}");
                    cx.notify();
                });
            }
        })
        .detach();
}

fn set_app_theme(_theme: &str, model: Entity<AppModel>, cx: &mut App) {
    apply_scribe_theme(cx);
    let settings = model.update(cx, |app, cx| {
        app.settings.theme = "scribe".to_owned();
        app.status = "Scribe parchment theme applied.".into();
        cx.notify();
        app.settings.clone()
    });
    cx.refresh_windows();
    cx.background_executor()
        .spawn(async move {
            if let Ok(manager) = SettingsManager::new() {
                let _ = manager.save(&settings);
            }
        })
        .detach();
}

fn apply_scribe_theme(cx: &mut App) {
    Theme::change(ThemeMode::Light, None, cx);
    let theme = Theme::global_mut(cx);
    let colors = &mut theme.colors;
    let color = |hex| gpui::rgb(hex).into();

    colors.background = color(0xf5ece0);
    colors.foreground = color(0x2c1f14);
    colors.border = color(0xc9b89e);
    colors.input = color(0xbba78b);
    colors.accent = color(0xe6d9c4);
    colors.accent_foreground = color(0x2c1f14);
    colors.muted = color(0xddd0bb);
    colors.muted_foreground = color(0x5f4c34);
    colors.popover = color(0xf8f0e4);
    colors.popover_foreground = color(0x2c1f14);
    colors.list = color(0xf5ece0);
    colors.list_even = color(0xf0e5d5);
    colors.list_head = color(0xeadcca);
    colors.list_hover = color(0xeadcc8);
    colors.list_active = color(0xe2d0b5);
    colors.list_active_border = color(0x9b7047);
    colors.primary = color(0x7a4f2e);
    colors.primary_hover = color(0x684126);
    colors.primary_active = color(0x57351f);
    colors.primary_foreground = color(0xfffaf3);
    colors.secondary = color(0xe6d9c4);
    colors.secondary_hover = color(0xd8c5a7);
    colors.secondary_active = color(0xcbb693);
    colors.secondary_foreground = color(0x2c1f14);
    colors.button_primary = colors.primary;
    colors.button_primary_hover = colors.primary_hover;
    colors.button_primary_active = colors.primary_active;
    colors.button_primary_foreground = colors.primary_foreground;
    colors.button_secondary = colors.secondary;
    colors.button_secondary_hover = colors.secondary_hover;
    colors.button_secondary_active = colors.secondary_active;
    colors.button_secondary_foreground = colors.secondary_foreground;
    colors.button = color(0xefe5d6);
    colors.button_hover = color(0xe2d0b5);
    colors.button_active = color(0xd4bea0);
    colors.button_foreground = color(0x2c1f14);
    colors.sidebar = color(0x1c1510);
    colors.sidebar_foreground = color(0xe3d3bd);
    colors.sidebar_border = color(0x3e3023);
    colors.sidebar_accent = color(0x33261b);
    colors.sidebar_accent_foreground = color(0xfff4e3);
    colors.sidebar_primary = color(0xd4a656);
    colors.sidebar_primary_foreground = color(0x1c1510);
    colors.title_bar = color(0x14100a);
    colors.title_bar_border = color(0x3a2b1e);
    colors.window_border = color(0x3a2b1e);
    colors.ring = color(0xb27a42);
    colors.link = color(0x69411f);
    colors.link_hover = color(0x4f2f16);
    colors.link_active = color(0x3c230f);
    colors.selection = color(0xd7b77e);
    colors.scrollbar = color(0xeadcca);
    colors.scrollbar_thumb = color(0xaa9271);
    colors.scrollbar_thumb_hover = color(0x80694d);
    colors.warning = color(0xa76712);
    colors.warning_foreground = color(0xfffbf4);
    colors.info = color(0x315f73);
    colors.info_foreground = color(0xffffff);
    colors.success = color(0x34633d);
    colors.success_foreground = color(0xffffff);
    colors.danger = color(0x9d2f2f);
    colors.danger_foreground = color(0xffffff);
    theme.tokens = (&theme.colors).into();
    theme.tokens.title_bar.color = color(0xd9c4a5);
    theme.radius = px(6.0);
    theme.radius_lg = px(10.0);
    theme.shadow = true;
}

fn browse_for_addons(model: Entity<AppModel>, window: &mut Window, cx: &mut App) {
    let receiver = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("Select the ESO AddOns folder".into()),
    });
    window
        .spawn(cx, async move |cx| {
            let mut paths = receiver.await.ok()?.ok()??;
            let path = paths.pop()?;
            let value = path.to_string_lossy().into_owned();
            let (settings, storage, catalog_index) = model.update(cx, |app, cx| {
                app.settings.addon_path = value;
                app.status = "Saving the AddOns folder and scanning…".into();
                cx.notify();
                (
                    app.settings.clone(),
                    app.storage.clone(),
                    app.catalog_index.clone(),
                )
            });
            let addon_path = PathBuf::from(&settings.addon_path);
            let scan = cx.background_executor().spawn(async move {
                SettingsManager::new()
                    .and_then(|manager| manager.save(&settings))
                    .map_err(|error| error.to_string())?;
                let scanner = Scanner::new(addon_path);
                let scanner = match storage {
                    Some(storage) => scanner.with_storage(storage),
                    None => scanner,
                };
                scanner.scan().map_err(|error| error.to_string())
            });
            match scan.await {
                Ok(installed) => {
                    let count = installed.len();
                    model.update(cx, |app, cx| {
                        replace_installed_state(app, installed, &catalog_index);
                        app.status =
                            format!("AddOns folder saved. Detected {count} installed addons.");
                        cx.notify();
                    });
                }
                Err(error) => {
                    model.update(cx, |app, cx| {
                        app.status = format!("Could not save or scan the AddOns folder: {error}");
                        cx.notify();
                    });
                }
            }
            Some(())
        })
        .detach();
}

fn refresh_catalog(model: Entity<AppModel>, window: &mut Window, cx: &mut App) {
    let service = model.read(cx).catalog_service.clone();
    let Some(service) = service else {
        model.update(cx, |app, cx| {
            app.status = "ESOUI refresh is unavailable while local storage is degraded.".into();
            cx.notify();
        });
        return;
    };
    model.update(cx, |app, cx| {
        app.status = "Refreshing the ESOUI catalog…".into();
        cx.notify();
    });
    let refresh = cx
        .background_executor()
        .spawn(async move { service.refresh(&CancellationToken::default()).await });
    window
        .spawn(cx, async move |cx| match refresh.await {
            Ok((catalog, outcome)) => {
                model.update(cx, |app, cx| {
                    replace_catalog_state(app, catalog);
                    app.status = format!("ESOUI catalog refreshed ({outcome:?}).");
                    cx.notify();
                });
            }
            Err(error) => {
                model.update(cx, |app, cx| {
                    app.status =
                        format!("ESOUI refresh failed: {error}. Cached data remains available.");
                    cx.notify();
                });
            }
        })
        .detach();
}

fn rescan_configured_addons(model: Entity<AppModel>, window: &mut Window, cx: &mut App) {
    let (addon_path, storage, catalog_index) = {
        let app = model.read(cx);
        (
            app.settings.addon_path.clone(),
            app.storage.clone(),
            app.catalog_index.clone(),
        )
    };
    if addon_path.is_empty() {
        model.update(cx, |app, cx| {
            app.status = "Select an AddOns folder before rescanning.".into();
            cx.notify();
        });
        return;
    }
    model.update(cx, |app, cx| {
        app.status = "Rescanning AddOns…".into();
        cx.notify();
    });
    let scan = cx.background_executor().spawn(async move {
        let scanner = Scanner::new(PathBuf::from(addon_path));
        let scanner = match storage {
            Some(storage) => scanner.with_storage(storage),
            None => scanner,
        };
        scanner.scan()
    });
    window
        .spawn(cx, async move |cx| match scan.await {
            Ok(installed) => {
                let count = installed.len();
                model.update(cx, |app, cx| {
                    replace_installed_state(app, installed, &catalog_index);
                    app.status = format!("Rescan complete. Detected {count} installed addons.");
                    cx.notify();
                });
            }
            Err(error) => {
                model.update(cx, |app, cx| {
                    app.status = format!("AddOns rescan failed: {error}");
                    cx.notify();
                });
            }
        })
        .detach();
}

async fn enrich_md5_decisions(
    storage: Arc<Storage>,
    service: Arc<CatalogService>,
    matched: Vec<MatchedAddon>,
) -> Result<Vec<MatchedAddon>, String> {
    let uids: Vec<String> = matched
        .iter()
        .filter_map(|decision| {
            decision
                .remote
                .as_ref()
                .map(|remote| remote.uid.to_string())
        })
        .collect();
    let records = storage
        .install_records(&uids)
        .map_err(|error| error.to_string())?;
    if records.is_empty() {
        return Ok(matched);
    }
    let installed_md5s: std::collections::HashMap<_, _> = records
        .into_iter()
        .filter(|record| !record.md5.is_empty())
        .map(|record| (record.uid, record.md5))
        .collect();
    let check_uids: Vec<_> = uids
        .into_iter()
        .filter(|uid| installed_md5s.contains_key(uid))
        .collect();
    let details = service
        .details(&check_uids, &CancellationToken::default())
        .await
        .map_err(|error| error.to_string())?;
    let remote_md5s = details
        .into_iter()
        .map(|details| (details.addon.uid.to_string(), details.ui_md5))
        .collect();
    Ok(Matcher::apply_md5_decisions(
        matched,
        &installed_md5s,
        &remote_md5s,
    ))
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn main() {
    trace_startup("main_enter");
    gpui_platform::application()
        .with_assets(ScribeAssets)
        .run(|cx: &mut App| {
            trace_startup("gpui_run");
            gpui_component::init(cx);
            trace_startup("component_init");
            apply_scribe_theme(cx);
            let http_client: Arc<dyn HttpClient> = Arc::new(LazyHttpClient::new(concat!(
                "Scribe/",
                env!("CARGO_PKG_VERSION")
            )));
            cx.set_http_client(http_client);
            cx.bind_keys([
                KeyBinding::new("tab", Tab, None),
                KeyBinding::new("shift-tab", TabPrevious, None),
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
                    titlebar: Some(TitleBar::title_bar_options()),
                    ..Default::default()
                },
                {
                    let model = model.clone();
                    move |window, cx| {
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

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;

    fn empty_model() -> AppModel {
        let catalog = Arc::new(Catalog::default());
        AppModel {
            page: Page::Installed,
            settings: AppSettings::default(),
            catalog_index: Arc::new(CatalogIndex::new(catalog.clone())),
            installed: Arc::new(Vec::new()),
            installed_index: Arc::new(InstalledIndex::default()),
            matched: Arc::new(Vec::new()),
            missing_dependencies: Arc::new(Vec::new()),
            storage: None,
            catalog_service: None,
            install_manager: None,
            loading: false,
            status: String::new(),
            selected_details: None,
            selected_local: None,
            lightbox_index: None,
            pending_uninstall: Vec::new(),
            observed_completions: HashSet::new(),
        }
    }

    #[test]
    fn addon_copy_removes_html_and_mmoui_bbcode() {
        assert_eq!(
            plain_text(
                r#"<p>[SIZE=4][B]Useful[/B][/SIZE] &amp; [URL="https://example.test"]safe[/URL]</p>"#,
            ),
            "Useful & safe"
        );
    }

    #[gpui::test]
    fn navigation_keeps_page_entities_and_search_state(cx: &mut TestAppContext) {
        let (window, view) = cx.update(|cx| {
            gpui_component::init(cx);
            apply_scribe_theme(cx);
            let model = cx.new(|_| empty_model());
            let window = cx
                .open_window(Default::default(), {
                    let model = model.clone();
                    move |window, cx| cx.new(|cx| ScribeWindow::new(model, window, cx))
                })
                .unwrap();
            let view = window
                .update(cx, |view, _, _| view.installed.clone())
                .unwrap();
            (window, view)
        });

        view.update(cx, |state, cx| {
            state.query = "persistent query".into();
            cx.notify();
        });
        window
            .update(cx, |window_view, _, cx| {
                window_view.model.update(cx, |model, cx| {
                    model.page = Page::FindMore;
                    cx.notify();
                });
                window_view.model.update(cx, |model, cx| {
                    model.page = Page::Installed;
                    cx.notify();
                });
                assert_eq!(window_view.installed.read(cx).query, "persistent query");
            })
            .unwrap();
    }
}
