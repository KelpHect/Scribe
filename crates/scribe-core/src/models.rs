use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv-catalog",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
#[serde(rename_all = "camelCase")]
pub struct Addon {
    pub id: String,
    pub folder_name: String,
    pub title: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub depends_on: Vec<String>,
    pub optional_depends_on: Vec<String>,
    pub saved_variables: Vec<String>,
    pub api_version: String,
    pub add_on_version: String,
    pub is_library: bool,
    pub enabled: bool,
    pub path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv-catalog",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct GameVersion {
    pub version: SmolStr,
    pub name: SmolStr,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv-catalog",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAddon {
    pub uid: SmolStr,
    pub category_id: SmolStr,
    pub ui_name: SmolStr,
    pub ui_author_name: SmolStr,
    pub ui_date: SmolStr,
    pub ui_version: SmolStr,
    pub ui_dirs: Vec<SmolStr>,
    pub ui_file_info_url: SmolStr,
    pub ui_download_total: i64,
    pub ui_download_monthly: i64,
    pub ui_favorite_total: i64,
    pub ui_img_thumbs: Vec<SmolStr>,
    pub ui_imgs: Vec<SmolStr>,
    pub compatabilities: Vec<GameVersion>,
    pub siblings: Vec<SmolStr>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv-catalog",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAddonDetails {
    #[serde(flatten)]
    pub addon: RemoteAddon,
    #[serde(rename = "uiMD5")]
    pub ui_md5: String,
    pub ui_file_name: String,
    pub ui_download: String,
    pub ui_description: String,
    pub ui_change_log: String,
    pub ui_hit_count: i64,
    pub ui_hit_count_monthly: i64,
    #[serde(rename = "uiDonationLink")]
    pub ui_donation: String,
    #[serde(rename = "UIPending")]
    pub ui_pending: bool,
    #[serde(rename = "uiCatId")]
    pub ui_cat_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "rkyv-catalog",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: SmolStr,
    pub name: SmolStr,
    pub icon_url: SmolStr,
    pub parent_id: SmolStr,
    pub parent_ids: Vec<SmolStr>,
    pub count: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ApiFeeds {
    pub file_list: String,
    pub file_details: String,
    pub category_list: String,
    pub list_files: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GameConfig {
    #[serde(rename = "APIFeeds")]
    pub api_feeds: ApiFeeds,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchedAddon {
    pub folder_name: String,
    pub remote: Option<RemoteAddon>,
    pub details: Option<RemoteAddonDetails>,
    pub update_available: bool,
    pub local_version: String,
    pub remote_version: String,
    pub update_state: String,
    pub update_reason: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingDependency {
    pub dep_folder_name: String,
    pub required_by: Vec<String>,
    pub version_constraints: Vec<String>,
    #[serde(rename = "remoteUID")]
    pub remote_uid: String,
    pub remote_name: String,
    pub can_install: bool,
    pub optional: bool,
    pub plan_state: String,
    pub plan_reason: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScannerCacheRecord {
    pub fingerprint: String,
    pub modified_millis: u64,
    pub size: u64,
    pub addon: Addon,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecord {
    pub uid: String,
    pub md5: String,
}
