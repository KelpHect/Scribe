use std::cell::Cell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Bounds, Entity, FocusHandle, MouseButton, Role, SharedString, div, px};
use gpui_component::{
    ElementExt as _, IconName, Sizable as _, Size, StyledExt as _, checkbox::Checkbox,
};
use scribe_core::{Addon, Category, MatchedAddon, RemoteAddon};

use crate::components::{
    Group, NativeButton, NativeIconButton, addon_artwork, format_count, update_state_badge,
};
use crate::flows::{enqueue_remote, show_addon_details, show_installed_details};
use crate::model::AppModel;
use crate::overlays::{
    claim_context_invoker, context_menu_key, menu_anchor, open_catalog_context_menu,
    open_installed_context_menu,
};
use crate::theme::*;
use crate::window::ScribeWindow;

pub(crate) fn matched_row(
    addon: Addon,
    decision: MatchedAddon,
    category: Option<Category>,
    selected: Option<bool>,
    context_focus: Option<FocusHandle>,
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
    let pointer_menu_addon = addon.clone();
    let pointer_menu_decision = decision.clone();
    let pointer_menu_owner = selection_owner.clone();
    let pointer_context_key = format!("installed:{}", addon.folder_name);
    let keyboard_menu_addon = addon.clone();
    let keyboard_menu_decision = decision.clone();
    let keyboard_menu_owner = selection_owner.clone();
    let keyboard_context_key = pointer_context_key.clone();
    let row_bounds = Rc::new(Cell::new(Bounds::default()));
    let paint_bounds = row_bounds.clone();
    let keyboard_bounds = row_bounds.clone();
    let row_id = addon.folder_name.clone();
    let category_name = category
        .as_ref()
        .map(|category| category.name.to_string())
        .unwrap_or_else(|| {
            if addon.is_library {
                "Libraries".into()
            } else {
                "Addons".into()
            }
        });
    let category_icon = category
        .as_ref()
        .filter(|category| !category.icon_url.is_empty())
        .map(|category| category.icon_url.to_string());
    let installed_version = addon.version.trim_start_matches(['v', 'V']).to_owned();
    let source_label = if let Some(remote) = decision.remote.as_ref() {
        format!("v{installed_version} · {}", remote.ui_name)
    } else if addon.author.is_empty() {
        format!("v{installed_version} · {}", addon.folder_name)
    } else {
        format!(
            "v{installed_version} · {}  •  {}",
            addon.author, addon.folder_name
        )
    };
    div()
        .id(SharedString::from(format!("installed-addon-{row_id}")))
        .when_some(context_focus, |row, focus| row.track_focus(&focus))
        .debug_selector(|| "installed-row".into())
        .focusable()
        .tab_stop(true)
        .role(Role::Button)
        .aria_label(format!("Open details for {}", addon.title))
        .cursor_pointer()
        .h(px(56.0))
        .w_full()
        .pl(px(10.0))
        .pr(px(12.0))
        .border_b_1()
        .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
        .flex()
        .items_center()
        .justify_start()
        .gap(px(12.0))
        .hover(|row| row.bg(gpui::rgba(SCRIBE_SURFACE_HOVER_RGBA)))
        .focus(|row| row.border_color(gpui::rgba(SCRIBE_FOCUS_RING_RGBA)))
        .on_prepaint(move |bounds, _, _| paint_bounds.set(bounds))
        .on_mouse_down(MouseButton::Right, move |event, window, cx| {
            cx.stop_propagation();
            let invocation = claim_context_invoker(
                &pointer_menu_owner,
                pointer_context_key.clone(),
                event.position,
                cx,
            );
            open_installed_context_menu(
                pointer_menu_addon.clone(),
                pointer_menu_decision.clone(),
                selected,
                pointer_menu_owner.clone(),
                invocation,
                window,
                cx,
            );
        })
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
            if event.is_held {
                return;
            }
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                cx.stop_propagation();
                show_installed_details(
                    keyboard_addon.clone(),
                    keyboard_decision.clone(),
                    model.clone(),
                    window,
                    cx,
                );
            } else if context_menu_key(event) {
                cx.stop_propagation();
                let invocation = claim_context_invoker(
                    &keyboard_menu_owner,
                    keyboard_context_key.clone(),
                    menu_anchor(keyboard_bounds.get()),
                    cx,
                );
                open_installed_context_menu(
                    keyboard_menu_addon.clone(),
                    keyboard_menu_decision.clone(),
                    selected,
                    keyboard_menu_owner.clone(),
                    invocation,
                    window,
                    cx,
                );
            }
        })
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
        .child(addon_artwork(
            thumbnail,
            category_icon,
            &category_name,
            40.0,
        ))
        .child(
            div()
                .debug_selector(|| "installed-row-copy".into())
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .child(
                    div()
                        .min_w_0()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .font_semibold()
                                .text_size(px(13.0))
                                .text_color(gpui::rgb(SCRIBE_FOREGROUND))
                                .child(addon.title.clone()),
                        )
                        .when(decision.update_state != "up-to-date", |title| {
                            title.child(update_state_badge(&decision.update_state))
                        }),
                )
                .child(
                    div()
                        .min_w_0()
                        .w_full()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(12.0))
                        .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                        .child(source_label),
                ),
        )
        .child(
            div()
                .debug_selector(|| "installed-row-actions".into())
                .flex_none()
                .child(
                    Group::new()
                        .gap(Size::XSmall)
                        .when_some(update, move |group, remote| {
                            group.child(
                                NativeButton::new(format!("update-{}", remote.uid), "Update")
                                    .on_activate(move |window, cx| {
                                        enqueue_remote(
                                            remote.clone(),
                                            update_model.clone(),
                                            window,
                                            cx,
                                        );
                                    }),
                            )
                        })
                        .child(
                            NativeIconButton::new(
                                format!("uninstall-{folder_name}"),
                                format!("Uninstall {}", addon.title),
                                IconName::Delete,
                            )
                            .asset_icon("scribe-trash.svg")
                            .danger()
                            .on_activate(move |_, cx| {
                                uninstall_model.update(cx, |app, cx| {
                                    app.pending_uninstall = vec![folder_name.clone()];
                                    cx.notify();
                                });
                            }),
                        ),
                ),
        )
        .into_any_element()
}

pub(crate) fn catalog_row(
    addon: RemoteAddon,
    category: Option<Category>,
    context_focus: Option<FocusHandle>,
    owner: Entity<ScribeWindow>,
    model: Entity<AppModel>,
) -> gpui::AnyElement {
    let title = addon.ui_name.clone();
    let author = addon.ui_author_name.clone();
    let version = addon.ui_version.clone();
    let version_label = if version.is_empty() {
        "Version unavailable".to_owned()
    } else {
        format!("v{}", version.trim_start_matches(['v', 'V']))
    };
    let uid = addon.uid.clone();
    let thumbnail = addon.ui_img_thumbs.first().map(ToString::to_string);
    let downloads = format_count(addon.ui_download_total);
    let monthly = format_count(addon.ui_download_monthly);
    let updated = addon.ui_date.to_string();
    let category_name = category
        .as_ref()
        .map(|category| category.name.to_string())
        .unwrap_or_else(|| "Other".into());
    let category_icon = category
        .as_ref()
        .filter(|category| !category.icon_url.is_empty())
        .map(|category| category.icon_url.to_string());
    let author_label = if author.is_empty() {
        version_label
    } else {
        format!("{author} · {version_label}")
    };
    let row_remote = addon.clone();
    let keyboard_remote = addon.clone();
    let details_model = model.clone();
    let keyboard_model = model.clone();
    let install_model = model.clone();
    let pointer_menu_addon = addon.clone();
    let pointer_menu_owner = owner.clone();
    let pointer_context_key = format!("catalog:{}", addon.uid);
    let pointer_menu_model = model.clone();
    let keyboard_menu_addon = addon.clone();
    let keyboard_menu_owner = owner;
    let keyboard_context_key = pointer_context_key.clone();
    let keyboard_menu_model = model.clone();
    let row_bounds = Rc::new(Cell::new(Bounds::default()));
    let paint_bounds = row_bounds.clone();
    let keyboard_bounds = row_bounds.clone();
    div()
        .h(px(72.0))
        .w_full()
        .pb(px(8.0))
        .child(
            div()
                .id(SharedString::from(format!("catalog-addon-{uid}")))
                .when_some(context_focus, |row, focus| row.track_focus(&focus))
                .debug_selector(|| "catalog-row".into())
                .focusable()
                .tab_stop(true)
                .role(Role::Button)
                .aria_label(format!("Open details for {title}"))
                .cursor_pointer()
                .h_full()
                .w_full()
                .pl(px(10.0))
                .pr(px(12.0))
                .rounded(px(SCRIBE_CARD_RADIUS))
                .border_1()
                .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
                .flex()
                .items_center()
                .justify_start()
                .gap(px(12.0))
                .hover(|row| row.bg(gpui::rgba(SCRIBE_SURFACE_HOVER_RGBA)))
                .focus(|row| row.border_color(gpui::rgba(SCRIBE_FOCUS_RING_RGBA)))
                .on_prepaint(move |bounds, _, _| paint_bounds.set(bounds))
                .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                    cx.stop_propagation();
                    let invocation = claim_context_invoker(
                        &pointer_menu_owner,
                        pointer_context_key.clone(),
                        event.position,
                        cx,
                    );
                    open_catalog_context_menu(
                        pointer_menu_addon.clone(),
                        pointer_menu_owner.clone(),
                        pointer_menu_model.clone(),
                        invocation,
                        window,
                        cx,
                    );
                })
                .on_click(move |_, window, cx| {
                    show_addon_details(row_remote.clone(), details_model.clone(), window, cx);
                })
                .on_key_down(move |event, window, cx| {
                    if event.is_held {
                        return;
                    }
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        show_addon_details(
                            keyboard_remote.clone(),
                            keyboard_model.clone(),
                            window,
                            cx,
                        );
                    } else if context_menu_key(event) {
                        cx.stop_propagation();
                        let invocation = claim_context_invoker(
                            &keyboard_menu_owner,
                            keyboard_context_key.clone(),
                            menu_anchor(keyboard_bounds.get()),
                            cx,
                        );
                        open_catalog_context_menu(
                            keyboard_menu_addon.clone(),
                            keyboard_menu_owner.clone(),
                            keyboard_menu_model.clone(),
                            invocation,
                            window,
                            cx,
                        );
                    }
                })
                .child(addon_artwork(
                    thumbnail,
                    category_icon.clone(),
                    &category_name,
                    44.0,
                ))
                .child(
                    div()
                        .debug_selector(|| "catalog-row-copy".into())
                        .min_w_0()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .min_w_0()
                                .w_full()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .font_semibold()
                                        .text_size(px(13.0))
                                        .text_color(gpui::rgb(SCRIBE_FOREGROUND))
                                        .child(title.to_string()),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .max_w(px(150.0))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .px(px(8.0))
                                        .py(px(2.0))
                                        .rounded(px(9.0))
                                        .bg(gpui::rgba(SCRIBE_ACCENT_SOFT_RGBA))
                                        .text_color(gpui::rgb(SCRIBE_PRIMARY))
                                        .text_size(px(11.0))
                                        .child(category_name),
                                ),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .w_full()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(12.0))
                                .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                                .child(author_label),
                        ),
                )
                .child(
                    div()
                        .w(px(150.0))
                        .flex_none()
                        .flex()
                        .flex_col()
                        .items_end()
                        .gap(px(1.0))
                        .text_size(px(12.0))
                        .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                        .child(
                            div()
                                .max_w(px(150.0))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(format!("{downloads} downloads")),
                        )
                        .child(
                            div()
                                .max_w(px(150.0))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(format!("{monthly} this month")),
                        )
                        .when(!updated.is_empty(), |column| {
                            column.child(
                                div()
                                    .max_w(px(150.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(format!("Updated {updated}")),
                            )
                        }),
                )
                .child(
                    div()
                        .debug_selector(|| "catalog-row-actions".into())
                        .flex_none()
                        .child(
                            Group::new().gap(Size::XSmall).child(
                                NativeButton::new(format!("install-{uid}"), "Install")
                                    .install()
                                    .icon(IconName::Plus)
                                    .on_activate(move |window, cx| {
                                        enqueue_remote(
                                            addon.clone(),
                                            install_model.clone(),
                                            window,
                                            cx,
                                        );
                                    }),
                            ),
                        ),
                ),
        )
        .into_any_element()
}
