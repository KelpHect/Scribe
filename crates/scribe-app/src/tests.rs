use std::collections::HashSet;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    Bounds, MouseButton, Point, Role, TestAppContext, WindowBounds, WindowOptions, div, point, px,
    size,
};
use gpui_component::Theme;
use scribe_core::{
    Addon, AppSettings, Catalog, CatalogIndex, Category, InstalledIndex, MatchedAddon,
    MissingDependency, RemoteAddon, TaskProgress, TaskState,
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
fn status_notice_keeps_readable_width_at_minimum_window_size(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        apply_scribe_theme(cx);
        Theme::global_mut(cx).font_size = px(18.0);
        let mut app = empty_model();
        app.status =
            "ESOUI refresh failed: network unavailable. Cached data remains available.".into();
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

    let notice = visual.debug_bounds("status-notice").expect("status notice");
    assert!(notice.size.width >= px(700.0), "notice={notice:?}");
    assert!(notice.size.height <= px(64.0), "notice={notice:?}");
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
