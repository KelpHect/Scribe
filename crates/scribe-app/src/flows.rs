use std::path::PathBuf;
use std::sync::Arc;

use gpui::{App, Entity, PathPromptOptions, Window};
use scribe_core::{
    Addon, CancellationToken, Catalog, CatalogService, EsouiClient, InstallManager, InstallRequest,
    Installer, MatchedAddon, Matcher, RemoteAddon, Scanner, SettingsManager, Storage,
};

use crate::model::{AppModel, Page, RecoveryPhase, replace_catalog_state, replace_installed_state};
use crate::theme::apply_scribe_theme;
use crate::unix_now;

pub(crate) fn enqueue_remote(
    remote: RemoteAddon,
    model: Entity<AppModel>,
    window: &mut Window,
    cx: &mut App,
) {
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

pub(crate) fn show_addon_details(
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
    load_remote_details(remote, Page::FindMore, model, window, cx);
}

pub(crate) fn show_installed_details(
    addon: Addon,
    decision: MatchedAddon,
    model: Entity<AppModel>,
    window: &mut Window,
    cx: &mut App,
) {
    let remote = decision.remote.clone();
    let expected_page = model.read(cx).page;
    model.update(cx, |app, cx| {
        app.selected_local = Some((addon.clone(), decision));
        app.selected_details = None;
        app.lightbox_index = None;
        app.status = format!("Viewing {}.", addon.title);
        cx.notify();
    });
    if let Some(remote) = remote {
        load_remote_details(remote, expected_page, model, window, cx);
    }
}

pub(crate) fn load_remote_details(
    remote: RemoteAddon,
    expected_page: Page,
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
        app.details_loading = true;
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
                    app.details_loading = false;
                    if app.page != expected_page {
                        if app.status.starts_with("Loading details for ") {
                            app.status.clear();
                        }
                        cx.notify();
                        return;
                    }
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
                    app.details_loading = false;
                    if app.page != expected_page {
                        if app.status.starts_with("Loading details for ") {
                            app.status.clear();
                        }
                        cx.notify();
                        return;
                    }
                    app.status = format!("Could not load {} details: {error}", remote.ui_name);
                    cx.notify();
                });
            }
        })
        .detach();
}

pub(crate) fn enqueue_dependency_uids(
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

pub(crate) fn uninstall_named_folders(
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

pub(crate) fn rebuild_local_storage(model: Entity<AppModel>, window: &mut Window, cx: &mut App) {
    model.update(cx, |app, cx| {
        app.status = "Rebuilding reconstructible local cache data…".into();
        app.health.recovery_phase = RecoveryPhase::Running;
        app.health.recovery_message = Some("Rebuilding reconstructible cache data.".into());
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
                    app.health.storage_issue = None;
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
                            app.health.catalog_issue = None;
                            app.health.last_catalog_success = Some(unix_now());
                            app.health.recovery_phase = RecoveryPhase::Succeeded;
                            app.health.recovery_message = Some(
                                "Local cache rebuilt and the ESOUI catalog refreshed successfully."
                                    .into(),
                            );
                            app.status = format!("Local cache rebuilt and refreshed ({outcome:?}).");
                            cx.notify();
                        });
                    }
                    Err(error) => {
                        model.update(cx, |app, cx| {
                            app.health.catalog_issue = Some(error.to_string());
                            app.health.recovery_phase = RecoveryPhase::Failed;
                            app.health.recovery_message = Some(format!(
                                "Cache rebuilt, but the catalog refresh failed: {error}"
                            ));
                            app.status =
                                format!("Local cache rebuilt, but ESOUI refresh failed: {error}");
                            cx.notify();
                        });
                    }
                }
            }
            Err(error) => {
                model.update(cx, |app, cx| {
                    app.health.storage_issue = Some(error.to_string());
                    app.health.recovery_phase = RecoveryPhase::Failed;
                    app.health.recovery_message =
                        Some(format!("Local cache rebuild failed: {error}"));
                    app.status = format!("Local cache rebuild failed: {error}");
                    cx.notify();
                });
            }
        })
        .detach();
}

pub(crate) fn set_app_theme(_theme: &str, model: Entity<AppModel>, cx: &mut App) {
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

pub(crate) fn set_background_alerts(enabled: bool, model: Entity<AppModel>, cx: &mut App) {
    let settings = model.update(cx, |app, cx| {
        app.settings.background_alerts = enabled;
        app.status = if enabled {
            "Background update alerts enabled.".into()
        } else {
            "Background update alerts disabled.".into()
        };
        cx.notify();
        app.settings.clone()
    });
    cx.background_executor()
        .spawn(async move {
            if let Ok(manager) = SettingsManager::new() {
                let _ = manager.save(&settings);
            }
        })
        .detach();
}

/// Records a committed Find More query in the recent-searches list (distinct,
/// most-recent-first, capped at 8) and persists it in the background.
pub(crate) fn commit_recent_search(query: &str, model: Entity<AppModel>, cx: &mut App) {
    let settings = model.update(cx, |app, cx| {
        crate::model::push_recent_search(&mut app.settings.recent_searches, query);
        cx.notify();
        app.settings.clone()
    });
    cx.background_executor()
        .spawn(async move {
            if let Ok(manager) = SettingsManager::new() {
                let _ = manager.save(&settings);
            }
        })
        .detach();
}

pub(crate) fn browse_for_addons(model: Entity<AppModel>, window: &mut Window, cx: &mut App) {
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
                        app.health.scan_issue = Some(error.clone());
                        app.status = format!("Could not save or scan the AddOns folder: {error}");
                        cx.notify();
                    });
                }
            }
            Some(())
        })
        .detach();
}

pub(crate) fn refresh_catalog(model: Entity<AppModel>, _window: &mut Window, cx: &mut App) {
    refresh_catalog_now(model, cx);
}

/// Windowless catalog refresh shared by the toolbar action, the periodic
/// freshness loop, and the tray "Check for updates now" command. All network
/// and storage I/O stays on the background executor.
pub(crate) fn refresh_catalog_now(model: Entity<AppModel>, cx: &mut App) {
    let service = model.read(cx).catalog_service.clone();
    let Some(service) = service else {
        model.update(cx, |app, cx| {
            app.health.storage_issue = Some("Local storage is unavailable.".into());
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
    model.update(cx, |_, cx| {
        cx.spawn(async move |model, cx| match refresh.await {
            Ok((catalog, outcome)) => {
                model
                    .update(cx, |app, cx| {
                        replace_catalog_state(app, catalog);
                        app.health.catalog_issue = None;
                        app.health.last_catalog_success = Some(unix_now());
                        app.status = format!("ESOUI catalog refreshed ({outcome:?}).");
                        cx.notify();
                    })
                    .ok();
            }
            Err(error) => {
                model
                    .update(cx, |app, cx| {
                        app.health.catalog_issue = Some(error.to_string());
                        let fallback = if app.catalog_index.is_empty() {
                            "No saved catalog is available in this profile."
                        } else {
                            "The saved catalog remains available."
                        };
                        app.status = format!("ESOUI refresh failed: {error}. {fallback}");
                        cx.notify();
                    })
                    .ok();
            }
        })
        .detach();
    });
}

pub(crate) fn rescan_configured_addons(model: Entity<AppModel>, window: &mut Window, cx: &mut App) {
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
                    app.health.scan_issue = Some(error.to_string());
                    app.status = format!("AddOns rescan failed: {error}");
                    cx.notify();
                });
            }
        })
        .detach();
}

pub(crate) async fn enrich_md5_decisions(
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
