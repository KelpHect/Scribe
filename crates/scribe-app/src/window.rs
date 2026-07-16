use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt as _, App, Bounds, ClipboardItem, Context, Entity, FocusHandle,
    Focusable, IntoElement, MouseButton, ObjectFit, Pixels, Point, Role, SharedString, StyledImage,
    Subscription, Window, WindowControlArea, div, img, px, uniform_list,
};
use gpui_component::{
    ActiveTheme as _, ElementExt as _, Icon, IconName, IndexPath, StyledExt as _, TitleBar,
    input::{Input, InputEvent, InputState},
    menu::PopupMenu,
    scroll::ScrollableElement as _,
    select::{SearchableVec, SelectEvent, SelectState},
};
use scribe_core::{CatalogSort, Category, RemoteAddon};

use crate::components::{
    CategoryPickerOverlay, FilterOption, NativeButton, catalog_sort_button, category_artwork,
    category_filter_trigger, compatibility_control, empty_state, health_status_row, metric_pill,
    render_category_picker_overlay, render_inline_notice, render_status_notice, settings_card,
    settings_section_label,
};
use crate::flows::{
    browse_for_addons, enqueue_remote, refresh_catalog, rescan_configured_addons, set_app_theme,
};
use crate::model::{
    AppModel, NoticeTone, OverlayKind, Page, PageState, RecoveryPhase, installed_groups,
    navigate_to_page, status_notice,
};
use crate::overlays::{
    claim_context_invoker, context_menu_key, menu_anchor, open_category_context_menu,
    render_context_menu_overlay, render_details_modal, render_lightbox, render_local_details_modal,
    render_missing_dependencies, render_rebuild_modal, render_task_activity,
    render_uninstall_modal, task_activity_relevant, task_state_is_terminal,
};
use crate::rows::{catalog_row, matched_row};
use crate::theme::*;
use crate::{
    FocusSearch, OpenSettings, ShowFindMore, ShowInstalled, ShowUpdates, duration_label,
    performance_report, record_keyboard_input, record_render_build, record_scroll_input,
    ui_metrics_snapshot,
};

#[derive(Clone, Copy)]
pub(crate) enum ScribeWindowControl {
    Minimize,
    Restore,
    Maximize,
    Close,
}

impl ScribeWindowControl {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Minimize => "scribe-window-minimize",
            Self::Restore => "scribe-window-restore",
            Self::Maximize => "scribe-window-maximize",
            Self::Close => "scribe-window-close",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Minimize => "Minimize window",
            Self::Restore => "Restore window",
            Self::Maximize => "Maximize window",
            Self::Close => "Close window",
        }
    }

    pub(crate) fn icon(self) -> IconName {
        match self {
            Self::Minimize => IconName::WindowMinimize,
            Self::Restore => IconName::WindowRestore,
            Self::Maximize => IconName::WindowMaximize,
            Self::Close => IconName::WindowClose,
        }
    }

    pub(crate) fn hit_area(self) -> WindowControlArea {
        match self {
            Self::Minimize => WindowControlArea::Min,
            Self::Restore | Self::Maximize => WindowControlArea::Max,
            Self::Close => WindowControlArea::Close,
        }
    }
}

pub(crate) fn scribe_window_control(control: ScribeWindowControl) -> gpui::AnyElement {
    let is_close = matches!(control, ScribeWindowControl::Close);
    let selector = control.id();
    div()
        .id(control.id())
        .debug_selector(move || selector.into())
        .role(Role::Button)
        .aria_label(control.label())
        .w(px(46.0))
        .h(px(32.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .text_color(gpui::rgb(SCRIBE_FOREGROUND).opacity(0.8))
        .window_control_area(control.hit_area())
        .hover(move |button| {
            if is_close {
                button
                    .bg(gpui::rgb(SCRIBE_CLOSE_HOVER))
                    .text_color(gpui::rgb(SCRIBE_OVERLAY_FOREGROUND))
            } else {
                button
                    .bg(gpui::rgba(SCRIBE_SURFACE_HOVER_RGBA))
                    .text_color(gpui::rgb(SCRIBE_FOREGROUND))
            }
        })
        .active(move |button| {
            if is_close {
                button
                    .bg(gpui::rgb(SCRIBE_CLOSE_HOVER))
                    .text_color(gpui::rgb(SCRIBE_OVERLAY_FOREGROUND))
            } else {
                button.bg(gpui::rgba(SCRIBE_SURFACE_ACTIVE_RGBA))
            }
        })
        .child(Icon::new(control.icon()).size(px(13.0)))
        .into_any_element()
}

/// Middle-truncates a display path so both the drive root and the leaf folder
/// stay visible in the settings library card.
fn truncate_middle(text: &str, max_chars: usize) -> String {
    let chars = text.chars().count();
    if chars <= max_chars {
        return text.to_owned();
    }
    let keep = max_chars.saturating_sub(1) / 2;
    let head: String = text.chars().take(keep).collect();
    let tail: String = text.chars().skip(chars - keep).collect();
    format!("{head}…{tail}")
}

pub(crate) fn scribe_window_controls(is_maximized: bool) -> gpui::AnyElement {
    div()
        .id("scribe-window-controls")
        .debug_selector(|| "scribe-window-controls".into())
        .absolute()
        .top_0()
        .right_0()
        .h(px(SCRIBE_TITLE_ROW_HEIGHT))
        .flex()
        .items_center()
        .child(scribe_window_control(ScribeWindowControl::Minimize))
        .child(scribe_window_control(if is_maximized {
            ScribeWindowControl::Restore
        } else {
            ScribeWindowControl::Maximize
        }))
        .child(scribe_window_control(ScribeWindowControl::Close))
        .into_any_element()
}

pub(crate) struct ScribeWindow {
    pub(crate) model: Entity<AppModel>,
    pub(crate) installed: Entity<PageState>,
    pub(crate) find_more: Entity<PageState>,
    pub(crate) updates: Entity<PageState>,
    pub(crate) settings: Entity<PageState>,
    pub(crate) category_search: Entity<InputState>,
    pub(crate) category_select: Entity<SelectState<SearchableVec<FilterOption>>>,
    pub(crate) version_select: Entity<SelectState<SearchableVec<FilterOption>>>,
    pub(crate) sort_select: Entity<SelectState<SearchableVec<FilterOption>>>,
    pub(crate) category_options: Vec<FilterOption>,
    pub(crate) version_options: Vec<FilterOption>,
    pub(crate) category_palette_open: bool,
    pub(crate) category_cursor: usize,
    pub(crate) category_query: String,
    pub(crate) category_trigger_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(crate) hide_installed: bool,
    pub(crate) sort_ascending: bool,
    pub(crate) expanded_categories: HashSet<String>,
    pub(crate) installed_groups_initialized: bool,
    pub(crate) selection_mode: bool,
    pub(crate) selected_folders: HashSet<String>,
    pub(crate) dismissed_required_dependencies: bool,
    pub(crate) dismissed_optional_dependencies: bool,
    pub(crate) dismissed_task_uids: HashSet<String>,
    pub(crate) task_center_open: bool,
    pub(crate) diagnostics_open: bool,
    pub(crate) focus: FocusHandle,
    pub(crate) modal_focus: FocusHandle,
    pub(crate) lightbox_focus: FocusHandle,
    pub(crate) overlay_kind: Option<OverlayKind>,
    pub(crate) overlay_return_focus: Option<FocusHandle>,
    pub(crate) lightbox_return_focus: Option<FocusHandle>,
    pub(crate) context_invoker_key: Option<String>,
    pub(crate) context_invoker_focus: FocusHandle,
    pub(crate) context_menu: Option<(Entity<PopupMenu>, Point<Pixels>)>,
    pub(crate) context_menu_subscription: Option<Subscription>,
    pub(crate) profiled_page: Page,
    pub(crate) profiled_viewport: gpui::Size<Pixels>,
    pub(crate) _subscriptions: Vec<Subscription>,
}

impl ScribeWindow {
    pub(crate) fn new(
        model: Entity<AppModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let modal_focus = cx.focus_handle();
        let lightbox_focus = cx.focus_handle();
        let context_invoker_focus = cx.focus_handle();
        window.focus(&focus, cx);
        let installed = cx.new(|cx| PageState::new("Search installed addons…", window, cx));
        let find_more =
            cx.new(|cx| PageState::new("Search addons, authors, or folders…", window, cx));
        let updates = cx.new(|cx| PageState::new("Search available updates…", window, cx));
        let settings = cx.new(|cx| PageState::new("Search settings…", window, cx));
        let category_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search categories…"));
        let category_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(vec![FilterOption::new("Any category", "")]),
                Some(IndexPath::default()),
                window,
                cx,
            )
            .searchable(true)
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
                    FilterOption::new("Downloads", "downloads"),
                    FilterOption::new("Favorites", "favorites"),
                    FilterOption::new("Updated", "date"),
                    FilterOption::new("Title", "title"),
                    FilterOption::new("Author", "author"),
                    FilterOption::new("Category", "category"),
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
            &version_select,
            |_, _, _: &SelectEvent<SearchableVec<FilterOption>>, cx| cx.notify(),
        ));
        subscriptions.push(cx.subscribe(
            &sort_select,
            |_, _, _: &SelectEvent<SearchableVec<FilterOption>>, cx| cx.notify(),
        ));
        subscriptions.push(cx.subscribe(&category_search, |this, _, event, cx| {
            if matches!(event, InputEvent::Change | InputEvent::PressEnter { .. }) {
                this.category_query = this.category_search.read(cx).value().to_string();
                this.category_cursor = 0;
                cx.notify();
            }
        }));
        Self {
            model,
            installed,
            find_more,
            updates,
            settings,
            category_search,
            category_select,
            version_select,
            sort_select,
            category_options: vec![FilterOption::new("Any category", "")],
            version_options: vec![FilterOption::new("All versions", "")],
            category_palette_open: false,
            category_cursor: 0,
            category_query: String::new(),
            category_trigger_bounds: Rc::new(Cell::new(Bounds::default())),
            hide_installed: true,
            sort_ascending: false,
            expanded_categories: HashSet::new(),
            installed_groups_initialized: false,
            selection_mode: false,
            selected_folders: HashSet::new(),
            dismissed_required_dependencies: false,
            dismissed_optional_dependencies: false,
            dismissed_task_uids: HashSet::new(),
            task_center_open: false,
            diagnostics_open: false,
            focus,
            modal_focus,
            lightbox_focus,
            overlay_kind: None,
            overlay_return_focus: None,
            lightbox_return_focus: None,
            context_invoker_key: None,
            context_invoker_focus,
            context_menu: None,
            context_menu_subscription: None,
            profiled_page: Page::Installed,
            profiled_viewport: window.bounds().size,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn page_state(&self, page: Page) -> &Entity<PageState> {
        match page {
            Page::Installed => &self.installed,
            Page::FindMore => &self.find_more,
            Page::Updates => &self.updates,
            Page::Settings => &self.settings,
        }
    }

    pub(crate) fn sync_overlay_focus(
        &mut self,
        next: Option<OverlayKind>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.overlay_kind == next {
            return;
        }

        let previous = self.overlay_kind;

        if previous.is_none() && next.is_some() {
            self.overlay_return_focus = window.focused(cx);
        }
        if previous != Some(OverlayKind::Lightbox) && next == Some(OverlayKind::Lightbox) {
            self.lightbox_return_focus = window.focused(cx);
        }

        self.overlay_kind = next;
        match next {
            Some(OverlayKind::Lightbox) => {
                self.context_menu = None;
                self.context_menu_subscription = None;
                window.focus(&self.lightbox_focus, cx);
                window.on_next_frame(|window, cx| window.focus_next(cx));
            }
            Some(_) => {
                self.context_menu = None;
                self.context_menu_subscription = None;
                if previous == Some(OverlayKind::Lightbox) {
                    if let Some(focus) = self.lightbox_return_focus.take() {
                        window.focus(&focus, cx);
                    } else {
                        window.focus(&self.modal_focus, cx);
                    }
                } else {
                    window.focus(&self.modal_focus, cx);
                    // The modal surface owns the trap, while its first tabbable control is
                    // always a safe Close action. Advance into that control after GPUI has
                    // mounted the overlay so destructive actions never receive initial focus.
                    window.on_next_frame(|window, cx| window.focus_next(cx));
                }
            }
            None => {
                self.lightbox_return_focus = None;
                if let Some(focus) = self.overlay_return_focus.take() {
                    window.focus(&focus, cx);
                } else {
                    window.focus(&self.focus, cx);
                }
            }
        }
    }

    pub(crate) fn sync_catalog_filter_options(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (mut categories, versions) = {
            let model = self.model.read(cx);
            (
                model.catalog_index.categories().to_vec(),
                model.catalog_index.compatibility_versions().to_vec(),
            )
        };
        categories.sort_unstable_by(|left, right| left.name.cmp(&right.name));
        let mut category_options = Vec::with_capacity(categories.len() + 1);
        category_options.push(FilterOption::new("Any category", ""));
        category_options.extend(
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
        if self.category_options != category_options {
            let selected = self
                .category_select
                .read(cx)
                .selected_value()
                .cloned()
                .unwrap_or_default();
            self.category_options = category_options.clone();
            self.category_select.update(cx, |state, cx| {
                state.set_items(SearchableVec::new(category_options), window, cx);
                state.set_selected_value(&selected, window, cx);
                if state.selected_value().is_none() {
                    state.set_selected_value(&SharedString::default(), window, cx);
                }
            });
        }

        let mut version_options = vec![FilterOption::new("All ESO versions", "")];
        if let Some(latest) = versions.into_iter().next() {
            version_options.push(FilterOption::new(format!("Latest ESO · {latest}"), latest));
        }
        if self.version_options != version_options {
            let selected = self
                .version_select
                .read(cx)
                .selected_value()
                .cloned()
                .unwrap_or_default();
            self.version_options = version_options.clone();
            self.version_select.update(cx, |state, cx| {
                state.set_items(SearchableVec::new(version_options), window, cx);
                state.set_selected_value(&selected, window, cx);
                if state.selected_value().is_none() {
                    state.set_selected_value(&SharedString::default(), window, cx);
                }
            });
        }
    }

    pub(crate) fn selected_catalog_sort(&self, cx: &App) -> CatalogSort {
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

    pub(crate) fn render_sidebar(&self, page: Page, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (installed_count, update_count) = {
            let model = self.model.read(cx);
            (
                model.installed.len(),
                model
                    .matched
                    .iter()
                    .filter(|item| item.update_available)
                    .count(),
            )
        };
        div()
            .id("scribe-sidebar")
            .debug_selector(|| "scribe-sidebar".into())
            .w(px(SCRIBE_SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(gpui::rgba(SCRIBE_SIDEBAR_TINT_RGBA))
            .border_r_1()
            .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
            .child(
                div()
                    .px(px(16.0))
                    .pt(px(18.0))
                    .pb(px(16.0))
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .child(
                        img("scribe-logo-v2.png")
                            .size(px(22.0))
                            .object_fit(ObjectFit::Contain),
                    )
                    .child(
                        div()
                            .font_semibold()
                            .text_size(px(13.0))
                            .text_color(gpui::rgb(SCRIBE_FOREGROUND))
                            .child("Scribe"),
                    ),
            )
            .child(
                div()
                    .px(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(self.render_sidebar_item(
                        Page::Installed,
                        page == Page::Installed,
                        Some(installed_count),
                        cx,
                    ))
                    .child(self.render_sidebar_item(
                        Page::FindMore,
                        page == Page::FindMore,
                        None,
                        cx,
                    ))
                    .child(self.render_sidebar_item(
                        Page::Updates,
                        page == Page::Updates,
                        Some(update_count),
                        cx,
                    )),
            )
            .child(div().flex_1())
            .child(
                div()
                    .mx(px(12.0))
                    .h(px(1.0))
                    .bg(gpui::rgba(SCRIBE_HAIRLINE_RGBA)),
            )
            .child(div().p(px(12.0)).child(self.render_sidebar_item(
                Page::Settings,
                page == Page::Settings,
                None,
                cx,
            )))
            .into_any_element()
    }

    pub(crate) fn render_sidebar_item(
        &self,
        page: Page,
        active: bool,
        count: Option<usize>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let _ = cx;
        let model = self.model.clone();
        let keyboard_model = self.model.clone();
        let text_color = if active {
            gpui::rgb(SCRIBE_PRIMARY)
        } else {
            gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA)
        };
        div()
            .id(SharedString::from(format!("nav-{}", page.title())))
            .focusable()
            .tab_stop(true)
            .role(Role::Button)
            .aria_label(page.title())
            .relative()
            .h(px(SCRIBE_NAV_HEIGHT))
            .px(px(10.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .when(active, |item| {
                item.bg(gpui::rgba(SCRIBE_ACCENT_SOFT_RGBA)).child(
                    div()
                        .absolute()
                        .left(px(2.0))
                        .top(px(9.0))
                        .bottom(px(9.0))
                        .w(px(3.0))
                        .rounded(px(1.5))
                        .bg(gpui::rgb(SCRIBE_PRIMARY)),
                )
            })
            .text_color(text_color)
            .hover(|item| {
                item.bg(if active {
                    gpui::rgba(SCRIBE_ACCENT_SOFT_RGBA)
                } else {
                    gpui::rgba(SCRIBE_SURFACE_HOVER_RGBA)
                })
            })
            .focus(|item| {
                item.border_1()
                    .border_color(gpui::rgba(SCRIBE_FOCUS_RING_RGBA))
            })
            .on_click(move |_, _, cx| {
                model.update(cx, |model, cx| {
                    navigate_to_page(model, page);
                    cx.notify();
                });
            })
            .on_key_down(move |event, _, cx| {
                if !event.is_held && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    keyboard_model.update(cx, |model, cx| {
                        navigate_to_page(model, page);
                        cx.notify();
                    });
                }
            })
            .child(Icon::new(page.icon()).size(px(18.0)).flex_none())
            .child(
                div()
                    .flex_1()
                    .text_size(px(13.0))
                    .when(active, |label| label.font_semibold())
                    .child(page.title()),
            )
            .when_some(count.filter(|count| *count > 0), |item, count| {
                item.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(if active {
                            gpui::rgb(SCRIBE_PRIMARY)
                        } else {
                            gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA)
                        })
                        .child(count.to_string()),
                )
            })
    }

    pub(crate) fn render_page_header(
        &self,
        page: Page,
        archive_status_label: &'static str,
        archive_status_color: u32,
        update_count: usize,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let max_width = if page == Page::Settings {
            SCRIBE_SETTINGS_MAX_WIDTH
        } else {
            SCRIBE_CONTENT_MAX_WIDTH
        };
        let hide_installed = self.hide_installed;
        let toggle_hidden = cx.entity();
        let keyboard_toggle_hidden = cx.entity();
        let refresh_model = self.model.clone();
        let rescan_model = self.model.clone();
        let bulk_model = self.model.clone();
        let updates_model = self.model.clone();
        let selection_mode = self.selection_mode;
        let selected_count = self.selected_folders.len();
        let selected_for_remove: Vec<String> = self.selected_folders.iter().cloned().collect();
        let available_updates: Vec<RemoteAddon> = if page == Page::Updates {
            self.model
                .read(cx)
                .matched
                .iter()
                .filter(|item| item.update_available)
                .filter_map(|item| item.remote.clone())
                .collect()
        } else {
            Vec::new()
        };
        let actions = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .when(page == Page::FindMore, |row| {
                row.child(
                    div()
                        .id("hide-installed-filter")
                        .focusable()
                        .tab_stop(true)
                        .role(Role::Switch)
                        .aria_selected(hide_installed)
                        .aria_label("Hide installed addons")
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(7.0))
                        .focus(|switch| {
                            switch
                                .border_1()
                                .border_color(gpui::rgba(SCRIBE_FOCUS_RING_RGBA))
                        })
                        .on_click(move |_, _, cx| {
                            toggle_hidden.update(cx, |this, cx| {
                                this.hide_installed = !this.hide_installed;
                                cx.notify();
                            });
                        })
                        .on_key_down(move |event, _, cx| {
                            if !event.is_held
                                && matches!(event.keystroke.key.as_str(), "enter" | "space")
                            {
                                cx.stop_propagation();
                                keyboard_toggle_hidden.update(cx, |this, cx| {
                                    this.hide_installed = !this.hide_installed;
                                    cx.notify();
                                });
                            }
                        })
                        .child(
                            div()
                                .w(px(28.0))
                                .h(px(16.0))
                                .p(px(2.0))
                                .rounded(px(8.0))
                                .bg(if hide_installed {
                                    gpui::rgb(SCRIBE_SWITCH_ACTIVE)
                                } else {
                                    gpui::rgba(SCRIBE_SURFACE_ACTIVE_RGBA)
                                })
                                .flex()
                                .justify_end()
                                .when(!hide_installed, |track| track.justify_start())
                                .child(div().size(px(12.0)).rounded(px(6.0)).bg(
                                    if hide_installed {
                                        gpui::rgb(SCRIBE_PRIMARY_FOREGROUND)
                                    } else {
                                        gpui::rgb(SCRIBE_FOREGROUND)
                                    },
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
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
            })
            .when(page == Page::Installed, |row| {
                row.when(selection_mode && selected_count > 0, |row| {
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
                        .secondary()
                        .icon(IconName::LoaderCircle)
                        .on_activate(move |window, cx| {
                            rescan_configured_addons(rescan_model.clone(), window, cx)
                        }),
                )
            })
            .when(page == Page::Updates && update_count > 0, |row| {
                row.child(
                    NativeButton::new("update-all-toolbar", format!("Update all · {update_count}"))
                        .icon(IconName::ArrowDown)
                        .on_activate(move |window, cx| {
                            for remote in available_updates.clone() {
                                enqueue_remote(remote, updates_model.clone(), window, cx);
                            }
                        }),
                )
            });

        div()
            .w_full()
            .flex()
            .justify_center()
            .child(
                div()
                    .debug_selector(|| "command-deck-primary-row".into())
                    .w_full()
                    .max_w(px(max_width))
                    .h(px(40.0))
                    .px(px(SCRIBE_CONTENT_GUTTER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.0))
                            .text_size(px(11.0))
                            .font_medium()
                            .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                            .child(
                                div()
                                    .size(px(6.0))
                                    .rounded(px(3.0))
                                    .bg(gpui::rgb(archive_status_color)),
                            )
                            .child(archive_status_label),
                    )
                    .child(actions),
            )
            .into_any_element()
    }

    pub(crate) fn render_filter_row(
        &self,
        page: Page,
        search: Entity<InputState>,
        update_count: usize,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if page == Page::Settings || (page == Page::Updates && update_count == 0) {
            return None;
        }
        let max_width = if page == Page::Settings {
            SCRIBE_SETTINGS_MAX_WIDTH
        } else {
            SCRIBE_CONTENT_MAX_WIDTH
        };
        let search_label = match page {
            Page::Installed => "Search installed addons",
            Page::FindMore => "Search addons, authors, or folders",
            Page::Updates => "Search available updates",
            Page::Settings => "Search settings",
        };
        let search_value = search.read(cx).value();
        let category_value = self
            .category_select
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_default();
        let category_label = if category_value.is_empty() {
            "Any".to_owned()
        } else {
            self.model
                .read(cx)
                .catalog_index
                .category(&category_value)
                .map(|category| category.name.to_string())
                .unwrap_or_else(|| category_value.to_string())
        };
        let category_icon = self
            .category_options
            .iter()
            .find(|option| option.value == category_value)
            .and_then(|option| option.icon_url.clone());
        let version_value = self
            .version_select
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_default();
        let sort_value = self
            .sort_select
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_else(|| SharedString::from("downloads"));
        let sort_label = match sort_value.as_ref() {
            "favorites" => "Most favorited",
            "date" => "Recently updated",
            "title" if self.sort_ascending => "Name A–Z",
            "title" => "Name Z–A",
            "category" => "Category A–Z",
            _ => "Popular",
        };
        let toggle_groups = cx.entity();
        let select_visible = cx.entity();
        let clear_selection = cx.entity();
        let enter_selection = cx.entity();
        let leave_selection = cx.entity();
        let selection_mode = self.selection_mode;
        let selected_count = self.selected_folders.len();
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
        let all_group_ids: Vec<String> = if page == Page::Installed {
            let model = self.model.read(cx);
            installed_groups(model, "", false)
                .into_iter()
                .map(|group| group.id)
                .collect()
        } else {
            Vec::new()
        };
        let all_groups_expanded = !all_group_ids.is_empty()
            && all_group_ids
                .iter()
                .all(|group| self.expanded_categories.contains(group));
        let toolbar_owner = cx.entity();
        let row = div()
            .debug_selector(|| "command-deck-context-row".into())
            .w_full()
            .max_w(px(max_width))
            .px(px(SCRIBE_CONTENT_GUTTER))
            .pb(px(12.0))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .id("command-deck-search-region")
                    .debug_selector(|| "command-deck-search-control".into())
                    .role(Role::SearchInput)
                    .aria_label(search_label)
                    .aria_value(search_value)
                    .min_w(px(200.0))
                    .max_w(px(320.0))
                    .flex_1()
                    .child(
                        Input::new(&search)
                            .prefix(IconName::Search)
                            .role(Role::SearchInput)
                            .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
                            .text_color(gpui::rgb(SCRIBE_FOREGROUND))
                            .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                            .shadow_none(),
                    ),
            )
            .when(page == Page::FindMore, |row| {
                row.child(category_filter_trigger(
                    SharedString::from(format!("Category: {category_label}")),
                    category_icon,
                    self.category_palette_open,
                    toolbar_owner.clone(),
                    self.category_search.clone(),
                    self.category_trigger_bounds.clone(),
                ))
                .child(compatibility_control(
                    version_value,
                    self.version_options.clone(),
                    self.version_select.clone(),
                    toolbar_owner.clone(),
                ))
                .child(catalog_sort_button(
                    "sort-filter-control",
                    format!("Sort: {sort_label}"),
                    154.0,
                    sort_value,
                    self.sort_ascending,
                    self.sort_select.clone(),
                    toolbar_owner,
                ))
            })
            .when(page == Page::Installed, |row| {
                row.child(
                    NativeButton::new(
                        "toggle-installed-groups",
                        if all_groups_expanded {
                            "Collapse groups"
                        } else {
                            "Expand groups"
                        },
                    )
                    .secondary()
                    .icon(if all_groups_expanded {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .on_activate(move |_, cx| {
                        toggle_groups.update(cx, |this, cx| {
                            if all_groups_expanded {
                                this.expanded_categories.clear();
                            } else {
                                this.expanded_categories.extend(all_group_ids.clone());
                            }
                            cx.notify();
                        });
                    }),
                )
                .when(!selection_mode, |row| {
                    row.child(
                        NativeButton::new("enter-installed-selection", "Select addons")
                            .secondary()
                            .icon(IconName::Check)
                            .on_activate(move |_, cx| {
                                enter_selection.update(cx, |this, cx| {
                                    this.selection_mode = true;
                                    cx.notify();
                                });
                            }),
                    )
                })
                .when(selection_mode, |row| {
                    row.child(
                        NativeButton::new("select-visible-installed", "Select visible")
                            .secondary()
                            .on_activate(move |_, cx| {
                                select_visible.update(cx, |this, cx| {
                                    this.selected_folders.extend(visible_folders.clone());
                                    cx.notify();
                                });
                            }),
                    )
                    .when(selected_count > 0, |row| {
                        row.child(
                            NativeButton::new("clear-installed-selection", "Clear")
                                .secondary()
                                .on_activate(move |_, cx| {
                                    clear_selection.update(cx, |this, cx| {
                                        this.selected_folders.clear();
                                        cx.notify();
                                    });
                                }),
                        )
                    })
                    .child(
                        NativeButton::new("leave-installed-selection", "Done")
                            .secondary()
                            .on_activate(move |_, cx| {
                                leave_selection.update(cx, |this, cx| {
                                    this.selection_mode = false;
                                    this.selected_folders.clear();
                                    cx.notify();
                                });
                            }),
                    )
                })
            });
        Some(
            div()
                .w_full()
                .flex()
                .justify_center()
                .child(row)
                .into_any_element(),
        )
    }

    pub(crate) fn render_installed(
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
        let addon_path_configured = !model.settings.addon_path.is_empty();
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
            } else if query.trim().is_empty() && !addon_path_configured {
                (
                    "Set up your addon library",
                    "1. Choose the ESO AddOns folder.  2. Scribe scans it safely.  3. Review the detected addons here.",
                )
            } else if query.trim().is_empty() {
                (
                    "No addons detected yet",
                    "The library folder is configured. Rescan it now, or install an addon from Find More.",
                )
            } else {
                (
                    "No matching addons",
                    "Try a different name, author, folder, or version.",
                )
            };
            let action = if updates_only && query.trim().is_empty() {
                let model = self.model.clone();
                Some(
                    NativeButton::new("empty-browse-catalog", "Browse addon catalog")
                        .icon(IconName::Search)
                        .on_activate(move |_, cx| {
                            model.update(cx, |app, cx| {
                                navigate_to_page(app, Page::FindMore);
                                cx.notify();
                            });
                        }),
                )
            } else if query.trim().is_empty() && !addon_path_configured {
                let model = self.model.clone();
                Some(
                    NativeButton::new("empty-configure-library", "Choose AddOns folder")
                        .icon(IconName::FolderOpen)
                        .on_activate(move |window, cx| {
                            browse_for_addons(model.clone(), window, cx);
                        }),
                )
            } else if query.trim().is_empty() {
                let model = self.model.clone();
                Some(
                    NativeButton::new("empty-rescan-library", "Rescan library")
                        .icon(IconName::LoaderCircle)
                        .on_activate(move |window, cx| {
                            rescan_configured_addons(model.clone(), window, cx);
                        }),
                )
            } else {
                let input = if updates_only {
                    self.updates.read(cx).search.clone()
                } else {
                    self.installed.read(cx).search.clone()
                };
                Some(
                    NativeButton::new("empty-clear-library-search", "Clear search")
                        .secondary()
                        .icon(IconName::Close)
                        .on_activate(move |window, cx| {
                            input.update(cx, |input, cx| input.set_value("", window, cx));
                        }),
                )
            };
            return empty_state(IconName::Inbox, title, message, action);
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
            let context_key = format!("category:{}", group.id);
            let context_focus = (self.context_invoker_key.as_deref() == Some(context_key.as_str()))
                .then(|| self.context_invoker_focus.clone());
            let pointer_context_key = context_key.clone();
            let keyboard_context_key = context_key;
            let toggle = cx.entity();
            let keyboard_toggle = cx.entity();
            let menu_owner = cx.entity();
            let pointer_menu_owner = menu_owner.clone();
            let keyboard_menu_owner = menu_owner.clone();
            let pointer_group_id = group.id.clone();
            let keyboard_group_id = group.id.clone();
            let header_bounds = Rc::new(Cell::new(Bounds::default()));
            let paint_bounds = header_bounds.clone();
            let keyboard_bounds = header_bounds.clone();
            let icon = group.icon_url.clone();
            let count = group.items.len();
            let name = group.name.clone();
            let mut section = div()
                .w_full()
                .mb(px(16.0))
                .flex()
                .flex_col()
                .rounded(px(SCRIBE_CARD_RADIUS))
                .border_1()
                .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
                .overflow_hidden()
                .child(
                    div()
                        .id(SharedString::from(format!(
                            "installed-category-{}",
                            group.id
                        )))
                        .when_some(context_focus, |header, focus| header.track_focus(&focus))
                        .focusable()
                        .tab_stop(true)
                        .role(Role::Button)
                        .aria_expanded(expanded)
                        .aria_label(format!("{name}, {count} addons"))
                        .cursor_pointer()
                        .h(px(40.0))
                        .px(px(12.0))
                        .border_b_1()
                        .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                        .hover(|header| header.bg(gpui::rgba(SCRIBE_SURFACE_HOVER_RGBA)))
                        .focus(|header| header.border_color(gpui::rgba(SCRIBE_FOCUS_RING_RGBA)))
                        .flex()
                        .items_center()
                        .gap(px(9.0))
                        .on_prepaint(move |bounds, _, _| paint_bounds.set(bounds))
                        .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                            cx.stop_propagation();
                            let invocation = claim_context_invoker(
                                &pointer_menu_owner,
                                pointer_context_key.clone(),
                                event.position,
                                cx,
                            );
                            open_category_context_menu(
                                pointer_group_id.clone(),
                                expanded,
                                pointer_menu_owner.clone(),
                                invocation,
                                window,
                                cx,
                            );
                        })
                        .on_click(move |_, _, cx| {
                            toggle.update(cx, |this, cx| {
                                if !this.expanded_categories.remove(&group_id) {
                                    this.expanded_categories.insert(group_id.clone());
                                }
                                cx.notify();
                            });
                        })
                        .on_key_down(move |event, window, cx| {
                            if event.is_held {
                                return;
                            }
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                cx.stop_propagation();
                                keyboard_toggle.update(cx, |view, cx| {
                                    if !view.expanded_categories.remove(&keyboard_group_id) {
                                        view.expanded_categories.insert(keyboard_group_id.clone());
                                    }
                                    cx.notify();
                                });
                            } else if context_menu_key(event) {
                                cx.stop_propagation();
                                let invocation = claim_context_invoker(
                                    &keyboard_menu_owner,
                                    keyboard_context_key.clone(),
                                    menu_anchor(keyboard_bounds.get()),
                                    cx,
                                );
                                open_category_context_menu(
                                    keyboard_group_id.clone(),
                                    expanded,
                                    keyboard_menu_owner.clone(),
                                    invocation,
                                    window,
                                    cx,
                                );
                            }
                        })
                        .child(category_artwork(icon, &name, 20.0))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .font_semibold()
                                .text_size(px(13.0))
                                .text_color(gpui::rgb(SCRIBE_FOREGROUND))
                                .child(name),
                        )
                        .child(
                            div()
                                .px(px(7.0))
                                .py(px(1.0))
                                .rounded(px(8.0))
                                .bg(gpui::rgba(SCRIBE_SURFACE_ACTIVE_RGBA))
                                .text_size(px(11.0))
                                .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                                .child(count.to_string()),
                        )
                        .child(
                            Icon::new(if expanded {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .size(px(16.0))
                            .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA)),
                        ),
                );
            if expanded {
                for (addon, decision) in group.items {
                    let category = decision
                        .remote
                        .as_ref()
                        .and_then(|remote| category_map.get(remote.category_id.as_str()))
                        .cloned();
                    let selected = (!updates_only && self.selection_mode)
                        .then(|| self.selected_folders.contains(&addon.folder_name));
                    let context_key = format!("installed:{}", addon.folder_name);
                    let context_focus = (self.context_invoker_key.as_deref()
                        == Some(context_key.as_str()))
                    .then(|| self.context_invoker_focus.clone());
                    section = section.child(matched_row(
                        addon,
                        decision,
                        category,
                        selected,
                        context_focus,
                        selection_owner.clone(),
                        self.model.clone(),
                    ));
                }
            }
            rows.push(section.into_any_element());
        }
        let selection_bar = (!updates_only && self.selection_mode).then(|| {
            let selected_count = self.selected_folders.len();
            let selected_for_remove: Vec<String> = self.selected_folders.iter().cloned().collect();
            let uninstall_model = self.model.clone();
            let clear_owner = cx.entity();
            let done_owner = cx.entity();
            div()
                .w_full()
                .mb(px(16.0))
                .h(px(44.0))
                .px(px(12.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                .bg(gpui::rgba(SCRIBE_SURFACE_RAISED_RGBA))
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.0))
                        .font_semibold()
                        .child(format!("{selected_count} selected")),
                )
                .child(
                    NativeButton::new("selection-bar-uninstall", "Uninstall selected")
                        .danger()
                        .icon(IconName::CircleX)
                        .on_activate(move |_, cx| {
                            uninstall_model.update(cx, |app, cx| {
                                app.pending_uninstall = selected_for_remove.clone();
                                cx.notify();
                            });
                        }),
                )
                .child(
                    NativeButton::new("selection-bar-clear", "Clear")
                        .ghost()
                        .on_activate(move |_, cx| {
                            clear_owner.update(cx, |this, cx| {
                                this.selected_folders.clear();
                                cx.notify();
                            });
                        }),
                )
                .child(
                    NativeButton::new("selection-bar-done", "Done")
                        .ghost()
                        .on_activate(move |_, cx| {
                            done_owner.update(cx, |this, cx| {
                                this.selection_mode = false;
                                this.selected_folders.clear();
                                cx.notify();
                            });
                        }),
                )
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .when(!updates_only && !missing.is_empty(), |layout| {
                layout.child(div().px(px(SCRIBE_CONTENT_GUTTER)).pb(px(12.0)).child(
                    render_missing_dependencies(
                        missing,
                        self.model.clone(),
                        cx.entity(),
                        self.dismissed_required_dependencies,
                        self.dismissed_optional_dependencies,
                    ),
                ))
            })
            .child(
                div()
                    .min_h_0()
                    .flex_1()
                    .on_scroll_wheel(|_, window, _| record_scroll_input(window))
                    .overflow_y_scrollbar()
                    .px(px(SCRIBE_CONTENT_GUTTER))
                    .py(px(16.0))
                    .flex()
                    .flex_col()
                    .when_some(selection_bar, |column, bar| column.child(bar))
                    .children(rows),
            )
            .into_any_element()
    }

    pub(crate) fn render_catalog(&self, query: &str, cx: &mut Context<Self>) -> gpui::AnyElement {
        let model = self.model.read(cx);
        let catalog_index = model.catalog_index.clone();
        let category_id = self
            .category_select
            .read(cx)
            .selected_value()
            .filter(|value| !value.is_empty())
            .map(SharedString::to_string);
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
            false,
            compatibility.as_deref(),
            &hidden_uids,
            self.selected_catalog_sort(cx),
            self.sort_ascending,
        ));
        let count = indices.len();
        if count == 0 {
            let catalog_is_empty = catalog_index.is_empty();
            let (title, message) = if catalog_is_empty && model.health.catalog_issue.is_some() {
                (
                    "Catalog unavailable offline",
                    "No saved catalog is available in this profile. Check the connection, then retry the ESOUI refresh.",
                )
            } else if catalog_is_empty {
                (
                    "Catalog is connecting",
                    "Scribe will show ESOUI addons here after the catalog refresh completes.",
                )
            } else {
                (
                    "No catalog matches",
                    "Try a broader addon name, author, category, or folder search.",
                )
            };
            let action = if catalog_is_empty {
                let model = self.model.clone();
                Some(
                    NativeButton::new("empty-refresh-catalog", "Refresh ESOUI catalog")
                        .icon(IconName::LoaderCircle)
                        .on_activate(move |window, cx| refresh_catalog(model.clone(), window, cx)),
                )
            } else {
                let input = self.find_more.read(cx).search.clone();
                let category = self.category_select.clone();
                let version = self.version_select.clone();
                Some(
                    NativeButton::new("empty-clear-catalog-filters", "Clear search and filters")
                        .secondary()
                        .icon(IconName::Close)
                        .on_activate(move |window, cx| {
                            input.update(cx, |input, cx| input.set_value("", window, cx));
                            category.update(cx, |state, cx| {
                                state.set_selected_value(&SharedString::default(), window, cx);
                            });
                            version.update(cx, |state, cx| {
                                state.set_selected_value(&SharedString::default(), window, cx);
                            });
                        }),
                )
            };
            return empty_state(IconName::Search, title, message, action);
        }
        let app_model = self.model.clone();
        let menu_owner = cx.entity();
        let category_index = catalog_index.clone();
        let context_invoker_key = self.context_invoker_key.clone();
        let context_invoker_focus = self.context_invoker_focus.clone();
        let active_category = category_id
            .as_deref()
            .and_then(|id| catalog_index.category(id))
            .map(|category| category.name.to_string());
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(36.0))
                    .px(px(SCRIBE_CONTENT_GUTTER))
                    .flex()
                    .items_center()
                    .gap(px(7.0))
                    .border_b_1()
                    .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                    .text_size(px(12.0))
                    .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                    .child(format!("{count} ESOUI addons"))
                    .when_some(active_category, |header, category| {
                        header.child("·").child(format!("Category: {category}"))
                    }),
            )
            .child(
                div()
                    .min_h_0()
                    .flex_1()
                    .px(px(SCRIBE_CONTENT_GUTTER))
                    .pt(px(12.0))
                    .on_scroll_wheel(|_, window, _| record_scroll_input(window))
                    .child(
                        uniform_list("catalog-list", count, move |range: Range<usize>, _, _| {
                            range
                                .filter_map(|index| indices.get(index))
                                .filter_map(|index| category_index.addon(*index))
                                .map(|addon| {
                                    let category =
                                        category_index.category(&addon.category_id).cloned();
                                    let context_key = format!("catalog:{}", addon.uid);
                                    let context_focus = (context_invoker_key.as_deref()
                                        == Some(context_key.as_str()))
                                    .then(|| context_invoker_focus.clone());
                                    catalog_row(
                                        addon,
                                        category,
                                        context_focus,
                                        menu_owner.clone(),
                                        app_model.clone(),
                                    )
                                })
                                .collect()
                        })
                        .size_full(),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn render_settings_page(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme().clone();
        let reduce_motion = cx.reduce_motion();
        let performance = ui_metrics_snapshot();
        let diagnostics = performance_report(performance);
        let (path, storage_degraded, installed_count, catalog_count, memory_limit, health) = {
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
                model.health.clone(),
            )
        };
        let browse_model = self.model.clone();
        let rescan_model = self.model.clone();
        let theme_model = self.model.clone();
        let theme_keyboard_model = self.model.clone();
        let rebuild_model = self.model.clone();
        let retry_catalog_model = self.model.clone();
        let diagnostics_owner = cx.entity();
        let reveal_path = path.clone();
        let copy_path = path.clone();
        let reading_pane = div()
            .size_full()
            .overflow_y_scrollbar()
            .px(px(SCRIBE_CONTENT_GUTTER))
            .py(px(20.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                settings_card(
                    IconName::FolderOpen,
                    "Add-on library",
                    "Scribe scans and safely changes addons only inside this folder.",
                )
                .child(
                    div()
                        .mt(px(14.0))
                        .px(px(12.0))
                        .py(px(10.0))
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                        .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
                        .text_size(px(12.0))
                        .font_family("Consolas")
                        .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
                        .overflow_x_hidden()
                        .whitespace_nowrap()
                        .child(truncate_middle(&path, 72)),
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
                            NativeButton::new("open-addons", "Open folder")
                                .secondary()
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
                                .secondary()
                                .icon(IconName::Copy)
                                .on_activate(move |_, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        copy_path.clone(),
                                    ));
                                }),
                        )
                        .child(
                            NativeButton::new("rescan-addons", "Rescan")
                                .secondary()
                                .icon(IconName::Search)
                                .on_activate(move |window, cx| {
                                    rescan_configured_addons(rescan_model.clone(), window, cx);
                                }),
                        ),
                ),
            )
            .child(
                settings_card(
                    IconName::Palette,
                    "Appearance",
                    "One dark glass theme across every surface.",
                )
                .child(
                    div()
                        .id("theme-scribe")
                        .focusable()
                        .tab_stop(true)
                        .role(Role::Button)
                        .aria_label("Scribe glass theme, selected")
                        .cursor_pointer()
                        .mt(px(14.0))
                        .h(px(36.0))
                        .w_full()
                        .max_w(px(360.0))
                        .px(px(12.0))
                        .rounded(px(10.0))
                        .border_1()
                        .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                        .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .hover(|row| row.bg(gpui::rgba(SCRIBE_SURFACE_HOVER_RGBA)))
                        .focus(|row| {
                            row.border_color(gpui::rgba(SCRIBE_FOCUS_RING_RGBA))
                        })
                        .on_click(move |_, _, cx| {
                            set_app_theme("scribe", theme_model.clone(), cx)
                        })
                        .on_key_down(move |event, _, cx| {
                            if !event.is_held
                                && matches!(event.keystroke.key.as_str(), "enter" | "space")
                            {
                                cx.stop_propagation();
                                set_app_theme("scribe", theme_keyboard_model.clone(), cx);
                            }
                        })
                        .child(
                            Icon::new(IconName::Palette)
                                .size(px(16.0))
                                .text_color(gpui::rgb(SCRIBE_PRIMARY)),
                        )
                        .child(div().flex_1().text_size(px(13.0)).child("Scribe glass"))
                        .child(
                            Icon::new(IconName::Check)
                                .size(px(14.0))
                                .text_color(gpui::rgb(SCRIBE_PRIMARY)),
                        ),
                )
                .child(
                    div()
                        .mt(px(10.0))
                        .text_size(px(12.0))
                        .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                        .child(format!(
                            "Windows reduced motion {} · honored across all animation",
                            if reduce_motion { "on" } else { "off" }
                        )),
                ),
            )
            .child(
                settings_card(
                    IconName::HardDrive,
                    "Health & recovery",
                    "Storage, catalog, and addon-folder checks with their last successful runs.",
                )
                .child(
                    div()
                        .mt(px(14.0))
                        .flex()
                        .flex_wrap()
                        .gap(px(10.0))
                        .child(metric_pill(installed_count.to_string(), "installed"))
                        .child(metric_pill(catalog_count.to_string(), "catalog"))
                        .child(metric_pill(
                            format!("{memory_limit} MB"),
                            "warning threshold",
                        ))
                        .child(metric_pill(
                            if storage_degraded { "Degraded" } else { "Healthy" },
                            "storage",
                        )),
                )
                .child(settings_section_label(
                    "Health checks",
                    "Storage, catalog, and addon-folder scan",
                ))
                .child(
                    div()
                        .mt(px(14.0))
                        .flex()
                        .flex_col()
                        .gap(px(7.0))
                        .child(health_status_row(
                            "Local database",
                            if let Some(issue) = health.storage_issue.as_deref() {
                                ("Unavailable", issue.to_owned(), "Catalog cache and install history may be unavailable.".to_owned())
                            } else {
                                ("Healthy", "redb opened successfully.".to_owned(), "Cached startup and install records are available.".to_owned())
                            },
                            None,
                        ))
                        .child(health_status_row(
                            "ESOUI catalog",
                            if let Some(issue) = health.catalog_issue.as_deref() {
                                ("Degraded", issue.to_owned(), "Cached results stay visible when available; new metadata may be stale.".to_owned())
                            } else if catalog_count == 0 {
                                ("Waiting", "No catalog snapshot is loaded yet.".to_owned(), "Find More remains empty until a refresh succeeds.".to_owned())
                            } else {
                                ("Healthy", format!("{catalog_count} addons indexed."), "Search, categories, matching, and details are available.".to_owned())
                            },
                            health.last_catalog_success,
                        ))
                        .child(health_status_row(
                            "Addon folder scan",
                            if let Some(issue) = health.scan_issue.as_deref() {
                                ("Failed", issue.to_owned(), "Installed addons and dependency results may be incomplete.".to_owned())
                            } else if path == "No AddOns folder selected" {
                                ("Not configured", "Choose the ESO AddOns folder to enable scanning.".to_owned(), "Installed and Updates remain empty until a folder is selected.".to_owned())
                            } else {
                                ("Healthy", format!("Detected {installed_count} installed addons."), "Installed metadata and dependency analysis are current as of the last scan.".to_owned())
                            },
                            health.last_scan_success,
                        ))
                        .when_some(health.recovery_message.clone(), |list, message| {
                            list.child(health_status_row(
                                "Last recovery",
                                (match health.recovery_phase {
                                    RecoveryPhase::Running => "Running",
                                    RecoveryPhase::Succeeded => "Succeeded",
                                    RecoveryPhase::Failed => "Failed",
                                    RecoveryPhase::Idle => "Idle",
                                }, message, "Only reconstructible cache data is changed; retained databases are never silently overwritten.".to_owned()),
                                None,
                            ))
                        }),
                )
                .child(
                    div()
                        .mt(px(12.0))
                        .flex()
                        .gap(px(8.0))
                        .child(
                            NativeButton::new("retry-catalog-page", "Retry catalog refresh")
                                .secondary()
                                .icon(IconName::Search)
                                .on_activate(move |window, cx| {
                                    refresh_catalog(retry_catalog_model.clone(), window, cx);
                                }),
                        )
                        .when(storage_degraded, |actions| {
                            actions.child(
                                NativeButton::new("rebuild-storage-page", "Rebuild local cache…")
                                    .danger()
                                    .on_activate(move |_, cx| {
                                        rebuild_model.update(cx, |app, cx| {
                                            app.pending_rebuild = true;
                                            cx.notify();
                                        });
                                    }),
                            )
                        }),
                ),
            )
            .child(
                settings_card(
                    IconName::BookOpen,
                    "About & diagnostics",
                    "Scribe is a fast, native ESO addon manager powered by GPUI. Addon metadata and downloads are provided by ESOUI/MMOUI.",
                )
                .child(
                    div()
                        .mt(px(12.0))
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .text_size(px(12.0))
                        .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
                        .child(format!("Scribe v{}", env!("CARGO_PKG_VERSION"))),
                )
                .child(
                    div().mt(px(12.0)).child(
                        NativeButton::new(
                            "toggle-performance-diagnostics",
                            if self.diagnostics_open {
                                "Hide technical details"
                            } else {
                                "Show technical details"
                            },
                        )
                        .secondary()
                        .icon(if self.diagnostics_open {
                            IconName::ChevronUp
                        } else {
                            IconName::ChevronDown
                        })
                        .on_activate(move |_, cx| {
                            diagnostics_owner.update(cx, |view, cx| {
                                view.diagnostics_open = !view.diagnostics_open;
                                cx.notify();
                            });
                        }),
                    ),
                )
                .when(self.diagnostics_open, |card| {
                    card.child(
                        div()
                            .mt(px(14.0))
                            .flex()
                            .flex_wrap()
                            .gap(px(10.0))
                            .child(metric_pill(
                                duration_label(performance.scroll_p50_us),
                                "scroll response p50",
                            ))
                            .child(metric_pill(
                                duration_label(performance.scroll_p95_us),
                                "scroll response p95",
                            ))
                            .child(metric_pill(
                                duration_label(performance.scroll_p99_us),
                                "scroll response p99",
                            ))
                            .child(metric_pill(
                                performance.slow_scroll_frames.to_string(),
                                "over 16.67 ms",
                            ))
                            .child(metric_pill(
                                duration_label(performance.keyboard_p95_us),
                                "keyboard response p95",
                            ))
                            .child(metric_pill(
                                performance.slow_keyboard_frames.to_string(),
                                "keyboard over 16.67 ms",
                            ))
                            .child(metric_pill(
                                duration_label(performance.render_p95_us),
                                "render build p95",
                            ))
                            .child(metric_pill(
                                duration_label(performance.resize_render_p95_us),
                                "resize build p95",
                            ))
                            .child(metric_pill(
                                duration_label(performance.overlay_render_p95_us),
                                "overlay build p95",
                            ))
                            .child(metric_pill(
                                duration_label(performance.page_render_p95_us),
                                "page build p95",
                            )),
                    )
                    .child(
                        div().mt(px(12.0)).child(
                            NativeButton::new(
                                "copy-performance-diagnostics",
                                "Copy diagnostics",
                            )
                            .secondary()
                            .icon(IconName::Copy)
                            .on_activate(move |_, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    diagnostics.clone(),
                                ));
                            }),
                        ),
                    )
                })
                .child(
                    div().mt(px(12.0)).child(
                        NativeButton::new("open-esoui", "Visit ESOUI")
                            .ghost()
                            .icon(IconName::ExternalLink)
                            .on_activate(|_, cx| cx.open_url("https://www.esoui.com/")),
                    ),
                ),
            )
            .child(render_inline_notice(
                "auto-update-inactive",
                "Automatic updates stay off",
                "Scribe always lets you review an update before changing addon files.",
                NoticeTone::Info,
                &theme,
            ));
        reading_pane.into_any_element()
    }
}

impl Render for ScribeWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let render_started = Instant::now();
        self.sync_catalog_filter_options(window, cx);
        let is_maximized = window.is_maximized();
        let theme = cx.theme().clone();
        let text_color = theme.foreground;
        let (
            page,
            status,
            loading,
            tasks,
            selected_details,
            selected_local,
            lightbox_index,
            pending_uninstall,
            pending_rebuild,
            health,
            catalog_count,
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
                model.pending_rebuild,
                model.health.clone(),
                model.catalog_index.len(),
            )
        };
        let overlay_kind = if lightbox_index.is_some() {
            Some(OverlayKind::Lightbox)
        } else if pending_rebuild {
            Some(OverlayKind::Rebuild)
        } else if !pending_uninstall.is_empty() {
            Some(OverlayKind::Uninstall)
        } else if selected_local.is_some() {
            Some(OverlayKind::LocalDetails)
        } else if selected_details.is_some() {
            Some(OverlayKind::RemoteDetails)
        } else {
            None
        };
        let viewport_size = window.bounds().size;
        let overlay_changed = self.overlay_kind != overlay_kind;
        let page_changed = self.profiled_page != page;
        let resized = self.profiled_viewport != viewport_size;
        self.profiled_page = page;
        self.profiled_viewport = viewport_size;
        self.sync_overlay_focus(overlay_kind, window, cx);
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
        let context_menu = self.context_menu.clone();
        let update_count = self
            .model
            .read(cx)
            .matched
            .iter()
            .filter(|item| item.update_available)
            .count();
        let escape_model = self.model.clone();
        let escape_window = cx.entity();
        let category_keyboard_open = page == Page::FindMore && self.category_palette_open;
        let normalized_category_query = self.category_query.trim().to_ascii_lowercase();
        let category_keyboard_options = self
            .category_options
            .iter()
            .filter(|option| {
                normalized_category_query.is_empty()
                    || option
                        .label
                        .to_ascii_lowercase()
                        .contains(&normalized_category_query)
            })
            .cloned()
            .collect::<Vec<_>>();
        let category_keyboard_cursor = self
            .category_cursor
            .min(category_keyboard_options.len().saturating_sub(1));
        let category_keyboard_state = self.category_select.clone();
        let category_keyboard_search = self.category_search.clone();
        let notice = status_notice(&status, loading, &health);
        let (archive_status_label, archive_status_color) = if health.storage_issue.is_some() {
            ("LOCAL ARCHIVE UNAVAILABLE", SCRIBE_HEALTH_DANGER)
        } else if health.catalog_issue.is_some() && catalog_count > 0 {
            ("OFFLINE · SAVED CATALOG", SCRIBE_HEALTH_WARNING)
        } else if health.catalog_issue.is_some() {
            ("CATALOG UNAVAILABLE", SCRIBE_HEALTH_DANGER)
        } else if catalog_count == 0 {
            ("CATALOG CONNECTING", SCRIBE_HEALTH_WARNING)
        } else {
            ("CATALOG CURRENT", SCRIBE_HEALTH_SUCCESS)
        };
        for task in &tasks {
            if !task_state_is_terminal(task.state) {
                self.dismissed_task_uids.remove(&task.uid);
            }
        }
        let tasks: Vec<_> = tasks
            .into_iter()
            .filter(task_activity_relevant)
            .filter(|task| {
                !task_state_is_terminal(task.state) || !self.dismissed_task_uids.contains(&task.uid)
            })
            .collect();
        let task_activity = (!tasks.is_empty()).then(|| {
            render_task_activity(
                tasks,
                &theme,
                self.model.clone(),
                self.task_center_open,
                cx.entity(),
            )
        });
        let category_picker = (page == Page::FindMore && self.category_palette_open).then(|| {
            let selected = self
                .category_select
                .read(cx)
                .selected_value()
                .cloned()
                .unwrap_or_default();
            render_category_picker_overlay(CategoryPickerOverlay {
                options: self.category_options.clone(),
                selected,
                query: self.category_query.clone(),
                cursor: self.category_cursor,
                search: self.category_search.clone(),
                state: self.category_select.clone(),
                owner: cx.entity(),
                trigger: self.category_trigger_bounds.get(),
                viewport: viewport_size,
            })
        });

        let sidebar = self.render_sidebar(page, cx);
        // The component TitleBar always paints its own platform window controls
        // at its right edge on Windows, and the pinned rev offers no way to
        // disable them. Extending the TitleBar 138px past the window's right
        // edge (clipped by the wrapper) parks those glyphs and their hit areas
        // fully off-screen, so the Scribe control strip above needs no
        // occluding fill and the acrylic title row stays seamless.
        let title_bar = TitleBar::new()
            .w(viewport_size.width - px(SCRIBE_SIDEBAR_WIDTH) + px(138.0))
            .flex_none()
            .h(px(SCRIBE_TITLE_ROW_HEIGHT))
            .border_b_0()
            .child(
                div()
                    .h_full()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .pl(px(SCRIBE_CONTENT_GUTTER - 12.0))
                    .child(
                        div()
                            .font_semibold()
                            .text_size(px(22.0))
                            .text_color(gpui::rgb(SCRIBE_FOREGROUND))
                            .child(page.title()),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                            .child(page.subtitle()),
                    ),
            );
        let page_header = self.render_page_header(
            page,
            archive_status_label,
            archive_status_color,
            update_count,
            cx,
        );
        let filter_row = self.render_filter_row(page, search.clone(), update_count, cx);

        let content = match page {
            Page::Installed => self.render_installed(&query, false, cx),
            Page::Updates => self.render_installed(&query, true, cx),
            Page::FindMore => self.render_catalog(&query, cx),
            Page::Settings => self.render_settings_page(cx),
        };
        let page_content_max_width = if page == Page::Settings {
            SCRIBE_SETTINGS_MAX_WIDTH
        } else {
            SCRIBE_CONTENT_MAX_WIDTH
        };
        let page_content = div().min_h_0().flex_1().flex().justify_center().child(
            div()
                .debug_selector(|| "page-content".into())
                .min_h_0()
                .w_full()
                .h_full()
                .max_w(px(page_content_max_width))
                .child(content),
        );
        let page_content = if cx.reduce_motion() {
            page_content.into_any_element()
        } else {
            page_content
                .with_animation(
                    SharedString::from(format!("page-transition-{}", page.title())),
                    Animation::new(Duration::from_millis(SCRIBE_MOTION_PAGE_MS)),
                    |page, delta| page.opacity(0.84 + delta * 0.16),
                )
                .into_any_element()
        };

        let body = div()
            .id("scribe-root")
            .role(Role::Application)
            .aria_label("Scribe ESO addon manager")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ShowInstalled, _, cx| {
                this.model.update(cx, |app, cx| {
                    navigate_to_page(app, Page::Installed);
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &ShowFindMore, _, cx| {
                this.model.update(cx, |app, cx| {
                    navigate_to_page(app, Page::FindMore);
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &ShowUpdates, _, cx| {
                this.model.update(cx, |app, cx| {
                    navigate_to_page(app, Page::Updates);
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                let page = this.model.read(cx).page;
                if page == Page::Settings {
                    return;
                }
                let input = this.page_state(page).read(cx).search.clone();
                window.focus(&input.read(cx).focus_handle(cx), cx);
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, _, cx| {
                this.model.update(cx, |app, cx| {
                    navigate_to_page(app, Page::Settings);
                    cx.notify();
                });
            }))
            .size_full()
            .flex()
            .text_color(text_color)
            .child(sidebar)
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(gpui::rgba(SCRIBE_WINDOW_TINT_RGBA))
                    .child(div().flex_none().overflow_x_hidden().child(title_bar))
                    .child(
                        div()
                            .min_h_0()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .child(page_header)
                            .when_some(filter_row, |column, row| column.child(row))
                            .when_some(notice, |column, notice| {
                                column.child(
                                    div().w_full().flex().justify_center().child(
                                        div()
                                            .w_full()
                                            .max_w(px(page_content_max_width))
                                            .px(px(SCRIBE_CONTENT_GUTTER))
                                            .pb(px(10.0))
                                            .child(render_status_notice(notice, &theme)),
                                    ),
                                )
                            })
                            .child(page_content),
                    ),
            );

        let root = div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .font_family(".Segoe UI Variable Text")
            .text_color(text_color)
            .on_key_down(move |event, window, cx| {
                if event.is_held {
                    return;
                }
                record_keyboard_input(window);
                if category_keyboard_open && !category_keyboard_options.is_empty() {
                    let next_cursor = match event.keystroke.key.as_str() {
                        "down" => {
                            Some((category_keyboard_cursor + 1) % category_keyboard_options.len())
                        }
                        "up" => Some(
                            category_keyboard_cursor
                                .checked_sub(1)
                                .unwrap_or(category_keyboard_options.len() - 1),
                        ),
                        "home" => Some(0),
                        "end" => Some(category_keyboard_options.len() - 1),
                        _ => None,
                    };
                    if let Some(next_cursor) = next_cursor {
                        cx.stop_propagation();
                        escape_window.update(cx, |view, cx| {
                            view.category_cursor = next_cursor;
                            cx.notify();
                        });
                        return;
                    }
                    if event.keystroke.key == "enter" {
                        cx.stop_propagation();
                        let value = category_keyboard_options[category_keyboard_cursor]
                            .value
                            .clone();
                        category_keyboard_state.update(cx, |state, cx| {
                            state.set_selected_value(&value, window, cx);
                        });
                        category_keyboard_search.update(cx, |search, cx| {
                            search.set_value("", window, cx);
                        });
                        escape_window.update(cx, |view, cx| {
                            view.category_palette_open = false;
                            view.category_query.clear();
                            cx.notify();
                        });
                        return;
                    }
                }
                if event.keystroke.key == "escape" {
                    escape_window.update(cx, |view, cx| {
                        if view.category_palette_open {
                            view.category_palette_open = false;
                            view.category_query.clear();
                            cx.notify();
                        }
                    });
                    escape_model.update(cx, |app, cx| {
                        if app.lightbox_index.take().is_none() {
                            app.selected_details = None;
                            app.selected_local = None;
                            app.pending_uninstall.clear();
                            app.pending_rebuild = false;
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
            .child(body)
            .child(scribe_window_controls(is_maximized))
            .when_some(category_picker, |root, picker| root.child(picker))
            .when_some(task_activity, |root, activity| root.child(activity))
            .when_some(context_menu, |root, (menu, position)| {
                root.child(render_context_menu_overlay(menu, position, viewport_size))
            })
            .when_some(selected_local, |root, local| {
                root.child(render_local_details_modal(
                    local,
                    selected_details.clone(),
                    selected_category.clone(),
                    self.model.clone(),
                    self.modal_focus.clone(),
                ))
            })
            .when(!has_selected_local && selected_details.is_some(), |root| {
                root.child(render_details_modal(
                    selected_details.clone().expect("checked above"),
                    selected_category.clone(),
                    self.model.clone(),
                    self.modal_focus.clone(),
                ))
            })
            .when(!pending_uninstall.is_empty(), |root| {
                root.child(render_uninstall_modal(
                    pending_uninstall.clone(),
                    self.model.clone(),
                    self.modal_focus.clone(),
                ))
            })
            .when(pending_rebuild, |root| {
                root.child(render_rebuild_modal(
                    self.model.clone(),
                    self.modal_focus.clone(),
                ))
            })
            .when_some(lightbox_index, |root, index| {
                root.when_some(selected_details, |root, details| {
                    root.child(render_lightbox(
                        details,
                        index,
                        self.model.clone(),
                        self.lightbox_focus.clone(),
                        cx.reduce_motion(),
                    ))
                })
            });
        record_render_build(
            render_started.elapsed(),
            resized,
            overlay_changed,
            page_changed,
        );
        root
    }
}
