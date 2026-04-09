package esoui

type GameVersion struct {
	Version string `json:"version"`
	Name    string `json:"name"`
}

type RemoteAddon struct {
	UID               string        `json:"uid"`
	CategoryID        string        `json:"categoryId"`
	UIName            string        `json:"uiName"`
	UIAuthorName      string        `json:"uiAuthorName"`
	UIDate            string        `json:"uiDate"`
	UIVersion         string        `json:"uiVersion"`
	UIDirs            []string      `json:"uiDirs"`
	UIFileInfoURL     string        `json:"uiFileInfoUrl"`
	UIDownloadTotal   int64         `json:"uiDownloadTotal"`
	UIDownloadMonthly int64         `json:"uiDownloadMonthly"`
	UIFavoriteTotal   int64         `json:"uiFavoriteTotal"`
	UIIMGThumbs       []string      `json:"uiIMGThumbs"`
	UIIMGs            []string      `json:"uiIMGs"`
	Compatabilities   []GameVersion `json:"compatabilities"`
	Siblings          []string      `json:"siblings"`
}

type RemoteAddonDetails struct {
	RemoteAddon
	UIMD5             string `json:"uiMD5"`
	UIFileName        string `json:"uiFileName"`
	UIDownload        string `json:"uiDownload"`
	UIDescription     string `json:"uiDescription"`
	UIChangeLog       string `json:"uiChangeLog"`
	UIHitCount        int64  `json:"uiHitCount"`
	UIHitCountMonthly int64  `json:"uiHitCountMonthly"`
	UIDonation        string `json:"uiDonationLink"`
	UIPending         bool   `json:"UIPending"`
	UICatID           string `json:"uiCatId"`
}

type Category struct {
	ID        string   `json:"id"`
	Name      string   `json:"name"`
	IconURL   string   `json:"iconUrl"`
	ParentID  string   `json:"parentId"`
	ParentIDs []string `json:"parentIds"`
	Count     int      `json:"count"`
}

type APIFeeds struct {
	FileList    string `json:"FileList"`
	FileDetails string `json:"FileDetails"`
	Categories  string `json:"CategoryList"`
	ListFiles   string `json:"ListFiles"`
}

type GameConfig struct {
	APIFeeds APIFeeds `json:"APIFeeds"`
}

type MatchedAddon struct {
	FolderName      string              `json:"folderName"`
	Remote          *RemoteAddon        `json:"remote"`
	Details         *RemoteAddonDetails `json:"details"`
	UpdateAvailable bool                `json:"updateAvailable"`
	LocalVersion    string              `json:"localVersion"`
	RemoteVersion   string              `json:"remoteVersion"`
}

type MissingDepInfo struct {
	DepFolderName string   `json:"depFolderName"`
	RequiredBy    []string `json:"requiredBy"`
	RemoteUID     string   `json:"remoteUID"`
	RemoteName    string   `json:"remoteName"`
	CanInstall    bool     `json:"canInstall"`
	Optional      bool     `json:"optional"`
}
