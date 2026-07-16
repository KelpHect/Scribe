use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt as _, App, Bounds, Context, DismissEvent, Entity, FocusHandle,
    Focusable, IntoElement, ObjectFit, Pixels, Point, Role, StyledImage, Window, anchored,
    deferred, div, img, point, px, relative,
};
use gpui_component::{
    FocusTrapElement as _, Icon, IconName, Size, StyledExt as _, Theme, h_flex,
    menu::{PopupMenu, PopupMenuItem},
    scroll::ScrollableElement as _,
};
use scribe_core::{
    Addon, Category, MatchedAddon, MissingDependency, RemoteAddon, RemoteAddonDetails,
    TaskProgress, TaskState,
};

use crate::bbcode::render_bbcode;
use crate::components::{
    Group, Modal, NativeButton, NativeIconButton, Title, addon_artwork, category_artwork,
    format_count, metric_pill, notice_visuals, update_state_label,
};
use crate::flows::{
    enqueue_dependency_uids, enqueue_remote, rebuild_local_storage, show_addon_details,
    show_installed_details, uninstall_named_folders,
};
use crate::model::{AppModel, NoticeTone, RecoveryPhase, installed_groups};
use crate::theme::*;
use crate::window::ScribeWindow;

pub(crate) fn render_missing_dependencies(
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

pub(crate) fn dependency_banner(
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
    let tone = if optional {
        gpui::rgb(SCRIBE_INFO)
    } else {
        gpui::rgb(SCRIBE_WARNING)
    };
    div()
        .debug_selector(|| "dependency-banner".to_owned())
        .w_full()
        .px(px(13.0))
        .py(px(10.0))
        .rounded(px(12.0))
        .border_1()
        .border_color(tone.opacity(0.35))
        .bg(tone.opacity(0.12))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .debug_selector(|| "dependency-banner-copy".to_owned())
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    Icon::new(if optional {
                        IconName::Info
                    } else {
                        IconName::TriangleAlert
                    })
                    .size(px(16.0))
                    .flex_none()
                    .text_color(tone),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(div().font_semibold().text_size(px(13.0)).child(format!(
                            "{count} missing {} dependencies detected",
                            if optional { "optional" } else { "required" }
                        )))
                        .child(
                            div()
                                .debug_selector(|| "dependency-banner-details".to_owned())
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(12.0))
                                .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
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
                .debug_selector(|| "dependency-banner-actions".to_owned())
                .flex_none()
                .gap(px(6.0))
                .when(!installable.is_empty(), |actions| {
                    actions.child(
                        NativeButton::new(
                            if optional {
                                "install-optional-dependencies"
                            } else {
                                "install-required-dependencies"
                            },
                            "Install missing",
                        )
                        .icon(IconName::ArrowDown)
                        .on_activate(move |host, cx| {
                            enqueue_dependency_uids(installable.clone(), model.clone(), host, cx)
                        }),
                    )
                })
                .child(
                    NativeIconButton::new(
                        if optional {
                            "dismiss-optional-dependencies"
                        } else {
                            "dismiss-required-dependencies"
                        },
                        "Dismiss",
                        IconName::Close,
                    )
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

pub(crate) fn details_screenshot_rail(
    screenshots: &[String],
    model: Entity<AppModel>,
) -> gpui::AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .font_semibold()
                .text_size(px(12.0))
                .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
                .child("Screenshots"),
        )
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
                            .rounded(px(8.0))
                            .overflow_hidden()
                            .border_1()
                            .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                            .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
                            .hover(|image| image.border_color(gpui::rgb(SCRIBE_PRIMARY)))
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

pub(crate) fn render_details_modal(
    details: RemoteAddonDetails,
    category: Option<Category>,
    model: Entity<AppModel>,
    focus: FocusHandle,
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
    let has_description = !details.ui_description.trim().is_empty();
    let has_change_log = !details.ui_change_log.trim().is_empty();
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
        .as_ref()
        .filter(|category| !category.icon_url.is_empty())
        .map(|category| category.icon_url.to_string());
    let title = details.addon.ui_name.to_string();
    let author = details.addon.ui_author_name.to_string();
    let version = details.addon.ui_version.to_string();
    let version_display = version.trim_start_matches(['v', 'V']).to_owned();
    let updated = details.addon.ui_date.to_string();
    let downloads = format_count(details.addon.ui_download_total);
    let favorites = format_count(details.addon.ui_favorite_total);
    let views = format_count(details.ui_hit_count);
    let title_for_aria = title.clone();
    Modal::new(focus)
        .title("Addon dossier")
        .width(860.0)
        .sheet()
        .on_close(move |_, cx| {
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
                .role(Role::Group)
                .aria_label(format!("{title_for_aria} addon details"))
                .min_h_0()
                .flex_1()
                .overflow_y_scrollbar()
                .pr(px(5.0))
                .flex()
                .flex_col()
                .gap(px(18.0))
                .child(
                    div()
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
                                .child(addon_artwork(
                                    thumbnail,
                                    category_icon.clone(),
                                    &category_name,
                                    56.0,
                                ))
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.0))
                                        .child(
                                            div()
                                                .min_w_0()
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .font_semibold()
                                                .text_size(px(17.0))
                                                .child(title),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                                                .child(format!(
                                                    "{author}  •  v{version_display}  •  {category_name}"
                                                )),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap(px(12.0))
                                                .text_size(px(12.0))
                                                .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                                                .child(format!("{downloads} downloads"))
                                                .child(format!("{favorites} favorites"))
                                                .child(format!("{views} views"))
                                                .when(!updated.is_empty(), |row| {
                                                    row.child(format!("Updated {updated}"))
                                                })
                                                .when(!compatibility.is_empty(), |row| {
                                                    row.child(compatibility)
                                                }),
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
                                    NativeButton::new("install-from-details", "Install addon")
                                        .install()
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
                .when(has_description, |column| {
                    column.child(detail_rich_section(
                        "Description",
                        "remote-description",
                        &details.ui_description,
                    ))
                })
                .when(has_change_log, |column| {
                    column.child(detail_rich_section(
                        "Latest changes",
                        "remote-changelog",
                        &details.ui_change_log,
                    ))
                }),
        )
        .into_any_element()
}

pub(crate) fn render_local_details_modal(
    local: (Addon, MatchedAddon),
    details: Option<RemoteAddonDetails>,
    category: Option<Category>,
    model: Entity<AppModel>,
    focus: FocusHandle,
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
    let description_source = details
        .as_ref()
        .and_then(|details| {
            (!details.ui_description.trim().is_empty()).then_some(details.ui_description.as_str())
        })
        .or_else(|| (!addon.description.trim().is_empty()).then_some(addon.description.as_str()));
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
        .as_ref()
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
    let version_display = addon.version.trim_start_matches(['v', 'V']).to_owned();
    let aria_title = addon.title.clone();
    Modal::new(focus)
        .title("Installed dossier")
        .width(860.0)
        .sheet()
        .on_close(move |_, cx| {
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
                .role(Role::Group)
                .aria_label(format!("{aria_title} installed addon details"))
                .min_h_0()
                .flex_1()
                .overflow_y_scrollbar()
                .pr(px(5.0))
                .flex()
                .flex_col()
                .gap(px(16.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.0))
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(13.0))
                                .child(addon_artwork(
                                    thumbnail,
                                    category_icon.clone(),
                                    &category_name,
                                    56.0,
                                ))
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.0))
                                        .child(
                                            div()
                                                .min_w_0()
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .font_semibold()
                                                .text_size(px(17.0))
                                                .child(addon.title.clone()),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(5.0))
                                                .text_size(px(12.0))
                                                .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                                                .child(category_artwork(
                                                    category_icon,
                                                    &category_name,
                                                    15.0,
                                                ))
                                                .child(category_name)
                                                .child(format!(
                                                    "•  v{version_display}  •  {}",
                                                    update_state_label(&decision.update_state)
                                                )),
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
                                        .icon(IconName::CircleX)
                                        .on_activate(move |_, cx| {
                                            remove_model.update(cx, |app, cx| {
                                                app.pending_uninstall = vec![folder.clone()];
                                                cx.notify();
                                            });
                                        }),
                                ),
                        ),
                )
                .when(!screenshots.is_empty(), |column| {
                    column.child(details_screenshot_rail(&screenshots, model.clone()))
                })
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
                .when_some(description_source, |column, text| {
                    column.child(detail_rich_section(
                        "Description",
                        "local-description",
                        text,
                    ))
                })
                .child(
                    div()
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
                        .child(detail_fact("Folder", addon.folder_name.clone()))
                        .child(detail_fact_consolas("Installed path", addon.path.clone())),
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

pub(crate) fn detail_rich_section(
    title: &'static str,
    id_prefix: &'static str,
    text: &str,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(7.0))
        .child(
            div()
                .pb(px(6.0))
                .border_b_1()
                .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                .font_semibold()
                .text_size(px(12.0))
                .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
                .child(title),
        )
        .child(render_bbcode(id_prefix, text))
        .into_any_element()
}

pub(crate) fn detail_text_section(title: &'static str, text: String) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(7.0))
        .child(
            div()
                .pb(px(6.0))
                .border_b_1()
                .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                .font_semibold()
                .text_size(px(12.0))
                .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
                .child(title),
        )
        .child(
            div()
                .text_size(px(13.0))
                .line_height(relative(1.45))
                .text_color(gpui::rgb(SCRIBE_FOREGROUND))
                .child(text),
        )
        .into_any_element()
}

pub(crate) fn detail_fact(label: &'static str, value: String) -> gpui::AnyElement {
    detail_fact_styled(label, value, None)
}

pub(crate) fn detail_fact_consolas(label: &'static str, value: String) -> gpui::AnyElement {
    detail_fact_styled(label, value, Some("Consolas"))
}

fn detail_fact_styled(
    label: &'static str,
    value: String,
    font_family: Option<&'static str>,
) -> gpui::AnyElement {
    div()
        .pb(px(8.0))
        .border_b_1()
        .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                .child(label),
        )
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .font_medium()
                .text_size(px(13.0))
                .text_color(gpui::rgb(SCRIBE_FOREGROUND))
                .when_some(font_family, |value, family| value.font_family(family))
                .child(value),
        )
        .into_any_element()
}

pub(crate) fn render_lightbox(
    details: RemoteAddonDetails,
    index: usize,
    model: Entity<AppModel>,
    focus: FocusHandle,
    reduce_motion: bool,
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
    let lightbox = div()
        .id("screenshot-lightbox")
        .role(Role::Dialog)
        .aria_label(format!("Screenshot {} of {count}", index + 1))
        .track_focus(&focus)
        .focus_trap("scribe-lightbox", &focus)
        .absolute()
        .inset_0()
        .bg(gpui::rgb(SCRIBE_OVERLAY).opacity(0.75))
        .flex()
        .items_center()
        .justify_center()
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(
            div().absolute().top(px(16.0)).right(px(16.0)).child(
                NativeIconButton::new("close-lightbox", "Close screenshot", IconName::Close)
                    .overlay()
                    .on_activate(move |_, cx| {
                        close_model.update(cx, |app, cx| {
                            app.lightbox_index = None;
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
            div().absolute().left(px(18.0)).child(
                NativeIconButton::new(
                    "lightbox-previous",
                    "Previous screenshot",
                    IconName::ChevronLeft,
                )
                .overlay()
                .on_activate(move |_, cx| {
                    previous_model.update(cx, |app, cx| {
                        app.lightbox_index = Some(index.checked_sub(1).unwrap_or(count - 1));
                        cx.notify();
                    });
                }),
            ),
        )
        .child(
            div().absolute().right(px(18.0)).child(
                NativeIconButton::new("lightbox-next", "Next screenshot", IconName::ChevronRight)
                    .overlay()
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
                .top(px(16.0))
                .px(px(10.0))
                .py(px(5.0))
                .rounded(px(12.0))
                .border_1()
                .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                .bg(gpui::rgba(SCRIBE_SURFACE_RAISED_RGBA))
                .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                .text_size(px(12.0))
                .child(format!("{} / {count}", index + 1)),
        );
    if reduce_motion {
        lightbox.into_any_element()
    } else {
        lightbox
            .with_animation(
                "lightbox-enter",
                Animation::new(Duration::from_millis(SCRIBE_MOTION_FAST_MS)),
                |overlay, delta| overlay.opacity(0.82 + delta * 0.18),
            )
            .into_any_element()
    }
}

pub(crate) fn render_uninstall_modal(
    folders: Vec<String>,
    model: Entity<AppModel>,
    focus: FocusHandle,
) -> gpui::AnyElement {
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
    Modal::new(focus)
        .title(title)
        .width(420.0)
        .on_close(move |_, cx| {
            close_model.update(cx, |app, cx| {
                app.pending_uninstall.clear();
                cx.notify();
            });
        })
        .child(
            div()
                .id("uninstall-dialog")
                .role(Role::Group)
                .aria_label(format!("Confirm uninstall of {count} addon folders"))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(relative(1.45))
                        .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
                        .child(description),
                )
                .when(count > 1, |dialog| {
                    dialog.child(
                        div()
                            .max_h(px(150.0))
                            .overflow_y_scrollbar()
                            .px(px(10.0))
                            .py(px(8.0))
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                            .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
                            .text_size(px(11.0))
                            .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
                            .children(folders.iter().cloned().map(|folder| div().child(folder))),
                    )
                })
                .child(
                    Group::new()
                        .gap(Size::Small)
                        .child(
                            NativeButton::new("cancel-uninstall", "Cancel")
                                .ghost()
                                .on_activate(move |_, cx| {
                                    cancel_model.update(cx, |app, cx| {
                                        app.pending_uninstall.clear();
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            NativeButton::new("confirm-uninstall", "Uninstall")
                                .danger()
                                .icon(IconName::CircleX)
                                .on_activate(move |window, cx| {
                                    uninstall_named_folders(
                                        folders.clone(),
                                        confirm_model.clone(),
                                        window,
                                        cx,
                                    );
                                }),
                        ),
                ),
        )
        .into_any_element()
}

pub(crate) fn render_rebuild_modal(
    model: Entity<AppModel>,
    focus: FocusHandle,
) -> gpui::AnyElement {
    let close_model = model.clone();
    let cancel_model = model.clone();
    let confirm_model = model.clone();
    Modal::new(focus)
        .title("Rebuild local cache?")
        .width(420.0)
        .on_close(move |_, cx| {
            close_model.update(cx, |app, cx| {
                app.pending_rebuild = false;
                cx.notify();
            });
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(relative(1.45))
                        .text_color(gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA))
                        .child("Scribe will rebuild only reconstructible catalog and scanner cache data. Install records are migrated separately, and any unreadable database is retained instead of overwritten."),
                )
                .child(
                    div()
                        .p(px(10.0))
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(gpui::rgb(SCRIBE_WARNING).opacity(0.35))
                        .bg(gpui::rgb(SCRIBE_WARNING).opacity(0.1))
                        .text_color(gpui::rgb(SCRIBE_WARNING))
                        .text_size(px(11.0))
                        .child("Use this only when Local database is unavailable. A catalog refresh follows automatically."),
                )
                .child(
                    Group::new()
                        .gap(Size::Small)
                        .child(
                            NativeButton::new("cancel-rebuild", "Cancel")
                                .ghost()
                                .on_activate(move |_, cx| {
                                    cancel_model.update(cx, |app, cx| {
                                        app.pending_rebuild = false;
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            NativeButton::new("confirm-rebuild", "Rebuild cache")
                                .on_activate(move |window, cx| {
                                    confirm_model.update(cx, |app, cx| {
                                        app.pending_rebuild = false;
                                        app.health.recovery_phase = RecoveryPhase::Running;
                                        app.health.recovery_message =
                                            Some("Rebuild started.".into());
                                        cx.notify();
                                    });
                                    rebuild_local_storage(confirm_model.clone(), window, cx);
                                }),
                        ),
                ),
        )
        .into_any_element()
}

pub(crate) fn render_context_menu_overlay(
    menu: Entity<PopupMenu>,
    position: Point<Pixels>,
    viewport: gpui::Size<Pixels>,
) -> gpui::AnyElement {
    deferred(
        anchored().child(
            div()
                .id("scribe-context-menu-layer")
                .role(Role::Menu)
                .aria_label("Context actions")
                .w(viewport.width)
                .h(viewport.height)
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(
                    anchored()
                        .position(position)
                        .snap_to_window_with_margin(px(8.0))
                        .child(menu),
                ),
        ),
    )
    .with_priority(2)
    .into_any_element()
}

pub(crate) struct ContextMenuInvocation {
    position: Point<Pixels>,
    action_context: FocusHandle,
}

pub(crate) fn open_context_menu(
    owner: Entity<ScribeWindow>,
    invocation: ContextMenuInvocation,
    window: &mut Window,
    cx: &mut App,
    build: impl FnOnce(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
) {
    let ContextMenuInvocation {
        position,
        action_context,
    } = invocation;
    let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
        build(menu.action_context(action_context), window, cx)
    });
    let subscription = window.subscribe(&menu, cx, {
        let owner = owner.clone();
        move |_, _: &DismissEvent, window, cx| {
            owner.update(cx, |view, cx| {
                view.context_menu = None;
                view.context_menu_subscription = None;
                cx.notify();
            });
            window.refresh();
        }
    });
    menu.focus_handle(cx).focus(window, cx);
    owner.update(cx, |view, cx| {
        view.context_menu = Some((menu, position));
        view.context_menu_subscription = Some(subscription);
        cx.notify();
    });
}

pub(crate) fn claim_context_invoker(
    owner: &Entity<ScribeWindow>,
    key: String,
    position: Point<Pixels>,
    cx: &mut App,
) -> ContextMenuInvocation {
    let action_context = owner.update(cx, |view, cx| {
        view.context_invoker_key = Some(key);
        cx.notify();
        view.context_invoker_focus.clone()
    });
    ContextMenuInvocation {
        position,
        action_context,
    }
}

pub(crate) fn context_menu_key(event: &gpui::KeyDownEvent) -> bool {
    !event.is_held
        && ((event.keystroke.key == "f10" && event.keystroke.modifiers.shift)
            || event.keystroke.key == "context-menu")
}

pub(crate) fn menu_anchor(bounds: Bounds<Pixels>) -> Point<Pixels> {
    point(bounds.left() + px(16.0), bounds.bottom() - px(6.0))
}

pub(crate) fn open_category_context_menu(
    group_id: String,
    expanded: bool,
    owner: Entity<ScribeWindow>,
    invocation: ContextMenuInvocation,
    window: &mut Window,
    cx: &mut App,
) {
    open_context_menu(owner.clone(), invocation, window, cx, move |menu, _, _| {
        let toggle_owner = owner.clone();
        let toggle_id = group_id.clone();
        let expand_owner = owner.clone();
        let collapse_owner = owner.clone();
        menu.item(
            PopupMenuItem::new(if expanded {
                "Collapse category"
            } else {
                "Expand category"
            })
            .on_click(move |_, _, cx| {
                toggle_owner.update(cx, |view, cx| {
                    if !view.expanded_categories.remove(&toggle_id) {
                        view.expanded_categories.insert(toggle_id.clone());
                    }
                    cx.notify();
                });
            }),
        )
        .separator()
        .item(
            PopupMenuItem::new("Expand all categories").on_click(move |_, _, cx| {
                expand_owner.update(cx, |view, cx| {
                    let model = view.model.read(cx);
                    view.expanded_categories = installed_groups(model, "", false)
                        .into_iter()
                        .map(|group| group.id)
                        .collect();
                    cx.notify();
                });
            }),
        )
        .item(
            PopupMenuItem::new("Collapse all categories").on_click(move |_, _, cx| {
                collapse_owner.update(cx, |view, cx| {
                    view.expanded_categories.clear();
                    cx.notify();
                });
            }),
        )
    });
}

pub(crate) fn open_installed_context_menu(
    addon: Addon,
    decision: MatchedAddon,
    selected: Option<bool>,
    owner: Entity<ScribeWindow>,
    invocation: ContextMenuInvocation,
    window: &mut Window,
    cx: &mut App,
) {
    open_context_menu(owner.clone(), invocation, window, cx, move |menu, _, cx| {
        let model = owner.read(cx).model.clone();
        let detail_addon = addon.clone();
        let detail_decision = decision.clone();
        let detail_model = model.clone();
        let selection_owner = owner.clone();
        let selection_folder = addon.folder_name.clone();
        let remove_model = model.clone();
        let remove_folder = addon.folder_name.clone();
        let mut menu = menu.item(PopupMenuItem::new("Open details").on_click(
            move |_, window, cx| {
                show_installed_details(
                    detail_addon.clone(),
                    detail_decision.clone(),
                    detail_model.clone(),
                    window,
                    cx,
                );
            },
        ));

        if let Some(selected) = selected {
            menu = menu.item(PopupMenuItem::new("Selected").checked(selected).on_click(
                move |_, _, cx| {
                    selection_owner.update(cx, |view, cx| {
                        if !view.selected_folders.remove(&selection_folder) {
                            view.selected_folders.insert(selection_folder.clone());
                        }
                        cx.notify();
                    });
                },
            ));
        }

        if let Some(remote) = decision
            .remote
            .clone()
            .filter(|_| decision.update_available)
        {
            let update_model = model.clone();
            menu = menu.item(PopupMenuItem::new("Update").on_click(move |_, window, cx| {
                enqueue_remote(remote.clone(), update_model.clone(), window, cx);
            }));
        }

        if let Some(remote) = decision.remote.clone() {
            let website = remote.ui_file_info_url.to_string();
            if !website.is_empty() {
                menu = menu.item(
                    PopupMenuItem::new("Open ESOUI page")
                        .on_click(move |_, _, cx| cx.open_url(&website)),
                );
            }
        }

        menu.separator()
            .item(PopupMenuItem::new("Uninstall").on_click(move |_, _, cx| {
                remove_model.update(cx, |app, cx| {
                    app.pending_uninstall = vec![remove_folder.clone()];
                    cx.notify();
                });
            }))
    });
}

pub(crate) fn open_catalog_context_menu(
    addon: RemoteAddon,
    owner: Entity<ScribeWindow>,
    model: Entity<AppModel>,
    invocation: ContextMenuInvocation,
    window: &mut Window,
    cx: &mut App,
) {
    open_context_menu(owner, invocation, window, cx, move |menu, _, _| {
        let details_addon = addon.clone();
        let details_model = model.clone();
        let install_addon = addon.clone();
        let install_model = model.clone();
        let website = addon.ui_file_info_url.to_string();
        menu.item(
            PopupMenuItem::new("Open details").on_click(move |_, window, cx| {
                show_addon_details(details_addon.clone(), details_model.clone(), window, cx);
            }),
        )
        .item(
            PopupMenuItem::new("Install").on_click(move |_, window, cx| {
                enqueue_remote(install_addon.clone(), install_model.clone(), window, cx);
            }),
        )
        .when(!website.is_empty(), |menu| {
            menu.separator().item(
                PopupMenuItem::new("Open ESOUI page")
                    .on_click(move |_, _, cx| cx.open_url(&website)),
            )
        })
    });
}

pub(crate) fn task_state_label(state: TaskState) -> &'static str {
    match state {
        TaskState::Queued => "Queued",
        TaskState::Planning => "Planning",
        TaskState::Downloading => "Downloading",
        TaskState::Extracting => "Installing",
        TaskState::Complete => "Complete",
        TaskState::Failed => "Failed",
        TaskState::Cancelled => "Cancelled",
    }
}

pub(crate) fn task_tone(state: TaskState) -> NoticeTone {
    match state {
        TaskState::Complete => NoticeTone::Success,
        TaskState::Failed => NoticeTone::Danger,
        TaskState::Cancelled => NoticeTone::Warning,
        TaskState::Queued
        | TaskState::Planning
        | TaskState::Downloading
        | TaskState::Extracting => NoticeTone::Info,
    }
}

pub(crate) fn task_accessible_label(task: &TaskProgress) -> String {
    let mut label = format!(
        "{}, {}, {:.0} percent",
        task.name,
        task_state_label(task.state),
        task.percent
    );
    if !task.error.trim().is_empty() {
        label.push_str(". ");
        label.push_str(task.error.trim());
    }
    label
}

pub(crate) fn task_activity_summary(tasks: &[TaskProgress]) -> (NoticeTone, String, IconName) {
    let failed = tasks
        .iter()
        .filter(|task| task.state == TaskState::Failed)
        .count();
    let active = tasks
        .iter()
        .filter(|task| {
            matches!(
                task.state,
                TaskState::Queued
                    | TaskState::Planning
                    | TaskState::Downloading
                    | TaskState::Extracting
            )
        })
        .count();
    let cancelled = tasks
        .iter()
        .filter(|task| task.state == TaskState::Cancelled)
        .count();
    if failed > 0 {
        (
            NoticeTone::Danger,
            format!("{failed} failed"),
            IconName::TriangleAlert,
        )
    } else if active > 0 {
        (
            NoticeTone::Info,
            format!("{active} active"),
            IconName::LoaderCircle,
        )
    } else if cancelled > 0 {
        (
            NoticeTone::Warning,
            format!("{cancelled} cancelled"),
            IconName::CircleX,
        )
    } else {
        (
            NoticeTone::Success,
            format!("{} complete", tasks.len()),
            IconName::CircleCheck,
        )
    }
}

pub(crate) fn task_activity_relevant(task: &TaskProgress) -> bool {
    matches!(
        task.state,
        TaskState::Queued
            | TaskState::Planning
            | TaskState::Downloading
            | TaskState::Extracting
            | TaskState::Complete
            | TaskState::Failed
            | TaskState::Cancelled
    )
}

pub(crate) fn task_state_is_terminal(state: TaskState) -> bool {
    matches!(
        state,
        TaskState::Complete | TaskState::Failed | TaskState::Cancelled
    )
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn task_progress_counters(task: &TaskProgress) -> String {
    if task.total_bytes > 0 {
        format!(
            "{} of {}",
            format_bytes(task.bytes_downloaded),
            format_bytes(task.total_bytes)
        )
    } else if task.total_files > 0 {
        format!("{} of {} files", task.files_extracted, task.total_files)
    } else {
        String::new()
    }
}

pub(crate) fn render_task_activity(
    tasks: Vec<TaskProgress>,
    theme: &Theme,
    model: Entity<AppModel>,
    open: bool,
    window: Entity<ScribeWindow>,
) -> gpui::AnyElement {
    let task_count = tasks.len();
    let (summary_tone, summary, summary_icon) = task_activity_summary(&tasks);
    let (summary_color, _) = notice_visuals(summary_tone, theme);
    let toggle_window = window.clone();
    let toggle_keyboard_window = window.clone();
    let close_window = window.clone();
    let dismiss_window = window;
    div()
        .id("task-activity-layer")
        .absolute()
        .right(px(SCRIBE_CONTENT_GUTTER))
        .bottom(px(SCRIBE_CONTENT_GUTTER))
        .w(px(380.0))
        .flex()
        .flex_col()
        .items_end()
        .gap(px(8.0))
        .when(open, |activity| {
            activity.child(
                div()
                    .id("task-center")
                    .role(Role::Group)
                    .aria_label(format!("Activity, {task_count} tasks"))
                    .w_full()
                    .max_h(px(320.0))
                    .p(px(12.0))
                    .rounded(px(SCRIBE_CARD_RADIUS))
                    .border_1()
                    .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                    .bg(gpui::rgba(SCRIBE_SURFACE_RAISED_RGBA))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        h_flex()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        Icon::new(summary_icon.clone())
                                            .size(px(15.0))
                                            .text_color(summary_color),
                                    )
                                    .child(Title::new("Activity").order(5))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                                            .child(summary.clone()),
                                    ),
                            )
                            .child(
                                NativeIconButton::new(
                                    "close-task-activity",
                                    "Close activity",
                                    IconName::Close,
                                )
                                .on_activate(move |_, cx| {
                                    close_window.update(cx, |view, cx| {
                                        view.task_center_open = false;
                                        cx.notify();
                                    });
                                }),
                            ),
                    )
                    .children(tasks.into_iter().rev().take(4).map(|task| {
                        let uid = task.uid.clone();
                        let cancel_model = model.clone();
                        let retry_model = model.clone();
                        let retry_uid = uid.clone();
                        let dismiss_owner = dismiss_window.clone();
                        let dismiss_uid = uid.clone();
                        let tone = task_tone(task.state);
                        let (color, icon) = notice_visuals(tone, theme);
                        let state = task_state_label(task.state);
                        let accessible_label = task_accessible_label(&task);
                        let error = task.error.trim().to_string();
                        let counters = task_progress_counters(&task);
                        let can_cancel = matches!(
                            task.state,
                            TaskState::Queued
                                | TaskState::Planning
                                | TaskState::Downloading
                                | TaskState::Extracting
                        );
                        let can_retry = task.state == TaskState::Failed;
                        let can_dismiss = task_state_is_terminal(task.state);
                        let fill = ((task.percent / 100.0).clamp(0.0, 1.0)) as f32;
                        div()
                            .id(format!("task-status-{uid}"))
                            .role(if tone == NoticeTone::Danger {
                                Role::Alert
                            } else {
                                Role::Status
                            })
                            .aria_label(accessible_label)
                            .px(px(8.0))
                            .py(px(6.0))
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                            .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
                            .flex()
                            .flex_col()
                            .gap(px(5.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(7.0))
                                    .child(Icon::new(icon).size(px(14.0)).text_color(color))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap(px(1.0))
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .flex()
                                                    .items_baseline()
                                                    .gap(px(8.0))
                                                    .child(
                                                        div()
                                                            .min_w_0()
                                                            .flex_1()
                                                            .overflow_hidden()
                                                            .whitespace_nowrap()
                                                            .text_ellipsis()
                                                            .font_medium()
                                                            .text_size(px(13.0))
                                                            .child(task.name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_none()
                                                            .text_size(px(11.0))
                                                            .font_medium()
                                                            .text_color(color)
                                                            .child(format!(
                                                                "{state} · {:.0}%",
                                                                task.percent
                                                            )),
                                                    ),
                                            )
                                            .when(!error.is_empty(), |column| {
                                                column.child(
                                                    div()
                                                        .min_w_0()
                                                        .overflow_hidden()
                                                        .whitespace_nowrap()
                                                        .text_ellipsis()
                                                        .text_size(px(11.0))
                                                        .text_color(color)
                                                        .child(error),
                                                )
                                            })
                                            .when(!counters.is_empty(), |column| {
                                                column.child(
                                                    div()
                                                        .min_w_0()
                                                        .overflow_hidden()
                                                        .whitespace_nowrap()
                                                        .text_ellipsis()
                                                        .text_size(px(11.0))
                                                        .text_color(gpui::rgba(
                                                            SCRIBE_TEXT_TERTIARY_RGBA,
                                                        ))
                                                        .child(counters),
                                                )
                                            }),
                                    )
                                    .child(
                                        div().flex_none().flex().items_center().child(
                                            Group::new()
                                                .gap(Size::XSmall)
                                                .when(can_retry, |group| {
                                                    group.child(
                                                        NativeIconButton::new(
                                                            format!("retry-task-{retry_uid}"),
                                                            "Retry",
                                                            IconName::LoaderCircle,
                                                        )
                                                        .on_activate(move |window, cx| {
                                                            let remote = retry_model
                                                                .read(cx)
                                                                .catalog_index
                                                                .by_uid(&retry_uid);
                                                            if let Some(remote) = remote {
                                                                enqueue_remote(
                                                                    remote,
                                                                    retry_model.clone(),
                                                                    window,
                                                                    cx,
                                                                );
                                                            } else {
                                                                retry_model.update(cx, |app, cx| {
                                                                    app.status = "This addon is no longer in the current ESOUI catalog. Refresh the catalog before retrying.".into();
                                                                    cx.notify();
                                                                });
                                                            }
                                                        }),
                                                    )
                                                })
                                                .when(can_cancel, |group| {
                                                    group.child(
                                                        NativeIconButton::new(
                                                            format!("cancel-task-{uid}"),
                                                            "Cancel",
                                                            IconName::CircleX,
                                                        )
                                                        .on_activate(move |_, cx| {
                                                            if let Some(manager) =
                                                                &cancel_model.read(cx).install_manager
                                                            {
                                                                manager.cancel(&uid);
                                                            }
                                                        }),
                                                    )
                                                })
                                                .when(can_dismiss, |group| {
                                                    group.child(
                                                        NativeIconButton::new(
                                                            format!("dismiss-task-{dismiss_uid}"),
                                                            "Dismiss",
                                                            IconName::Close,
                                                        )
                                                        .on_activate(move |_, cx| {
                                                            dismiss_owner.update(cx, |view, cx| {
                                                                view.dismissed_task_uids
                                                                    .insert(dismiss_uid.clone());
                                                                cx.notify();
                                                            });
                                                        }),
                                                    )
                                                }),
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .h(px(3.0))
                                    .w_full()
                                    .rounded(px(1.5))
                                    .bg(gpui::rgba(SCRIBE_SURFACE_ACTIVE_RGBA))
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(fill))
                                            .rounded(px(1.5))
                                            .bg(color),
                                    ),
                            )
                    })),
            )
        })
        .child(
            div()
                .id("toggle-task-activity")
                .focusable()
                .tab_stop(true)
                .role(Role::Button)
                .aria_label(format!("Activity, {summary}"))
                .cursor_pointer()
                .h(px(36.0))
                .px(px(14.0))
                .rounded(px(18.0))
                .border_1()
                .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                .bg(gpui::rgba(SCRIBE_SURFACE_RAISED_RGBA))
                .shadow_lg()
                .flex()
                .items_center()
                .gap(px(8.0))
                .hover(|pill| pill.border_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA)))
                .focus(|pill| {
                    pill.border_color(gpui::rgba(SCRIBE_FOCUS_RING_RGBA))
                })
                .on_click(move |_, _, cx| {
                    toggle_window.update(cx, |view, cx| {
                        view.task_center_open = !view.task_center_open;
                        cx.notify();
                    });
                })
                .on_key_down(move |event, _, cx| {
                    if !event.is_held && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        toggle_keyboard_window.update(cx, |view, cx| {
                            view.task_center_open = !view.task_center_open;
                            cx.notify();
                        });
                    }
                })
                .child(
                    Icon::new(summary_icon)
                        .size(px(15.0))
                        .text_color(summary_color),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_medium()
                        .child(summary.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                        .child(task_count.to_string()),
                ),
        )
        .into_any_element()
}
