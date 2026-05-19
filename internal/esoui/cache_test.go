package esoui

import (
	"Scribe/internal/addon"
	"Scribe/internal/scanner"
	"path/filepath"
	"reflect"
	"testing"
	"time"

	"gorm.io/gorm"
)

func TestOpenDBConfiguresSQLitePragmas(t *testing.T) {
	t.Setenv(sqliteMmapSizeEnv, "64")
	db, err := OpenDB(filepath.Join(t.TempDir(), "pragma.db"))
	if err != nil {
		t.Fatalf("OpenDB: %v", err)
	}

	if got := rawPragmaString(t, db, "journal_mode"); got != "wal" {
		t.Fatalf("journal_mode = %q, want wal", got)
	}
	if got := rawPragmaInt(t, db, "busy_timeout"); got != sqliteBusyTimeoutMS {
		t.Fatalf("busy_timeout = %d, want %d", got, sqliteBusyTimeoutMS)
	}
	if got := rawPragmaInt(t, db, "journal_size_limit"); got != sqliteJournalSizeLimit {
		t.Fatalf("journal_size_limit = %d, want %d", got, sqliteJournalSizeLimit)
	}
	if got := rawPragmaInt(t, db, "cache_size"); got != -sqliteCacheSizeKiB {
		t.Fatalf("cache_size = %d, want %d", got, -sqliteCacheSizeKiB)
	}
	if got := rawPragmaInt64(t, db, "mmap_size"); got != 64*sqliteBytesPerMebiByte {
		t.Fatalf("mmap_size = %d, want %d", got, 64*sqliteBytesPerMebiByte)
	}
}

func TestCacheRoundTripPersistsFeedsAddonsAndCategories(t *testing.T) {
	db := newCacheTestDB(t)
	cache := NewCacheFromDB(db)

	feeds := APIFeeds{
		FileList:    "https://api.example/files",
		FileDetails: "https://api.example/details",
		Categories:  "https://api.example/categories",
		ListFiles:   "https://api.example/list",
	}
	addons := []RemoteAddon{{
		UID:               "123",
		CategoryID:        "cat",
		UIName:            "Remote Addon",
		UIAuthorName:      "Author",
		UIDate:            "2026-05-18",
		UIVersion:         "1.2.3",
		UIDirs:            []string{"RemoteAddon", "RemoteLib"},
		UIFileInfoURL:     "https://example/addon",
		UIDownloadTotal:   100,
		UIDownloadMonthly: 10,
		UIFavoriteTotal:   5,
		UIIMGThumbs:       []string{"thumb-a", "thumb-b"},
		UIIMGs:            []string{"image-a", "image-b"},
		Compatabilities:   []GameVersion{{Version: "10.0", Name: "ESO"}},
		Siblings:          []string{"sibling"},
	}}
	categories := []Category{{
		ID:        "cat",
		Name:      "Category",
		IconURL:   "https://example/icon.png",
		ParentID:  "parent",
		ParentIDs: []string{"root", "parent"},
		Count:     42,
	}}

	if err := cache.Set(feeds, addons, categories); err != nil {
		t.Fatalf("Set: %v", err)
	}
	reloaded := NewCacheFromDB(db)
	gotAddons, gotFeeds, gotCategories := reloaded.Get()

	if gotFeeds == nil || !reflect.DeepEqual(*gotFeeds, feeds) {
		t.Fatalf("feeds = %#v, want %#v", gotFeeds, feeds)
	}
	if !reflect.DeepEqual(gotAddons, addons) {
		t.Fatalf("addons = %#v, want %#v", gotAddons, addons)
	}
	if !reflect.DeepEqual(gotCategories, categories) {
		t.Fatalf("categories = %#v, want %#v", gotCategories, categories)
	}
	if reloaded.IsStale() {
		t.Fatal("freshly reloaded cache is stale")
	}
}

func TestCacheStaleDetectionAndInvalidate(t *testing.T) {
	cache := NewCacheFromDB(newCacheTestDB(t))
	if !cache.IsStale() {
		t.Fatal("empty cache should be stale")
	}
	if err := cache.Set(APIFeeds{}, []RemoteAddon{{UID: "1"}}, []Category{{ID: "cat"}}); err != nil {
		t.Fatalf("Set: %v", err)
	}
	if cache.IsStale() {
		t.Fatal("fresh cache should not be stale")
	}

	cache.mu.Lock()
	cache.snap.FetchedAt = time.Now().Add(-cacheTTL - time.Minute)
	cache.mu.Unlock()
	if !cache.IsStale() {
		t.Fatal("expired cache should be stale")
	}

	cache.Invalidate()
	addons, feeds, categories := cache.Get()
	if addons != nil || feeds != nil || categories != nil {
		t.Fatalf("Get after Invalidate = %#v %#v %#v, want nils", addons, feeds, categories)
	}
}

func TestCacheSchemaMismatchInvalidatesPersistedRows(t *testing.T) {
	db := newCacheTestDB(t)
	cache := NewCacheFromDB(db)
	if err := cache.Set(APIFeeds{FileList: "files"}, []RemoteAddon{{UID: "1"}}, []Category{{ID: "cat"}}); err != nil {
		t.Fatalf("Set: %v", err)
	}
	if err := db.Model(&DBCacheMeta{}).Where("key = ?", metaKeySchemaVersion).Update("value", "old").Error; err != nil {
		t.Fatalf("force schema mismatch: %v", err)
	}

	reloaded := NewCacheFromDB(db)
	addons, feeds, categories := reloaded.Get()
	if addons != nil || feeds != nil || categories != nil {
		t.Fatalf("Get after schema mismatch = %#v %#v %#v, want nils", addons, feeds, categories)
	}

	var addonCount int64
	if err := db.Model(&DBRemoteAddon{}).Count(&addonCount).Error; err != nil {
		t.Fatalf("count addons: %v", err)
	}
	var categoryCount int64
	if err := db.Model(&DBCategory{}).Count(&categoryCount).Error; err != nil {
		t.Fatalf("count categories: %v", err)
	}
	var metaCount int64
	if err := db.Model(&DBCacheMeta{}).Count(&metaCount).Error; err != nil {
		t.Fatalf("count meta: %v", err)
	}
	if addonCount != 0 || categoryCount != 0 || metaCount != 0 {
		t.Fatalf("cache rows remain after schema mismatch: addons=%d categories=%d meta=%d", addonCount, categoryCount, metaCount)
	}
}

func TestScannerCacheStoreRoundTrip(t *testing.T) {
	db := newCacheTestDB(t)
	store := NewScannerCacheStore(db)
	addonPath := filepath.Join(t.TempDir(), "AddOns")
	entries := []scanner.CachedAddon{{
		FolderName:  "LibFoo",
		Fingerprint: "fingerprint",
		Addon: &addon.Addon{
			ID:         "LibFoo",
			FolderName: "LibFoo",
			Title:      "Lib Foo",
			Version:    "1.2.3",
		},
	}}

	if err := store.SaveScanCache(addonPath, entries); err != nil {
		t.Fatalf("SaveScanCache: %v", err)
	}
	got, err := store.LoadScanCache(addonPath)
	if err != nil {
		t.Fatalf("LoadScanCache: %v", err)
	}
	entry, ok := got["LibFoo"]
	if !ok {
		t.Fatalf("cache keys = %#v, want LibFoo", got)
	}
	if entry.Fingerprint != "fingerprint" || entry.Addon == nil || entry.Addon.Title != "Lib Foo" {
		t.Fatalf("cached entry = %+v", entry)
	}

	replacement := []scanner.CachedAddon{{
		FolderName:  "LibBar",
		Fingerprint: "next",
		Addon: &addon.Addon{
			ID:         "LibBar",
			FolderName: "LibBar",
			Title:      "Lib Bar",
		},
	}}
	if err := store.SaveScanCache(addonPath, replacement); err != nil {
		t.Fatalf("SaveScanCache replacement: %v", err)
	}
	got, err = store.LoadScanCache(addonPath)
	if err != nil {
		t.Fatalf("LoadScanCache replacement: %v", err)
	}
	if _, ok := got["LibFoo"]; ok {
		t.Fatalf("stale entry remained after replacement save: %#v", got)
	}
	if got["LibBar"].Fingerprint != "next" {
		t.Fatalf("replacement entry = %#v", got["LibBar"])
	}
}

func newCacheTestDB(t *testing.T) *gorm.DB {
	t.Helper()

	db, err := OpenDB(filepath.Join(t.TempDir(), "cache.db"))
	if err != nil {
		t.Fatalf("OpenDB: %v", err)
	}
	return db
}

func rawPragmaString(t *testing.T, db *gorm.DB, name string) string {
	t.Helper()
	var got string
	if err := db.Raw("PRAGMA " + name).Scan(&got).Error; err != nil {
		t.Fatalf("PRAGMA %s: %v", name, err)
	}
	return got
}

func rawPragmaInt(t *testing.T, db *gorm.DB, name string) int {
	t.Helper()
	var got int
	if err := db.Raw("PRAGMA " + name).Scan(&got).Error; err != nil {
		t.Fatalf("PRAGMA %s: %v", name, err)
	}
	return got
}

func rawPragmaInt64(t *testing.T, db *gorm.DB, name string) int64 {
	t.Helper()
	var got int64
	if err := db.Raw("PRAGMA " + name).Scan(&got).Error; err != nil {
		t.Fatalf("PRAGMA %s: %v", name, err)
	}
	return got
}
