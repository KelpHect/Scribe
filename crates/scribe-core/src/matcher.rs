use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::CatalogIndex;
use crate::models::{Addon, MatchedAddon, MissingDependency, RemoteAddon};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateState {
    UpToDate,
    RemoteNewer,
    LocalNewer,
    Md5OnlyChanged,
    UnknownVersion,
    Unmatched,
}

impl UpdateState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UpToDate => "up-to-date",
            Self::RemoteNewer => "remote-newer",
            Self::LocalNewer => "local-newer",
            Self::Md5OnlyChanged => "md5-only-changed",
            Self::UnknownVersion => "unknown-version",
            Self::Unmatched => "unmatched",
        }
    }
}

pub struct Matcher;

impl Matcher {
    pub fn match_installed(locals: &[Addon], remotes: &[RemoteAddon]) -> Vec<MatchedAddon> {
        let index = directory_index(remotes);
        match_installed_with_index(locals, &index)
    }

    pub fn analyze(
        locals: &[Addon],
        remotes: &[RemoteAddon],
    ) -> (Vec<MatchedAddon>, Vec<MissingDependency>) {
        let index = directory_index(remotes);
        (
            match_installed_with_index(locals, &index),
            resolve_dependencies_with_index(locals, &index),
        )
    }

    pub fn analyze_index(
        locals: &[Addon],
        catalog: &CatalogIndex,
    ) -> (Vec<MatchedAddon>, Vec<MissingDependency>) {
        let mut directories = BTreeSet::new();
        for addon in locals {
            directories.insert(normalize_directory(&addon.folder_name));
            for dependency in addon.depends_on.iter().chain(&addon.optional_depends_on) {
                let (folder, _) = split_dependency(dependency);
                directories.insert(normalize_directory(folder));
            }
        }
        let mut seen = BTreeSet::new();
        let remotes: Vec<_> = directories
            .into_iter()
            .flat_map(|directory| catalog.by_directory_candidates(&directory))
            .filter(|remote| seen.insert(remote.uid.to_string()))
            .collect();
        Self::analyze(locals, &remotes)
    }

    pub fn plan_updates(locals: &[Addon], remotes: &[RemoteAddon]) -> Vec<MatchedAddon> {
        Self::match_installed(locals, remotes)
            .into_iter()
            .filter(|matched| matched.update_available)
            .collect()
    }

    pub fn apply_md5_decisions(
        mut matched: Vec<MatchedAddon>,
        installed_md5s: &HashMap<String, String>,
        remote_md5s: &HashMap<String, String>,
    ) -> Vec<MatchedAddon> {
        for decision in &mut matched {
            let Some(remote) = &decision.remote else {
                continue;
            };
            let Some(installed) = installed_md5s
                .get(remote.uid.as_str())
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let Some(current) = remote_md5s
                .get(remote.uid.as_str())
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if installed.eq_ignore_ascii_case(current) {
                decision.update_available = false;
                decision.update_state = UpdateState::UpToDate.as_str().into();
                decision.update_reason = "Installed download MD5 matches ESOUI, so Scribe is suppressing a version-text false positive.".into();
            } else if !decision.update_available
                && decision.update_state == UpdateState::UpToDate.as_str()
            {
                decision.update_available = true;
                decision.update_state = UpdateState::Md5OnlyChanged.as_str().into();
                decision.update_reason =
                    "ESOUI download MD5 changed while the version text stayed the same.".into();
            }
        }
        matched
    }

    pub fn resolve_dependencies(
        locals: &[Addon],
        remotes: &[RemoteAddon],
    ) -> Vec<MissingDependency> {
        let index = directory_index(remotes);
        resolve_dependencies_with_index(locals, &index)
    }

    pub fn best_remote_for_directory<'a>(
        remotes: &'a [RemoteAddon],
        directory: &str,
    ) -> Option<&'a RemoteAddon> {
        let key = normalize_directory(directory);
        remotes
            .iter()
            .filter(|remote| {
                remote
                    .ui_dirs
                    .iter()
                    .any(|directory| normalize_directory(directory) == key)
            })
            .min_by(|left, right| compare_remote(left, right, &key))
    }
}

fn match_installed_with_index(
    locals: &[Addon],
    index: &HashMap<String, Vec<&RemoteAddon>>,
) -> Vec<MatchedAddon> {
    locals
        .iter()
        .map(|local| {
            let key = normalize_directory(&local.folder_name);
            let Some(remote) = index
                .get(&key)
                .and_then(|candidates| best_remote(candidates, &key))
            else {
                return MatchedAddon {
                    folder_name: local.folder_name.clone(),
                    remote: None,
                    details: None,
                    update_available: false,
                    local_version: local.version.clone(),
                    remote_version: String::new(),
                    update_state: UpdateState::Unmatched.as_str().into(),
                    update_reason: "No ESOUI catalog entry matched this addon folder.".into(),
                };
            };

            let (state, update_available, reason) =
                classify_version_update(&local.version, &remote.ui_version);
            MatchedAddon {
                folder_name: local.folder_name.clone(),
                remote: Some(remote.clone()),
                details: None,
                update_available,
                local_version: local.version.clone(),
                remote_version: remote.ui_version.to_string(),
                update_state: state.as_str().into(),
                update_reason: reason.into(),
            }
        })
        .collect()
}

fn resolve_dependencies_with_index(
    locals: &[Addon],
    index: &HashMap<String, Vec<&RemoteAddon>>,
) -> Vec<MissingDependency> {
    #[derive(Default)]
    struct Entry {
        required_by: BTreeSet<String>,
        constraints: BTreeSet<String>,
        optional: bool,
    }

    let installed: BTreeSet<String> = locals
        .iter()
        .map(|addon| normalize_directory(&addon.folder_name))
        .collect();
    let mut missing: BTreeMap<String, Entry> = BTreeMap::new();

    for addon in locals {
        for (dependency, optional) in addon
            .depends_on
            .iter()
            .map(|dependency| (dependency, false))
            .chain(
                addon
                    .optional_depends_on
                    .iter()
                    .map(|dependency| (dependency, true)),
            )
        {
            let (folder, constraint) = split_dependency(dependency);
            let folder = normalize_directory(folder);
            if folder.is_empty() || installed.contains(&folder) {
                continue;
            }

            let entry = missing.entry(folder).or_insert_with(|| Entry {
                optional,
                ..Entry::default()
            });
            entry.required_by.insert(addon.folder_name.clone());
            if !optional {
                entry.optional = false;
            }
            if !constraint.is_empty() {
                entry.constraints.insert(constraint.to_owned());
            }
        }
    }

    let mut result: Vec<_> = missing
            .into_iter()
            .map(|(folder, entry)| {
                let remote = index
                    .get(&folder)
                    .and_then(|candidates| best_remote(candidates, &folder));
                MissingDependency {
                    dep_folder_name: folder,
                    required_by: entry.required_by.into_iter().collect(),
                    version_constraints: entry.constraints.into_iter().collect(),
                    remote_uid: remote
                        .map(|addon| addon.uid.to_string())
                        .unwrap_or_default(),
                    remote_name: remote
                        .map(|addon| addon.ui_name.to_string())
                        .unwrap_or_default(),
                    can_install: remote.is_some(),
                    optional: entry.optional,
                    plan_state: if remote.is_some() {
                        "installable".into()
                    } else {
                        "unresolved".into()
                    },
                    plan_reason: if remote.is_some() {
                        "Matched the latest canonical ESOUI addon entry; dependency version constraints are informational and do not pin downloads.".into()
                    } else {
                        "No ESOUI catalog entry matched this dependency folder.".into()
                    },
                }
            })
            .collect();
    result.sort_by(|left, right| {
        left.optional
            .cmp(&right.optional)
            .then_with(|| left.dep_folder_name.cmp(&right.dep_folder_name))
    });
    result
}

fn directory_index(remotes: &[RemoteAddon]) -> HashMap<String, Vec<&RemoteAddon>> {
    let mut index: HashMap<String, Vec<&RemoteAddon>> = HashMap::new();
    for remote in remotes {
        for directory in &remote.ui_dirs {
            let key = normalize_directory(directory);
            if !key.is_empty() {
                index.entry(key).or_default().push(remote);
            }
        }
    }
    index
}

fn best_remote<'a>(candidates: &'a [&RemoteAddon], key: &str) -> Option<&'a RemoteAddon> {
    candidates
        .iter()
        .copied()
        .min_by(|left, right| compare_remote(left, right, key))
}

fn compare_remote(left: &RemoteAddon, right: &RemoteAddon, key: &str) -> Ordering {
    let left_count = remote_directory_count(left);
    let right_count = remote_directory_count(right);
    let left_exact = left_count == 1;
    let right_exact = right_count == 1;

    right_exact
        .cmp(&left_exact)
        .then_with(|| left_count.cmp(&right_count))
        .then_with(|| right.ui_date.trim().cmp(left.ui_date.trim()))
        .then_with(|| {
            let (state, _, _) = classify_version_update(&right.ui_version, &left.ui_version);
            match state {
                UpdateState::RemoteNewer => Ordering::Less,
                UpdateState::LocalNewer => Ordering::Greater,
                _ => Ordering::Equal,
            }
        })
        .then_with(|| right.ui_download_total.cmp(&left.ui_download_total))
        .then_with(|| {
            let left_name_matches = normalize_directory(&left.ui_name) == key;
            let right_name_matches = normalize_directory(&right.ui_name) == key;
            right_name_matches.cmp(&left_name_matches)
        })
        .then_with(|| left.uid.cmp(&right.uid))
}

fn remote_directory_count(remote: &RemoteAddon) -> usize {
    remote
        .ui_dirs
        .iter()
        .filter(|directory| !normalize_directory(directory).is_empty())
        .count()
}

fn classify_version_update(local: &str, remote: &str) -> (UpdateState, bool, &'static str) {
    let local = local.trim();
    let remote = remote.trim();
    if local.is_empty() || remote.is_empty() {
        return (
            UpdateState::UnknownVersion,
            false,
            "Local or remote version is missing, so Scribe will not auto-offer an update from version text alone.",
        );
    }
    if local == remote {
        return (
            UpdateState::UpToDate,
            false,
            "Local and ESOUI versions match.",
        );
    }

    let mut local_parts = NumericParts::new(local).peekable();
    let mut remote_parts = NumericParts::new(remote).peekable();
    if local_parts.peek().is_none() || remote_parts.peek().is_none() {
        return (
            UpdateState::UnknownVersion,
            false,
            "Version text could not be compared safely.",
        );
    }

    loop {
        let local = local_parts.next();
        let remote = remote_parts.next();
        if local.is_none() && remote.is_none() {
            break;
        }
        let local = local.unwrap_or_default();
        let remote = remote.unwrap_or_default();
        match remote.cmp(&local) {
            Ordering::Greater => {
                return (UpdateState::RemoteNewer, true, "ESOUI has a newer version.");
            }
            Ordering::Less => {
                return (
                    UpdateState::LocalNewer,
                    false,
                    "Local version appears newer than ESOUI.",
                );
            }
            Ordering::Equal => {}
        }
    }

    (
        UpdateState::UpToDate,
        false,
        "Local and ESOUI versions compare as equal.",
    )
}

struct NumericParts<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> NumericParts<'a> {
    fn new(value: &'a str) -> Self {
        Self {
            bytes: value.as_bytes(),
            offset: 0,
        }
    }
}

impl Iterator for NumericParts<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        while self.offset < self.bytes.len() {
            while self.offset < self.bytes.len() && !self.bytes[self.offset].is_ascii_digit() {
                self.offset += 1;
            }
            if self.offset == self.bytes.len() {
                return None;
            }
            let mut value = 0_u64;
            let mut overflowed = false;
            while self.offset < self.bytes.len() && self.bytes[self.offset].is_ascii_digit() {
                value = value
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(u64::from(self.bytes[self.offset] - b'0')))
                    .unwrap_or_else(|| {
                        overflowed = true;
                        0
                    });
                self.offset += 1;
            }
            if !overflowed {
                return Some(value);
            }
        }
        None
    }
}

fn normalize_directory(directory: &str) -> String {
    directory.trim().to_lowercase()
}

fn split_dependency(dependency: &str) -> (&str, &str) {
    let index = dependency
        .char_indices()
        .find(|(_, character)| matches!(character, '>' | '<' | '!' | '='))
        .map(|(index, _)| index);
    match index {
        Some(index) => (dependency[..index].trim(), dependency[index..].trim()),
        None => (dependency.trim(), ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(folder: &str, version: &str) -> Addon {
        Addon {
            folder_name: folder.into(),
            version: version.into(),
            ..Addon::default()
        }
    }

    fn remote(uid: &str, version: &str, directories: &[&str]) -> RemoteAddon {
        RemoteAddon {
            uid: uid.into(),
            ui_name: directories.first().copied().unwrap_or_default().into(),
            ui_version: version.into(),
            ui_dirs: directories.iter().map(|value| (*value).into()).collect(),
            ..RemoteAddon::default()
        }
    }

    #[test]
    fn preserves_version_update_decisions() {
        for (local_version, remote_version, expected, available) in [
            ("1.2.3", "1.2.3", UpdateState::UpToDate, false),
            ("1.2.3", "1.2.4", UpdateState::RemoteNewer, true),
            ("1.3.0", "1.2.9", UpdateState::LocalNewer, false),
            ("v2.4", "Version 2.5b", UpdateState::RemoteNewer, true),
            ("release-2.5b", "Version 2.5", UpdateState::UpToDate, false),
            ("", "2.0", UpdateState::UnknownVersion, false),
            ("release", "new", UpdateState::UnknownVersion, false),
        ] {
            let matched = Matcher::match_installed(
                &[local("MyAddon", local_version)],
                &[remote("1", remote_version, &["MyAddon"])],
            );
            assert_eq!(matched[0].update_state, expected.as_str());
            assert_eq!(matched[0].update_available, available);
            assert!(!matched[0].update_reason.is_empty());
        }
    }

    #[test]
    fn chooses_latest_specific_canonical_entry() {
        let mut bundle = remote("bundle", "9.0", &["SharedLib", "OtherLib"]);
        bundle.ui_date = "2030-01-02".into();
        let mut latest = remote("latest", "3.0", &["SharedLib"]);
        latest.ui_date = "2026-01-02".into();
        let mut older = remote("older", "2.0", &["SharedLib"]);
        older.ui_date = "2024-01-02".into();

        let remotes = [bundle, older, latest];
        let selected = Matcher::best_remote_for_directory(&remotes, "sharedlib").expect("remote");
        assert_eq!(selected.uid, "latest");
    }

    #[test]
    fn required_dependency_wins_and_constraints_are_informational() {
        // Golden parity fixture from old_app/missing_dependencies_test.go:
        // TestFindMissingDependenciesPureHelper.
        let mut root = local("RootAddon", "1");
        root.depends_on = vec!["LibRequired>=1.0".into()];
        root.optional_depends_on = vec!["LibOptional".into(), "LibInstalled".into()];
        let mut other = local("OtherAddon", "1");
        other.optional_depends_on = vec!["LibRequired<=2.0".into()];

        let mut required = remote("required-uid", "3", &["LibRequired"]);
        required.ui_name = "Required Library".into();
        let mut optional = remote("optional-uid", "2", &["Nested", "LibOptional"]);
        optional.ui_name = "Optional Library".into();

        let plan = Matcher::resolve_dependencies(
            &[root, other, local("LibInstalled", "1")],
            &[required, optional],
        );
        let required = plan
            .iter()
            .find(|dependency| dependency.dep_folder_name == "librequired")
            .unwrap();
        assert!(!required.optional);
        assert!(required.can_install);
        assert_eq!(required.remote_uid, "required-uid");
        assert_eq!(required.remote_name, "Required Library");
        assert_eq!(required.required_by, ["OtherAddon", "RootAddon"]);
        assert_eq!(required.version_constraints, ["<=2.0", ">=1.0"]);
        assert_eq!(required.plan_state, "installable");
        assert!(!required.plan_reason.is_empty());

        let optional = plan
            .iter()
            .find(|dependency| dependency.dep_folder_name == "liboptional")
            .unwrap();
        assert!(optional.optional);
        assert!(optional.can_install);
        assert_eq!(optional.remote_uid, "optional-uid");
        assert_eq!(optional.remote_name, "Optional Library");
        assert!(
            !plan
                .iter()
                .any(|dependency| dependency.dep_folder_name == "libinstalled")
        );
    }

    #[test]
    fn md5_decisions_suppress_false_positives_and_detect_changed_archives() {
        // Golden parity fixture from old_app/md5_suppression_test.go.
        let decision = |uid: &str, update_available: bool, state: UpdateState| MatchedAddon {
            remote: Some(remote(uid, "1", &[uid])),
            update_available,
            update_state: state.as_str().into(),
            ..MatchedAddon::default()
        };
        let matched = vec![
            decision("same", true, UpdateState::RemoteNewer),
            decision("different", true, UpdateState::RemoteNewer),
            decision("missing-remote-md5", true, UpdateState::RemoteNewer),
            decision("already-current", false, UpdateState::UpToDate),
            decision("same-version-new-download", false, UpdateState::UpToDate),
        ];
        let installed = HashMap::from([
            ("same".into(), "abc".into()),
            ("different".into(), "old".into()),
            ("missing-remote-md5".into(), "stored".into()),
            ("already-current".into(), "abc".into()),
            ("same-version-new-download".into(), "old".into()),
        ]);
        let current = HashMap::from([
            ("same".into(), "abc".into()),
            ("different".into(), "new".into()),
            ("already-current".into(), "abc".into()),
            ("same-version-new-download".into(), "new".into()),
        ]);
        let decisions = Matcher::apply_md5_decisions(matched, &installed, &current);
        assert!(!decisions[0].update_available);
        assert_eq!(decisions[0].update_state, UpdateState::UpToDate.as_str());
        assert!(decisions[1].update_available);
        assert!(decisions[2].update_available);
        assert!(!decisions[3].update_available);
        assert!(decisions[4].update_available);
        assert_eq!(
            decisions[4].update_state,
            UpdateState::Md5OnlyChanged.as_str()
        );
    }
}
