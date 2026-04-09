package esoui

import (
	"encoding/json"
	"fmt"
	"time"

	"github.com/glebarez/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/logger"
)

type DBRemoteAddon struct {
	UID                 string `gorm:"primaryKey"`
	CategoryID          string `gorm:"index"`
	UIName              string `gorm:"index"`
	UIAuthorName        string
	UIDate              string
	UIVersion           string
	UIDirsJSON          string
	UIFileInfoURL       string
	UIDownloadTotal     int64
	UIDownloadMonthly   int64
	UIFavoriteTotal     int64
	UIIMGThumbsJSON     string
	UIIMGsJSON          string
	CompatabilitiesJSON string
	SiblingsJSON        string
}

func (DBRemoteAddon) TableName() string { return "remote_addons" }

type DBCategory struct {
	ID            string `gorm:"primaryKey"`
	Name          string
	IconURL       string
	ParentID      string
	ParentIDsJSON string
	Count         int
}

func (DBCategory) TableName() string { return "categories" }

type DBCacheMeta struct {
	Key   string `gorm:"primaryKey"`
	Value string
}

func (DBCacheMeta) TableName() string { return "cache_meta" }

type DBInstallRecord struct {
	UID          string `gorm:"primaryKey"`
	InstalledMD5 string
}

func (DBInstallRecord) TableName() string { return "install_records" }

type DBSetting struct {
	Key   string `gorm:"primaryKey"`
	Value string
}

func (DBSetting) TableName() string { return "settings" }

type DBSearchPreset struct {
	ID             string `gorm:"primaryKey"`
	Name           string `gorm:"uniqueIndex"`
	SearchQuery    string
	CategoryFilter string
	SortBy         string
	HideInstalled  bool
	CreatedAt      string
}

func (DBSearchPreset) TableName() string { return "search_presets" }

func OpenDB(path string) (*gorm.DB, error) {
	dsn := fmt.Sprintf("%s?_pragma=journal_mode(WAL)&_pragma=synchronous(NORMAL)", path)
	db, err := gorm.Open(sqlite.Open(dsn), &gorm.Config{
		Logger:      logger.Default.LogMode(logger.Silent),
		PrepareStmt: true,
	})
	if err != nil {
		return nil, fmt.Errorf("open sqlite db: %w", err)
	}
	sqlDB, err := db.DB()
	if err != nil {
		return nil, fmt.Errorf("open sql db handle: %w", err)
	}
	sqlDB.SetMaxOpenConns(4)
	sqlDB.SetMaxIdleConns(4)
	sqlDB.SetConnMaxIdleTime(10 * time.Minute)
	sqlDB.SetConnMaxLifetime(1 * time.Hour)
	if err := db.AutoMigrate(&DBRemoteAddon{}, &DBCategory{}, &DBCacheMeta{}, &DBSetting{}, &DBSearchPreset{}, &DBInstallRecord{}); err != nil {
		return nil, fmt.Errorf("automigrate: %w", err)
	}
	return db, nil
}

func toJSON(v any) string {
	b, _ := json.Marshal(v)
	return string(b)
}

func fromJSONStrings(s string) []string {
	if s == "" || s == "null" {
		return nil
	}
	var out []string
	_ = json.Unmarshal([]byte(s), &out)
	return out
}

func fromJSONGameVersions(s string) []GameVersion {
	if s == "" || s == "null" {
		return nil
	}
	var out []GameVersion
	_ = json.Unmarshal([]byte(s), &out)
	return out
}
func toDBRemoteAddon(r RemoteAddon) DBRemoteAddon {
	return DBRemoteAddon{
		UID:                 r.UID,
		CategoryID:          r.CategoryID,
		UIName:              r.UIName,
		UIAuthorName:        r.UIAuthorName,
		UIDate:              r.UIDate,
		UIVersion:           r.UIVersion,
		UIDirsJSON:          toJSON(r.UIDirs),
		UIFileInfoURL:       r.UIFileInfoURL,
		UIDownloadTotal:     r.UIDownloadTotal,
		UIDownloadMonthly:   r.UIDownloadMonthly,
		UIFavoriteTotal:     r.UIFavoriteTotal,
		UIIMGThumbsJSON:     toJSON(r.UIIMGThumbs),
		UIIMGsJSON:          toJSON(r.UIIMGs),
		CompatabilitiesJSON: toJSON(r.Compatabilities),
		SiblingsJSON:        toJSON(r.Siblings),
	}
}

func fromDBRemoteAddon(d DBRemoteAddon) RemoteAddon {
	return RemoteAddon{
		UID:               d.UID,
		CategoryID:        d.CategoryID,
		UIName:            d.UIName,
		UIAuthorName:      d.UIAuthorName,
		UIDate:            d.UIDate,
		UIVersion:         d.UIVersion,
		UIDirs:            fromJSONStrings(d.UIDirsJSON),
		UIFileInfoURL:     d.UIFileInfoURL,
		UIDownloadTotal:   d.UIDownloadTotal,
		UIDownloadMonthly: d.UIDownloadMonthly,
		UIFavoriteTotal:   d.UIFavoriteTotal,
		UIIMGThumbs:       fromJSONStrings(d.UIIMGThumbsJSON),
		UIIMGs:            fromJSONStrings(d.UIIMGsJSON),
		Compatabilities:   fromJSONGameVersions(d.CompatabilitiesJSON),
		Siblings:          fromJSONStrings(d.SiblingsJSON),
	}
}
func toDBCategory(c Category) DBCategory {
	return DBCategory{
		ID:            c.ID,
		Name:          c.Name,
		IconURL:       c.IconURL,
		ParentID:      c.ParentID,
		ParentIDsJSON: toJSON(c.ParentIDs),
		Count:         c.Count,
	}
}

func fromDBCategory(d DBCategory) Category {
	return Category{
		ID:        d.ID,
		Name:      d.Name,
		IconURL:   d.IconURL,
		ParentID:  d.ParentID,
		ParentIDs: fromJSONStrings(d.ParentIDsJSON),
		Count:     d.Count,
	}
}

func SaveInstallMD5(db *gorm.DB, uid, md5Hash string) error {
	if db == nil || uid == "" || md5Hash == "" {
		return nil
	}
	return db.Save(&DBInstallRecord{UID: uid, InstalledMD5: md5Hash}).Error
}

func GetInstallMD5s(db *gorm.DB, uids []string) map[string]string {
	if db == nil || len(uids) == 0 {
		return nil
	}
	var rows []DBInstallRecord
	if err := db.Select("uid", "installed_md5").Where("uid IN ?", uids).Find(&rows).Error; err != nil {
		return nil
	}
	result := make(map[string]string, len(rows))
	for _, r := range rows {
		result[r.UID] = r.InstalledMD5
	}
	return result
}
