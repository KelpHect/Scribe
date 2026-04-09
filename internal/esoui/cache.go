package esoui

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	"gorm.io/gorm"
)

const (
	dbFileName = "esoui_cache.db"
	cacheTTL   = 4 * time.Hour

	metaKeyFeedURLs      = "feed_urls"
	metaKeyFetchedAt     = "fetched_at"
	metaKeySchemaVersion = "schema_version"

	cacheSchemaVersion = "2"
)

type snapshot struct {
	FetchedAt  time.Time
	FeedURLs   APIFeeds
	Addons     []RemoteAddon
	Categories []Category
}

type Cache struct {
	mu   sync.RWMutex
	snap *snapshot
	db   *gorm.DB
}

func NewCache() (*Cache, error) {
	dir, err := os.UserConfigDir()
	if err != nil {
		return nil, fmt.Errorf("get user config dir: %w", err)
	}
	// keeping the old app dir name so existing installs keep their cache and settings
	appDir := filepath.Join(dir, "Scribe")
	if err := os.MkdirAll(appDir, 0o755); err != nil {
		return nil, fmt.Errorf("create config dir: %w", err)
	}
	dbPath := filepath.Join(appDir, dbFileName)
	db, err := OpenDB(dbPath)
	if err != nil {
		return nil, fmt.Errorf("open cache db: %w", err)
	}
	return NewCacheFromDB(db), nil
}

func NewCacheFromDB(db *gorm.DB) *Cache {
	c := &Cache{db: db}
	_ = c.loadFromDB()
	return c
}

func OpenAppDB() (*gorm.DB, error) {
	dir, err := os.UserConfigDir()
	if err != nil {
		return nil, fmt.Errorf("get user config dir: %w", err)
	}
	// keeping the old app dir name so existing installs keep their cache and settings
	appDir := filepath.Join(dir, "Scribe")
	if err := os.MkdirAll(appDir, 0o755); err != nil {
		return nil, fmt.Errorf("create config dir: %w", err)
	}
	return OpenDB(filepath.Join(appDir, dbFileName))
}

func (c *Cache) IsStale() bool {
	c.mu.RLock()
	defer c.mu.RUnlock()
	if c.snap == nil {
		return true
	}
	return time.Since(c.snap.FetchedAt) > cacheTTL
}

func (c *Cache) Get() ([]RemoteAddon, *APIFeeds, []Category) {
	c.mu.RLock()
	defer c.mu.RUnlock()
	if c.snap == nil {
		return nil, nil, nil
	}
	feeds := c.snap.FeedURLs
	return c.snap.Addons, &feeds, c.snap.Categories
}

func (c *Cache) Set(feeds APIFeeds, addons []RemoteAddon, categories []Category) error {
	now := time.Now()
	snap := &snapshot{
		FetchedAt:  now,
		FeedURLs:   feeds,
		Addons:     addons,
		Categories: categories,
	}

	c.mu.Lock()
	c.snap = snap
	c.mu.Unlock()

	return c.saveToDB(snap)
}

func (c *Cache) Invalidate() {
	c.mu.Lock()
	c.snap = nil
	c.mu.Unlock()

	c.db.Exec("DELETE FROM remote_addons")
	c.db.Exec("DELETE FROM categories")
	c.db.Exec("DELETE FROM cache_meta")
}

func (c *Cache) loadFromDB() error {
	var schemaMeta DBCacheMeta
	if err := c.db.Select("key", "value").Where("key = ?", metaKeySchemaVersion).First(&schemaMeta).Error; err != nil {
		c.Invalidate()
		return fmt.Errorf("schema version missing: invalidating cache")
	}
	if schemaMeta.Value != cacheSchemaVersion {
		c.Invalidate()
		return fmt.Errorf("schema version mismatch (have %q, want %q): invalidating cache", schemaMeta.Value, cacheSchemaVersion)
	}

	var feedMeta DBCacheMeta
	if err := c.db.Select("key", "value").Where("key = ?", metaKeyFeedURLs).First(&feedMeta).Error; err != nil {
		return fmt.Errorf("read feed meta: %w", err)
	}
	var feeds APIFeeds
	if err := json.Unmarshal([]byte(feedMeta.Value), &feeds); err != nil {
		return fmt.Errorf("decode feed urls: %w", err)
	}

	var tsMeta DBCacheMeta
	if err := c.db.Select("key", "value").Where("key = ?", metaKeyFetchedAt).First(&tsMeta).Error; err != nil {
		return fmt.Errorf("read timestamp meta: %w", err)
	}
	var fetchedAt time.Time
	if err := json.Unmarshal([]byte(tsMeta.Value), &fetchedAt); err != nil {
		return fmt.Errorf("decode fetched_at: %w", err)
	}

	var dbAddons []DBRemoteAddon
	if err := c.db.Find(&dbAddons).Error; err != nil {
		return fmt.Errorf("read addons: %w", err)
	}
	addons := make([]RemoteAddon, len(dbAddons))
	for i, d := range dbAddons {
		addons[i] = fromDBRemoteAddon(d)
	}

	var dbCats []DBCategory
	if err := c.db.Find(&dbCats).Error; err != nil {
		return fmt.Errorf("read categories: %w", err)
	}
	cats := make([]Category, len(dbCats))
	for i, d := range dbCats {
		cats[i] = fromDBCategory(d)
	}

	c.mu.Lock()
	c.snap = &snapshot{
		FetchedAt:  fetchedAt,
		FeedURLs:   feeds,
		Addons:     addons,
		Categories: cats,
	}
	c.mu.Unlock()
	return nil
}

func (c *Cache) saveToDB(snap *snapshot) error {
	return c.db.Transaction(func(tx *gorm.DB) error {
		if err := tx.Exec("DELETE FROM remote_addons").Error; err != nil {
			return err
		}
		if err := tx.Exec("DELETE FROM categories").Error; err != nil {
			return err
		}
		if err := tx.Exec("DELETE FROM cache_meta").Error; err != nil {
			return err
		}

		dbAddons := make([]DBRemoteAddon, len(snap.Addons))
		for i, a := range snap.Addons {
			dbAddons[i] = toDBRemoteAddon(a)
		}
		if len(dbAddons) > 0 {
			if err := tx.CreateInBatches(dbAddons, 500).Error; err != nil {
				return fmt.Errorf("insert addons: %w", err)
			}
		}
		dbCats := make([]DBCategory, len(snap.Categories))
		for i, cat := range snap.Categories {
			dbCats[i] = toDBCategory(cat)
		}
		if len(dbCats) > 0 {
			if err := tx.CreateInBatches(dbCats, 200).Error; err != nil {
				return fmt.Errorf("insert categories: %w", err)
			}
		}
		feedsJSON, _ := json.Marshal(snap.FeedURLs)
		fetchedAtJSON, _ := json.Marshal(snap.FetchedAt)
		meta := []DBCacheMeta{
			{Key: metaKeyFeedURLs, Value: string(feedsJSON)},
			{Key: metaKeyFetchedAt, Value: string(fetchedAtJSON)},
			{Key: metaKeySchemaVersion, Value: cacheSchemaVersion},
		}
		if err := tx.Create(&meta).Error; err != nil {
			return fmt.Errorf("insert cache meta: %w", err)
		}

		return nil
	})
}
