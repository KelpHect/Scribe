use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[cfg(feature = "rkyv-catalog")]
use crate::CatalogArchive;
use crate::{Addon, Category, MatchedAddon, RemoteAddon};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv-catalog",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct Catalog {
    pub addons: Vec<RemoteAddon>,
    pub categories: Vec<Category>,
}

#[derive(Clone)]
pub struct CatalogIndex {
    backing: CatalogBacking,
    by_uid: Vec<usize>,
    by_directory: Vec<DirectoryEntry>,
    by_popularity: Vec<usize>,
    categories: Vec<Category>,
    compatibility_versions: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CatalogSort {
    Title,
    Author,
    Category,
    #[default]
    Downloads,
    Favorites,
    Date,
}

#[derive(Clone)]
enum CatalogBacking {
    Owned(Arc<Catalog>),
    #[cfg(feature = "rkyv-catalog")]
    Archived(Arc<CatalogArchive>),
}

#[derive(Clone, Copy, Debug)]
struct DirectoryEntry {
    addon: usize,
    directory: usize,
}

#[derive(Clone, Debug, Default)]
pub struct InstalledIndex {
    search_text: Vec<String>,
    update_available: Vec<bool>,
}

impl CatalogIndex {
    pub fn new(catalog: Arc<Catalog>) -> Self {
        let mut by_uid: Vec<_> = (0..catalog.addons.len()).collect();
        by_uid.sort_unstable_by(|left, right| {
            catalog.addons[*left].uid.cmp(&catalog.addons[*right].uid)
        });
        let directory_count = catalog.addons.iter().map(|addon| addon.ui_dirs.len()).sum();
        let mut by_directory = Vec::with_capacity(directory_count);
        for (index, addon) in catalog.addons.iter().enumerate() {
            by_directory.extend((0..addon.ui_dirs.len()).map(|directory| DirectoryEntry {
                addon: index,
                directory,
            }));
        }
        by_directory.sort_unstable_by(|left, right| {
            let left_directory = &catalog.addons[left.addon].ui_dirs[left.directory];
            let right_directory = &catalog.addons[right.addon].ui_dirs[right.directory];
            cmp_ascii_case_insensitive(left_directory, right_directory)
                .then_with(|| left.addon.cmp(&right.addon))
        });
        let mut by_popularity: Vec<_> = (0..catalog.addons.len()).collect();
        by_popularity.sort_unstable_by(|left, right| {
            catalog.addons[*right]
                .ui_download_total
                .cmp(&catalog.addons[*left].ui_download_total)
                .then_with(|| {
                    catalog.addons[*left]
                        .ui_name
                        .cmp(&catalog.addons[*right].ui_name)
                })
        });

        let categories = catalog.categories.clone();
        let compatibility_versions = collect_compatibility_versions(
            catalog
                .addons
                .iter()
                .flat_map(|addon| addon.compatabilities.iter())
                .map(|version| (version.version.as_str(), version.name.as_str())),
        );

        Self {
            backing: CatalogBacking::Owned(catalog),
            by_uid,
            by_directory,
            by_popularity,
            categories,
            compatibility_versions,
        }
    }

    #[cfg(feature = "rkyv-catalog")]
    pub fn from_archive(archive: Arc<CatalogArchive>) -> Self {
        let (by_uid, by_directory, by_popularity, compatibility_versions) =
            archive.with_catalog(|catalog| {
                let mut by_uid: Vec<_> = (0..catalog.addons.len()).collect();
                by_uid.sort_unstable_by(|left, right| {
                    catalog.addons[*left]
                        .uid
                        .as_str()
                        .cmp(catalog.addons[*right].uid.as_str())
                });
                let directory_count = catalog.addons.iter().map(|addon| addon.ui_dirs.len()).sum();
                let mut by_directory = Vec::with_capacity(directory_count);
                for (index, addon) in catalog.addons.iter().enumerate() {
                    by_directory.extend((0..addon.ui_dirs.len()).map(|directory| DirectoryEntry {
                        addon: index,
                        directory,
                    }));
                }
                by_directory.sort_unstable_by(|left, right| {
                    let left_directory =
                        catalog.addons[left.addon].ui_dirs[left.directory].as_str();
                    let right_directory =
                        catalog.addons[right.addon].ui_dirs[right.directory].as_str();
                    cmp_ascii_case_insensitive(left_directory, right_directory)
                        .then_with(|| left.addon.cmp(&right.addon))
                });
                let mut by_popularity: Vec<_> = (0..catalog.addons.len()).collect();
                by_popularity.sort_unstable_by(|left, right| {
                    catalog.addons[*right]
                        .ui_download_total
                        .cmp(&catalog.addons[*left].ui_download_total)
                        .then_with(|| {
                            catalog.addons[*left]
                                .ui_name
                                .as_str()
                                .cmp(catalog.addons[*right].ui_name.as_str())
                        })
                });
                let compatibility_versions = collect_compatibility_versions(
                    catalog
                        .addons
                        .iter()
                        .flat_map(|addon| addon.compatabilities.iter())
                        .map(|version| (version.version.as_str(), version.name.as_str())),
                );
                (by_uid, by_directory, by_popularity, compatibility_versions)
            });
        let categories = archive.categories_owned().unwrap_or_default();
        Self {
            backing: CatalogBacking::Archived(archive),
            by_uid,
            by_directory,
            by_popularity,
            categories,
            compatibility_versions,
        }
    }

    pub fn owned_catalog(&self) -> Option<&Arc<Catalog>> {
        match &self.backing {
            CatalogBacking::Owned(catalog) => Some(catalog),
            #[cfg(feature = "rkyv-catalog")]
            CatalogBacking::Archived(_) => None,
        }
    }

    pub fn len(&self) -> usize {
        match &self.backing {
            CatalogBacking::Owned(catalog) => catalog.addons.len(),
            #[cfg(feature = "rkyv-catalog")]
            CatalogBacking::Archived(archive) => archive.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn categories(&self) -> &[Category] {
        &self.categories
    }

    pub fn category(&self, id: &str) -> Option<&Category> {
        self.categories.iter().find(|category| category.id == id)
    }

    pub fn compatibility_versions(&self) -> &[String] {
        &self.compatibility_versions
    }

    pub fn addon(&self, index: usize) -> Option<RemoteAddon> {
        match &self.backing {
            CatalogBacking::Owned(catalog) => catalog.addons.get(index).cloned(),
            #[cfg(feature = "rkyv-catalog")]
            CatalogBacking::Archived(archive) => archive.addon_owned(index).ok().flatten(),
        }
    }

    pub fn by_uid(&self, uid: &str) -> Option<RemoteAddon> {
        let position = match &self.backing {
            CatalogBacking::Owned(catalog) => self
                .by_uid
                .binary_search_by(|index| catalog.addons[*index].uid.as_str().cmp(uid))
                .ok(),
            #[cfg(feature = "rkyv-catalog")]
            CatalogBacking::Archived(archive) => archive.with_catalog(|catalog| {
                self.by_uid
                    .binary_search_by(|index| catalog.addons[*index].uid.as_str().cmp(uid))
                    .ok()
            }),
        }?;
        self.addon(self.by_uid[position])
    }

    pub fn by_directory(&self, directory: &str) -> Option<RemoteAddon> {
        self.by_directory_candidates(directory).into_iter().next()
    }

    pub fn by_directory_candidates(&self, directory: &str) -> Vec<RemoteAddon> {
        let indices = match &self.backing {
            CatalogBacking::Owned(catalog) => {
                directory_matches(&self.by_directory, directory, |entry, target| {
                    cmp_ascii_case_insensitive(
                        catalog.addons[entry.addon].ui_dirs[entry.directory].as_str(),
                        target,
                    )
                })
            }
            #[cfg(feature = "rkyv-catalog")]
            CatalogBacking::Archived(archive) => archive.with_catalog(|catalog| {
                directory_matches(&self.by_directory, directory, |entry, target| {
                    cmp_ascii_case_insensitive(
                        catalog.addons[entry.addon].ui_dirs[entry.directory].as_str(),
                        target,
                    )
                })
            }),
        };
        indices
            .into_iter()
            .filter_map(|index| self.addon(index))
            .collect()
    }

    pub fn search(&self, query: &str) -> Vec<usize> {
        let query = query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return self.by_popularity.clone();
        }
        match &self.backing {
            CatalogBacking::Owned(catalog) => self
                .by_popularity
                .iter()
                .copied()
                .filter(|index| {
                    let addon = &catalog.addons[*index];
                    addon_matches(
                        &addon.ui_name,
                        &addon.ui_author_name,
                        &addon.uid,
                        addon.ui_dirs.iter().map(AsRef::as_ref),
                        &query,
                    )
                })
                .collect(),
            #[cfg(feature = "rkyv-catalog")]
            CatalogBacking::Archived(archive) => archive.with_catalog(|catalog| {
                self.by_popularity
                    .iter()
                    .copied()
                    .filter(|index| {
                        let addon = &catalog.addons[*index];
                        addon_matches(
                            addon.ui_name.as_str(),
                            addon.ui_author_name.as_str(),
                            addon.uid.as_str(),
                            addon.ui_dirs.iter().map(|directory| directory.as_str()),
                            &query,
                        )
                    })
                    .collect()
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn filter_sort(
        &self,
        query: &str,
        category_id: Option<&str>,
        libraries_only: bool,
        compatibility: Option<&str>,
        hidden_uids: &HashSet<String>,
        sort: CatalogSort,
        ascending: bool,
    ) -> Vec<usize> {
        let query = query.trim().to_ascii_lowercase();
        let mut indices: Vec<usize> = match &self.backing {
            CatalogBacking::Owned(catalog) => self
                .by_popularity
                .iter()
                .copied()
                .filter(|index| {
                    let addon = &catalog.addons[*index];
                    catalog_addon_matches(
                        &addon.ui_name,
                        &addon.ui_author_name,
                        &addon.uid,
                        addon.ui_dirs.iter().map(AsRef::as_ref),
                        &addon.category_id,
                        addon
                            .compatabilities
                            .iter()
                            .map(|version| (version.version.as_str(), version.name.as_str())),
                        &query,
                        category_id,
                        libraries_only,
                        compatibility,
                        hidden_uids,
                        &self.categories,
                    )
                })
                .collect(),
            #[cfg(feature = "rkyv-catalog")]
            CatalogBacking::Archived(archive) => archive.with_catalog(|catalog| {
                self.by_popularity
                    .iter()
                    .copied()
                    .filter(|index| {
                        let addon = &catalog.addons[*index];
                        catalog_addon_matches(
                            addon.ui_name.as_str(),
                            addon.ui_author_name.as_str(),
                            addon.uid.as_str(),
                            addon.ui_dirs.iter().map(|directory| directory.as_str()),
                            addon.category_id.as_str(),
                            addon
                                .compatabilities
                                .iter()
                                .map(|version| (version.version.as_str(), version.name.as_str())),
                            &query,
                            category_id,
                            libraries_only,
                            compatibility,
                            hidden_uids,
                            &self.categories,
                        )
                    })
                    .collect()
            }),
        };

        if sort != CatalogSort::Downloads {
            match &self.backing {
                CatalogBacking::Owned(catalog) => indices.sort_unstable_by(|left, right| {
                    compare_catalog_addons(
                        &catalog.addons[*left],
                        &catalog.addons[*right],
                        sort,
                        &self.categories,
                    )
                }),
                #[cfg(feature = "rkyv-catalog")]
                CatalogBacking::Archived(archive) => archive.with_catalog(|catalog| {
                    indices.sort_unstable_by(|left, right| {
                        compare_archived_catalog_addons(
                            &catalog.addons[*left],
                            &catalog.addons[*right],
                            sort,
                            &self.categories,
                        )
                    })
                }),
            }
        }
        if ascending {
            indices.reverse();
        }
        indices
    }
}

fn collect_compatibility_versions<'a>(
    versions: impl Iterator<Item = (&'a str, &'a str)>,
) -> Vec<String> {
    let mut versions: Vec<String> = versions
        .flat_map(|(version, name)| [version, name])
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect();
    versions.sort_unstable();
    versions.dedup();
    versions.reverse();
    versions
}

#[allow(clippy::too_many_arguments)]
fn catalog_addon_matches<'a>(
    name: &str,
    author: &str,
    uid: &str,
    directories: impl Iterator<Item = &'a str>,
    addon_category: &str,
    compatibilities: impl Iterator<Item = (&'a str, &'a str)>,
    query: &str,
    category_id: Option<&str>,
    libraries_only: bool,
    compatibility: Option<&str>,
    hidden_uids: &HashSet<String>,
    categories: &[Category],
) -> bool {
    !hidden_uids.contains(uid)
        && category_id.is_none_or(|category| category == addon_category)
        && (!libraries_only || is_library_category(categories, addon_category))
        && compatibility.is_none_or(|target| {
            compatibilities
                .into_iter()
                .any(|(version, name)| version == target || name == target)
        })
        && (query.is_empty()
            || addon_matches(name, author, uid, directories, query)
            || contains_ascii_case_insensitive(category_name(categories, addon_category), query))
}

fn category_name<'a>(categories: &'a [Category], id: &str) -> &'a str {
    categories
        .iter()
        .find(|category| category.id == id)
        .map(|category| category.name.as_str())
        .unwrap_or("Other")
}

fn is_library_category(categories: &[Category], id: &str) -> bool {
    let Some(category) = categories.iter().find(|category| category.id == id) else {
        return false;
    };
    category.name.to_ascii_lowercase().contains("librar")
        || category.parent_ids.iter().any(|parent_id| {
            categories.iter().any(|parent| {
                parent.id == *parent_id && parent.name.to_ascii_lowercase().contains("librar")
            })
        })
}

fn compare_catalog_addons(
    left: &RemoteAddon,
    right: &RemoteAddon,
    sort: CatalogSort,
    categories: &[Category],
) -> Ordering {
    match sort {
        CatalogSort::Title => right.ui_name.cmp(&left.ui_name),
        CatalogSort::Author => right.ui_author_name.cmp(&left.ui_author_name),
        CatalogSort::Category => category_name(categories, &right.category_id)
            .cmp(category_name(categories, &left.category_id)),
        CatalogSort::Downloads => right.ui_download_total.cmp(&left.ui_download_total),
        CatalogSort::Favorites => right.ui_favorite_total.cmp(&left.ui_favorite_total),
        CatalogSort::Date => right.ui_date.cmp(&left.ui_date),
    }
}

#[cfg(feature = "rkyv-catalog")]
fn compare_archived_catalog_addons(
    left: &rkyv::Archived<RemoteAddon>,
    right: &rkyv::Archived<RemoteAddon>,
    sort: CatalogSort,
    categories: &[Category],
) -> Ordering {
    match sort {
        CatalogSort::Title => right.ui_name.as_str().cmp(left.ui_name.as_str()),
        CatalogSort::Author => right
            .ui_author_name
            .as_str()
            .cmp(left.ui_author_name.as_str()),
        CatalogSort::Category => category_name(categories, right.category_id.as_str())
            .cmp(category_name(categories, left.category_id.as_str())),
        CatalogSort::Downloads => right.ui_download_total.cmp(&left.ui_download_total),
        CatalogSort::Favorites => right.ui_favorite_total.cmp(&left.ui_favorite_total),
        CatalogSort::Date => right.ui_date.as_str().cmp(left.ui_date.as_str()),
    }
}

fn directory_matches(
    entries: &[DirectoryEntry],
    directory: &str,
    compare: impl Fn(&DirectoryEntry, &str) -> Ordering,
) -> Vec<usize> {
    let start = entries.partition_point(|entry| compare(entry, directory) == Ordering::Less);
    let end = entries[start..]
        .partition_point(|entry| compare(entry, directory) == Ordering::Equal)
        + start;
    let mut result = Vec::with_capacity(end.saturating_sub(start));
    for entry in &entries[start..end] {
        if result.last() != Some(&entry.addon) {
            result.push(entry.addon);
        }
    }
    result
}

fn addon_matches<'a>(
    name: &str,
    author: &str,
    uid: &str,
    mut directories: impl Iterator<Item = &'a str>,
    query: &str,
) -> bool {
    contains_ascii_case_insensitive(name, query)
        || contains_ascii_case_insensitive(author, query)
        || contains_ascii_case_insensitive(uid, query)
        || directories.any(|directory| contains_ascii_case_insensitive(directory, query))
}

fn cmp_ascii_case_insensitive(left: &str, right: &str) -> Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase()))
}

fn contains_ascii_case_insensitive(haystack: &str, lowercase_needle: &str) -> bool {
    let needle = lowercase_needle.as_bytes();
    needle.is_empty()
        || haystack.as_bytes().windows(needle.len()).any(|window| {
            window
                .iter()
                .zip(needle)
                .all(|(left, right)| left.to_ascii_lowercase() == *right)
        })
}

impl InstalledIndex {
    pub fn new(installed: &[Addon], matched: &[MatchedAddon]) -> Self {
        let search_text = installed
            .iter()
            .map(|addon| {
                format!("{} {} {}", addon.folder_name, addon.title, addon.author)
                    .to_ascii_lowercase()
            })
            .collect();
        let update_available = installed
            .iter()
            .enumerate()
            .map(|(index, _)| {
                matched
                    .get(index)
                    .is_some_and(|decision| decision.update_available)
            })
            .collect();
        Self {
            search_text,
            update_available,
        }
    }

    pub fn search(&self, query: &str, updates_only: bool) -> Vec<usize> {
        let query = query.trim().to_ascii_lowercase();
        self.search_text
            .iter()
            .enumerate()
            .filter_map(|(index, text)| {
                ((!updates_only || self.update_available[index])
                    && (query.is_empty() || text.contains(&query)))
                .then_some(index)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_uid_directory_and_search_once() {
        let catalog = Arc::new(Catalog {
            addons: vec![RemoteAddon {
                uid: "42".into(),
                ui_name: "Lib Async".into(),
                ui_author_name: "Author".into(),
                ui_dirs: vec!["LibAsync".into()],
                ..RemoteAddon::default()
            }],
            categories: vec![],
        });
        let index = CatalogIndex::new(catalog);
        assert_eq!(index.by_uid("42").unwrap().ui_name, "Lib Async");
        assert_eq!(index.by_directory("libasync").unwrap().uid, "42");
        assert_eq!(index.search("AUTHOR"), vec![0]);
    }

    #[test]
    fn catalog_defaults_to_download_popularity_without_sorting_each_query() {
        let catalog = Arc::new(Catalog {
            addons: vec![
                RemoteAddon {
                    ui_name: "Small".into(),
                    ui_download_total: 10,
                    ..RemoteAddon::default()
                },
                RemoteAddon {
                    ui_name: "Popular".into(),
                    ui_download_total: 1_000,
                    ..RemoteAddon::default()
                },
            ],
            categories: vec![],
        });
        let index = CatalogIndex::new(catalog);
        assert_eq!(index.search(""), vec![1, 0]);
    }

    #[test]
    fn catalog_filters_categories_libraries_versions_hidden_and_sort_order() {
        let catalog = Arc::new(Catalog {
            addons: vec![
                RemoteAddon {
                    uid: "map".into(),
                    category_id: "maps".into(),
                    ui_name: "Zulu Map".into(),
                    ui_author_name: "Mapper".into(),
                    ui_download_total: 50,
                    ui_favorite_total: 10,
                    compatabilities: vec![crate::GameVersion {
                        version: "101049".into(),
                        name: "Season Zero".into(),
                    }],
                    ..RemoteAddon::default()
                },
                RemoteAddon {
                    uid: "lib".into(),
                    category_id: "libs".into(),
                    ui_name: "Alpha Library".into(),
                    ui_author_name: "Author".into(),
                    ui_download_total: 100,
                    ..RemoteAddon::default()
                },
            ],
            categories: vec![
                Category {
                    id: "maps".into(),
                    name: "Map, Coords, Compasses".into(),
                    ..Category::default()
                },
                Category {
                    id: "libs".into(),
                    name: "Libraries".into(),
                    ..Category::default()
                },
            ],
        });
        let index = CatalogIndex::new(catalog);

        assert_eq!(
            index.filter_sort(
                "map",
                Some("maps"),
                false,
                Some("Season Zero"),
                &HashSet::new(),
                CatalogSort::Downloads,
                false,
            ),
            vec![0]
        );
        assert_eq!(
            index.filter_sort(
                "",
                None,
                true,
                None,
                &HashSet::new(),
                CatalogSort::Downloads,
                false,
            ),
            vec![1]
        );
        assert_eq!(
            index.filter_sort(
                "",
                None,
                false,
                None,
                &HashSet::from(["lib".to_owned()]),
                CatalogSort::Title,
                false,
            ),
            vec![0]
        );
    }

    #[cfg(feature = "rkyv-catalog")]
    #[test]
    fn archived_index_matches_owned_lookup_and_materializes_selected_rows() {
        let catalog = Catalog {
            addons: vec![RemoteAddon {
                uid: "42".into(),
                ui_name: "Lib Async".into(),
                ui_author_name: "Author".into(),
                ui_dirs: vec!["LibAsync".into()],
                ..RemoteAddon::default()
            }],
            categories: vec![],
        };
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&catalog).unwrap();
        let archive = Arc::new(CatalogArchive::from_bytes(&bytes).unwrap());
        let index = CatalogIndex::from_archive(archive);

        assert!(index.owned_catalog().is_none());
        assert_eq!(index.len(), 1);
        assert_eq!(index.by_uid("42").unwrap().ui_name, "Lib Async");
        assert_eq!(index.by_directory("libasync").unwrap().uid, "42");
        assert_eq!(index.search("AUTHOR"), vec![0]);
    }

    #[test]
    fn installed_index_filters_search_and_updates_without_rebuilding_text() {
        let installed = vec![
            Addon {
                folder_name: "Alpha".into(),
                title: "First Addon".into(),
                author: "One".into(),
                ..Addon::default()
            },
            Addon {
                folder_name: "Beta".into(),
                title: "Second Addon".into(),
                author: "Two".into(),
                ..Addon::default()
            },
        ];
        let matched = vec![
            MatchedAddon {
                update_available: false,
                ..MatchedAddon::default()
            },
            MatchedAddon {
                update_available: true,
                ..MatchedAddon::default()
            },
        ];
        let index = InstalledIndex::new(&installed, &matched);
        assert_eq!(index.search("SECOND", false), vec![1]);
        assert_eq!(index.search("", true), vec![1]);
    }
}
