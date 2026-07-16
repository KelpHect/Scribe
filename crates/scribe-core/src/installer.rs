use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use zip::ZipArchive;

use crate::CancellationToken;

const MAX_ENTRY_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlanEntry {
    pub folder_name: String,
    pub action: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub removed: Vec<String>,
    pub retained: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("install cancelled")]
    Cancelled,
    #[error("invalid AddOns directory: {0}")]
    InvalidDestination(String),
    #[error("invalid addon folder name: {0:?}")]
    InvalidFolder(String),
    #[error("archive error: {0}")]
    Archive(#[from] zip::result::ZipError),
    #[error("filesystem operation failed: {0}")]
    Io(#[from] io::Error),
    #[error("zip entry escapes destination: {0}")]
    EscapingEntry(String),
    #[error("archive contains a symbolic link: {0}")]
    Symlink(String),
    #[error("archive entry is too large: {0}")]
    OversizedEntry(String),
    #[error("archive expands beyond the 2 GiB safety limit")]
    OversizedArchive,
    #[error("archive contains root file outside addon folder: {0}")]
    RootFile(String),
    #[error("archive contains no addon folders")]
    EmptyArchive,
    #[error("addon folder {0} has no canonical manifest")]
    MissingManifest(String),
    #[error("archive folder {0} is not listed by ESOUI metadata")]
    UnexpectedFolder(String),
    #[error("staged addon folder is missing: {0}")]
    MissingStagedFolder(String),
    #[error("failed to commit {message}; rollback also failed: {rollback}")]
    Rollback { message: String, rollback: String },
}

pub struct Installer;

struct CommittedMove {
    destination: PathBuf,
    backup: Option<PathBuf>,
    installed: bool,
}

impl Installer {
    pub fn plan_archive(
        archive_path: impl AsRef<Path>,
        addon_path: impl AsRef<Path>,
        expected_directories: &[String],
    ) -> Result<Vec<InstallPlanEntry>, InstallError> {
        let addon_path = validate_destination(addon_path.as_ref())?;
        let expected: BTreeSet<String> = expected_directories
            .iter()
            .map(|folder| folder.trim().to_lowercase())
            .filter(|folder| !folder.is_empty())
            .collect();
        let mut archive = ZipArchive::new(File::open(archive_path)?)?;
        plan_open_archive(&mut archive, &addon_path, &expected)
    }

    pub fn install_archive(
        archive_path: impl AsRef<Path>,
        addon_path: impl AsRef<Path>,
        expected_directories: &[String],
        cancel: &CancellationToken,
        mut progress: impl FnMut(usize, usize),
    ) -> Result<Vec<InstallPlanEntry>, InstallError> {
        let archive_path = archive_path.as_ref();
        let addon_path = validate_destination(addon_path.as_ref())?;
        let expected: BTreeSet<String> = expected_directories
            .iter()
            .map(|folder| folder.trim().to_lowercase())
            .filter(|folder| !folder.is_empty())
            .collect();
        let mut archive = ZipArchive::new(File::open(archive_path)?)?;
        let plan = plan_open_archive(&mut archive, &addon_path, &expected)?;
        let staging = tempfile::Builder::new()
            .prefix(".scribe-staging-")
            .tempdir_in(&addon_path)?;
        extract_archive(&mut archive, staging.path(), cancel, &mut progress)?;
        if cancel.is_cancelled() {
            return Err(InstallError::Cancelled);
        }
        commit_staging(staging.path(), &addon_path, &plan)?;
        Ok(plan)
    }

    pub fn uninstall(addon_path: impl AsRef<Path>, folder_name: &str) -> Result<(), InstallError> {
        validate_folder_name(folder_name)?;
        let addon_path = validate_destination(addon_path.as_ref())?;
        let target = addon_path.join(folder_name);
        let metadata = fs::symlink_metadata(&target).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                InstallError::InvalidDestination(format!("addon folder not found: {folder_name}"))
            } else {
                error.into()
            }
        })?;
        if metadata.file_type().is_symlink() {
            if metadata.is_dir() {
                fs::remove_dir(&target)?;
            } else {
                fs::remove_file(&target)?;
            }
        } else if metadata.is_dir() {
            fs::remove_dir_all(&target)?;
        } else {
            return Err(InstallError::InvalidDestination(format!(
                "addon target is not a directory: {folder_name}"
            )));
        }
        Ok(())
    }

    pub fn clean_stale_artifacts(
        addon_path: impl AsRef<Path>,
        older_than: Duration,
    ) -> CleanupReport {
        let mut report = CleanupReport::default();
        let addon_path = match validate_destination(addon_path.as_ref()) {
            Ok(path) => path,
            Err(error) => {
                report.errors.push(error.to_string());
                return report;
            }
        };
        let threshold = SystemTime::now()
            .checked_sub(older_than)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let entries = match fs::read_dir(&addon_path) {
            Ok(entries) => entries,
            Err(error) => {
                report.errors.push(error.to_string());
                return report;
            }
        };

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_scribe_artifact(&name) {
                continue;
            }
            let result = (|| -> Result<bool, io::Error> {
                let file_type = entry.file_type()?;
                if !file_type.is_dir() || file_type.is_symlink() {
                    return Ok(false);
                }
                let modified = entry.metadata()?.modified()?;
                if modified > threshold {
                    return Ok(false);
                }
                fs::remove_dir_all(entry.path())?;
                Ok(true)
            })();
            match result {
                Ok(true) => report.removed.push(name),
                Ok(false) => report.retained.push(name),
                Err(error) => report.errors.push(format!("{name}: {error}")),
            }
        }
        report.removed.sort();
        report.retained.sort();
        report.errors.sort();
        report
    }
}

fn plan_open_archive(
    archive: &mut ZipArchive<File>,
    addon_path: &Path,
    expected: &BTreeSet<String>,
) -> Result<Vec<InstallPlanEntry>, InstallError> {
    let folders = inspect_archive(archive)?;
    let mut plan = Vec::with_capacity(folders.len());
    for (folder, has_manifest) in folders {
        if !has_manifest {
            return Err(InstallError::MissingManifest(folder));
        }
        if !expected.is_empty() && !expected.contains(&folder.to_lowercase()) {
            return Err(InstallError::UnexpectedFolder(folder));
        }
        let installed = addon_path.join(&folder);
        let (action, reason) = match fs::symlink_metadata(&installed) {
            Ok(metadata) if metadata.is_dir() => ("replace", "folder already exists"),
            Ok(_) => {
                return Err(InstallError::InvalidDestination(format!(
                    "{} exists but is not a directory",
                    installed.display()
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                ("add", "folder is not installed")
            }
            Err(error) => return Err(error.into()),
        };
        plan.push(InstallPlanEntry {
            folder_name: folder,
            action: action.into(),
            reason: reason.into(),
        });
    }
    plan.sort_by_key(|entry| entry.folder_name.to_lowercase());
    Ok(plan)
}

fn inspect_archive(archive: &mut ZipArchive<File>) -> Result<BTreeMap<String, bool>, InstallError> {
    let mut folders = BTreeMap::new();
    let mut total_size = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        validate_zip_entry(entry.name(), entry.unix_mode(), entry.size())?;
        total_size = total_size
            .checked_add(entry.size())
            .ok_or(InstallError::OversizedArchive)?;
        if total_size > MAX_ARCHIVE_BYTES {
            return Err(InstallError::OversizedArchive);
        }
        let mut components = entry.name().split('/').filter(|part| !part.is_empty());
        let Some(folder) = components.next() else {
            continue;
        };
        let child = components.next();
        let has_grandchild = components.next().is_some();
        if child.is_none() && !entry.is_dir() {
            return Err(InstallError::RootFile(entry.name().into()));
        }
        validate_folder_name(folder)?;
        let has_manifest = folders.entry(folder.to_owned()).or_insert(false);
        if !has_grandchild && let Some(entry_name) = child {
            *has_manifest |= is_canonical_manifest(entry_name, folder);
        }
    }
    if folders.is_empty() {
        return Err(InstallError::EmptyArchive);
    }
    Ok(folders)
}

fn is_canonical_manifest(entry_name: &str, folder: &str) -> bool {
    [".txt", ".addon"].iter().any(|extension| {
        entry_name
            .get(..entry_name.len().saturating_sub(extension.len()))
            .is_some_and(|stem| {
                stem.eq_ignore_ascii_case(folder)
                    && entry_name[stem.len()..].eq_ignore_ascii_case(extension)
            })
    })
}

fn extract_archive(
    archive: &mut ZipArchive<File>,
    staging: &Path,
    cancel: &CancellationToken,
    progress: &mut impl FnMut(usize, usize),
) -> Result<(), InstallError> {
    let total = archive.len();
    let mut created_directories = HashSet::new();
    for index in 0..total {
        if cancel.is_cancelled() {
            return Err(InstallError::Cancelled);
        }
        let entry = archive.by_index(index)?;
        validate_zip_entry(entry.name(), entry.unix_mode(), entry.size())?;
        let relative = safe_relative_zip_path(entry.name())?;
        let destination = staging.join(relative);
        if entry.is_dir() {
            if created_directories.insert(destination.clone()) {
                fs::create_dir_all(&destination)?;
            }
        } else {
            if let Some(parent) = destination.parent()
                && created_directories.insert(parent.to_owned())
            {
                fs::create_dir_all(parent)?;
            }
            let mut output = File::create(&destination)?;
            io::copy(&mut entry.take(MAX_ENTRY_BYTES + 1), &mut output)?;
        }
        progress(index + 1, total);
    }
    Ok(())
}

fn commit_staging(
    staging: &Path,
    addon_path: &Path,
    plan: &[InstallPlanEntry],
) -> Result<(), InstallError> {
    let backup = tempfile::Builder::new()
        .prefix(".scribe-backup-")
        .tempdir_in(addon_path)?;
    let mut moved: Vec<CommittedMove> = Vec::new();

    for item in plan {
        let source = staging.join(&item.folder_name);
        let destination = addon_path.join(&item.folder_name);
        if !source.is_dir() {
            let error = InstallError::MissingStagedFolder(item.folder_name.clone());
            if let Err(rollback) = rollback(&moved) {
                let _preserved_backup = backup.keep();
                return Err(InstallError::Rollback {
                    message: error.to_string(),
                    rollback,
                });
            }
            return Err(error);
        }

        let mut action = CommittedMove {
            destination: destination.clone(),
            backup: None,
            installed: false,
        };
        if item.action == "replace" {
            let backup_path = backup.path().join(&item.folder_name);
            if let Err(error) = fs::rename(&destination, &backup_path) {
                let message = format!("backup {}: {error}", item.folder_name);
                if let Err(rollback) = rollback(&moved) {
                    let _preserved_backup = backup.keep();
                    return Err(InstallError::Rollback { message, rollback });
                }
                return Err(InstallError::Io(error));
            }
            action.backup = Some(backup_path);
        }
        if let Err(error) = fs::rename(&source, &destination) {
            moved.push(action);
            let message = format!("install {}: {error}", item.folder_name);
            if let Err(rollback) = rollback(&moved) {
                let _preserved_backup = backup.keep();
                return Err(InstallError::Rollback { message, rollback });
            }
            return Err(InstallError::Io(error));
        }
        action.installed = true;
        moved.push(action);
    }
    Ok(())
}

fn rollback(moved: &[CommittedMove]) -> Result<(), String> {
    let mut errors = Vec::new();
    for item in moved.iter().rev() {
        if item.installed
            && let Err(error) = fs::remove_dir_all(&item.destination)
        {
            errors.push(error.to_string());
        }
        if let Some(backup) = &item.backup
            && let Err(error) = fs::rename(backup, &item.destination)
        {
            errors.push(error.to_string());
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn validate_destination(path: &Path) -> Result<PathBuf, InstallError> {
    if !path.is_absolute() {
        return Err(InstallError::InvalidDestination(
            "path must be absolute".into(),
        ));
    }
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(path.to_path_buf()),
        Ok(_) => Err(InstallError::InvalidDestination(
            "path is not a directory".into(),
        )),
        Err(error) => Err(error.into()),
    }
}

fn validate_zip_entry(name: &str, unix_mode: Option<u32>, size: u64) -> Result<(), InstallError> {
    let _ = safe_relative_zip_path(name)?;
    if unix_mode.is_some_and(|mode| mode & 0o170000 == 0o120000) {
        return Err(InstallError::Symlink(name.into()));
    }
    if size > MAX_ENTRY_BYTES {
        return Err(InstallError::OversizedEntry(name.into()));
    }
    Ok(())
}

fn safe_relative_zip_path(name: &str) -> Result<PathBuf, InstallError> {
    if name.contains('\\') || name.starts_with('/') || has_windows_volume_prefix(name) {
        return Err(InstallError::EscapingEntry(name.into()));
    }
    let path = Path::new(name);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(InstallError::EscapingEntry(name.into()));
    }
    Ok(path.to_path_buf())
}

fn validate_folder_name(folder: &str) -> Result<(), InstallError> {
    let trimmed = folder.trim();
    let uppercase = trimmed.trim_end_matches(['.', ' ']).to_ascii_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed != folder
        || trimmed.ends_with(['.', ' '])
        || trimmed.contains(['/', '\\', ':'])
        || reserved.contains(&uppercase.as_str())
        || has_windows_volume_prefix(trimmed)
    {
        return Err(InstallError::InvalidFolder(folder.into()));
    }
    Ok(())
}

fn has_windows_volume_prefix(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn is_scribe_artifact(name: &str) -> bool {
    name.starts_with(".scribe-staging-") || name.starts_with(".scribe-backup-")
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    fn create_archive(entries: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("addon.zip");
        let file = File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);
        for (name, contents) in entries {
            if name.ends_with('/') {
                writer
                    .add_directory(*name, SimpleFileOptions::default())
                    .unwrap();
            } else {
                writer
                    .start_file(*name, SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(contents.as_bytes()).unwrap();
            }
        }
        writer.finish().unwrap();
        (temp, path)
    }

    #[test]
    fn rejects_traversal_absolute_backslash_and_root_files() {
        // Golden parity cases from old_app/internal/esoui/installer_test.go:
        // TestPlanInstallArchiveRejectsAmbiguousUnsafeArchives.
        for name in ["../evil.txt", "/evil.txt", r"Addon\..\evil.txt"] {
            let (_temp, archive) = create_archive(&[(name, "evil")]);
            let addon_path = tempfile::tempdir().unwrap();
            assert!(matches!(
                Installer::plan_archive(&archive, addon_path.path(), &[]),
                Err(InstallError::EscapingEntry(_))
            ));
        }

        let (_temp, archive) = create_archive(&[("README.txt", "root")]);
        let addon_path = tempfile::tempdir().unwrap();
        assert!(matches!(
            Installer::plan_archive(&archive, addon_path.path(), &[]),
            Err(InstallError::RootFile(_))
        ));

        let (_temp, archive) = create_archive(&[("Addon/file.lua", "content")]);
        let addon_path = tempfile::tempdir().unwrap();
        assert!(matches!(
            Installer::plan_archive(&archive, addon_path.path(), &["Addon".into()]),
            Err(InstallError::MissingManifest(folder)) if folder == "Addon"
        ));

        let (_temp, archive) = create_archive(&[("OtherAddon/OtherAddon.txt", "## Title: Other")]);
        let addon_path = tempfile::tempdir().unwrap();
        assert!(matches!(
            Installer::plan_archive(&archive, addon_path.path(), &["Addon".into()]),
            Err(InstallError::UnexpectedFolder(folder)) if folder == "OtherAddon"
        ));
    }

    #[test]
    fn plans_add_and_replace_without_modifying_destination() {
        let addon_path = tempfile::tempdir().unwrap();
        fs::create_dir(addon_path.path().join("ExistingAddon")).unwrap();
        let (_temp, archive) = create_archive(&[
            ("ExistingAddon/ExistingAddon.txt", "## Title: Existing"),
            ("NewAddon/NewAddon.addon", "## Title: New"),
        ]);
        let plan = Installer::plan_archive(
            &archive,
            addon_path.path(),
            &["ExistingAddon".into(), "NewAddon".into()],
        )
        .unwrap();
        assert_eq!(plan[0].folder_name, "ExistingAddon");
        assert_eq!(plan[0].action, "replace");
        assert_eq!(plan[1].folder_name, "NewAddon");
        assert_eq!(plan[1].action, "add");
        assert!(!addon_path.path().join("NewAddon").exists());
    }

    #[test]
    fn stages_and_atomically_replaces_named_folders() {
        let addon_path = tempfile::tempdir().unwrap();
        let existing = addon_path.path().join("ExistingAddon");
        fs::create_dir(&existing).unwrap();
        fs::write(existing.join("old.lua"), "old").unwrap();
        let (_temp, archive) = create_archive(&[
            ("ExistingAddon/ExistingAddon.txt", "## Title: Existing"),
            ("ExistingAddon/new.lua", "new"),
            ("NewAddon/NewAddon.txt", "## Title: New"),
            ("NewAddon/new.lua", "new"),
        ]);
        let mut updates = Vec::new();
        let plan = Installer::install_archive(
            &archive,
            addon_path.path(),
            &["ExistingAddon".into(), "NewAddon".into()],
            &CancellationToken::default(),
            |done, total| updates.push((done, total)),
        )
        .unwrap();
        assert_eq!(plan.len(), 2);
        assert!(!existing.join("old.lua").exists());
        assert_eq!(fs::read_to_string(existing.join("new.lua")).unwrap(), "new");
        assert_eq!(
            fs::read_to_string(addon_path.path().join("NewAddon/new.lua")).unwrap(),
            "new"
        );
        assert_eq!(updates.last(), Some(&(4, 4)));
        assert!(fs::read_dir(addon_path.path()).unwrap().all(|entry| {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            !is_scribe_artifact(&name)
        }));
    }

    #[test]
    fn cancellation_leaves_installed_folders_untouched() {
        let addon_path = tempfile::tempdir().unwrap();
        let existing = addon_path.path().join("Addon");
        fs::create_dir(&existing).unwrap();
        fs::write(existing.join("old.lua"), "old").unwrap();
        let (_temp, archive) =
            create_archive(&[("Addon/Addon.txt", "manifest"), ("Addon/new.lua", "new")]);
        let token = CancellationToken::default();
        token.cancel();
        assert!(matches!(
            Installer::install_archive(
                &archive,
                addon_path.path(),
                &["Addon".into()],
                &token,
                |_, _| {}
            ),
            Err(InstallError::Cancelled)
        ));
        assert_eq!(fs::read_to_string(existing.join("old.lua")).unwrap(), "old");
        assert!(!existing.join("new.lua").exists());
    }

    #[test]
    fn uninstall_rejects_unsafe_names_and_removes_only_named_folder() {
        let base = tempfile::tempdir().unwrap();
        let addon_path = base.path().join("AddOns");
        let target = addon_path.join("Target");
        let sibling = addon_path.join("Sibling");
        fs::create_dir_all(&target).unwrap();
        fs::create_dir(&sibling).unwrap();
        for unsafe_name in ["", ".", "..", "../Outside", r"..\Outside", "C:/Outside"] {
            assert!(matches!(
                Installer::uninstall(&addon_path, unsafe_name),
                Err(InstallError::InvalidFolder(_))
            ));
        }
        Installer::uninstall(&addon_path, "Target").unwrap();
        assert!(!target.exists());
        assert!(sibling.exists());
    }

    #[test]
    fn cleanup_removes_only_old_scribe_owned_directories() {
        let addon_path = tempfile::tempdir().unwrap();
        let old = addon_path.path().join(".scribe-staging-old");
        let normal = addon_path.path().join("NormalAddon");
        fs::create_dir(&old).unwrap();
        fs::create_dir(&normal).unwrap();
        std::thread::sleep(Duration::from_millis(10));
        let report = Installer::clean_stale_artifacts(addon_path.path(), Duration::from_millis(1));
        assert_eq!(report.removed, [".scribe-staging-old"]);
        assert!(!old.exists());
        assert!(normal.exists());
    }
}
