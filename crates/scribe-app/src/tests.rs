use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    Bounds, MouseButton, Point, Role, TestAppContext, WindowBounds, WindowOptions, div, point, px,
    size,
};
use gpui_component::Theme;
use scribe_core::{
    Addon, AppSettings, Catalog, CatalogIndex, Category, InstalledIndex, MatchedAddon,
    MissingDependency, RemoteAddon, RemoteAddonDetails, TaskProgress, TaskState,
};

use crate::components::*;
use crate::embedded_assets_ready;
use crate::model::*;
use crate::overlays::*;
use crate::theme::*;
use crate::window::*;

/// Opaque RGB base of the glass window tint (alpha byte stripped).
const GLASS_BASE: u32 = (SCRIBE_WINDOW_TINT_RGBA >> 8) & 0x00ff_ffff;

fn relative_luminance(rgb: u32) -> f64 {
    let channel = |shift| {
        let value = ((rgb >> shift) & 0xff_u32) as f64 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

fn contrast_ratio(first: u32, second: u32) -> f64 {
    let mut luminance = [relative_luminance(first), relative_luminance(second)];
    luminance.sort_by(f64::total_cmp);
    (luminance[1] + 0.05) / (luminance[0] + 0.05)
}

#[test]
fn semantic_theme_pairs_meet_wcag_contrast() {
    let normal_text_pairs = [
        ("body", SCRIBE_FOREGROUND, GLASS_BASE),
        ("primary", SCRIBE_PRIMARY_FOREGROUND, SCRIBE_PRIMARY),
        (
            "primary hover",
            SCRIBE_PRIMARY_FOREGROUND,
            SCRIBE_PRIMARY_HOVER,
        ),
        (
            "primary pressed",
            SCRIBE_PRIMARY_FOREGROUND,
            SCRIBE_PRIMARY_PRESSED,
        ),
        ("link", SCRIBE_LINK, GLASS_BASE),
        ("link hover", SCRIBE_LINK_HOVER, GLASS_BASE),
        ("link pressed", SCRIBE_LINK_PRESSED, GLASS_BASE),
        ("warning", SCRIBE_WARNING_FOREGROUND, SCRIBE_WARNING),
        ("info", SCRIBE_INFO_FOREGROUND, SCRIBE_INFO),
        ("success", SCRIBE_SUCCESS_FOREGROUND, SCRIBE_SUCCESS),
        ("danger", SCRIBE_DANGER_FOREGROUND, SCRIBE_DANGER),
        ("overlay", SCRIBE_OVERLAY_FOREGROUND, SCRIBE_OVERLAY),
        (
            "window controls",
            SCRIBE_OVERLAY_FOREGROUND,
            SCRIBE_CLOSE_HOVER,
        ),
        ("subtle body text", SCRIBE_TEXT_SUBTLE, GLASS_BASE),
        (
            "subtle panel text",
            SCRIBE_TEXT_SUBTLE,
            SCRIBE_PARCHMENT_PANEL,
        ),
        (
            "subtle elevated text",
            SCRIBE_TEXT_SUBTLE,
            SCRIBE_PARCHMENT_ELEVATED,
        ),
        (
            "panel muted text",
            SCRIBE_TEXT_SUBTLE,
            SCRIBE_PARCHMENT_PANEL,
        ),
        ("action text", SCRIBE_TEXT_ACTION, GLASS_BASE),
        ("danger tint text", SCRIBE_DANGER, SCRIBE_PARCHMENT_ELEVATED),
        ("health success", SCRIBE_HEALTH_SUCCESS, GLASS_BASE),
        ("health warning", SCRIBE_HEALTH_WARNING, GLASS_BASE),
        ("health danger", SCRIBE_HEALTH_DANGER, GLASS_BASE),
    ];
    for (name, foreground, background) in normal_text_pairs {
        let ratio = contrast_ratio(foreground, background);
        assert!(ratio >= 4.5, "{name} contrast was {ratio:.2}:1");
    }

    for (name, indicator, surface) in [
        ("focus indicator", SCRIBE_FOCUS_RING, GLASS_BASE),
        ("selected border", SCRIBE_ACTIVE_BORDER, GLASS_BASE),
    ] {
        let ratio = contrast_ratio(indicator, surface);
        assert!(ratio >= 3.0, "{name} contrast was {ratio:.2}:1");
    }
}

/// The glass shell composites translucent fills over the acrylic backdrop.
/// Composite the tints over pure white (the worst-case bright backdrop) and
/// require primary text to stay readable; secondary/accent chrome keeps a
/// conservative 3.0 floor in that worst case.
#[test]
fn glass_tint_keeps_text_legible_over_bright_backdrops() {
    // Tokens are 0xRRGGBBAA: alpha in the low byte, blue at shift 8.
    let composite = |tint: u32, backdrop: u32| -> u32 {
        let alpha = ((tint & 0xff) as f64) / 255.0;
        let channel = |shift: u32| -> u32 {
            let fg = ((tint >> shift) & 0xff) as f64;
            let bg = ((backdrop >> shift) & 0xff) as f64;
            (alpha * fg + (1.0 - alpha) * bg).round() as u32
        };
        (channel(24) << 16) | (channel(16) << 8) | channel(8)
    };
    let tint_over_white = composite(SCRIBE_WINDOW_TINT_RGBA, 0xffffff);
    let ratio = contrast_ratio(SCRIBE_FOREGROUND, tint_over_white);
    assert!(ratio >= 4.5, "body contrast was {ratio:.2}:1");
    for (name, foreground) in [
        ("secondary", SCRIBE_TEXT_SUBTLE),
        ("accent", SCRIBE_PRIMARY),
    ] {
        let ratio = contrast_ratio(foreground, tint_over_white);
        assert!(ratio >= 3.0, "{name} contrast was {ratio:.2}:1");
    }
    let sidebar_over_white = composite(SCRIBE_SIDEBAR_TINT_RGBA, 0xffffff);
    let ratio = contrast_ratio(SCRIBE_FOREGROUND, sidebar_over_white);
    assert!(ratio >= 3.0, "sidebar contrast was {ratio:.2}:1");
}

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
        pending_rebuild: false,
        health: HealthState::default(),
        observed_completions: HashSet::new(),
        alerted_update_count: None,
        pending_update_alert: None,
        details_loading: false,
    }
}

#[test]
fn update_states_use_readable_product_language() {
    assert_eq!(update_state_label("up-to-date"), "Up to date");
    assert_eq!(update_state_label("remote-newer"), "Update available");
    assert_eq!(
        update_state_label("md5-only-changed"),
        "Files changed upstream"
    );
    assert_eq!(update_state_label("custom state"), "custom state");
}

fn task(state: TaskState, name: &str, error: &str) -> TaskProgress {
    TaskProgress {
        uid: format!("{name}-uid"),
        name: name.into(),
        state,
        percent: 42.0,
        error: error.into(),
        ..TaskProgress::default()
    }
}

#[test]
fn task_activity_prioritizes_failures_and_reports_transient_work() {
    let failed = task(TaskState::Failed, "HarvestMap", "archive checksum mismatch");
    let cancelled = task(TaskState::Cancelled, "LoreBooks", "");
    let (tone, summary, _) = task_activity_summary(&[failed, cancelled]);
    assert_eq!(tone, NoticeTone::Danger);
    assert_eq!(summary, "1 failed");

    let active = task(TaskState::Downloading, "SkyShards", "");
    let (tone, summary, _) = task_activity_summary(&[active]);
    assert_eq!(tone, NoticeTone::Info);
    assert_eq!(summary, "1 active");

    assert!(task_activity_relevant(&task(
        TaskState::Downloading,
        "SkyShards",
        ""
    )));
    assert!(task_activity_relevant(&task(
        TaskState::Failed,
        "HarvestMap",
        "network unavailable"
    )));
    assert!(task_activity_relevant(&task(
        TaskState::Complete,
        "LoreBooks",
        ""
    )));
    assert!(task_activity_relevant(&task(
        TaskState::Cancelled,
        "Bandits UI",
        ""
    )));
}

#[test]
fn recovery_state_has_priority_and_uses_explicit_semantics() {
    for (phase, tone, title) in [
        (
            RecoveryPhase::Running,
            NoticeTone::Info,
            "Recovery in progress",
        ),
        (
            RecoveryPhase::Succeeded,
            NoticeTone::Success,
            "Recovery complete",
        ),
        (
            RecoveryPhase::Failed,
            NoticeTone::Danger,
            "Recovery needs attention",
        ),
    ] {
        let health = HealthState {
            recovery_phase: phase,
            recovery_message: Some("fixture recovery detail".into()),
            ..HealthState::default()
        };
        let notice = status_notice("ignored", false, &health).unwrap();
        assert_eq!(notice.tone, tone);
        assert_eq!(notice.title, title);
        assert_eq!(notice.message, "fixture recovery detail");
    }
}

#[test]
fn status_surface_keeps_loading_and_actionable_messages_only() {
    let health = HealthState::default();
    let loading = status_notice("Reading cached catalog…", true, &health).unwrap();
    assert_eq!(loading.tone, NoticeTone::Info);
    assert_eq!(loading.title, "Loading library");

    let details = status_notice("Loading details for LoreBooks…", false, &health).unwrap();
    assert_eq!(details.tone, NoticeTone::Info);
    assert_eq!(details.title, "Loading addon details");

    assert!(status_notice("ESOUI catalog refreshed.", false, &health).is_none());
    assert!(status_notice("Rescan complete. Detected 22 addons.", false, &health).is_none());
    assert!(status_notice("Loaded details for LoreBooks.", false, &health).is_none());

    let failed = status_notice("ESOUI refresh failed.", false, &health).unwrap();
    assert_eq!(failed.tone, NoticeTone::Danger);
    assert_eq!(failed.title, "Action needed");
    assert_eq!(
        status_notice(
            "Installation completed, but the rescan failed: access denied",
            false,
            &health,
        )
        .unwrap()
        .tone,
        NoticeTone::Danger
    );

    assert!(status_notice("", false, &health).is_none());
    assert!(
        status_notice(
            "Cached ESOUI catalog loaded. Detected 22 installed addons.",
            false,
            &health
        )
        .is_none()
    );
}

#[test]
fn task_accessibility_includes_state_progress_and_error() {
    let failed = task(TaskState::Failed, "Bandits UI", "network unavailable");
    assert_eq!(
        task_accessible_label(&failed),
        "Bandits UI, Failed, 42 percent. network unavailable"
    );
}

#[test]
fn dynamic_status_region_requests_native_live_announcements() {
    let region = LiveRegion::new(
        div()
            .id("live-region-fixture")
            .role(Role::Status)
            .aria_label("Catalog refresh complete"),
        gpui::accesskit::Live::Polite,
    );
    let mut node = gpui::accesskit::Node::new(Role::Status);
    region.write_a11y_info(&mut node);
    assert_eq!(node.live(), Some(gpui::accesskit::Live::Polite));
    assert_eq!(node.label(), Some("Catalog refresh complete"));
}

#[test]
fn embedded_brand_assets_are_available() {
    assert!(embedded_assets_ready());
}

#[test]
fn installed_groups_keep_mmoui_category_artwork() {
    let remote = RemoteAddon {
        uid: "42".into(),
        category_id: "19".into(),
        ui_name: "Fixture Addon".into(),
        ..RemoteAddon::default()
    };
    let catalog = Arc::new(Catalog {
        addons: vec![remote.clone()],
        categories: vec![Category {
            id: "19".into(),
            name: "Action Bar Mods".into(),
            icon_url: "https://cdn-eso.mmoui.com/images/icons/m19.jpg".into(),
            ..Category::default()
        }],
    });
    let addon = Addon {
        title: "Fixture Addon".into(),
        folder_name: "FixtureAddon".into(),
        ..Addon::default()
    };
    let decision = MatchedAddon {
        remote: Some(remote),
        ..MatchedAddon::default()
    };
    let mut model = empty_model();
    model.catalog_index = Arc::new(CatalogIndex::new(catalog));
    model.installed = Arc::new(vec![addon.clone()]);
    model.matched = Arc::new(vec![decision.clone()]);
    model.installed_index = Arc::new(InstalledIndex::new(&[addon], &[decision]));

    let groups = installed_groups(&model, "", false);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, "Action Bar Mods");
    assert_eq!(
        groups[0].icon_url.as_deref(),
        Some("https://cdn-eso.mmoui.com/images/icons/m19.jpg")
    );
}

#[gpui::test]
fn toasts_stay_bounded_and_legible_at_minimum_window_size(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        Theme::global_mut(cx).font_size = px(18.0);
        let model = cx.new(|_| empty_model());
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1024.0), px(640.0)),
                })),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| ScribeWindow::new(model, window, cx)),
        )
        .unwrap()
    });
    window
        .update(cx, |view, _, cx| {
            let notice = StatusNotice {
                tone: NoticeTone::Danger,
                title: "Action needed",
                message:
                    "ESOUI refresh failed: network unavailable. Cached data remains available."
                        .into(),
            };
            push_toast(
                &mut view.toasts,
                1,
                notice,
                "ESOUI refresh failed.".into(),
                std::time::Instant::now(),
            );
            cx.notify();
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();

    let card = visual.debug_bounds("toast-card").expect("toast card");
    assert!(card.size.width <= px(360.0), "card={card:?}");
    assert!(
        card.origin.x + card.size.width <= px(1024.0),
        "card={card:?}"
    );
    let stack = visual.debug_bounds("toast-stack").expect("toast stack");
    assert!(stack.origin.x >= px(0.0));
}

#[gpui::test]
fn dependency_banner_keeps_details_visible_before_actions(cx: &mut TestAppContext) {
    let addon = Addon {
        title: "Dependency Fixture".into(),
        folder_name: "DependencyFixture".into(),
        ..Addon::default()
    };
    let decision = MatchedAddon::default();
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        Theme::global_mut(cx).font_size = px(18.0);
        let mut app = empty_model();
        app.installed = Arc::new(vec![addon.clone()]);
        app.matched = Arc::new(vec![decision.clone()]);
        app.installed_index = Arc::new(InstalledIndex::new(&[addon], &[decision]));
        app.missing_dependencies = Arc::new(vec![MissingDependency {
            dep_folder_name: "A long localized optional dependency name that must remain readable"
                .into(),
            remote_uid: "dependency-fixture".into(),
            can_install: true,
            optional: true,
            ..MissingDependency::default()
        }]);
        let model = cx.new(|_| app);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1024.0), px(640.0)),
                })),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| ScribeWindow::new(model, window, cx)),
        )
        .unwrap()
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();

    let banner = visual
        .debug_bounds("dependency-banner")
        .expect("dependency banner");
    let copy = visual
        .debug_bounds("dependency-banner-copy")
        .expect("dependency banner copy");
    let details = visual
        .debug_bounds("dependency-banner-details")
        .expect("dependency banner details");
    let actions = visual
        .debug_bounds("dependency-banner-actions")
        .expect("dependency banner actions");
    assert!(details.size.width >= px(250.0), "details={details:?}");
    assert!(copy.origin.x + copy.size.width <= actions.origin.x);
    assert!(actions.origin.x + actions.size.width <= banner.origin.x + banner.size.width);
}

#[gpui::test]
fn context_menu_pointer_and_keyboard_paths_restore_the_invoking_row(cx: &mut TestAppContext) {
    let remote = RemoteAddon {
        uid: "context-menu-fixture".into(),
        category_id: "context-menu-category".into(),
        ui_name: "Context Menu Fixture".into(),
        ..RemoteAddon::default()
    };
    let catalog = Arc::new(Catalog {
        addons: vec![remote.clone()],
        categories: vec![Category {
            id: "context-menu-category".into(),
            name: "Context Menu Category".into(),
            ..Category::default()
        }],
    });
    let addon = Addon {
        title: "Context Menu Fixture".into(),
        folder_name: "ContextMenuFixture".into(),
        ..Addon::default()
    };
    let decision = MatchedAddon {
        remote: Some(remote),
        ..MatchedAddon::default()
    };
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        let mut app = empty_model();
        app.catalog_index = Arc::new(CatalogIndex::new(catalog));
        app.installed = Arc::new(vec![addon.clone()]);
        app.matched = Arc::new(vec![decision.clone()]);
        app.installed_index = Arc::new(InstalledIndex::new(&[addon], &[decision]));
        let model = cx.new(|_| app);
        cx.open_window(Default::default(), move |window, cx| {
            cx.new(|cx| ScribeWindow::new(model, window, cx))
        })
        .unwrap()
    });
    window
        .update(cx, |view, _, cx| {
            view.expanded_categories
                .insert("context-menu-category".into());
            view.installed_groups_initialized = true;
            cx.notify();
        })
        .unwrap();
    let any_window = window.into();
    let mut visual = gpui::VisualTestContext::from_window(any_window, cx);
    visual.run_until_parked();
    let row = visual.debug_bounds("installed-row").expect("installed row");
    let position = point(
        row.origin.x + row.size.width / 2.0,
        row.origin.y + row.size.height / 2.0,
    );

    visual.simulate_mouse_down(position, MouseButton::Right, gpui::Modifiers::default());
    let menu_open = visual
        .update_window(any_window, |view, _, cx| {
            view.downcast::<ScribeWindow>()
                .unwrap()
                .read(cx)
                .context_menu
                .is_some()
        })
        .unwrap();
    assert!(menu_open);

    visual.simulate_keystrokes("escape");
    let (menu_open, invoker_focused) = visual
        .update_window(any_window, |view, window, cx| {
            let view = view.downcast::<ScribeWindow>().unwrap();
            let view = view.read(cx);
            (
                view.context_menu.is_some(),
                view.context_invoker_focus.is_focused(window),
            )
        })
        .unwrap();
    assert!(!menu_open);
    assert!(invoker_focused);

    visual.simulate_keystrokes("shift-f10");
    let keyboard_position = visual
        .update_window(any_window, |view, _, cx| {
            view.downcast::<ScribeWindow>()
                .unwrap()
                .read(cx)
                .context_menu
                .as_ref()
                .map(|(_, position)| *position)
        })
        .unwrap()
        .expect("keyboard context menu");
    assert!(keyboard_position.x >= row.origin.x);
    assert!(keyboard_position.x <= row.origin.x + row.size.width);
    assert!(keyboard_position.y >= row.origin.y);
    assert!(keyboard_position.y <= row.origin.y + row.size.height);

    visual.simulate_keystrokes("escape");
    let invoker_focused = visual
        .update_window(any_window, |view, window, cx| {
            view.downcast::<ScribeWindow>()
                .unwrap()
                .read(cx)
                .context_invoker_focus
                .is_focused(window)
        })
        .unwrap();
    assert!(invoker_focused);
}

#[gpui::test]
fn catalog_toolbar_controls_stay_legible_and_unclipped_at_minimum_window_size(
    cx: &mut TestAppContext,
) {
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        Theme::global_mut(cx).font_size = px(18.0);
        let mut app = empty_model();
        app.page = Page::FindMore;
        let model = cx.new(|_| app);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1024.0), px(640.0)),
                })),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| ScribeWindow::new(model, window, cx)),
        )
        .unwrap()
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();

    let row = visual
        .debug_bounds("command-deck-context-row")
        .expect("toolbar filter row");
    let controls = [
        "command-deck-search-control",
        "category-filter-control",
        "compatibility-filter-control",
        "sort-filter-control",
    ];
    for selector in controls {
        let control = visual.debug_bounds(selector).expect(selector);
        assert!(control.size.height >= px(28.0), "{selector}={control:?}");
        assert!(control.origin.x >= row.origin.x, "{selector}={control:?}");
        assert!(
            control.origin.x + control.size.width <= row.origin.x + row.size.width,
            "row={row:?}, {selector}={control:?}"
        );
    }
}

#[gpui::test]
fn work_surface_stays_centered_and_bounded_on_wide_windows(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        let mut app = empty_model();
        app.page = Page::FindMore;
        let model = cx.new(|_| app);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(2560.0), px(1392.0)),
                })),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| ScribeWindow::new(model, window, cx)),
        )
        .unwrap()
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();

    let toolbar = visual
        .debug_bounds("command-deck-context-row")
        .expect("toolbar filter row");
    let content = visual.debug_bounds("page-content").expect("page content");
    assert_eq!(toolbar.size.width, px(SCRIBE_CONTENT_MAX_WIDTH));
    assert_eq!(content.size.width, px(SCRIBE_CONTENT_MAX_WIDTH));
    assert_eq!(toolbar.origin.x, content.origin.x);
}

#[gpui::test]
fn settings_surface_uses_a_readable_wide_window_measure(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        let mut app = empty_model();
        app.page = Page::Settings;
        let model = cx.new(|_| app);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1920.0), px(1080.0)),
                })),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| ScribeWindow::new(model, window, cx)),
        )
        .unwrap()
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();

    let toolbar = visual
        .debug_bounds("command-deck-primary-row")
        .expect("toolbar title row");
    let content = visual.debug_bounds("page-content").expect("page content");
    assert_eq!(toolbar.size.width, px(SCRIBE_SETTINGS_MAX_WIDTH));
    assert_eq!(content.size.width, px(SCRIBE_SETTINGS_MAX_WIDTH));
    assert_eq!(toolbar.origin.x, content.origin.x);
}

#[gpui::test]
fn settings_column_keeps_reading_measure_at_minimum_window_size(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        let mut app = empty_model();
        app.page = Page::Settings;
        let model = cx.new(|_| app);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1024.0), px(640.0)),
                })),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| ScribeWindow::new(model, window, cx)),
        )
        .unwrap()
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();

    // 1024 - 228 sidebar = 796 content column; the 760 reading measure fits.
    let content = visual.debug_bounds("page-content").expect("page content");
    assert_eq!(content.size.width, px(SCRIBE_SETTINGS_MAX_WIDTH));
}

#[gpui::test]
fn title_bar_controls_use_full_windows_hit_targets(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        let model = cx.new(|_| empty_model());
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1024.0), px(640.0)),
                })),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| ScribeWindow::new(model, window, cx)),
        )
        .unwrap()
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();

    let controls = visual
        .debug_bounds("scribe-window-controls")
        .expect("title bar controls");
    assert_eq!(controls.size.width, px(138.0));
    for selector in [
        "scribe-window-minimize",
        "scribe-window-maximize",
        "scribe-window-close",
    ] {
        let control = visual.debug_bounds(selector).expect(selector);
        assert_eq!(control.size, size(px(46.0), px(32.0)));
    }
}

#[gpui::test]
fn long_catalog_content_stays_clear_of_actions_at_minimum_window_size(cx: &mut TestAppContext) {
    let long = "Very long localized addon metadata ".repeat(12);
    let remote = RemoteAddon {
        uid: "layout-fixture".into(),
        category_id: "long-category".into(),
        ui_name: long.clone().into(),
        ui_author_name: long.clone().into(),
        ui_version: long.clone().into(),
        ui_date: long.clone().into(),
        ui_download_total: 9_999_999,
        ui_favorite_total: 999_999,
        ..RemoteAddon::default()
    };
    let catalog = Arc::new(Catalog {
        addons: vec![remote],
        categories: vec![Category {
            id: "long-category".into(),
            name: long.into(),
            icon_url: "https://cdn-eso.mmoui.com/images/icons/m19.jpg".into(),
            ..Category::default()
        }],
    });
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        Theme::global_mut(cx).font_size = px(18.0);
        let mut app = empty_model();
        app.page = Page::FindMore;
        app.catalog_index = Arc::new(CatalogIndex::new(catalog));
        let model = cx.new(|_| app);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1024.0), px(640.0)),
                })),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| ScribeWindow::new(model, window, cx)),
        )
        .unwrap()
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();

    let row = visual.debug_bounds("catalog-row").expect("catalog row");
    let copy = visual
        .debug_bounds("catalog-row-copy")
        .expect("catalog row copy");
    let actions = visual
        .debug_bounds("catalog-row-actions")
        .expect("catalog row actions");
    let row_right = row.origin.x + row.size.width;
    let copy_right = copy.origin.x + copy.size.width;
    assert!(row.origin.x >= px(0.0));
    assert!(row_right <= px(1024.0));
    assert!(copy_right <= actions.origin.x);
    assert!(actions.origin.y >= row.origin.y);
    assert!(actions.origin.y + actions.size.height <= row.origin.y + row.size.height);
    assert!(
        actions.origin.x + actions.size.width <= row_right,
        "row={row:?}, copy={copy:?}, actions={actions:?}"
    );
    // The Install pill stays reachable inside the card at minimum window size.
    assert!(actions.size.width >= px(60.0), "actions={actions:?}");
    assert!(actions.size.height >= px(28.0), "actions={actions:?}");
    assert!(actions.origin.x >= row.origin.x, "actions={actions:?}");
}

#[gpui::test]
fn long_installed_content_stays_clear_of_actions_at_minimum_window_size(cx: &mut TestAppContext) {
    let long = "Very long localized installed addon metadata ".repeat(10);
    let remote = RemoteAddon {
        uid: "installed-layout-fixture".into(),
        category_id: "long-installed-category".into(),
        ui_name: long.clone().into(),
        ui_author_name: long.clone().into(),
        ui_version: long.clone().into(),
        ..RemoteAddon::default()
    };
    let catalog = Arc::new(Catalog {
        addons: vec![remote.clone()],
        categories: vec![Category {
            id: "long-installed-category".into(),
            name: long.clone().into(),
            icon_url: "https://cdn-eso.mmoui.com/images/icons/m19.jpg".into(),
            ..Category::default()
        }],
    });
    let addon = Addon {
        title: long.clone(),
        author: long.clone(),
        version: long.clone(),
        folder_name: "LongInstalledLayoutFixture".into(),
        ..Addon::default()
    };
    let decision = MatchedAddon {
        remote: Some(remote),
        update_available: true,
        update_state: "Update available".into(),
        update_reason: long,
        ..MatchedAddon::default()
    };
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        Theme::global_mut(cx).font_size = px(18.0);
        let mut app = empty_model();
        app.catalog_index = Arc::new(CatalogIndex::new(catalog));
        app.installed = Arc::new(vec![addon.clone()]);
        app.matched = Arc::new(vec![decision.clone()]);
        app.installed_index = Arc::new(InstalledIndex::new(&[addon], &[decision]));
        let model = cx.new(|_| app);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1024.0), px(640.0)),
                })),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| ScribeWindow::new(model, window, cx)),
        )
        .unwrap()
    });
    window
        .update(cx, |view, _, cx| {
            view.expanded_categories
                .insert("long-installed-category".into());
            view.installed_groups_initialized = true;
            cx.notify();
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();

    let row = visual.debug_bounds("installed-row").expect("installed row");
    let copy = visual
        .debug_bounds("installed-row-copy")
        .expect("installed row copy");
    let actions = visual
        .debug_bounds("installed-row-actions")
        .expect("installed row actions");
    let row_right = row.origin.x + row.size.width;
    let copy_right = copy.origin.x + copy.size.width;
    assert!(row.origin.x >= px(0.0));
    assert!(row_right <= px(1024.0));
    assert!(copy_right <= actions.origin.x);
    assert!(actions.origin.y >= row.origin.y);
    assert!(actions.origin.y + actions.size.height <= row.origin.y + row.size.height);
    assert!(
        actions.origin.x + actions.size.width <= row_right,
        "row={row:?}, copy={copy:?}, actions={actions:?}"
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

#[test]
fn update_alert_fires_once_per_rise_and_re_arms_at_zero() {
    // First rise above zero fires and records the alerted count.
    assert_eq!(alert_decision(None, 3), (Some(3), Some(3)));
    // Same or lower count never nags again.
    assert_eq!(alert_decision(Some(3), 3), (Some(3), None));
    assert_eq!(alert_decision(Some(3), 1), (Some(3), None));
    // A rise above the alerted count fires exactly once more.
    assert_eq!(alert_decision(Some(3), 5), (Some(5), Some(5)));
    assert_eq!(alert_decision(Some(5), 5), (Some(5), None));
    // Dropping to zero re-arms; the next rise alerts again.
    assert_eq!(alert_decision(Some(5), 0), (None, None));
    assert_eq!(alert_decision(None, 2), (Some(2), Some(2)));
}

#[test]
fn update_alert_evaluation_respects_the_settings_switch() {
    let mut model = empty_model();
    model.matched = Arc::new(vec![
        MatchedAddon {
            update_available: true,
            ..MatchedAddon::default()
        },
        MatchedAddon::default(),
    ]);
    model.settings.background_alerts = true;
    evaluate_update_alerts(&mut model);
    assert_eq!(model.pending_update_alert, Some(1));
    model.pending_update_alert = None;
    // Same count: nothing new fires.
    evaluate_update_alerts(&mut model);
    assert_eq!(model.pending_update_alert, None);
    // Disabled switch suppresses alerts entirely (state still tracks).
    model.settings.background_alerts = false;
    model.matched = Arc::new(vec![
        MatchedAddon {
            update_available: true,
            ..MatchedAddon::default()
        },
        MatchedAddon {
            update_available: true,
            ..MatchedAddon::default()
        },
    ]);
    evaluate_update_alerts(&mut model);
    assert_eq!(model.pending_update_alert, None);
    assert_eq!(model.alerted_update_count, Some(2));
}

#[test]
fn updates_badge_is_loud_only_with_waiting_updates() {
    assert!(updates_badge_loud(Page::Updates, 1));
    assert!(updates_badge_loud(Page::Updates, 12));
    assert!(!updates_badge_loud(Page::Updates, 0));
    assert!(!updates_badge_loud(Page::Installed, 4));
    assert!(!updates_badge_loud(Page::FindMore, 4));
}

#[test]
fn tray_menu_commands_map_to_events() {
    assert_eq!(
        crate::tray::menu_command_event(1),
        Some(crate::tray::TrayEvent::Open)
    );
    assert_eq!(
        crate::tray::menu_command_event(2),
        Some(crate::tray::TrayEvent::CheckUpdates)
    );
    assert_eq!(
        crate::tray::menu_command_event(3),
        Some(crate::tray::TrayEvent::Quit)
    );
    assert_eq!(crate::tray::menu_command_event(0), None);
    assert_eq!(crate::tray::menu_command_event(99), None);
}

#[test]
fn catalog_selection_toggles_and_respects_unselectable_rows() {
    let mut selection = BTreeSet::new();
    assert!(toggle_catalog_selection(&mut selection, "uid-a", true));
    assert!(selection.contains("uid-a"));
    assert!(toggle_catalog_selection(&mut selection, "uid-a", true));
    assert!(!selection.contains("uid-a"));
    // Unselectable (installed/queued) rows never toggle.
    assert!(!toggle_catalog_selection(&mut selection, "uid-b", false));
    assert!(!selection.contains("uid-b"));
}

#[test]
fn catalog_blocked_uids_covers_installed_matches() {
    let remote = RemoteAddon {
        uid: "installed-uid".into(),
        ..RemoteAddon::default()
    };
    let mut model = empty_model();
    model.matched = Arc::new(vec![MatchedAddon {
        remote: Some(remote),
        ..MatchedAddon::default()
    }]);
    let blocked = catalog_blocked_uids(&model);
    assert!(blocked.contains("installed-uid"));
    assert!(!blocked.contains("other-uid"));
}

#[test]
fn installable_selection_skips_blocked_and_unknown_uids() {
    let catalog = Arc::new(Catalog {
        addons: vec![
            RemoteAddon {
                uid: "ok".into(),
                ..RemoteAddon::default()
            },
            RemoteAddon {
                uid: "blocked".into(),
                ..RemoteAddon::default()
            },
        ],
        ..Catalog::default()
    });
    let index = CatalogIndex::new(catalog);
    let uids = BTreeSet::from([
        "ok".to_string(),
        "blocked".to_string(),
        "missing".to_string(),
    ]);
    let blocked = HashSet::from(["blocked".to_string()]);
    let remotes = installable_selection(&index, &uids, &blocked);
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0].uid.as_str(), "ok");
}

fn toast_notice(tone: NoticeTone, title: &'static str) -> StatusNotice {
    StatusNotice {
        tone,
        title,
        message: "message".into(),
    }
}

#[test]
fn toast_expiry_maps_tones_to_durations() {
    let now = std::time::Instant::now();
    let info = toast_expires_at(&toast_notice(NoticeTone::Info, "Hint"), false, now);
    assert_eq!(info, Some(now + std::time::Duration::from_secs(4)));
    let success = toast_expires_at(&toast_notice(NoticeTone::Success, "Done"), false, now);
    assert_eq!(success, Some(now + std::time::Duration::from_secs(4)));
    assert_eq!(
        toast_expires_at(&toast_notice(NoticeTone::Warning, "Careful"), false, now),
        None
    );
    assert_eq!(
        toast_expires_at(&toast_notice(NoticeTone::Danger, "Failed"), false, now),
        None
    );
    // Loading toasts persist until the status changes.
    assert_eq!(
        toast_expires_at(
            &toast_notice(NoticeTone::Info, "Loading library"),
            true,
            now
        ),
        None
    );
}

#[test]
fn toast_push_trims_to_three_and_replaces_loading() {
    let now = std::time::Instant::now();
    let mut toasts = Vec::new();
    push_toast(
        &mut toasts,
        1,
        toast_notice(NoticeTone::Info, "Loading library"),
        "a".into(),
        now,
    );
    assert!(toasts[0].loading);
    // A new toast retires the loading toast.
    push_toast(
        &mut toasts,
        2,
        toast_notice(NoticeTone::Danger, "Failed"),
        "b".into(),
        now,
    );
    assert_eq!(toasts.len(), 1);
    assert!(!toasts[0].loading);
    // The stack caps at three visible cards.
    for seq in 3..=5 {
        push_toast(
            &mut toasts,
            seq,
            toast_notice(NoticeTone::Info, "Hint"),
            seq.to_string(),
            now,
        );
    }
    assert_eq!(toasts.len(), 3);
    assert_eq!(toasts[0].seq, 3);
    // Expired toasts prune; sticky toasts stay.
    let later = now + std::time::Duration::from_secs(5);
    prune_toasts(&mut toasts, later);
    assert!(toasts.is_empty());
    push_toast(
        &mut toasts,
        6,
        toast_notice(NoticeTone::Danger, "Failed"),
        "c".into(),
        now,
    );
    prune_toasts(&mut toasts, later);
    assert_eq!(toasts.len(), 1);
}

#[test]
fn dismissed_status_does_not_reappear() {
    assert!(toast_should_show("refresh failed", true, ""));
    assert!(!toast_should_show("refresh failed", true, "refresh failed"));
    assert!(!toast_should_show("refresh failed", false, ""));
    assert!(toast_should_show(
        "different status",
        true,
        "refresh failed"
    ));
}

#[test]
fn toast_roles_match_notice_tones() {
    assert_eq!(toast_role(NoticeTone::Danger), Role::Alert);
    assert_eq!(toast_role(NoticeTone::Info), Role::Status);
    assert_eq!(toast_role(NoticeTone::Success), Role::Status);
    assert_eq!(toast_role(NoticeTone::Warning), Role::Status);
}

#[gpui::test]
fn loading_state_shows_skeleton_rows_instead_of_empty_state(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        let mut app = empty_model();
        app.page = Page::FindMore;
        app.loading = true;
        let model = cx.new(|_| app);
        cx.open_window(Default::default(), move |window, cx| {
            cx.new(|cx| ScribeWindow::new(model, window, cx))
        })
        .unwrap()
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();

    assert!(visual.debug_bounds("skeleton-row").is_some());
}

#[gpui::test]
fn skeletons_render_statically_under_reduced_motion(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        cx.set_reduce_motion(true);
        let mut app = empty_model();
        app.page = Page::FindMore;
        app.loading = true;
        let model = cx.new(|_| app);
        cx.open_window(Default::default(), move |window, cx| {
            cx.new(|cx| ScribeWindow::new(model, window, cx))
        })
        .unwrap()
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();

    // Static skeleton blocks still render under reduced motion.
    assert!(visual.debug_bounds("skeleton-row").is_some());
}

#[test]
fn list_cursor_moves_clamps_and_handles_empty_lists() {
    assert_eq!(move_cursor(None, 1, 5), Some(0));
    assert_eq!(move_cursor(None, -1, 5), Some(4));
    assert_eq!(move_cursor(Some(0), -1, 5), Some(0));
    assert_eq!(move_cursor(Some(4), 1, 5), Some(4));
    assert_eq!(move_cursor(Some(2), 1, 5), Some(3));
    assert_eq!(move_cursor(Some(2), 1, 0), None);
    assert_eq!(clamp_cursor(Some(9), 3), Some(2));
    assert_eq!(clamp_cursor(Some(1), 0), None);
    assert_eq!(clamp_cursor(None, 3), None);
}

#[test]
fn catalog_cursor_resets_on_filter_change_and_clamps_on_shrink() {
    assert_eq!(cursor_after_filter_change(true, Some(3), 10), None);
    assert_eq!(cursor_after_filter_change(false, Some(3), 10), Some(3));
    assert_eq!(cursor_after_filter_change(false, Some(9), 4), Some(3));
    assert_eq!(cursor_after_filter_change(false, Some(2), 0), None);
}

#[test]
fn visible_library_rows_skips_collapsed_groups() {
    let addon = |name: &str| Addon {
        title: name.into(),
        folder_name: name.into(),
        ..Addon::default()
    };
    let groups = vec![
        InstalledGroup {
            id: "a".into(),
            name: "A".into(),
            icon_url: None,
            items: vec![
                (addon("a1"), MatchedAddon::default()),
                (addon("a2"), MatchedAddon::default()),
            ],
        },
        InstalledGroup {
            id: "b".into(),
            name: "B".into(),
            icon_url: None,
            items: vec![(addon("b1"), MatchedAddon::default())],
        },
    ];
    // All expanded: a1, a2, b1 in order.
    let expanded = HashSet::from(["a".to_string(), "b".to_string()]);
    let rows = visible_library_rows(&groups, &expanded);
    assert_eq!(rows, vec![(0, 0), (0, 1), (1, 0)]);
    // Group "a" collapsed: only b1 remains visible.
    let expanded = HashSet::from(["b".to_string()]);
    let rows = visible_library_rows(&groups, &expanded);
    assert_eq!(rows, vec![(1, 0)]);
    // Everything collapsed: no rows.
    assert!(visible_library_rows(&groups, &HashSet::new()).is_empty());
}

#[test]
fn fuzzy_score_matches_subsequence_with_style_bonuses() {
    assert!(fuzzy_score("dolg", "Dolgubon").is_some());
    assert!(fuzzy_score("lwc", "Lazy Writ Crafter").is_some());
    assert!(fuzzy_score("xyz", "SkyShards").is_none());
    assert_eq!(fuzzy_score("", "anything"), Some(0));
    assert_eq!(fuzzy_score("   ", "anything"), Some(0));
    assert!(fuzzy_score("SKY", "skyshards").is_some());
    // Consecutive, word-start, and camel matches outrank scattered ones.
    let consecutive = fuzzy_score("abc", "abcd").unwrap();
    let scattered = fuzzy_score("abc", "axbxcx").unwrap();
    assert!(consecutive > scattered);
    let word_start = fuzzy_score("shards", "shards").unwrap();
    let buried = fuzzy_score("shards", "xshards").unwrap();
    assert!(word_start > buried);
    let camel = fuzzy_score("sw", "StarWars").unwrap();
    let plain = fuzzy_score("sw", "stwxx").unwrap();
    assert!(camel > plain);
}

fn ranked_catalog() -> CatalogIndex {
    CatalogIndex::new(Arc::new(Catalog {
        addons: vec![
            RemoteAddon {
                uid: "1".into(),
                ui_name: "SkyShards".into(),
                ui_author_name: "Dolgubon".into(),
                ..RemoteAddon::default()
            },
            RemoteAddon {
                uid: "2".into(),
                ui_name: "LoreBooks".into(),
                ui_author_name: "Someone".into(),
                ..RemoteAddon::default()
            },
            RemoteAddon {
                uid: "3".into(),
                ui_name: "Lazy Writ Crafter".into(),
                ui_author_name: "Dolgubon".into(),
                ..RemoteAddon::default()
            },
        ],
        ..Catalog::default()
    }))
}

#[test]
fn fuzzy_rank_orders_by_score_then_popularity() {
    let index = ranked_catalog();
    // "lwc" matches only Lazy Writ Crafter via initials.
    assert_eq!(fuzzy_filter_rank(&index, &[0, 1, 2], "lwc"), vec![2]);
    // "dolg" matches both Dolgubon authors with equal scores; popularity
    // order (base position) decides between them.
    assert_eq!(fuzzy_filter_rank(&index, &[0, 1, 2], "dolg"), vec![0, 2]);
    // Stronger title match beats the weaker author match.
    let ranked = fuzzy_filter_rank(&index, &[0, 1, 2], "writ");
    assert_eq!(ranked, vec![2]);
    // Nothing matches: empty result, no crash.
    assert!(fuzzy_filter_rank(&index, &[0, 1, 2], "zzzz").is_empty());
}

#[test]
fn recent_searches_dedupe_order_and_cap() {
    let mut recent = Vec::new();
    push_recent_search(&mut recent, "skyshards");
    push_recent_search(&mut recent, "harvestmap");
    push_recent_search(&mut recent, "SKYSHARDS");
    assert_eq!(
        recent,
        vec!["SKYSHARDS".to_string(), "harvestmap".to_string()]
    );
    push_recent_search(&mut recent, "   ");
    assert_eq!(recent.len(), 2);
    for index in 0..10 {
        push_recent_search(&mut recent, &format!("query-{index}"));
    }
    assert_eq!(recent.len(), 8);
    assert_eq!(recent[0], "query-9");
}

#[test]
fn recent_dropdown_visibility_rules() {
    assert!(should_show_recent_dropdown(true, true, true, true, false));
    // Not Find More, not focused, typing, empty recents, or overlay open.
    assert!(!should_show_recent_dropdown(false, true, true, true, false));
    assert!(!should_show_recent_dropdown(true, false, true, true, false));
    assert!(!should_show_recent_dropdown(true, true, false, true, false));
    assert!(!should_show_recent_dropdown(true, true, true, false, false));
    assert!(!should_show_recent_dropdown(true, true, true, true, true));
}

#[test]
fn details_breakpoint_selects_inline_at_1400px() {
    assert!(!details_inline_width(px(1399.0)));
    assert!(details_inline_width(px(1400.0)));
    assert!(details_inline_width(px(2560.0)));
}

#[test]
fn overlay_kind_maps_details_identically_for_modal_and_inline() {
    assert_eq!(
        derive_overlay_kind(false, false, false, false, true),
        Some(OverlayKind::RemoteDetails)
    );
    assert_eq!(
        derive_overlay_kind(false, false, false, true, true),
        Some(OverlayKind::LocalDetails)
    );
    assert_eq!(
        derive_overlay_kind(false, false, true, true, true),
        Some(OverlayKind::Uninstall)
    );
    assert_eq!(
        derive_overlay_kind(true, true, true, true, true),
        Some(OverlayKind::Lightbox)
    );
    assert_eq!(derive_overlay_kind(false, false, false, false, false), None);
}

#[test]
fn keyboard_guards_suspend_browsing_while_details_open() {
    assert!(keyboard_nav_allowed(None, false, false, false));
    assert!(!keyboard_nav_allowed(
        Some(OverlayKind::RemoteDetails),
        false,
        false,
        false
    ));
    assert!(!keyboard_nav_allowed(
        Some(OverlayKind::LocalDetails),
        false,
        false,
        false
    ));
    assert!(!keyboard_nav_allowed(None, true, false, false));
    assert!(!keyboard_nav_allowed(None, false, true, false));
    assert!(!keyboard_nav_allowed(None, false, false, true));
}

#[test]
fn window_chrome_hit_areas_suspend_under_overlays() {
    // No overlay: caption drag + window controls register their hit areas.
    assert!(window_chrome_active(None, false, false));
    // Inline details leave the chrome interactive (nothing covers it).
    assert!(window_chrome_active(
        Some(OverlayKind::RemoteDetails),
        true,
        false
    ));
    // Modal overlays suspend the chrome so overlay close buttons above the
    // caption strip keep receiving client clicks.
    for kind in [
        OverlayKind::RemoteDetails,
        OverlayKind::LocalDetails,
        OverlayKind::Uninstall,
        OverlayKind::Rebuild,
        OverlayKind::Lightbox,
    ] {
        assert!(!window_chrome_active(Some(kind), false, false));
    }
    // A context menu can open over the strip, so it suspends the chrome too.
    assert!(!window_chrome_active(None, false, true));
}

#[test]
fn back_action_clears_details_state() {
    let mut model = empty_model();
    model.selected_details = Some(RemoteAddonDetails::default());
    model.selected_local = Some((Addon::default(), MatchedAddon::default()));
    model.details_loading = true;
    clear_details_overlay(&mut model);
    assert!(model.selected_details.is_none());
    assert!(model.selected_local.is_none());
    assert!(!model.details_loading);
}

fn library_fixture() -> Vec<InstalledGroup> {
    let addon = |name: &str| Addon {
        title: name.into(),
        folder_name: name.into(),
        ..Addon::default()
    };
    vec![
        InstalledGroup {
            id: "a".into(),
            name: "A".into(),
            icon_url: None,
            items: vec![
                (addon("a1"), MatchedAddon::default()),
                (addon("a2"), MatchedAddon::default()),
            ],
        },
        InstalledGroup {
            id: "b".into(),
            name: "B".into(),
            icon_url: None,
            items: vec![(addon("b1"), MatchedAddon::default())],
        },
    ]
}

#[test]
fn flatten_library_orders_headers_and_rows() {
    let groups = library_fixture();
    let expanded = HashSet::from(["a".to_string(), "b".to_string()]);
    let flat = flatten_library(&groups, &expanded, false);
    assert_eq!(
        flat,
        vec![
            LibraryRow::GroupHeader {
                group_ix: 0,
                last_in_group: false
            },
            LibraryRow::AddonRow {
                group_ix: 0,
                row_ix: 0,
                last_in_group: false
            },
            LibraryRow::AddonRow {
                group_ix: 0,
                row_ix: 1,
                last_in_group: true
            },
            LibraryRow::GroupHeader {
                group_ix: 1,
                last_in_group: false
            },
            LibraryRow::AddonRow {
                group_ix: 1,
                row_ix: 0,
                last_in_group: true
            },
        ]
    );
    // Collapsed group: header only, marked as group end.
    let flat = flatten_library(&groups, &HashSet::from(["b".to_string()]), false);
    assert_eq!(
        flat,
        vec![
            LibraryRow::GroupHeader {
                group_ix: 0,
                last_in_group: true
            },
            LibraryRow::GroupHeader {
                group_ix: 1,
                last_in_group: false
            },
            LibraryRow::AddonRow {
                group_ix: 1,
                row_ix: 0,
                last_in_group: true
            },
        ]
    );
    // Selection mode does not change the model.
    assert_eq!(
        flat,
        flatten_library(&groups, &HashSet::from(["b".to_string()]), true)
    );
}

#[test]
fn flattened_addon_rows_match_keyboard_cursor_mapping() {
    let groups = library_fixture();
    let expanded = HashSet::from(["a".to_string(), "b".to_string()]);
    let flat = flatten_library(&groups, &expanded, false);
    let keyboard_rows = visible_library_rows(&groups, &expanded);
    let flat_rows: Vec<(usize, usize)> = flat
        .iter()
        .filter_map(|row| match row {
            LibraryRow::AddonRow {
                group_ix, row_ix, ..
            } => Some((*group_ix, *row_ix)),
            _ => None,
        })
        .collect();
    assert_eq!(keyboard_rows, flat_rows);
    // Cursor -> flattened index accounts for interleaved headers.
    assert_eq!(flat_index_of_row(&flat, 0), Some(1));
    assert_eq!(flat_index_of_row(&flat, 1), Some(2));
    assert_eq!(flat_index_of_row(&flat, 2), Some(4));
    assert_eq!(flat_index_of_row(&flat, 3), None);
}

#[test]
fn list_state_splice_keeps_offset_clamped_on_collapse() {
    let state = gpui::ListState::new(0, gpui::ListAlignment::Top, px(10.0));
    state.splice(0..0, 5);
    assert_eq!(state.item_count(), 5);
    state.splice(0..5, 2);
    assert_eq!(state.item_count(), 2);
    state.splice(0..2, 0);
    assert_eq!(state.item_count(), 0);
}

#[gpui::test]
fn details_open_inline_as_page_on_wide_windows(cx: &mut TestAppContext) {
    let remote = RemoteAddon {
        uid: "inline-fixture".into(),
        category_id: "inline-category".into(),
        ui_name: "Inline Fixture".into(),
        ..RemoteAddon::default()
    };
    let catalog = Arc::new(Catalog {
        addons: vec![remote.clone()],
        categories: vec![Category {
            id: "inline-category".into(),
            name: "Inline Category".into(),
            ..Category::default()
        }],
    });
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        let mut app = empty_model();
        app.catalog_index = Arc::new(CatalogIndex::new(catalog));
        app.selected_details = Some(RemoteAddonDetails {
            addon: remote,
            ..RemoteAddonDetails::default()
        });
        let model = cx.new(|_| app);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1500.0), px(900.0)),
                })),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| ScribeWindow::new(model, window, cx)),
        )
        .unwrap()
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();

    // Wide viewport renders the inline page instead of the modal sheet.
    assert!(visual.debug_bounds("details-page").is_some());
    assert!(visual.debug_bounds("modal-surface").is_none());
}
