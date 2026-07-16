//! Native application core for Scribe.

pub mod catalog;
pub mod client;
pub mod install_manager;
pub mod installer;
pub mod matcher;
pub mod models;
#[cfg(all(feature = "rkyv-bench", test))]
mod rkyv_gate;
pub mod scanner;
pub mod settings;
pub mod storage;

#[cfg(feature = "rkyv-catalog")]
pub use archive::CatalogArchive;
pub use catalog::{Catalog, CatalogIndex, CatalogSort, InstalledIndex, latest_compatibility};
pub use client::{CancellationToken, CatalogService, EsouiClient};
pub use install_manager::{InstallManager, InstallRequest, TaskProgress, TaskState};
pub use installer::{CleanupReport, InstallPlanEntry, Installer};
pub use matcher::{Matcher, UpdateState};
pub use models::*;
pub use scanner::Scanner;
pub use settings::{AppSettings, SettingsManager, app_config_directory};
pub use storage::{CacheLoad, RebuildOutcome, SaveOutcome, Storage};
#[cfg(feature = "rkyv-catalog")]
mod archive;
