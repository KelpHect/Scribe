use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use atomicwrites::{AllowOverwrite, AtomicFile};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub addon_path: String,
    #[serde(default)]
    pub auto_update: bool,
    #[serde(default = "default_memory_limit")]
    pub memory_limit_mb: u32,
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            addon_path: String::new(),
            auto_update: false,
            memory_limit_mb: default_memory_limit(),
            theme: default_theme(),
        }
    }
}

fn default_memory_limit() -> u32 {
    150
}

fn default_theme() -> String {
    "scribe".into()
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("the settings directory is unavailable")]
    ConfigDirectoryUnavailable,
    #[error("addon path must be absolute")]
    AddonPathNotAbsolute,
    #[error("addon path contains a null character")]
    InvalidAddonPath,
    #[error("settings I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("settings TOML is invalid: {0}")]
    Decode(#[from] toml::de::Error),
    #[error("settings TOML could not be encoded: {0}")]
    Encode(#[from] toml::ser::Error),
}

#[derive(Clone, Debug)]
pub struct SettingsManager {
    path: PathBuf,
}

impl SettingsManager {
    pub fn new() -> Result<Self, SettingsError> {
        Ok(Self::at(
            app_config_directory()
                .ok_or(SettingsError::ConfigDirectoryUnavailable)?
                .join("settings.toml"),
        ))
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<AppSettings, SettingsError> {
        let data = match fs::read_to_string(&self.path) {
            Ok(data) => data,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(AppSettings::default());
            }
            Err(error) => return Err(error.into()),
        };
        normalize(toml::from_str(&data)?)
    }

    pub fn save(&self, settings: &AppSettings) -> Result<(), SettingsError> {
        let settings = normalize(settings.clone())?;
        let data = toml::to_string_pretty(&settings)?;
        let parent = self
            .path
            .parent()
            .ok_or(SettingsError::ConfigDirectoryUnavailable)?;
        fs::create_dir_all(parent)?;
        AtomicFile::new(&self.path, AllowOverwrite)
            .write(|file| {
                file.write_all(data.as_bytes())?;
                file.sync_all()
            })
            .map_err(std::io::Error::from)?;
        Ok(())
    }
}

fn normalize(mut settings: AppSettings) -> Result<AppSettings, SettingsError> {
    settings.auto_update = false;
    settings.theme = match settings.theme.as_str() {
        "scribe" | "neutral" | "dark" => settings.theme,
        _ => default_theme(),
    };
    let trimmed = settings.addon_path.trim();
    if trimmed.contains('\0') {
        return Err(SettingsError::InvalidAddonPath);
    }
    if !trimmed.is_empty() {
        let path = Path::new(trimmed);
        if !path.is_absolute() {
            return Err(SettingsError::AddonPathNotAbsolute);
        }
        settings.addon_path = path.to_string_lossy().into_owned();
    } else {
        settings.addon_path.clear();
    }
    Ok(settings)
}

pub fn app_config_directory() -> Option<PathBuf> {
    if cfg!(windows) {
        env::var_os("APPDATA").map(|path| PathBuf::from(path).join("Scribe"))
    } else if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        Some(PathBuf::from(path).join("Scribe"))
    } else {
        env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join("Scribe"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_historical_toml_keys_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let manager = SettingsManager::at(temp.path().join("Scribe/settings.toml"));
        let settings = AppSettings {
            addon_path: temp.path().to_string_lossy().into_owned(),
            auto_update: true,
            memory_limit_mb: 192,
            theme: "dark".into(),
        };
        manager.save(&settings).unwrap();
        let loaded = manager.load().unwrap();
        assert!(!loaded.auto_update);
        assert_eq!(loaded.memory_limit_mb, 192);
        assert_eq!(loaded.theme, "dark");
    }

    #[test]
    fn rejects_relative_addon_path() {
        let manager = SettingsManager::at("ignored.toml");
        let result = manager.save(&AppSettings {
            addon_path: "relative/AddOns".into(),
            ..AppSettings::default()
        });
        assert!(matches!(result, Err(SettingsError::AddonPathNotAbsolute)));
    }
}
