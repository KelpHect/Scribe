use gpui::{App, px};
use gpui_component::{Theme, ThemeMode};

// ---------------------------------------------------------------------------
// Scribe Glass palette (docs/ui-rework-design.md)
//
// Opaque colors are plain 0xRRGGBB values used with `gpui::rgb`. Translucent
// glass surfaces are 0xRRGGBBAA values used with `gpui::rgba`. Constants that
// keep the historical SCRIBE_* parchment names are opaque composites of the
// glass tokens over the #0B0E13 window tint so existing `rgb` call sites
// (rows, overlays) render correctly on the dark shell.
// ---------------------------------------------------------------------------

// Core opaque colors.
pub(crate) const SCRIBE_FOREGROUND: u32 = 0xf2f4f8;
pub(crate) const SCRIBE_PRIMARY: u32 = 0xd9a648;
pub(crate) const SCRIBE_PRIMARY_FOREGROUND: u32 = 0x1a1407;
pub(crate) const SCRIBE_PRIMARY_HOVER: u32 = 0xe2b45a;
pub(crate) const SCRIBE_PRIMARY_PRESSED: u32 = 0xc4953c;
pub(crate) const SCRIBE_CLOSE_HOVER: u32 = 0xe81123;

// Semantic colors.
pub(crate) const SCRIBE_WARNING: u32 = 0xe0a020;
pub(crate) const SCRIBE_WARNING_FOREGROUND: u32 = 0x1a1407;
pub(crate) const SCRIBE_INFO: u32 = 0x0a84ff;
pub(crate) const SCRIBE_INFO_FOREGROUND: u32 = 0x1a1407;
pub(crate) const SCRIBE_SUCCESS: u32 = 0x30d158;
pub(crate) const SCRIBE_SUCCESS_FOREGROUND: u32 = 0x1a1407;
pub(crate) const SCRIBE_DANGER: u32 = 0xff453a;
pub(crate) const SCRIBE_DANGER_FOREGROUND: u32 = 0x1a1407;
pub(crate) const SCRIBE_HEALTH_SUCCESS: u32 = 0x30d158;
pub(crate) const SCRIBE_HEALTH_WARNING: u32 = 0xe0a020;
pub(crate) const SCRIBE_HEALTH_DANGER: u32 = 0xff453a;

// Translucent glass tokens (0xRRGGBBAA, use with `gpui::rgba`).
pub(crate) const SCRIBE_WINDOW_TINT_RGBA: u32 = 0x0b0e13c7;
pub(crate) const SCRIBE_SIDEBAR_TINT_RGBA: u32 = 0x0b0e138c;
pub(crate) const SCRIBE_SURFACE_RGBA: u32 = 0xffffff0f;
pub(crate) const SCRIBE_BUTTON_FILL_RGBA: u32 = 0xffffff14;
pub(crate) const SCRIBE_SURFACE_HOVER_RGBA: u32 = 0xffffff1a;
pub(crate) const SCRIBE_SURFACE_ACTIVE_RGBA: u32 = 0xffffff24;
pub(crate) const SCRIBE_SURFACE_RAISED_RGBA: u32 = 0x171b23f0;
pub(crate) const SCRIBE_HAIRLINE_RGBA: u32 = 0xffffff17;
pub(crate) const SCRIBE_TEXT_SECONDARY_RGBA: u32 = 0xf2f4f89e;
pub(crate) const SCRIBE_TEXT_TERTIARY_RGBA: u32 = 0xf2f4f866;
pub(crate) const SCRIBE_ACCENT_SOFT_RGBA: u32 = 0xd9a64829;
pub(crate) const SCRIBE_FOCUS_RING_RGBA: u32 = 0xd9a6488c;
pub(crate) const SCRIBE_OVERLAY_RGBA: u32 = 0x00000073;
pub(crate) const SCRIBE_SCROLLBAR_THUMB_RGBA: u32 = 0xffffff2e;
pub(crate) const SCRIBE_SCROLLBAR_THUMB_HOVER_RGBA: u32 = 0xffffff47;
pub(crate) const SCRIBE_DANGER_TINT_RGBA: u32 = 0xff453a1f;
pub(crate) const SCRIBE_DANGER_TINT_HOVER_RGBA: u32 = 0xff453a33;
pub(crate) const SCRIBE_DANGER_TINT_ACTIVE_RGBA: u32 = 0xff453a47;
pub(crate) const SCRIBE_DANGER_BORDER_RGBA: u32 = 0xff453a59;

// Opaque composites over the window tint (historical names, `gpui::rgb`).
pub(crate) const SCRIBE_SIDEBAR_FOREGROUND: u32 = 0xf2f4f8;
pub(crate) const SCRIBE_SIDEBAR_ACTIVE_FOREGROUND: u32 = 0xd9a648;
pub(crate) const SCRIBE_SIDEBAR_ACCENT: u32 = 0xd9a648;
pub(crate) const SCRIBE_FOCUS_RING: u32 = 0x7c6230;
pub(crate) const SCRIBE_TOOLBAR_INPUT: u32 = 0x1f2126;
pub(crate) const SCRIBE_SWITCH_ACTIVE: u32 = 0xd9a648;
pub(crate) const SCRIBE_PARCHMENT_ELEVATED: u32 = 0x1f2126;
pub(crate) const SCRIBE_PARCHMENT_HOVER: u32 = 0x23262b;
pub(crate) const SCRIBE_PARCHMENT_BORDER: u32 = 0x212428;
pub(crate) const SCRIBE_PARCHMENT_PANEL: u32 = 0x1a1d21;
pub(crate) const SCRIBE_TEXT_SUBTLE: u32 = 0x9a9da1;
pub(crate) const SCRIBE_TEXT_ACTION: u32 = 0xd9a648;
pub(crate) const SCRIBE_INPUT_BORDER: u32 = 0x212428;
pub(crate) const SCRIBE_ACTIVE_BORDER: u32 = 0xd9a648;
pub(crate) const SCRIBE_LINK: u32 = 0x0a84ff;
pub(crate) const SCRIBE_LINK_HOVER: u32 = 0x409cff;
pub(crate) const SCRIBE_LINK_PRESSED: u32 = 0x1c8dff;
pub(crate) const SCRIBE_OVERLAY: u32 = 0x000000;
pub(crate) const SCRIBE_OVERLAY_FOREGROUND: u32 = 0xffffff;

// Layout and motion.
pub(crate) const SCRIBE_NAV_HEIGHT: f32 = 34.0;
pub(crate) const SCRIBE_INPUT_RADIUS: f32 = 10.0;
pub(crate) const SCRIBE_CARD_RADIUS: f32 = 14.0;
pub(crate) const SCRIBE_SHEET_RADIUS: f32 = 18.0;
pub(crate) const SCRIBE_CONTENT_GUTTER: f32 = 28.0;
pub(crate) const SCRIBE_CONTENT_MAX_WIDTH: f32 = 1200.0;
pub(crate) const SCRIBE_SETTINGS_MAX_WIDTH: f32 = 760.0;
pub(crate) const SCRIBE_SIDEBAR_WIDTH: f32 = 228.0;
pub(crate) const SCRIBE_TITLE_ROW_HEIGHT: f32 = 52.0;
pub(crate) const SCRIBE_MOTION_FAST_MS: u64 = 120;
pub(crate) const SCRIBE_MOTION_PAGE_MS: u64 = 180;

pub(crate) fn apply_scribe_theme(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    let theme = Theme::global_mut(cx);
    theme.font_family = ".Segoe UI Variable Text".into();
    let colors = &mut theme.colors;
    let color = |hex| gpui::rgb(hex).into();
    let alpha = |hex| gpui::rgba(hex).into();
    let transparent: gpui::Hsla = gpui::rgba(0x00000000).into();

    // The component Root stays transparent; ScribeWindow's window root paints
    // SCRIBE_WINDOW_TINT_RGBA as a single layer over the acrylic backdrop so
    // overlay chrome (e.g. the window-controls strip) can match it seamlessly.
    colors.background = transparent;
    colors.foreground = color(SCRIBE_FOREGROUND);
    colors.border = alpha(SCRIBE_HAIRLINE_RGBA);
    colors.input = alpha(SCRIBE_HAIRLINE_RGBA);
    colors.accent = alpha(SCRIBE_SURFACE_HOVER_RGBA);
    colors.accent_foreground = color(SCRIBE_FOREGROUND);
    colors.muted = alpha(SCRIBE_SURFACE_RGBA);
    colors.muted_foreground = alpha(SCRIBE_TEXT_SECONDARY_RGBA);
    colors.popover = alpha(SCRIBE_SURFACE_RAISED_RGBA);
    colors.popover_foreground = color(SCRIBE_FOREGROUND);
    colors.list = transparent;
    colors.list_even = alpha(SCRIBE_SURFACE_RGBA);
    colors.list_head = alpha(SCRIBE_SURFACE_RGBA);
    colors.list_hover = alpha(SCRIBE_SURFACE_HOVER_RGBA);
    colors.list_active = alpha(SCRIBE_SURFACE_ACTIVE_RGBA);
    colors.list_active_border = color(SCRIBE_PRIMARY);
    colors.primary = color(SCRIBE_PRIMARY);
    colors.primary_hover = color(SCRIBE_PRIMARY_HOVER);
    colors.primary_active = color(SCRIBE_PRIMARY_PRESSED);
    colors.primary_foreground = color(SCRIBE_PRIMARY_FOREGROUND);
    colors.secondary = alpha(SCRIBE_SURFACE_RGBA);
    colors.secondary_hover = alpha(SCRIBE_SURFACE_HOVER_RGBA);
    colors.secondary_active = alpha(SCRIBE_SURFACE_ACTIVE_RGBA);
    colors.secondary_foreground = color(SCRIBE_FOREGROUND);
    colors.button = alpha(SCRIBE_SURFACE_RGBA);
    colors.button_hover = alpha(SCRIBE_SURFACE_HOVER_RGBA);
    colors.button_active = alpha(SCRIBE_SURFACE_ACTIVE_RGBA);
    colors.button_foreground = color(SCRIBE_FOREGROUND);
    colors.button_primary = colors.primary;
    colors.button_primary_hover = colors.primary_hover;
    colors.button_primary_active = colors.primary_active;
    colors.button_primary_foreground = colors.primary_foreground;
    colors.button_secondary = colors.secondary;
    colors.button_secondary_hover = colors.secondary_hover;
    colors.button_secondary_active = colors.secondary_active;
    colors.button_secondary_foreground = colors.secondary_foreground;
    colors.warning = color(SCRIBE_WARNING);
    colors.warning_hover = color(0xeab04a);
    colors.warning_active = color(0xc78d1c);
    colors.warning_foreground = color(SCRIBE_WARNING_FOREGROUND);
    colors.info = color(SCRIBE_INFO);
    colors.info_hover = color(0x2f94ff);
    colors.info_active = color(0x0870d9);
    colors.info_foreground = color(SCRIBE_INFO_FOREGROUND);
    colors.success = color(SCRIBE_SUCCESS);
    colors.success_hover = color(0x45d96b);
    colors.success_active = color(0x2abb52);
    colors.success_foreground = color(SCRIBE_SUCCESS_FOREGROUND);
    colors.danger = color(SCRIBE_DANGER);
    colors.danger_hover = color(0xff5a50);
    colors.danger_active = color(0xe03e34);
    colors.danger_foreground = color(SCRIBE_DANGER_FOREGROUND);
    colors.button_warning = colors.warning;
    colors.button_warning_hover = colors.warning_hover;
    colors.button_warning_active = colors.warning_active;
    colors.button_warning_foreground = colors.warning_foreground;
    colors.button_info = colors.info;
    colors.button_info_hover = colors.info_hover;
    colors.button_info_active = colors.info_active;
    colors.button_info_foreground = colors.info_foreground;
    colors.button_success = colors.success;
    colors.button_success_hover = colors.success_hover;
    colors.button_success_active = colors.success_active;
    colors.button_success_foreground = colors.success_foreground;
    colors.button_danger = colors.danger;
    colors.button_danger_hover = colors.danger_hover;
    colors.button_danger_active = colors.danger_active;
    colors.button_danger_foreground = colors.danger_foreground;
    colors.sidebar = alpha(SCRIBE_SIDEBAR_TINT_RGBA);
    colors.sidebar_foreground = color(SCRIBE_SIDEBAR_FOREGROUND);
    colors.sidebar_border = alpha(SCRIBE_HAIRLINE_RGBA);
    colors.sidebar_accent = alpha(SCRIBE_ACCENT_SOFT_RGBA);
    colors.sidebar_accent_foreground = color(SCRIBE_SIDEBAR_ACTIVE_FOREGROUND);
    colors.sidebar_primary = color(SCRIBE_SIDEBAR_ACCENT);
    colors.sidebar_primary_foreground = color(SCRIBE_PRIMARY_FOREGROUND);
    colors.title_bar = transparent;
    colors.title_bar_border = transparent;
    colors.window_border = alpha(SCRIBE_HAIRLINE_RGBA);
    colors.ring = alpha(SCRIBE_FOCUS_RING_RGBA);
    colors.link = color(SCRIBE_LINK);
    colors.link_hover = color(SCRIBE_LINK_HOVER);
    colors.link_active = color(SCRIBE_LINK_PRESSED);
    colors.selection = alpha(SCRIBE_ACCENT_SOFT_RGBA);
    colors.scrollbar = transparent;
    colors.scrollbar_thumb = alpha(SCRIBE_SCROLLBAR_THUMB_RGBA);
    colors.scrollbar_thumb_hover = alpha(SCRIBE_SCROLLBAR_THUMB_HOVER_RGBA);
    colors.caret = color(SCRIBE_FOREGROUND);
    colors.overlay = alpha(SCRIBE_OVERLAY_RGBA);
    colors.progress_bar = color(SCRIBE_PRIMARY);
    colors.accordion = alpha(SCRIBE_SURFACE_RGBA);
    colors.accordion_hover = alpha(SCRIBE_SURFACE_HOVER_RGBA);
    colors.group_box = alpha(SCRIBE_SURFACE_RGBA);
    colors.group_box_foreground = color(SCRIBE_FOREGROUND);
    colors.description_list_label = alpha(SCRIBE_SURFACE_RGBA);
    colors.description_list_label_foreground = alpha(SCRIBE_TEXT_SECONDARY_RGBA);
    colors.drag_border = color(SCRIBE_PRIMARY);
    colors.drop_target = alpha(SCRIBE_ACCENT_SOFT_RGBA);
    colors.skeleton = alpha(SCRIBE_SURFACE_HOVER_RGBA);
    colors.slider_bar = color(SCRIBE_PRIMARY);
    colors.slider_thumb = color(SCRIBE_FOREGROUND);
    colors.switch = alpha(SCRIBE_SURFACE_ACTIVE_RGBA);
    colors.switch_thumb = color(SCRIBE_FOREGROUND);
    colors.tab = transparent;
    colors.tab_foreground = alpha(SCRIBE_TEXT_SECONDARY_RGBA);
    colors.tab_active = alpha(SCRIBE_SURFACE_ACTIVE_RGBA);
    colors.tab_active_foreground = color(SCRIBE_FOREGROUND);
    colors.tab_bar = transparent;
    colors.tab_bar_segmented = alpha(SCRIBE_SURFACE_RGBA);
    colors.table = transparent;
    colors.table_even = alpha(SCRIBE_SURFACE_RGBA);
    colors.table_head = alpha(SCRIBE_SURFACE_RGBA);
    colors.table_head_foreground = alpha(SCRIBE_TEXT_SECONDARY_RGBA);
    colors.table_hover = alpha(SCRIBE_SURFACE_HOVER_RGBA);
    colors.table_active = alpha(SCRIBE_SURFACE_ACTIVE_RGBA);
    colors.table_active_border = color(SCRIBE_PRIMARY);
    colors.table_row_border = alpha(SCRIBE_HAIRLINE_RGBA);
    colors.table_foot = alpha(SCRIBE_SURFACE_RGBA);
    colors.table_foot_foreground = alpha(SCRIBE_TEXT_SECONDARY_RGBA);
    colors.tiles = alpha(SCRIBE_SURFACE_RGBA);
    colors.status_bar = alpha(SCRIBE_SIDEBAR_TINT_RGBA);
    colors.status_bar_border = alpha(SCRIBE_HAIRLINE_RGBA);
    theme.tokens = (&theme.colors).into();
    theme.radius = px(SCRIBE_INPUT_RADIUS);
    theme.radius_lg = px(SCRIBE_CARD_RADIUS);
    theme.shadow = true;
}
