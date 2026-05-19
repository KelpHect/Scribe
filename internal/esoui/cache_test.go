package esoui

import (
	"Scribe/internal/addon"
	"Scribe/internal/scanner"
	"encoding/json"
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

func TestCacheSnapshotWinsWhenCompatibilityRowsDrift(t *testing.T) {
	db := newCacheTestDB(t)
	cache := NewCacheFromDB(db)
	feeds := APIFeeds{ListFiles: "fixture"}
	addons := []RemoteAddon{{
		UID:        "123",
		CategoryID: "cat",
		UIName:     "Remote Addon",
		UIVersion:  "1.0",
		UIDirs:     []string{"RemoteAddon"},
	}}
	categories := []Category{{ID: "cat", Name: "Category"}}

	if err := cache.Set(feeds, addons, categories); err != nil {
		t.Fatalf("Set: %v", err)
	}
	if err := db.Model(&DBRemoteAddon{}).Where("uid = ?", "123").Update("ui_name", "Sentinel Drift").Error; err != nil {
		t.Fatalf("force row-table drift: %v", err)
	}
	reloaded := NewCacheFromDB(db)
	got, _, _ := reloaded.Get()
	if len(got) != 1 || got[0].UIName != "Remote Addon" {
		t.Fatalf("snapshot load = %#v, want original snapshot value", got)
	}
}

func TestCacheLoadsLegacyJSONSnapshot(t *testing.T) {
	db := newCacheTestDB(t)
	feeds := APIFeeds{ListFiles: "fixture"}
	fetchedAt := time.Now().UTC()
	addons := []RemoteAddon{{
		UID:        "legacy",
		CategoryID: "cat",
		UIName:     "Legacy Snapshot Addon",
		UIVersion:  "1.0",
		UIDirs:     []string{"LegacySnapshotAddon"},
	}}
	categories := []Category{{ID: "cat", Name: "Category"}}
	feedsJSON := mustJSON(t, feeds)
	fetchedAtJSON := mustJSON(t, fetchedAt)
	addonsJSON := mustJSON(t, addons)
	categoriesJSON := mustJSON(t, categories)

	if err := db.Create(&DBCacheMeta{Key: metaKeySchemaVersion, Value: cacheSchemaVersion}).Error; err != nil {
		t.Fatalf("write schema meta: %v", err)
	}
	if err := db.Create(&DBRemoteCatalogSnapshot{
		Key:            remoteCatalogSnapshotKey,
		FeedURLsJSON:   feedsJSON,
		FetchedAtJSON:  fetchedAtJSON,
		AddonsJSON:     addonsJSON,
		CategoriesJSON: categoriesJSON,
		CatalogHash:    "legacy-hash",
	}).Error; err != nil {
		t.Fatalf("write legacy snapshot: %v", err)
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
}

func TestCacheUnchangedSaveUpgradesLegacyJSONSnapshotToBinary(t *testing.T) {
	db := newCacheTestDB(t)
	feeds := APIFeeds{ListFiles: "fixture"}
	addons := []RemoteAddon{{
		UID:        "legacy",
		CategoryID: "cat",
		UIName:     "Legacy Snapshot Addon",
		UIVersion:  "1.0",
		UIDirs:     []string{"LegacySnapshotAddon"},
	}}
	categories := []Category{{ID: "cat", Name: "Category"}}
	legacySnap := &snapshot{
		FetchedAt:  time.Now().UTC(),
		FeedURLs:   feeds,
		Addons:     addons,
		Categories: categories,
	}
	payload, err := buildCatalogPayload(legacySnap)
	if err != nil {
		t.Fatalf("buildCatalogPayload: %v", err)
	}
	if err := db.Create(&DBCacheMeta{Key: metaKeySchemaVersion, Value: cacheSchemaVersion}).Error; err != nil {
		t.Fatalf("write schema meta: %v", err)
	}
	if err := db.Create(&DBCacheMeta{Key: metaKeyCatalogHash, Value: payload.Hash}).Error; err != nil {
		t.Fatalf("write catalog hash meta: %v", err)
	}
	if err := db.Create(&DBRemoteCatalogSnapshot{
		Key:            remoteCatalogSnapshotKey,
		FeedURLsJSON:   payload.FeedURLsJSON,
		FetchedAtJSON:  payload.FetchedAtJSON,
		AddonsJSON:     mustJSON(t, addons),
		CategoriesJSON: mustJSON(t, categories),
		CatalogHash:    payload.Hash,
	}).Error; err != nil {
		t.Fatalf("write legacy snapshot: %v", err)
	}

	cache := NewCacheFromDB(db)
	if err := cache.Set(feeds, addons, categories); err != nil {
		t.Fatalf("Set unchanged legacy snapshot: %v", err)
	}
	var row DBRemoteCatalogSnapshot
	if err := db.Where("key = ?", remoteCatalogSnapshotKey).First(&row).Error; err != nil {
		t.Fatalf("read upgraded snapshot: %v", err)
	}
	if len(row.AddonsBlob) == 0 || len(row.CategoriesBlob) == 0 {
		t.Fatalf("snapshot was not upgraded to binary blobs: addon=%d category=%d", len(row.AddonsBlob), len(row.CategoriesBlob))
	}
	var compatRows int64
	if err := db.Model(&DBRemoteAddon{}).Count(&compatRows).Error; err != nil {
		t.Fatalf("count compatibility rows: %v", err)
	}
	if compatRows != 0 {
		t.Fatalf("unchanged binary upgrade rewrote compatibility rows: %d", compatRows)
	}
}

func TestCacheUnchangedSaveSkipsCompatibilityRowRewrite(t *testing.T) {
	db := newCacheTestDB(t)
	cache := NewCacheFromDB(db)
	feeds := APIFeeds{ListFiles: "fixture"}
	addons := []RemoteAddon{{
		UID:        "123",
		CategoryID: "cat",
		UIName:     "Remote Addon",
		UIVersion:  "1.0",
		UIDirs:     []string{"RemoteAddon"},
	}}
	categories := []Category{{ID: "cat", Name: "Category"}}

	if err := cache.Set(feeds, addons, categories); err != nil {
		t.Fatalf("Set initial: %v", err)
	}
	if err := db.Model(&DBRemoteAddon{}).Where("uid = ?", "123").Update("ui_name", "Sentinel Drift").Error; err != nil {
		t.Fatalf("force row-table drift: %v", err)
	}
	var beforeHash DBCacheMeta
	if err := db.Where("key = ?", metaKeyCatalogHash).First(&beforeHash).Error; err != nil {
		t.Fatalf("read catalog hash: %v", err)
	}
	time.Sleep(time.Millisecond)
	if err := cache.Set(feeds, addons, categories); err != nil {
		t.Fatalf("Set unchanged: %v", err)
	}

	var row DBRemoteAddon
	if err := db.Where("uid = ?", "123").First(&row).Error; err != nil {
		t.Fatalf("read compatibility row: %v", err)
	}
	if row.UIName != "Sentinel Drift" {
		t.Fatalf("unchanged save rewrote compatibility row: ui_name=%q", row.UIName)
	}
	var afterHash DBCacheMeta
	if err := db.Where("key = ?", metaKeyCatalogHash).First(&afterHash).Error; err != nil {
		t.Fatalf("read catalog hash after unchanged save: %v", err)
	}
	if afterHash.Value != beforeHash.Value {
		t.Fatalf("catalog hash changed on identical payload: before=%q after=%q", beforeHash.Value, afterHash.Value)
	}
	var fetched DBCacheMeta
	if err := db.Where("key = ?", metaKeyFetchedAt).First(&fetched).Error; err != nil {
		t.Fatalf("read fetched_at after unchanged save: %v", err)
	}
	if fetched.Value == "" {
		t.Fatal("fetched_at metadata was not maintained")
	}
}

func TestCacheChangedSaveDiffsCompatibilityRows(t *testing.T) {
	db := newCacheTestDB(t)
	cache := NewCacheFromDB(db)
	feeds := APIFeeds{ListFiles: "fixture"}
	initial := []RemoteAddon{
		{UID: "a", CategoryID: "cat-a", UIName: "Addon A", UIVersion: "1.0", UIDirs: []string{"A"}},
		{UID: "b", CategoryID: "cat-b", UIName: "Addon B", UIVersion: "1.0", UIDirs: []string{"B"}},
	}
	if err := cache.Set(feeds, initial, []Category{{ID: "cat-a", Name: "A"}, {ID: "cat-b", Name: "B"}}); err != nil {
		t.Fatalf("Set initial: %v", err)
	}

	next := []RemoteAddon{
		{UID: "b", CategoryID: "cat-b", UIName: "Addon B", UIVersion: "2.0", UIDirs: []string{"B"}},
		{UID: "c", CategoryID: "cat-c", UIName: "Addon C", UIVersion: "1.0", UIDirs: []string{"C"}},
	}
	if err := cache.Set(feeds, next, []Category{{ID: "cat-b", Name: "B"}, {ID: "cat-c", Name: "C"}}); err != nil {
		t.Fatalf("Set changed: %v", err)
	}

	var addonRows []DBRemoteAddon
	if err := db.Order("uid").Find(&addonRows).Error; err != nil {
		t.Fatalf("read addon rows: %v", err)
	}
	if len(addonRows) != 2 || addonRows[0].UID != "b" || addonRows[0].UIVersion != "2.0" || addonRows[1].UID != "c" {
		t.Fatalf("addon compatibility rows = %#v", addonRows)
	}
	var categoryRows []DBCategory
	if err := db.Order("id").Find(&categoryRows).Error; err != nil {
		t.Fatalf("read category rows: %v", err)
	}
	if len(categoryRows) != 2 || categoryRows[0].ID != "cat-b" || categoryRows[1].ID != "cat-c" {
		t.Fatalf("category compatibility rows = %#v", categoryRows)
	}

	reloaded := NewCacheFromDB(db)
	got, _, gotCategories := reloaded.Get()
	if !reflect.DeepEqual(got, next) {
		t.Fatalf("snapshot addons = %#v, want %#v", got, next)
	}
	if len(gotCategories) != 2 || gotCategories[0].ID != "cat-b" || gotCategories[1].ID != "cat-c" {
		t.Fatalf("snapshot categories = %#v", gotCategories)
	}
}

func TestCatalogHashIgnoresAddonAndCategoryOrder(t *testing.T) {
	feedsJSON := []byte(`{"ListFiles":"fixture"}`)
	addonsA := []DBRemoteAddon{{UID: "b", UIName: "B"}, {UID: "a", UIName: "A"}}
	addonsB := []DBRemoteAddon{{UID: "a", UIName: "A"}, {UID: "b", UIName: "B"}}
	categoriesA := []DBCategory{{ID: "2", Name: "Two"}, {ID: "1", Name: "One"}}
	categoriesB := []DBCategory{{ID: "1", Name: "One"}, {ID: "2", Name: "Two"}}

	hashA, err := hashCatalog(feedsJSON, addonsA, categoriesA)
	if err != nil {
		t.Fatalf("hashCatalog A: %v", err)
	}
	hashB, err := hashCatalog(feedsJSON, addonsB, categoriesB)
	if err != nil {
		t.Fatalf("hashCatalog B: %v", err)
	}
	if hashA != hashB {
		t.Fatalf("hash differs by row order: %q != %q", hashA, hashB)
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
	var snapshotCount int64
	if err := db.Model(&DBRemoteCatalogSnapshot{}).Count(&snapshotCount).Error; err != nil {
		t.Fatalf("count snapshots: %v", err)
	}
	if addonCount != 0 || categoryCount != 0 || metaCount != 0 || snapshotCount != 0 {
		t.Fatalf("cache rows remain after schema mismatch: addons=%d categories=%d meta=%d snapshots=%d", addonCount, categoryCount, metaCount, snapshotCount)
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

func mustJSON(t *testing.T, value any) string {
	t.Helper()
	payload, err := json.Marshal(value)
	if err != nil {
		t.Fatalf("marshal json: %v", err)
	}
	return string(payload)
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
