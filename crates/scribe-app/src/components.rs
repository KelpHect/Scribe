use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    Anchor, Animation, AnimationExt as _, AnyElement, App, Bounds, Element, ElementId, Entity,
    FocusHandle, Focusable, GlobalElementId, IntoElement, LayoutId, ObjectFit, Pixels, Role,
    SharedString, StyledImage, Window, div, img, px, relative,
};
use gpui_component::{
    ActiveTheme as _, ElementExt as _, FocusTrapElement as _, Icon, IconName, Sizable as _, Size,
    StyledExt as _, Theme,
    button::Button,
    h_flex,
    input::{Input, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    scroll::ScrollableElement as _,
    select::{SearchableVec, SelectItem, SelectState},
};

use crate::model::{NoticeTone, StatusNotice};
use crate::theme::*;
use crate::unix_now;
use crate::window::ScribeWindow;

pub(crate) struct LiveRegion<E> {
    inner: E,
    live: gpui::accesskit::Live,
}

impl<E> LiveRegion<E> {
    pub(crate) fn new(inner: E, live: gpui::accesskit::Live) -> Self {
        Self { inner, live }
    }
}

impl<E: Element> Element for LiveRegion<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        Element::id(&self.inner)
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        Element::source_location(&self.inner)
    }

    fn a11y_role(&self) -> Option<Role> {
        self.inner.a11y_role()
    }

    fn write_a11y_info(&self, node: &mut gpui::accesskit::Node) {
        self.inner.write_a11y_info(node);
        node.set_live(self.live);
    }

    fn a11y_synthetic_children(
        &mut self,
        prepaint: &mut Self::PrepaintState,
        builder: &mut gpui::A11ySubtreeBuilder,
    ) {
        self.inner.a11y_synthetic_children(prepaint, builder);
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.inner
            .prepaint(id, inspector_id, bounds, request_layout, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.paint(
            id,
            inspector_id,
            bounds,
            request_layout,
            prepaint,
            window,
            cx,
        );
    }
}

impl<E: Element> IntoElement for LiveRegion<E> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

pub(crate) type ActivateHandler = dyn Fn(&mut Window, &mut App) + 'static;

#[derive(IntoElement)]
pub(crate) struct NativeButton {
    id: ElementId,
    label: SharedString,
    variant: NativeButtonVariant,
    icon: Option<IconName>,
    on_activate: Arc<ActivateHandler>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum NativeButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Danger,
    Install,
}

impl NativeButton {
    pub(crate) fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant: NativeButtonVariant::Primary,
            icon: None,
            on_activate: Arc::new(|_, _| {}),
        }
    }

    pub(crate) fn secondary(mut self) -> Self {
        self.variant = NativeButtonVariant::Secondary;
        self
    }

    pub(crate) fn ghost(mut self) -> Self {
        self.variant = NativeButtonVariant::Ghost;
        self
    }

    pub(crate) fn danger(mut self) -> Self {
        self.variant = NativeButtonVariant::Danger;
        self
    }

    pub(crate) fn install(mut self) -> Self {
        self.variant = NativeButtonVariant::Install;
        self
    }

    pub(crate) fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(crate) fn on_activate(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Arc::new(handler);
        self
    }
}

impl RenderOnce for NativeButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let pointer_activate = self.on_activate.clone();
        let keyboard_activate = self.on_activate;
        let label = self.label;
        let aria_label = label.clone();
        let (background, foreground, border, hover, pressed) = match self.variant {
            NativeButtonVariant::Primary => (
                gpui::rgb(SCRIBE_PRIMARY),
                gpui::rgb(SCRIBE_PRIMARY_FOREGROUND),
                gpui::rgb(SCRIBE_PRIMARY),
                gpui::rgb(SCRIBE_PRIMARY_HOVER),
                gpui::rgb(SCRIBE_PRIMARY_PRESSED),
            ),
            NativeButtonVariant::Secondary => (
                gpui::rgba(SCRIBE_BUTTON_FILL_RGBA),
                gpui::rgb(SCRIBE_FOREGROUND),
                gpui::rgba(SCRIBE_HAIRLINE_RGBA),
                gpui::rgba(SCRIBE_SURFACE_HOVER_RGBA),
                gpui::rgba(SCRIBE_SURFACE_ACTIVE_RGBA),
            ),
            NativeButtonVariant::Ghost => (
                gpui::rgba(0x00000000),
                gpui::rgb(SCRIBE_TEXT_ACTION),
                gpui::rgba(0x00000000),
                gpui::rgba(SCRIBE_BUTTON_FILL_RGBA),
                gpui::rgba(SCRIBE_SURFACE_ACTIVE_RGBA),
            ),
            NativeButtonVariant::Danger => (
                gpui::rgba(SCRIBE_DANGER_TINT_RGBA),
                gpui::rgb(SCRIBE_DANGER),
                gpui::rgba(SCRIBE_DANGER_BORDER_RGBA),
                gpui::rgba(SCRIBE_DANGER_TINT_HOVER_RGBA),
                gpui::rgba(SCRIBE_DANGER_TINT_ACTIVE_RGBA),
            ),
            NativeButtonVariant::Install => (
                gpui::rgb(SCRIBE_PRIMARY),
                gpui::rgb(SCRIBE_PRIMARY_FOREGROUND),
                gpui::rgb(SCRIBE_PRIMARY),
                gpui::rgb(SCRIBE_PRIMARY_HOVER),
                gpui::rgb(SCRIBE_PRIMARY_PRESSED),
            ),
        };
        let is_install = self.variant == NativeButtonVariant::Install;
        let icon = self.icon.or(is_install.then_some(IconName::Plus));
        div()
            .id(self.id)
            .focusable()
            .tab_stop(true)
            .role(Role::Button)
            .aria_label(aria_label)
            .cursor_pointer()
            .h(px(32.0))
            .min_w(px(if is_install { 100.0 } else { 32.0 }))
            .px(px(if is_install { 8.0 } else { 14.0 }))
            .rounded(px(16.0))
            .border_1()
            .border_color(border)
            .bg(background)
            .text_color(foreground)
            .text_size(px(13.0))
            .font_medium()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(if is_install { 8.0 } else { 7.0 }))
            .hover(move |button| button.bg(hover))
            .active(move |button| button.bg(pressed))
            .focus(|button| {
                button
                    .border_1()
                    .border_color(gpui::rgba(SCRIBE_FOCUS_RING_RGBA))
            })
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                pointer_activate(window, cx);
            })
            .on_key_down(move |event, window, cx| {
                if !event.is_held && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    keyboard_activate(window, cx);
                }
            })
            .when_some(icon, move |button, icon| {
                button.child(Icon::new(icon).size(px(if is_install { 14.0 } else { 15.0 })))
            })
            .child(label)
    }
}

#[derive(IntoElement)]
pub(crate) struct NativeIconButton {
    id: ElementId,
    label: SharedString,
    icon: IconName,
    icon_path: Option<&'static str>,
    danger: bool,
    overlay: bool,
    on_activate: Arc<ActivateHandler>,
}

impl NativeIconButton {
    pub(crate) fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        icon: IconName,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon,
            icon_path: None,
            danger: false,
            overlay: false,
            on_activate: Arc::new(|_, _| {}),
        }
    }

    pub(crate) fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub(crate) fn overlay(mut self) -> Self {
        self.overlay = true;
        self
    }

    pub(crate) fn asset_icon(mut self, path: &'static str) -> Self {
        self.icon_path = Some(path);
        self
    }

    pub(crate) fn on_activate(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Arc::new(handler);
        self
    }
}

impl RenderOnce for NativeIconButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let pointer_activate = self.on_activate.clone();
        let keyboard_activate = self.on_activate;
        let foreground = if self.overlay {
            gpui::rgb(SCRIBE_OVERLAY_FOREGROUND)
        } else if self.danger {
            gpui::rgb(SCRIBE_DANGER)
        } else {
            gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA)
        };
        let hover = if self.overlay {
            gpui::rgb(SCRIBE_OVERLAY_FOREGROUND).opacity(0.22)
        } else if self.danger {
            gpui::rgba(SCRIBE_DANGER_TINT_HOVER_RGBA)
        } else {
            gpui::rgba(SCRIBE_BUTTON_FILL_RGBA)
        };
        let icon = match self.icon_path {
            Some(path) => Icon::default().path(path),
            None => Icon::new(self.icon),
        };
        div()
            .id(self.id)
            .focusable()
            .tab_stop(true)
            .role(Role::Button)
            .aria_label(self.label)
            .size(px(if self.overlay { 40.0 } else { 32.0 }))
            .flex_none()
            .rounded(px(if self.overlay { 20.0 } else { 8.0 }))
            .when(self.overlay, |button| {
                button.bg(gpui::rgb(SCRIBE_OVERLAY_FOREGROUND).opacity(0.12))
            })
            .flex()
            .items_center()
            .justify_center()
            .text_color(foreground)
            .cursor_pointer()
            .hover(move |button| button.bg(hover))
            .focus(|button| {
                button
                    .border_1()
                    .border_color(gpui::rgba(SCRIBE_FOCUS_RING_RGBA))
            })
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                pointer_activate(window, cx);
            })
            .on_key_down(move |event, window, cx| {
                if !event.is_held && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    keyboard_activate(window, cx);
                }
            })
            .child(icon.size(px(15.0)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FilterOption {
    pub(crate) label: SharedString,
    pub(crate) value: SharedString,
    pub(crate) icon_url: Option<SharedString>,
}

impl FilterOption {
    pub(crate) fn new(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            icon_url: None,
        }
    }

    pub(crate) fn with_icon(mut self, icon_url: impl Into<SharedString>) -> Self {
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

pub(crate) fn catalog_sort_button(
    id: &'static str,
    label: impl Into<SharedString>,
    width: f32,
    selected: SharedString,
    ascending: bool,
    state: Entity<SelectState<SearchableVec<FilterOption>>>,
    owner: Entity<ScribeWindow>,
) -> gpui::AnyElement {
    let choices = [
        ("Popular · high to low", "downloads", false),
        ("Most favorited · high to low", "favorites", false),
        ("Recently updated · newest first", "date", false),
        ("Name · A to Z", "title", true),
        ("Name · Z to A", "title", false),
        ("Category · A to Z", "category", true),
    ];
    let menu_state = state.clone();
    let menu_owner = owner.clone();
    Button::new(id)
        .debug_selector(move || id.into())
        .label(label)
        .dropdown_caret(true)
        .compact()
        .with_size(Size::Small)
        .w(px(width))
        .h(px(32.0))
        .px(px(10.0))
        .text_size(px(11.0))
        .text_color(gpui::rgb(SCRIBE_FOREGROUND))
        .bg(gpui::rgba(SCRIBE_BUTTON_FILL_RGBA))
        .border_1()
        .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
        .dropdown_menu_with_anchor(Anchor::TopLeft, move |menu, _, _| {
            choices.iter().fold(menu, |menu, (label, value, order)| {
                let value = SharedString::from(*value);
                let order = *order;
                let state = menu_state.clone();
                let owner = menu_owner.clone();
                menu.item(
                    PopupMenuItem::new(*label)
                        .checked(value == selected && order == ascending)
                        .on_click(move |_, window, cx| {
                            state.update(cx, |state, cx| {
                                state.set_selected_value(&value, window, cx);
                            });
                            owner.update(cx, |view, cx| {
                                view.sort_ascending = order;
                                cx.notify();
                            });
                        }),
                )
            })
        })
        .into_any_element()
}

pub(crate) fn category_filter_trigger(
    label: SharedString,
    selected_icon: Option<SharedString>,
    open: bool,
    owner: Entity<ScribeWindow>,
    search: Entity<InputState>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
) -> gpui::AnyElement {
    let pointer_owner = owner.clone();
    let keyboard_owner = owner;
    let pointer_search = search.clone();
    let keyboard_search = search;
    let paint_bounds = bounds;
    let aria_label = label.clone();
    let icon_label = label.clone();
    div()
        .id("category-filter-control")
        .debug_selector(|| "category-filter-control".into())
        .focusable()
        .tab_stop(true)
        .role(Role::Button)
        .aria_label(aria_label)
        .aria_expanded(open)
        .cursor_pointer()
        .w(px(174.0))
        .h(px(32.0))
        .px(px(10.0))
        .rounded(px(SCRIBE_INPUT_RADIUS))
        .border_1()
        .border_color(if open {
            gpui::rgba(SCRIBE_FOCUS_RING_RGBA)
        } else {
            gpui::rgba(SCRIBE_HAIRLINE_RGBA)
        })
        .bg(if open {
            gpui::rgba(SCRIBE_SURFACE_HOVER_RGBA)
        } else {
            gpui::rgba(SCRIBE_BUTTON_FILL_RGBA)
        })
        .text_color(gpui::rgb(SCRIBE_FOREGROUND))
        .text_size(px(11.0))
        .flex()
        .items_center()
        .gap(px(7.0))
        .hover(|button| button.bg(gpui::rgba(SCRIBE_SURFACE_HOVER_RGBA)))
        .focus(|button| button.border_color(gpui::rgba(SCRIBE_FOCUS_RING_RGBA)))
        .on_prepaint(move |bounds, _, _| paint_bounds.set(bounds))
        .on_click(move |_, window, cx| {
            if open {
                pointer_search.update(cx, |search, cx| {
                    search.set_value("", window, cx);
                });
            } else {
                window.focus(&pointer_search.read(cx).focus_handle(cx), cx);
            }
            pointer_owner.update(cx, |view, cx| {
                if !view.category_palette_open {
                    let selected = view
                        .category_select
                        .read(cx)
                        .selected_value()
                        .cloned()
                        .unwrap_or_default();
                    view.category_cursor = view
                        .category_options
                        .iter()
                        .position(|option| option.value == selected)
                        .unwrap_or(0);
                }
                view.category_palette_open = !view.category_palette_open;
                cx.notify();
            });
        })
        .on_key_down(move |event, window, cx| {
            if event.is_held {
                return;
            }
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                cx.stop_propagation();
                if open {
                    keyboard_search.update(cx, |search, cx| {
                        search.set_value("", window, cx);
                    });
                } else {
                    window.focus(&keyboard_search.read(cx).focus_handle(cx), cx);
                }
                keyboard_owner.update(cx, |view, cx| {
                    if !view.category_palette_open {
                        let selected = view
                            .category_select
                            .read(cx)
                            .selected_value()
                            .cloned()
                            .unwrap_or_default();
                        view.category_cursor = view
                            .category_options
                            .iter()
                            .position(|option| option.value == selected)
                            .unwrap_or(0);
                    }
                    view.category_palette_open = !view.category_palette_open;
                    cx.notify();
                });
            } else if event.keystroke.key == "escape" && open {
                cx.stop_propagation();
                keyboard_owner.update(cx, |view, cx| {
                    view.category_palette_open = false;
                    cx.notify();
                });
            }
        })
        .when_some(selected_icon, move |button, icon_url| {
            button.child(category_artwork(
                Some(icon_url.to_string()),
                icon_label.as_ref(),
                17.0,
            ))
        })
        .child(
            div()
                .min_w_0()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(label),
        )
        .child(
            Icon::new(if open {
                IconName::ChevronUp
            } else {
                IconName::ChevronDown
            })
            .size(px(12.0)),
        )
        .into_any_element()
}

pub(crate) fn compatibility_control(
    selected: SharedString,
    options: Vec<FilterOption>,
    state: Entity<SelectState<SearchableVec<FilterOption>>>,
    owner: Entity<ScribeWindow>,
) -> gpui::AnyElement {
    let latest_value = options
        .iter()
        .find(|option| !option.value.is_empty())
        .map(|option| option.value.clone())
        .unwrap_or_default();
    let choices = [("All", SharedString::default()), ("Latest", latest_value)];
    div()
        .id("compatibility-filter-control")
        .debug_selector(|| "compatibility-filter-control".into())
        .role(Role::Group)
        .aria_label("ESO compatibility")
        .w(px(168.0))
        .h(px(32.0))
        .p(px(3.0))
        .rounded(px(SCRIBE_INPUT_RADIUS))
        .border_1()
        .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
        .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
        .flex()
        .items_center()
        .gap(px(2.0))
        .children(
            choices
                .into_iter()
                .enumerate()
                .map(|(index, (label, value))| {
                    let active = selected == value;
                    let pointer_value = value.clone();
                    let keyboard_value = value;
                    let pointer_state = state.clone();
                    let keyboard_state = state.clone();
                    let pointer_owner = owner.clone();
                    let keyboard_owner = owner.clone();
                    div()
                        .id(format!("compatibility-choice-{index}"))
                        .focusable()
                        .tab_stop(true)
                        .role(Role::Button)
                        .aria_label(format!("Compatibility: {label}"))
                        .aria_selected(active)
                        .cursor_pointer()
                        .h_full()
                        .flex_1()
                        .rounded(px(5.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(11.0))
                        .font_medium()
                        .bg(if active {
                            gpui::rgba(SCRIBE_SURFACE_ACTIVE_RGBA)
                        } else {
                            gpui::rgba(0x00000000)
                        })
                        .text_color(if active {
                            gpui::rgb(SCRIBE_FOREGROUND)
                        } else {
                            gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA)
                        })
                        .hover(|choice| choice.bg(gpui::rgba(SCRIBE_SURFACE_HOVER_RGBA)))
                        .focus(|choice| {
                            choice
                                .border_1()
                                .border_color(gpui::rgba(SCRIBE_FOCUS_RING_RGBA))
                        })
                        .on_click(move |_, window, cx| {
                            pointer_state.update(cx, |state, cx| {
                                state.set_selected_value(&pointer_value, window, cx);
                            });
                            pointer_owner.update(cx, |_, cx| cx.notify());
                        })
                        .on_key_down(move |event, window, cx| {
                            if !event.is_held
                                && matches!(event.keystroke.key.as_str(), "enter" | "space")
                            {
                                cx.stop_propagation();
                                keyboard_state.update(cx, |state, cx| {
                                    state.set_selected_value(&keyboard_value, window, cx);
                                });
                                keyboard_owner.update(cx, |_, cx| cx.notify());
                            }
                        })
                        .child(label)
                }),
        )
        .into_any_element()
}

pub(crate) fn category_palette_option(
    option: FilterOption,
    selected: bool,
    cursor: bool,
    state: Entity<SelectState<SearchableVec<FilterOption>>>,
    owner: Entity<ScribeWindow>,
) -> gpui::AnyElement {
    let id = SharedString::from(format!("category-atlas-{}", option.value));
    let label = option.label.clone();
    let icon = option.icon_url.clone().map(|icon| icon.to_string());
    let is_all = option.value.is_empty();
    let pointer_value = option.value.clone();
    let keyboard_value = option.value;
    let pointer_state = state.clone();
    let keyboard_state = state;
    let pointer_owner = owner.clone();
    let keyboard_owner = owner;
    div()
        .id(id)
        .focusable()
        .tab_stop(true)
        .role(Role::Button)
        .aria_label(format!("Filter by {label}"))
        .aria_selected(selected)
        .cursor_pointer()
        .w_full()
        .h(px(40.0))
        .px(px(10.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(gpui::rgb(if cursor {
            SCRIBE_FOCUS_RING
        } else if selected {
            SCRIBE_ACTIVE_BORDER
        } else {
            SCRIBE_PARCHMENT_BORDER
        }))
        .bg(gpui::rgb(if selected || cursor {
            SCRIBE_PARCHMENT_PANEL
        } else {
            SCRIBE_PARCHMENT_ELEVATED
        }))
        .flex()
        .items_center()
        .gap(px(9.0))
        .hover(|tile| {
            tile.bg(gpui::rgb(SCRIBE_PARCHMENT_HOVER))
                .border_color(gpui::rgb(SCRIBE_ACTIVE_BORDER))
        })
        .focus(|tile| tile.border_color(gpui::rgb(SCRIBE_FOCUS_RING)))
        .on_click(move |_, window, cx| {
            pointer_state.update(cx, |state, cx| {
                state.set_selected_value(&pointer_value, window, cx);
            });
            pointer_owner.update(cx, |view, cx| {
                view.category_palette_open = false;
                cx.notify();
            });
        })
        .on_key_down(move |event, window, cx| {
            if !event.is_held && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                cx.stop_propagation();
                keyboard_state.update(cx, |state, cx| {
                    state.set_selected_value(&keyboard_value, window, cx);
                });
                keyboard_owner.update(cx, |view, cx| {
                    view.category_palette_open = false;
                    cx.notify();
                });
            }
        })
        .child(if is_all {
            div()
                .size(px(20.0))
                .flex_none()
                .rounded(px(6.0))
                .bg(gpui::rgba(SCRIBE_SURFACE_ACTIVE_RGBA))
                .text_color(gpui::rgb(SCRIBE_PRIMARY))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(IconName::LayoutDashboard).size(px(12.0)))
                .into_any_element()
        } else {
            category_artwork(icon, &label, 20.0)
        })
        .child(
            div()
                .min_w_0()
                .flex_1()
                .overflow_hidden()
                .text_ellipsis()
                .font_medium()
                .text_size(px(11.0))
                .child(label),
        )
        .when(selected, |tile| {
            tile.child(
                Icon::new(IconName::Check)
                    .size(px(13.0))
                    .text_color(gpui::rgb(SCRIBE_HEALTH_SUCCESS)),
            )
        })
        .into_any_element()
}

pub(crate) struct CategoryPickerOverlay {
    pub(crate) options: Vec<FilterOption>,
    pub(crate) selected: SharedString,
    pub(crate) query: String,
    pub(crate) cursor: usize,
    pub(crate) search: Entity<InputState>,
    pub(crate) state: Entity<SelectState<SearchableVec<FilterOption>>>,
    pub(crate) owner: Entity<ScribeWindow>,
    pub(crate) trigger: Bounds<Pixels>,
    pub(crate) viewport: gpui::Size<Pixels>,
}

pub(crate) fn render_category_picker_overlay(picker: CategoryPickerOverlay) -> gpui::AnyElement {
    let CategoryPickerOverlay {
        options,
        selected,
        query,
        cursor,
        search,
        state,
        owner,
        trigger,
        viewport,
    } = picker;
    let width = px(360.0);
    let margin = px(8.0);
    let left = if trigger.origin.x + width + margin > viewport.width {
        viewport.width - width - margin
    } else {
        trigger.origin.x
    };
    let top = trigger.origin.y + trigger.size.height + px(6.0);
    let normalized_query = query.trim().to_ascii_lowercase();
    let filtered = options
        .into_iter()
        .filter(|option| {
            normalized_query.is_empty()
                || option
                    .label
                    .to_ascii_lowercase()
                    .contains(&normalized_query)
        })
        .collect::<Vec<_>>();
    let result_count = filtered.len();
    let rows = filtered
        .into_iter()
        .enumerate()
        .map(|(index, option)| {
            category_palette_option(
                option.clone(),
                option.value == selected,
                index == cursor,
                state.clone(),
                owner.clone(),
            )
        })
        .collect::<Vec<_>>();
    let dismiss_owner = owner;
    let dismiss_search = search.clone();
    div()
        .id("category-picker-layer")
        .absolute()
        .inset_0()
        .on_click(move |_, window, cx| {
            dismiss_search.update(cx, |search, cx| search.set_value("", window, cx));
            dismiss_owner.update(cx, |view, cx| {
                view.category_palette_open = false;
                cx.notify();
            });
        })
        .child(
            div()
                .id("category-picker-popover")
                .role(Role::Group)
                .aria_label("Choose an addon category")
                .absolute()
                .left(left)
                .top(top)
                .w(width)
                .max_h(px(430.0))
                .p(px(10.0))
                .rounded(px(12.0))
                .border_1()
                .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                .bg(gpui::rgba(SCRIBE_SURFACE_RAISED_RGBA))
                .shadow_lg()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .px(px(2.0))
                        .flex()
                        .items_baseline()
                        .justify_between()
                        .child(
                            div()
                                .font_semibold()
                                .text_size(px(12.0))
                                .child("Addon categories"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(gpui::rgb(SCRIBE_TEXT_SUBTLE))
                                .child(format!("{result_count} choices")),
                        ),
                )
                .child(
                    Input::new(&search)
                        .prefix(IconName::Search)
                        .role(Role::SearchInput)
                        .bg(gpui::rgb(SCRIBE_TOOLBAR_INPUT))
                        .text_color(gpui::rgb(SCRIBE_FOREGROUND))
                        .border_color(gpui::rgb(SCRIBE_INPUT_BORDER))
                        .shadow_none(),
                )
                .child(
                    div()
                        .max_h(px(330.0))
                        .overflow_y_scrollbar()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .when(rows.is_empty(), |list| {
                            list.child(
                                div()
                                    .p(px(18.0))
                                    .text_size(px(11.0))
                                    .text_color(gpui::rgb(SCRIBE_TEXT_SUBTLE))
                                    .child("No categories match that search."),
                            )
                        })
                        .children(rows),
                ),
        )
        .into_any_element()
}

#[derive(IntoElement)]
pub(crate) struct Title {
    text: SharedString,
    order: u8,
}

impl Title {
    pub(crate) fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            order: 3,
        }
    }

    pub(crate) fn order(mut self, order: u8) -> Self {
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
pub(crate) struct Group {
    gap: Size,
    children: Vec<AnyElement>,
}

impl Group {
    pub(crate) fn new() -> Self {
        Self {
            gap: Size::Medium,
            children: Vec::new(),
        }
    }

    pub(crate) fn gap(mut self, gap: Size) -> Self {
        self.gap = gap;
        self
    }

    pub(crate) fn child(mut self, child: impl IntoElement) -> Self {
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
pub(crate) struct Modal {
    title: SharedString,
    width: f32,
    sheet: bool,
    child: Option<AnyElement>,
    focus: FocusHandle,
    on_close: Arc<ActivateHandler>,
}

impl Modal {
    pub(crate) fn new(focus: FocusHandle) -> Self {
        Self {
            title: SharedString::default(),
            width: 560.0,
            sheet: false,
            child: None,
            focus,
            on_close: Arc::new(|_, _| {}),
        }
    }

    pub(crate) fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub(crate) fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub(crate) fn sheet(mut self) -> Self {
        self.sheet = true;
        self
    }

    pub(crate) fn on_close(mut self, close: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Arc::new(close);
        self
    }

    pub(crate) fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl RenderOnce for Modal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let close = self.on_close;
        let title = self.title.clone();
        let sheet = self.sheet;
        let compact_sheet = sheet && window.bounds().size.width < px(1320.0);
        let backdrop = div()
            .id("modal-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .when(sheet, |backdrop| backdrop.justify_end())
            .when(!sheet, |backdrop| backdrop.justify_center())
            .bg(gpui::rgb(SCRIBE_OVERLAY).opacity(0.45))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("modal-surface")
                    .role(Role::Dialog)
                    .aria_label(title.clone())
                    .w(px(self.width))
                    .max_w(relative(0.94))
                    .max_h(relative(0.92))
                    .when(sheet, |surface| {
                        surface
                            .h_full()
                            .max_h(relative(1.0))
                            .rounded(px(0.0))
                            .border_l_1()
                    })
                    .when(compact_sheet, |surface| {
                        surface.w_full().max_w(relative(1.0))
                    })
                    .min_h_0()
                    .overflow_hidden()
                    .p(px(16.0))
                    .when(!sheet, |surface| surface.rounded(px(SCRIBE_SHEET_RADIUS)))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().popover)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .track_focus(&self.focus)
                    .focus_trap("scribe-modal", &self.focus)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(Title::new(title).order(3))
                            .child(
                                NativeIconButton::new(
                                    "modal-close",
                                    "Close dialog",
                                    IconName::Close,
                                )
                                .on_activate(move |window, cx| close(window, cx)),
                            ),
                    )
                    .children(self.child),
            );
        if cx.reduce_motion() {
            backdrop.into_any_element()
        } else {
            backdrop
                .with_animation(
                    "modal-enter",
                    Animation::new(Duration::from_millis(SCRIBE_MOTION_FAST_MS)),
                    |overlay, delta| overlay.opacity(0.86 + delta * 0.14),
                )
                .into_any_element()
        }
    }
}

pub(crate) fn empty_state(
    icon: IconName,
    title: &'static str,
    message: &'static str,
    action: Option<NativeButton>,
) -> gpui::AnyElement {
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
                        .rounded(px(12.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui::rgba(SCRIBE_ACCENT_SOFT_RGBA))
                        .text_color(gpui::rgb(SCRIBE_PRIMARY))
                        .child(Icon::new(icon).size(px(23.0))),
                )
                .child(div().font_semibold().text_size(px(15.0)).child(title))
                .child(
                    div()
                        .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
                        .text_size(px(13.0))
                        .child(message),
                )
                .when_some(action, |state, action| {
                    state.child(div().mt(px(8.0)).child(action))
                }),
        )
        .into_any_element()
}

pub(crate) fn settings_section_label(title: &'static str, description: &'static str) -> gpui::Div {
    div()
        .mt(px(4.0))
        .px(px(2.0))
        .flex()
        .items_end()
        .justify_between()
        .child(
            div()
                .text_size(px(12.0))
                .font_semibold()
                .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                .child(description),
        )
}

pub(crate) fn settings_card(
    icon: IconName,
    title: &'static str,
    description: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(title)
        .role(Role::Group)
        .aria_label(format!("{title}. {description}"))
        .w_full()
        .p(px(18.0))
        .rounded(px(SCRIBE_CARD_RADIUS))
        .border_1()
        .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
        .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .size(px(30.0))
                        .flex_none()
                        .rounded(px(9.0))
                        .bg(gpui::rgba(SCRIBE_ACCENT_SOFT_RGBA))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(gpui::rgb(SCRIBE_PRIMARY))
                        .child(Icon::new(icon).size(px(16.0))),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(div().font_semibold().text_size(px(13.0)).child(title))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
                                .child(description),
                        ),
                ),
        )
}

pub(crate) fn metric_pill(value: impl Into<SharedString>, label: &'static str) -> gpui::Div {
    div()
        .px(px(11.0))
        .py(px(7.0))
        .rounded(px(10.0))
        .border_1()
        .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
        .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
        .flex()
        .flex_col()
        .gap(px(1.0))
        .child(
            div()
                .font_semibold()
                .text_size(px(12.0))
                .child(value.into()),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                .child(label),
        )
}

pub(crate) fn relative_success_time(timestamp: i64) -> String {
    let elapsed = unix_now().saturating_sub(timestamp);
    if elapsed < 60 {
        "just now".into()
    } else if elapsed < 60 * 60 {
        format!("{} minutes ago", elapsed / 60)
    } else if elapsed < 24 * 60 * 60 {
        format!("{} hours ago", elapsed / (60 * 60))
    } else {
        format!("{} days ago", elapsed / (24 * 60 * 60))
    }
}

pub(crate) fn health_status_row(
    name: &'static str,
    state: (&str, String, String),
    last_success: Option<i64>,
) -> gpui::AnyElement {
    let (label, cause, impact) = state;
    let last_success_label = last_success
        .map(|timestamp| format!(" Last success {}.", relative_success_time(timestamp)))
        .unwrap_or_default();
    let color = match label {
        "Healthy" | "Succeeded" => gpui::rgb(SCRIBE_HEALTH_SUCCESS),
        "Running" | "Waiting" | "Not configured" => gpui::rgb(SCRIBE_HEALTH_WARNING),
        _ => gpui::rgb(SCRIBE_HEALTH_DANGER),
    };
    div()
        .id(name)
        .role(Role::Group)
        .aria_label(format!(
            "{name}: {label}. {cause} {impact}{last_success_label}"
        ))
        .w_full()
        .px(px(12.0))
        .py(px(10.0))
        .rounded(px(10.0))
        .border_1()
        .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
        .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
        .flex()
        .items_start()
        .gap(px(10.0))
        .child(
            div()
                .mt(px(5.0))
                .size(px(8.0))
                .flex_none()
                .rounded(px(4.0))
                .bg(color),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(div().font_semibold().text_size(px(12.0)).child(name))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_medium()
                                .text_color(color)
                                .child(label.to_owned()),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
                        .child(cause),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                        .child(impact),
                ),
        )
        .when_some(last_success, |row, timestamp| {
            row.child(
                div()
                    .flex_none()
                    .text_size(px(10.0))
                    .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                    .child(format!("Last success {}", relative_success_time(timestamp))),
            )
        })
        .into_any_element()
}

pub(crate) fn addon_artwork_fallback(
    category_source: Option<String>,
    category_title: String,
    extent: f32,
) -> gpui::AnyElement {
    category_artwork(category_source, &category_title, extent)
}

pub(crate) fn addon_artwork(
    source: Option<String>,
    category_source: Option<String>,
    category_title: &str,
    extent: f32,
) -> gpui::AnyElement {
    let frame = div()
        .size(px(extent))
        .flex_none()
        .rounded(px(10.0))
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::rgba(SCRIBE_SURFACE_ACTIVE_RGBA));
    match source {
        Some(source) if !source.is_empty() => {
            let loading_category = category_source.clone();
            let failed_category = category_source.clone();
            let loading_title = category_title.to_owned();
            let failed_title = category_title.to_owned();
            frame
                .child(
                    img(source)
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                        .with_loading(move || {
                            addon_artwork_fallback(
                                loading_category.clone(),
                                loading_title.clone(),
                                extent,
                            )
                        })
                        .with_fallback(move || {
                            addon_artwork_fallback(
                                failed_category.clone(),
                                failed_title.clone(),
                                extent,
                            )
                        }),
                )
                .into_any_element()
        }
        _ => frame
            .child(addon_artwork_fallback(
                category_source,
                category_title.to_owned(),
                extent,
            ))
            .into_any_element(),
    }
}

pub(crate) fn category_placeholder(title: &str, extent: f32) -> gpui::AnyElement {
    let title = title.to_ascii_lowercase();
    let icon = if title.contains("map") || title.contains("coord") || title.contains("compass") {
        IconName::Map
    } else if title.contains("librar") {
        IconName::BookOpen
    } else if title.contains("chat") || title.contains("social") {
        IconName::Globe
    } else if title.contains("interface") || title.contains("info") {
        IconName::LayoutDashboard
    } else if title.contains("bag") || title.contains("bank") || title.contains("inventory") {
        IconName::FolderClosed
    } else if title.contains("action") {
        IconName::Frame
    } else if title.contains("combat") || title.contains("buff") || title.contains("spell") {
        IconName::Star
    } else if title.contains("utility") || title.contains("patch") || title.contains("plug-in") {
        IconName::Settings2
    } else {
        IconName::BookOpen
    };
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(gpui::rgb(SCRIBE_PRIMARY))
        .child(Icon::new(icon).size(px((extent * 0.58).max(10.0))))
        .into_any_element()
}

pub(crate) fn category_artwork(
    source: Option<String>,
    title: &str,
    extent: f32,
) -> gpui::AnyElement {
    let loading_title = title.to_owned();
    let failed_title = title.to_owned();
    let frame = div()
        .size(px(extent))
        .flex_none()
        .rounded(px((extent * 0.18).max(3.0)))
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::rgba(SCRIBE_SURFACE_ACTIVE_RGBA));
    match source {
        Some(source) if !source.is_empty() => frame
            .child(
                img(source)
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .with_loading(move || category_placeholder(&loading_title, extent))
                    .with_fallback(move || category_placeholder(&failed_title, extent)),
            )
            .into_any_element(),
        _ => frame
            .child(category_placeholder(title, extent))
            .into_any_element(),
    }
}

pub(crate) fn format_count(value: i64) -> String {
    let value = value.max(0) as f64;
    if value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}

pub(crate) fn update_state_label(state: &str) -> String {
    match state {
        "up-to-date" => "Up to date".into(),
        "remote-newer" => "Update available".into(),
        "local-newer" => "Local version is newer".into(),
        "md5-only-changed" => "Files changed upstream".into(),
        "unknown-version" => "Version needs review".into(),
        "unmatched" => "Not matched to ESOUI".into(),
        other => other.to_owned(),
    }
}

pub(crate) fn update_state_badge(state: &str) -> gpui::AnyElement {
    let (fill, foreground) = match state {
        "remote-newer" | "md5-only-changed" => (
            gpui::rgb(SCRIBE_PRIMARY),
            gpui::rgb(SCRIBE_PRIMARY_FOREGROUND),
        ),
        "up-to-date" => (
            gpui::rgb(SCRIBE_HEALTH_SUCCESS).opacity(0.14),
            gpui::rgb(SCRIBE_HEALTH_SUCCESS),
        ),
        "local-newer" | "unknown-version" | "unmatched" => {
            (gpui::rgb(SCRIBE_INFO).opacity(0.14), gpui::rgb(SCRIBE_INFO))
        }
        _ => (
            gpui::rgba(SCRIBE_SURFACE_ACTIVE_RGBA),
            gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA),
        ),
    };
    div()
        .flex_none()
        .px(px(7.0))
        .py(px(2.0))
        .rounded(px(9.0))
        .bg(fill)
        .text_color(foreground)
        .text_size(px(10.0))
        .font_medium()
        .child(update_state_label(state))
        .into_any_element()
}

pub(crate) fn notice_visuals(tone: NoticeTone, theme: &Theme) -> (gpui::Hsla, IconName) {
    match tone {
        NoticeTone::Info => (theme.info, IconName::Info),
        NoticeTone::Success => (theme.success, IconName::CircleCheck),
        NoticeTone::Warning => (theme.warning, IconName::TriangleAlert),
        NoticeTone::Danger => (theme.danger, IconName::TriangleAlert),
    }
}

pub(crate) fn render_status_notice(notice: StatusNotice, theme: &Theme) -> gpui::AnyElement {
    let (color, icon) = notice_visuals(notice.tone, theme);
    let label = format!("{}. {}", notice.title, notice.message);
    let live = if notice.tone == NoticeTone::Danger {
        gpui::accesskit::Live::Assertive
    } else {
        gpui::accesskit::Live::Polite
    };
    let notice = div()
        .id("status-notice")
        .debug_selector(|| "status-notice".to_owned())
        .role(if notice.tone == NoticeTone::Danger {
            Role::Alert
        } else {
            Role::Status
        })
        .aria_label(label)
        .min_h(px(40.0))
        .px(px(14.0))
        .py(px(7.0))
        .flex()
        .items_center()
        .gap(px(9.0))
        .rounded(px(10.0))
        .border_1()
        .border_color(color.opacity(0.32))
        .bg(color.opacity(0.09))
        .text_color(color)
        .child(Icon::new(icon).size(px(16.0)).flex_none())
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .text_size(px(12.0))
                .child(div().font_semibold().child(notice.title))
                .child(
                    div()
                        .min_w_0()
                        .line_clamp(2)
                        .text_ellipsis()
                        .opacity(0.88)
                        .child(notice.message),
                ),
        );
    LiveRegion::new(notice, live).into_any_element()
}

pub(crate) fn render_inline_notice(
    id: impl Into<ElementId>,
    title: &'static str,
    message: &'static str,
    tone: NoticeTone,
    theme: &Theme,
) -> gpui::AnyElement {
    let (color, icon) = notice_visuals(tone, theme);
    div()
        .id(id)
        .role(if tone == NoticeTone::Danger {
            Role::Alert
        } else {
            Role::Note
        })
        .aria_label(format!("{title}. {message}"))
        .w_full()
        .px(px(13.0))
        .py(px(10.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(color.opacity(0.32))
        .bg(color.opacity(0.08))
        .flex()
        .items_start()
        .gap(px(9.0))
        .child(Icon::new(icon).size(px(16.0)).text_color(color))
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(div().font_semibold().text_color(color).child(title))
                .child(div().text_size(px(12.0)).opacity(0.82).child(message)),
        )
        .into_any_element()
}
