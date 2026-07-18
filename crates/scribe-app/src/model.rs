use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{AppContext, Context, Entity, Pixels, Subscription, Window, px};
use gpui_component::{
    IconName,
    input::{InputEvent, InputState},
};
use scribe_core::{
    Addon, AppSettings, CACHE_TTL_SECONDS, CancellationToken, Catalog, CatalogIndex,
    CatalogService, Category, EsouiClient, InstallManager, InstalledIndex, Installer, MatchedAddon,
    Matcher, MissingDependency, RemoteAddon, RemoteAddonDetails, Scanner, SettingsManager, Storage,
    TaskState,
};

use crate::flows::enrich_md5_decisions;
use crate::theme::apply_scribe_theme;
use crate::{trace_startup, unix_now};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    Installed,
    FindMore,
    Updates,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverlayKind {
    RemoteDetails,
    LocalDetails,
    Uninstall,
    Rebuild,
    Lightbox,
}

impl Page {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Installed => "Installed",
            Self::FindMore => "Find More",
            Self::Updates => "Updates",
            Self::Settings => "Settings",
        }
    }

    pub(crate) fn subtitle(self) -> &'static str {
        match self {
            Self::Installed => "Your addon library",
            Self::FindMore => "Browse the ESOUI catalog",
            Self::Updates => "Review and apply available updates",
            Self::Settings => "Library, appearance, health, and diagnostics",
        }
    }

    pub(crate) fn icon(self) -> IconName {
        match self {
            Self::Installed => IconName::BookOpen,
            Self::FindMore => IconName::Search,
            Self::Updates => IconName::ArrowUp,
            Self::Settings => IconName::Settings2,
        }
    }
}

pub(crate) struct PageState {
    pub(crate) query: String,
    pub(crate) search: Entity<InputState>,
    pub(crate) _subscription: Subscription,
}

impl PageState {
    pub(crate) fn new(
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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

pub(crate) struct AppModel {
    pub(crate) page: Page,
    pub(crate) settings: AppSettings,
    pub(crate) catalog_index: Arc<CatalogIndex>,
    pub(crate) installed: Arc<Vec<Addon>>,
    pub(crate) installed_index: Arc<InstalledIndex>,
    pub(crate) matched: Arc<Vec<MatchedAddon>>,
    pub(crate) missing_dependencies: Arc<Vec<MissingDependency>>,
    pub(crate) storage: Option<Arc<Storage>>,
    pub(crate) catalog_service: Option<Arc<CatalogService>>,
    pub(crate) install_manager: Option<Arc<InstallManager>>,
    pub(crate) loading: bool,
    pub(crate) status: String,
    pub(crate) selected_details: Option<RemoteAddonDetails>,
    pub(crate) selected_local: Option<(Addon, MatchedAddon)>,
    pub(crate) lightbox_index: Option<usize>,
    pub(crate) pending_uninstall: Vec<String>,
    pub(crate) pending_rebuild: bool,
    pub(crate) health: HealthState,
    pub(crate) observed_completions: HashSet<String>,
    /// Update-available count from the last alert that was surfaced, used to
    /// fire one alert per rise and re-arm only when the count returns to 0.
    pub(crate) alerted_update_count: Option<usize>,
    /// A newly raised update-available count waiting to be surfaced by the
    /// window layer (balloon when hidden, otherwise just consumed once).
    pub(crate) pending_update_alert: Option<usize>,
    /// Remote details are being fetched; the dossier shows a skeleton sheet.
    pub(crate) details_loading: bool,
}

/// Update-alert state machine: returns the next alerted count and the alert
/// to fire. Alerts only fire when the count rises above the previously
/// alerted count (or above zero when never alerted), and the state re-arms
/// when the count returns to zero.
pub(crate) fn alert_decision(
    alerted: Option<usize>,
    new_count: usize,
) -> (Option<usize>, Option<usize>) {
    if new_count == 0 {
        (None, None)
    } else if alerted.is_none_or(|alerted| new_count > alerted) {
        (Some(new_count), Some(new_count))
    } else {
        (alerted, None)
    }
}

/// Uids that cannot be batch-selected in Find More: already installed (per
/// matched decisions) or currently queued/downloading in the install manager.
pub(crate) fn catalog_blocked_uids(app: &AppModel) -> HashSet<String> {
    let mut blocked: HashSet<String> = app
        .matched
        .iter()
        .filter_map(|decision| {
            decision
                .remote
                .as_ref()
                .map(|remote| remote.uid.to_string())
        })
        .collect();
    if let Some(manager) = &app.install_manager {
        blocked.extend(
            manager
                .statuses()
                .into_iter()
                .filter(|task| {
                    !matches!(
                        task.state,
                        TaskState::Complete | TaskState::Failed | TaskState::Cancelled
                    )
                })
                .map(|task| task.uid),
        );
    }
    blocked
}

/// Selected catalog uids that can actually be enqueued: skips uids blocked
/// as installed/queued and resolves the rest to current catalog entries.
pub(crate) fn installable_selection(
    index: &CatalogIndex,
    uids: &BTreeSet<String>,
    blocked: &HashSet<String>,
) -> Vec<RemoteAddon> {
    uids.iter()
        .filter(|uid| !blocked.contains(*uid))
        .filter_map(|uid| index.by_uid(uid))
        .collect()
}

// ---------------------------------------------------------------------------
// Keyboard navigation + fuzzy search helpers (pure; unit-tested)
// ---------------------------------------------------------------------------

/// Moves a list cursor by `delta`, clamped to `len`; starts at the nearest
/// edge when unset, and is `None` for empty lists.
pub(crate) fn move_cursor(cursor: Option<usize>, delta: i64, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let len = len as i64;
    let next = match cursor {
        None => {
            if delta >= 0 {
                0
            } else {
                len - 1
            }
        }
        Some(index) => (index as i64 + delta).clamp(0, len - 1),
    };
    Some(next as usize)
}

/// Clamps a cursor after its list shrinks; drops it for empty lists.
pub(crate) fn clamp_cursor(cursor: Option<usize>, len: usize) -> Option<usize> {
    match (cursor, len) {
        (_, 0) => None,
        (Some(index), len) => Some(index.min(len - 1)),
        (None, _) => None,
    }
}

/// The catalog cursor resets when the filter/sort/search signature changes;
/// otherwise it only clamps to the current result count.
pub(crate) fn cursor_after_filter_change(
    signature_changed: bool,
    cursor: Option<usize>,
    len: usize,
) -> Option<usize> {
    if signature_changed {
        None
    } else {
        clamp_cursor(cursor, len)
    }
}

/// Maps a flattened library cursor position to `(group, row)` pairs, skipping
/// collapsed groups and counting only data rows.
pub(crate) fn visible_library_rows(
    groups: &[InstalledGroup],
    expanded: &HashSet<String>,
) -> Vec<(usize, usize)> {
    let mut rows = Vec::new();
    for (group_ix, group) in groups.iter().enumerate() {
        if expanded.contains(&group.id) {
            for row_ix in 0..group.items.len() {
                rows.push((group_ix, row_ix));
            }
        }
    }
    rows
}

/// Subsequence fuzzy match with scoring: consecutive-run, word-start, and
/// camelCase/delimiter bonuses, shorter-candidate and early-start preference.
/// Case-insensitive; empty query matches everything at score 0.
pub(crate) fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
    let query: Vec<char> = query.trim().chars().flat_map(char::to_lowercase).collect();
    if query.is_empty() {
        return Some(0);
    }
    let candidate: Vec<char> = candidate.chars().collect();
    let mut score = 0i32;
    let mut query_ix = 0;
    let mut last_match: Option<usize> = None;
    let mut first_match = 0usize;
    for (ix, character) in candidate.iter().enumerate() {
        if query_ix >= query.len() {
            break;
        }
        let lower = character.to_lowercase().next().unwrap_or(*character);
        if lower != query[query_ix] {
            continue;
        }
        let word_start =
            ix == 0 || matches!(candidate[ix - 1], ' ' | '-' | '_' | '.' | '/' | '(' | '&');
        let camel = ix > 0 && candidate[ix - 1].is_lowercase() && character.is_uppercase();
        score += 10;
        if last_match.is_some_and(|last| ix == last + 1) {
            score += 14;
        }
        if word_start {
            score += 12;
        }
        if camel {
            score += 10;
        }
        if last_match.is_none() {
            first_match = ix;
        }
        last_match = Some(ix);
        query_ix += 1;
    }
    if query_ix < query.len() {
        return None;
    }
    score -= (candidate.len() as i32) / 4;
    score -= (first_match as i32) / 2;
    Some(score)
}

/// Ranks a popularity-ordered base index list by fuzzy score (best of title,
/// author, category name) descending, ties keeping the base order.
pub(crate) fn fuzzy_filter_rank(index: &CatalogIndex, base: &[usize], query: &str) -> Vec<usize> {
    let mut scored: Vec<(i32, usize, usize)> = Vec::with_capacity(base.len());
    for (position, addon_ix) in base.iter().enumerate() {
        let Some(addon) = index.addon(*addon_ix) else {
            continue;
        };
        let category_name = index
            .category(&addon.category_id)
            .map(|category| category.name.to_string())
            .unwrap_or_default();
        let score = fuzzy_score(query, &addon.ui_name)
            .into_iter()
            .chain(fuzzy_score(query, &addon.ui_author_name))
            .chain(fuzzy_score(query, &category_name))
            .max();
        if let Some(score) = score {
            scored.push((score, position, *addon_ix));
        }
    }
    scored.sort_by_key(|(score, position, _)| (std::cmp::Reverse(*score), *position));
    scored
        .into_iter()
        .map(|(_, _, addon_ix)| addon_ix)
        .collect()
}

/// Records a committed search query: distinct, most-recent-first, capped at 8,
/// empty/whitespace queries ignored.
pub(crate) fn push_recent_search(recent: &mut Vec<String>, query: &str) {
    let query = query.trim();
    if query.is_empty() {
        return;
    }
    recent.retain(|entry| !entry.eq_ignore_ascii_case(query));
    recent.insert(0, query.to_string());
    recent.truncate(8);
}

/// Whether the dossier opens inline as a page (wide windows) rather than as a
/// modal sheet: inline at ≥1400px viewport width.
pub(crate) fn details_inline_width(viewport_width: Pixels) -> bool {
    viewport_width >= px(1400.0)
}

/// Maps overlay state to the current overlay kind, in precedence order. The
/// inline (wide-window) details page maps to the same kinds as the modal
/// dossier so focus, Escape, and keyboard guards behave identically.
pub(crate) fn derive_overlay_kind(
    lightbox_open: bool,
    rebuild_pending: bool,
    uninstall_pending: bool,
    local_details_open: bool,
    remote_details_open: bool,
) -> Option<OverlayKind> {
    if lightbox_open {
        Some(OverlayKind::Lightbox)
    } else if rebuild_pending {
        Some(OverlayKind::Rebuild)
    } else if uninstall_pending {
        Some(OverlayKind::Uninstall)
    } else if local_details_open {
        Some(OverlayKind::LocalDetails)
    } else if remote_details_open {
        Some(OverlayKind::RemoteDetails)
    } else {
        None
    }
}

/// Whether the caption strip's non-client hit areas (drag region and the
/// min/max/close overlay) may be registered this frame. While a modal
/// overlay or a context menu is open, those areas are suspended: the
/// non-client hit test resolves purely by hitbox geometry, so an area
/// beneath an overlay's close button would otherwise swallow that click —
/// or close the window when the user aimed at the overlay. Inline details
/// leave the chrome untouched because nothing covers the strip.
pub(crate) fn window_chrome_active(
    overlay_kind: Option<OverlayKind>,
    inline_details: bool,
    context_menu_open: bool,
) -> bool {
    (overlay_kind.is_none() || inline_details) && !context_menu_open
}

/// Whether list keyboard browsing keys are active: only with no overlay, no
/// context menu, no category palette, and an unfocused page search.
pub(crate) fn keyboard_nav_allowed(
    overlay: Option<OverlayKind>,
    context_menu_open: bool,
    palette_open: bool,
    search_focused: bool,
) -> bool {
    overlay.is_none() && !context_menu_open && !palette_open && !search_focused
}

/// Clears any open dossier state (back action and Escape share this).
pub(crate) fn clear_details_overlay(app: &mut AppModel) {
    app.selected_details = None;
    app.selected_local = None;
    app.details_loading = false;
}

// ---------------------------------------------------------------------------
// Flattened library model for virtualized Installed/Updates lists
// ---------------------------------------------------------------------------

/// One item in the virtualized library list: a group header or one addon row.
/// `last_in_group` drives border/radius styling at group boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LibraryRow {
    GroupHeader {
        group_ix: usize,
        last_in_group: bool,
    },
    AddonRow {
        group_ix: usize,
        row_ix: usize,
        last_in_group: bool,
    },
}

/// Flattens category groups into the virtualized list model: every group
/// yields a header; expanded groups also yield their rows. Selection mode
/// does not change the model (checkboxes live inside rows).
pub(crate) fn flatten_library(
    groups: &[InstalledGroup],
    expanded: &HashSet<String>,
    _selection_mode: bool,
) -> Vec<LibraryRow> {
    let mut rows = Vec::new();
    for (group_ix, group) in groups.iter().enumerate() {
        let is_expanded = expanded.contains(&group.id);
        rows.push(LibraryRow::GroupHeader {
            group_ix,
            last_in_group: !is_expanded,
        });
        if is_expanded {
            for row_ix in 0..group.items.len() {
                rows.push(LibraryRow::AddonRow {
                    group_ix,
                    row_ix,
                    last_in_group: row_ix + 1 == group.items.len(),
                });
            }
        }
    }
    rows
}

/// Position of the `cursor`-th AddonRow inside the flattened model (headers
/// included), for scroll-to-reveal.
pub(crate) fn flat_index_of_row(model: &[LibraryRow], cursor: usize) -> Option<usize> {
    let mut seen = 0usize;
    model.iter().position(|row| {
        if matches!(row, LibraryRow::AddonRow { .. }) {
            let hit = seen == cursor;
            seen += 1;
            hit
        } else {
            false
        }
    })
}

/// Whether the recent-searches dropdown is visible: Find More, search input
/// focused and empty, entries exist, and no overlay surface is open.
pub(crate) fn should_show_recent_dropdown(
    page_is_find_more: bool,
    search_focused: bool,
    query_empty: bool,
    has_recent: bool,
    overlay_open: bool,
) -> bool {
    page_is_find_more && search_focused && query_empty && has_recent && !overlay_open
}

/// Recomputes the update-available count after any matched-state change and
/// records a pending alert when it rises. Honors `background_alerts`.
pub(crate) fn evaluate_update_alerts(app: &mut AppModel) {
    let count = app
        .matched
        .iter()
        .filter(|decision| decision.update_available)
        .count();
    if !app.settings.background_alerts {
        app.alerted_update_count = (count > 0).then_some(count);
        app.pending_update_alert = None;
        return;
    }
    let (alerted, fire) = alert_decision(app.alerted_update_count, count);
    app.alerted_update_count = alerted;
    if fire.is_some() {
        app.pending_update_alert = fire;
    }
}

pub(crate) fn navigate_to_page(app: &mut AppModel, page: Page) {
    app.page = page;
    app.selected_details = None;
    app.selected_local = None;
    app.lightbox_index = None;
    if app.status.starts_with("Loading details for ") {
        app.status.clear();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RecoveryPhase {
    #[default]
    Idle,
    Running,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HealthState {
    pub(crate) storage_issue: Option<String>,
    pub(crate) catalog_issue: Option<String>,
    pub(crate) scan_issue: Option<String>,
    pub(crate) last_catalog_success: Option<i64>,
    pub(crate) last_scan_success: Option<i64>,
    pub(crate) recovery_phase: RecoveryPhase,
    pub(crate) recovery_message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoticeTone {
    Info,
    Success,
    Warning,
    Danger,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StatusNotice {
    pub(crate) tone: NoticeTone,
    pub(crate) title: &'static str,
    pub(crate) message: String,
}

pub(crate) fn status_notice(
    status: &str,
    loading: bool,
    health: &HealthState,
) -> Option<StatusNotice> {
    let notice = |tone, title, message: String| StatusNotice {
        tone,
        title,
        message,
    };

    match health.recovery_phase {
        RecoveryPhase::Running => {
            return Some(notice(
                NoticeTone::Info,
                "Recovery in progress",
                health
                    .recovery_message
                    .clone()
                    .unwrap_or_else(|| "Scribe is rebuilding reconstructible local data.".into()),
            ));
        }
        RecoveryPhase::Failed => {
            return Some(notice(
                NoticeTone::Danger,
                "Recovery needs attention",
                health.recovery_message.clone().unwrap_or_else(|| {
                    "Recovery did not complete. Your existing data was retained.".into()
                }),
            ));
        }
        RecoveryPhase::Succeeded => {
            return Some(notice(
                NoticeTone::Success,
                "Recovery complete",
                health
                    .recovery_message
                    .clone()
                    .unwrap_or_else(|| "Scribe recovered its reconstructible local data.".into()),
            ));
        }
        RecoveryPhase::Idle => {}
    }

    let trimmed = status.trim();
    if trimmed.is_empty() && !loading {
        return None;
    }
    if loading {
        return Some(notice(
            NoticeTone::Info,
            "Loading library",
            if trimmed.is_empty() {
                "Reading settings, cached catalog, and installed addons.".into()
            } else {
                trimmed.into()
            },
        ));
    }

    let normalized = trimmed.to_ascii_lowercase();
    if normalized.starts_with("loading details") {
        return Some(notice(
            NoticeTone::Info,
            "Loading addon details",
            trimmed.into(),
        ));
    }
    if normalized.starts_with("cached esoui catalog loaded") {
        return None;
    }
    if normalized.starts_with("loaded details for") {
        return None;
    }
    if normalized.contains("failed")
        || normalized.contains("could not")
        || normalized.contains("unavailable")
        || normalized.contains("error")
    {
        return Some(notice(NoticeTone::Danger, "Action needed", trimmed.into()));
    } else if normalized.contains("warning") || normalized.contains("cancel") {
        return Some(notice(
            NoticeTone::Warning,
            "Review requested",
            trimmed.into(),
        ));
    }
    if normalized.contains("refreshed")
        || normalized.contains("complete")
        || normalized.contains("saved")
        || normalized.contains("detected")
        || normalized.contains("applied")
        || normalized.contains("ready")
    {
        return None;
    }
    Some(notice(NoticeTone::Info, "Scribe status", trimmed.into()))
}

pub(crate) struct InitialState {
    settings: AppSettings,
    catalog_index: Arc<CatalogIndex>,
    installed: Arc<Vec<Addon>>,
    installed_index: Arc<InstalledIndex>,
    matched: Arc<Vec<MatchedAddon>>,
    missing_dependencies: Arc<Vec<MissingDependency>>,
    status: String,
    storage: Option<Arc<Storage>>,
    refresh_required: bool,
    health: HealthState,
}

impl AppModel {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        let http = cx.http_client();
        let load = cx.background_executor().spawn(async move {
            let settings = SettingsManager::new()
                .and_then(|manager| manager.load())
                .unwrap_or_default();
            let mut health = HealthState::default();
            let storage = Storage::open_default().map(Arc::new);
            if let Err(error) = &storage {
                health.storage_issue = Some(error.to_string());
            }
            let (catalog_index, mut cache_status, refresh_required) = match &storage {
                Ok(storage) => match storage.load_catalog(unix_now()) {
                    Ok(Some(cached)) => {
                        health.last_catalog_success = Some(cached.fetched_at);
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
                    Err(error) => {
                        health.catalog_issue = Some(error.to_string());
                        (
                            Arc::new(CatalogIndex::new(Arc::new(Catalog::default()))),
                            format!("Catalog is unavailable: {error}. The database was retained."),
                            false,
                        )
                    }
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
                        health.last_scan_success = Some(unix_now());
                        cache_status
                            .push_str(&format!(" Detected {} installed addons.", installed.len()));
                        installed
                    }
                    Err(error) => {
                        health.scan_issue = Some(error.to_string());
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
                health,
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
                this.health = state.health;
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
                            this.health.catalog_issue = None;
                            this.health.last_catalog_success = Some(unix_now());
                            this.status = format!("ESOUI catalog refreshed ({outcome:?}).");
                        }
                        Err(error) => {
                            this.health.catalog_issue = Some(error.to_string());
                            let fallback = if this.catalog_index.is_empty() {
                                "No saved catalog is available in this profile."
                            } else {
                                "The saved catalog remains available."
                            };
                            this.status = format!("ESOUI refresh failed: {error}. {fallback}");
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
                        evaluate_update_alerts(app);
                        cx.notify();
                    })
                    .ok();
                }
            }

            // Periodic catalog freshness loop: every 15 minutes, refresh the
            // catalog when the cached snapshot is past the 4h TTL. Runs on the
            // background executor; the refresh itself is the shared flow.
            let mut last_freshness_check = std::time::Instant::now();

            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                if last_freshness_check.elapsed() >= std::time::Duration::from_secs(15 * 60) {
                    last_freshness_check = std::time::Instant::now();
                    let refresh_due = this
                        .update(cx, |app, _| {
                            app.catalog_service.is_some()
                                && app.health.last_catalog_success.is_none_or(|at| {
                                    unix_now().saturating_sub(at) >= CACHE_TTL_SECONDS
                                })
                        })
                        .unwrap_or(false);
                    if refresh_due {
                        this.update(cx, |app, cx| {
                            if app.catalog_service.is_some() {
                                crate::flows::refresh_catalog_now(cx.entity(), cx);
                            }
                        })
                        .ok();
                    }
                }
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
                                app.health.scan_issue = Some(error.to_string());
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
            pending_rebuild: false,
            health: HealthState::default(),
            observed_completions: HashSet::new(),
            alerted_update_count: None,
            pending_update_alert: None,
            details_loading: false,
        }
    }
}

pub(crate) fn replace_catalog_state(app: &mut AppModel, catalog: Arc<Catalog>) {
    let catalog_index = Arc::new(CatalogIndex::new(catalog));
    let (matched, missing_dependencies) = Matcher::analyze_index(&app.installed, &catalog_index);
    app.matched = Arc::new(matched);
    app.missing_dependencies = Arc::new(missing_dependencies);
    app.installed_index = Arc::new(InstalledIndex::new(&app.installed, &app.matched));
    app.catalog_index = catalog_index;
    evaluate_update_alerts(app);
}

pub(crate) fn replace_installed_state(
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
    app.health.scan_issue = None;
    app.health.last_scan_success = Some(unix_now());
    evaluate_update_alerts(app);
}

impl Drop for AppModel {
    fn drop(&mut self) {
        if let Some(manager) = self.install_manager.take() {
            manager.shutdown();
        }
    }
}

#[derive(Clone)]
pub(crate) struct InstalledGroup {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) icon_url: Option<String>,
    pub(crate) items: Vec<(Addon, MatchedAddon)>,
}

pub(crate) fn installed_groups(
    model: &AppModel,
    query: &str,
    updates_only: bool,
) -> Vec<InstalledGroup> {
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
