package esoui

import (
	"crypto/sha256"
	"database/sql"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
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
	metaKeyCatalogHash   = "catalog_hash"

	cacheSchemaVersion = "2"

	remoteCatalogSnapshotKey = "current"
	remoteAddonSnapshotMagic = "scribe-remote-addons-v1"
	categorySnapshotMagic    = "scribe-categories-v1"
	maxSnapshotItems         = 1_000_000
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
	c.db.Exec("DELETE FROM remote_catalog_snapshots")
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

	if err := c.loadSnapshotFromDB(); err == nil {
		return nil
	}

	return c.loadRowsFromDB()
}

func (c *Cache) loadRowsFromDB() error {
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

func (c *Cache) loadSnapshotFromDB() error {
	var row DBRemoteCatalogSnapshot
	sqlDB, err := c.db.DB()
	if err != nil {
		return fmt.Errorf("open sql db handle: %w", err)
	}
	err = sqlDB.QueryRow(
		`SELECT feed_urls_json, addons_json, categories_json, addons_blob, categories_blob, fetched_at_json
		   FROM remote_catalog_snapshots
		  WHERE key = ?`,
		remoteCatalogSnapshotKey,
	).Scan(
		&row.FeedURLsJSON,
		&row.AddonsJSON,
		&row.CategoriesJSON,
		&row.AddonsBlob,
		&row.CategoriesBlob,
		&row.FetchedAtJSON,
	)
	if err != nil {
		return err
	}
	var feeds APIFeeds
	if err := json.Unmarshal([]byte(row.FeedURLsJSON), &feeds); err != nil {
		return fmt.Errorf("decode snapshot feed urls: %w", err)
	}
	var fetchedAt time.Time
	if err := json.Unmarshal([]byte(row.FetchedAtJSON), &fetchedAt); err != nil {
		return fmt.Errorf("decode snapshot fetched_at: %w", err)
	}
	addons, categories, err := decodeSnapshotCatalog(row)
	if err != nil {
		return err
	}

	c.mu.Lock()
	c.snap = &snapshot{
		FetchedAt:  fetchedAt,
		FeedURLs:   feeds,
		Addons:     addons,
		Categories: categories,
	}
	c.mu.Unlock()
	return nil
}

func (c *Cache) saveToDB(snap *snapshot) error {
	payload, err := buildCatalogPayload(snap)
	if err != nil {
		return err
	}
	sqlDB, err := c.db.DB()
	if err != nil {
		return fmt.Errorf("open sql db handle: %w", err)
	}
	tx, err := sqlDB.Begin()
	if err != nil {
		return fmt.Errorf("begin cache transaction: %w", err)
	}
	committed := false
	defer func() {
		if !committed {
			_ = tx.Rollback()
		}
	}()

	existingHash, err := readCatalogHash(tx)
	if err != nil {
		return err
	}
	if existingHash == payload.Hash {
		hasBinarySnapshot, err := remoteCatalogSnapshotHasBinary(tx)
		if err != nil {
			return err
		}
		if !hasBinarySnapshot {
			if err := payload.encodeSnapshot(snap); err != nil {
				return err
			}
			if err := upsertRemoteCatalogSnapshot(tx, payload); err != nil {
				return err
			}
		}
		if err := upsertCacheMeta(tx, metaKeyFetchedAt, payload.FetchedAtJSON); err != nil {
			return err
		}
		if err := updateRemoteCatalogSnapshotFetchedAt(tx, payload.FetchedAtJSON); err != nil {
			return err
		}
		if err := tx.Commit(); err != nil {
			return fmt.Errorf("commit unchanged cache metadata: %w", err)
		}
		committed = true
		return nil
	}

	if err := payload.encodeSnapshot(snap); err != nil {
		return err
	}
	if err := upsertRemoteCatalogSnapshot(tx, payload); err != nil {
		return err
	}
	if err := saveRemoteAddonRows(tx, payload.AddonRows); err != nil {
		return err
	}
	if err := saveCategoryRows(tx, payload.CategoryRows); err != nil {
		return err
	}
	if err := upsertCacheMeta(tx, metaKeyFeedURLs, payload.FeedURLsJSON); err != nil {
		return err
	}
	if err := upsertCacheMeta(tx, metaKeyFetchedAt, payload.FetchedAtJSON); err != nil {
		return err
	}
	if err := upsertCacheMeta(tx, metaKeySchemaVersion, cacheSchemaVersion); err != nil {
		return err
	}
	if err := upsertCacheMeta(tx, metaKeyCatalogHash, payload.Hash); err != nil {
		return err
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit cache transaction: %w", err)
	}
	committed = true
	return nil
}

type catalogPayload struct {
	FeedURLsJSON   string
	FetchedAtJSON  string
	AddonsJSON     string
	CategoriesJSON string
	AddonsBlob     []byte
	CategoriesBlob []byte
	Hash           string
	AddonRows      []DBRemoteAddon
	CategoryRows   []DBCategory
}

func buildCatalogPayload(snap *snapshot) (*catalogPayload, error) {
	addonRows := make([]DBRemoteAddon, len(snap.Addons))
	for i, addon := range snap.Addons {
		addonRows[i] = toDBRemoteAddon(addon)
	}
	categoryRows := make([]DBCategory, len(snap.Categories))
	for i, category := range snap.Categories {
		categoryRows[i] = toDBCategory(category)
	}

	feedURLsJSON, err := json.Marshal(snap.FeedURLs)
	if err != nil {
		return nil, fmt.Errorf("encode feed urls: %w", err)
	}
	fetchedAtJSON, err := json.Marshal(snap.FetchedAt)
	if err != nil {
		return nil, fmt.Errorf("encode fetched_at: %w", err)
	}
	hash, err := hashCatalog(feedURLsJSON, addonRows, categoryRows)
	if err != nil {
		return nil, err
	}

	return &catalogPayload{
		FeedURLsJSON:  string(feedURLsJSON),
		FetchedAtJSON: string(fetchedAtJSON),
		Hash:          hash,
		AddonRows:     addonRows,
		CategoryRows:  categoryRows,
	}, nil
}

func (p *catalogPayload) encodeSnapshot(snap *snapshot) error {
	p.AddonsBlob = encodeRemoteAddonSnapshot(snap.Addons)
	p.CategoriesBlob = encodeCategorySnapshot(snap.Categories)
	return nil
}

func decodeSnapshotCatalog(row DBRemoteCatalogSnapshot) ([]RemoteAddon, []Category, error) {
	if len(row.AddonsBlob) > 0 && len(row.CategoriesBlob) > 0 {
		addons, err := decodeRemoteAddonSnapshot(row.AddonsBlob)
		if err != nil {
			return nil, nil, fmt.Errorf("decode binary snapshot addons: %w", err)
		}
		categories, err := decodeCategorySnapshot(row.CategoriesBlob)
		if err != nil {
			return nil, nil, fmt.Errorf("decode binary snapshot categories: %w", err)
		}
		return addons, categories, nil
	}

	var addons []RemoteAddon
	if err := json.Unmarshal([]byte(row.AddonsJSON), &addons); err != nil {
		return nil, nil, fmt.Errorf("decode snapshot addons: %w", err)
	}
	var categories []Category
	if err := json.Unmarshal([]byte(row.CategoriesJSON), &categories); err != nil {
		return nil, nil, fmt.Errorf("decode snapshot categories: %w", err)
	}
	return addons, categories, nil
}

func encodeRemoteAddonSnapshot(addons []RemoteAddon) []byte {
	buf := make([]byte, 0, estimateRemoteAddonSnapshotSize(addons))
	buf = appendSnapshotString(buf, remoteAddonSnapshotMagic)
	buf = appendSnapshotCount(buf, len(addons))
	for _, addon := range addons {
		buf = appendSnapshotString(buf, addon.UID)
		buf = appendSnapshotString(buf, addon.CategoryID)
		buf = appendSnapshotString(buf, addon.UIName)
		buf = appendSnapshotString(buf, addon.UIAuthorName)
		buf = appendSnapshotString(buf, addon.UIDate)
		buf = appendSnapshotString(buf, addon.UIVersion)
		buf = appendSnapshotStringSlice(buf, addon.UIDirs)
		buf = appendSnapshotString(buf, addon.UIFileInfoURL)
		buf = appendSnapshotInt(buf, addon.UIDownloadTotal)
		buf = appendSnapshotInt(buf, addon.UIDownloadMonthly)
		buf = appendSnapshotInt(buf, addon.UIFavoriteTotal)
		buf = appendSnapshotStringSlice(buf, addon.UIIMGThumbs)
		buf = appendSnapshotStringSlice(buf, addon.UIIMGs)
		buf = appendSnapshotGameVersions(buf, addon.Compatabilities)
		buf = appendSnapshotStringSlice(buf, addon.Siblings)
	}
	return buf
}

func decodeRemoteAddonSnapshot(data []byte) ([]RemoteAddon, error) {
	r := snapshotReader{data: data}
	if err := r.readMagic(remoteAddonSnapshotMagic); err != nil {
		return nil, err
	}
	count, err := r.readCount()
	if err != nil {
		return nil, err
	}
	addons := make([]RemoteAddon, count)
	for i := range addons {
		if addons[i].UID, err = r.readString(); err != nil {
			return nil, err
		}
		if addons[i].CategoryID, err = r.readString(); err != nil {
			return nil, err
		}
		if addons[i].UIName, err = r.readString(); err != nil {
			return nil, err
		}
		if addons[i].UIAuthorName, err = r.readString(); err != nil {
			return nil, err
		}
		if addons[i].UIDate, err = r.readString(); err != nil {
			return nil, err
		}
		if addons[i].UIVersion, err = r.readString(); err != nil {
			return nil, err
		}
		if addons[i].UIDirs, err = r.readStringSlice(); err != nil {
			return nil, err
		}
		if addons[i].UIFileInfoURL, err = r.readString(); err != nil {
			return nil, err
		}
		if addons[i].UIDownloadTotal, err = r.readInt(); err != nil {
			return nil, err
		}
		if addons[i].UIDownloadMonthly, err = r.readInt(); err != nil {
			return nil, err
		}
		if addons[i].UIFavoriteTotal, err = r.readInt(); err != nil {
			return nil, err
		}
		if addons[i].UIIMGThumbs, err = r.readStringSlice(); err != nil {
			return nil, err
		}
		if addons[i].UIIMGs, err = r.readStringSlice(); err != nil {
			return nil, err
		}
		if addons[i].Compatabilities, err = r.readGameVersions(); err != nil {
			return nil, err
		}
		if addons[i].Siblings, err = r.readStringSlice(); err != nil {
			return nil, err
		}
	}
	if r.remaining() != 0 {
		return nil, fmt.Errorf("snapshot has %d trailing bytes", r.remaining())
	}
	return addons, nil
}

func encodeCategorySnapshot(categories []Category) []byte {
	buf := make([]byte, 0, estimateCategorySnapshotSize(categories))
	buf = appendSnapshotString(buf, categorySnapshotMagic)
	buf = appendSnapshotCount(buf, len(categories))
	for _, category := range categories {
		buf = appendSnapshotString(buf, category.ID)
		buf = appendSnapshotString(buf, category.Name)
		buf = appendSnapshotString(buf, category.IconURL)
		buf = appendSnapshotString(buf, category.ParentID)
		buf = appendSnapshotStringSlice(buf, category.ParentIDs)
		buf = appendSnapshotInt(buf, int64(category.Count))
	}
	return buf
}

func decodeCategorySnapshot(data []byte) ([]Category, error) {
	r := snapshotReader{data: data}
	if err := r.readMagic(categorySnapshotMagic); err != nil {
		return nil, err
	}
	count, err := r.readCount()
	if err != nil {
		return nil, err
	}
	categories := make([]Category, count)
	for i := range categories {
		if categories[i].ID, err = r.readString(); err != nil {
			return nil, err
		}
		if categories[i].Name, err = r.readString(); err != nil {
			return nil, err
		}
		if categories[i].IconURL, err = r.readString(); err != nil {
			return nil, err
		}
		if categories[i].ParentID, err = r.readString(); err != nil {
			return nil, err
		}
		if categories[i].ParentIDs, err = r.readStringSlice(); err != nil {
			return nil, err
		}
		value, err := r.readInt()
		if err != nil {
			return nil, err
		}
		categories[i].Count = int(value)
	}
	if r.remaining() != 0 {
		return nil, fmt.Errorf("snapshot has %d trailing bytes", r.remaining())
	}
	return categories, nil
}

func estimateRemoteAddonSnapshotSize(addons []RemoteAddon) int {
	size := len(remoteAddonSnapshotMagic) + 8 + len(addons)*64
	for _, addon := range addons {
		size += len(addon.UID) +
			len(addon.CategoryID) +
			len(addon.UIName) +
			len(addon.UIAuthorName) +
			len(addon.UIDate) +
			len(addon.UIVersion) +
			len(addon.UIFileInfoURL)
		size += estimateStringSliceSnapshotSize(addon.UIDirs)
		size += estimateStringSliceSnapshotSize(addon.UIIMGThumbs)
		size += estimateStringSliceSnapshotSize(addon.UIIMGs)
		size += estimateGameVersionsSnapshotSize(addon.Compatabilities)
		size += estimateStringSliceSnapshotSize(addon.Siblings)
	}
	return size
}

func estimateCategorySnapshotSize(categories []Category) int {
	size := len(categorySnapshotMagic) + 8 + len(categories)*24
	for _, category := range categories {
		size += len(category.ID) +
			len(category.Name) +
			len(category.IconURL) +
			len(category.ParentID)
		size += estimateStringSliceSnapshotSize(category.ParentIDs)
	}
	return size
}

func estimateStringSliceSnapshotSize(values []string) int {
	size := 8 + len(values)*8
	for _, value := range values {
		size += len(value)
	}
	return size
}

func estimateGameVersionsSnapshotSize(values []GameVersion) int {
	size := 8 + len(values)*16
	for _, value := range values {
		size += len(value.Version) + len(value.Name)
	}
	return size
}

func appendSnapshotCount(buf []byte, value int) []byte {
	var scratch [binary.MaxVarintLen64]byte
	n := binary.PutUvarint(scratch[:], uint64(value))
	return append(buf, scratch[:n]...)
}

func appendSnapshotInt(buf []byte, value int64) []byte {
	var scratch [binary.MaxVarintLen64]byte
	n := binary.PutVarint(scratch[:], value)
	return append(buf, scratch[:n]...)
}

func appendSnapshotString(buf []byte, value string) []byte {
	buf = appendSnapshotCount(buf, len(value))
	buf = append(buf, value...)
	return buf
}

func appendSnapshotStringSlice(buf []byte, values []string) []byte {
	buf = appendSnapshotCount(buf, len(values))
	for _, value := range values {
		buf = appendSnapshotString(buf, value)
	}
	return buf
}

func appendSnapshotGameVersions(buf []byte, values []GameVersion) []byte {
	buf = appendSnapshotCount(buf, len(values))
	for _, value := range values {
		buf = appendSnapshotString(buf, value.Version)
		buf = appendSnapshotString(buf, value.Name)
	}
	return buf
}

type snapshotReader struct {
	data []byte
	pos  int
}

func (r *snapshotReader) readMagic(expected string) error {
	got, err := r.readString()
	if err != nil {
		return err
	}
	if got != expected {
		return fmt.Errorf("invalid snapshot magic %q", got)
	}
	return nil
}

func (r *snapshotReader) readCount() (int, error) {
	value, n := binary.Uvarint(r.data[r.pos:])
	if n <= 0 {
		return 0, fmt.Errorf("decode snapshot count at offset %d", r.pos)
	}
	if value > maxSnapshotItems {
		return 0, fmt.Errorf("snapshot count %d exceeds limit", value)
	}
	r.pos += n
	return int(value), nil
}

func (r *snapshotReader) readInt() (int64, error) {
	value, n := binary.Varint(r.data[r.pos:])
	if n <= 0 {
		return 0, fmt.Errorf("decode snapshot int at offset %d", r.pos)
	}
	r.pos += n
	return value, nil
}

func (r *snapshotReader) readString() (string, error) {
	size, err := r.readCount()
	if err != nil {
		return "", err
	}
	if size > r.remaining() {
		return "", fmt.Errorf("snapshot string length %d exceeds remaining %d", size, r.remaining())
	}
	start := r.pos
	r.pos += size
	return string(r.data[start:r.pos]), nil
}

func (r *snapshotReader) readStringSlice() ([]string, error) {
	count, err := r.readCount()
	if err != nil {
		return nil, err
	}
	if count == 0 {
		return nil, nil
	}
	values := make([]string, count)
	for i := range values {
		if values[i], err = r.readString(); err != nil {
			return nil, err
		}
	}
	return values, nil
}

func (r *snapshotReader) readGameVersions() ([]GameVersion, error) {
	count, err := r.readCount()
	if err != nil {
		return nil, err
	}
	if count == 0 {
		return nil, nil
	}
	values := make([]GameVersion, count)
	for i := range values {
		if values[i].Version, err = r.readString(); err != nil {
			return nil, err
		}
		if values[i].Name, err = r.readString(); err != nil {
			return nil, err
		}
	}
	return values, nil
}

func (r *snapshotReader) remaining() int {
	return len(r.data) - r.pos
}

func hashCatalog(feedURLsJSON []byte, addons []DBRemoteAddon, categories []DBCategory) (string, error) {
	hashInput := struct {
		FeedURLs   json.RawMessage `json:"feedUrls"`
		Addons     []DBRemoteAddon `json:"addons"`
		Categories []DBCategory    `json:"categories"`
	}{FeedURLs: feedURLsJSON, Addons: append([]DBRemoteAddon(nil), addons...), Categories: append([]DBCategory(nil), categories...)}
	sortRemoteAddonRows(hashInput.Addons)
	sortCategoryRows(hashInput.Categories)
	encoded, err := json.Marshal(hashInput)
	if err != nil {
		return "", fmt.Errorf("encode catalog hash payload: %w", err)
	}
	sum := sha256.Sum256(encoded)
	return hex.EncodeToString(sum[:]), nil
}

func readCatalogHash(tx *sql.Tx) (string, error) {
	var hash string
	err := tx.QueryRow("SELECT value FROM cache_meta WHERE key = ?", metaKeyCatalogHash).Scan(&hash)
	if err == sql.ErrNoRows {
		err = tx.QueryRow("SELECT catalog_hash FROM remote_catalog_snapshots WHERE key = ?", remoteCatalogSnapshotKey).Scan(&hash)
		if err == sql.ErrNoRows {
			return "", nil
		}
	}
	if err != nil {
		return "", fmt.Errorf("read catalog hash: %w", err)
	}
	return hash, nil
}

func upsertCacheMeta(tx *sql.Tx, key, value string) error {
	_, err := tx.Exec(
		`INSERT INTO cache_meta (key, value) VALUES (?, ?)
		 ON CONFLICT(key) DO UPDATE SET value = excluded.value`,
		key,
		value,
	)
	if err != nil {
		return fmt.Errorf("upsert cache meta %q: %w", key, err)
	}
	return nil
}

func upsertRemoteCatalogSnapshot(tx *sql.Tx, payload *catalogPayload) error {
	_, err := tx.Exec(
		`INSERT INTO remote_catalog_snapshots (key, feed_urls_json, addons_json, categories_json, addons_blob, categories_blob, catalog_hash, fetched_at_json)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?)
		 ON CONFLICT(key) DO UPDATE SET
		   feed_urls_json = excluded.feed_urls_json,
		   addons_json = excluded.addons_json,
		   categories_json = excluded.categories_json,
		   addons_blob = excluded.addons_blob,
		   categories_blob = excluded.categories_blob,
		   catalog_hash = excluded.catalog_hash,
		   fetched_at_json = excluded.fetched_at_json`,
		remoteCatalogSnapshotKey,
		payload.FeedURLsJSON,
		payload.AddonsJSON,
		payload.CategoriesJSON,
		payload.AddonsBlob,
		payload.CategoriesBlob,
		payload.Hash,
		payload.FetchedAtJSON,
	)
	if err != nil {
		return fmt.Errorf("upsert remote catalog snapshot: %w", err)
	}
	return nil
}

func updateRemoteCatalogSnapshotFetchedAt(tx *sql.Tx, fetchedAtJSON string) error {
	_, err := tx.Exec(
		"UPDATE remote_catalog_snapshots SET fetched_at_json = ? WHERE key = ?",
		fetchedAtJSON,
		remoteCatalogSnapshotKey,
	)
	if err != nil {
		return fmt.Errorf("update remote catalog snapshot timestamp: %w", err)
	}
	return nil
}

func remoteCatalogSnapshotHasBinary(tx *sql.Tx) (bool, error) {
	var addonBytes int
	var categoryBytes int
	err := tx.QueryRow(
		"SELECT coalesce(length(addons_blob), 0), coalesce(length(categories_blob), 0) FROM remote_catalog_snapshots WHERE key = ?",
		remoteCatalogSnapshotKey,
	).Scan(&addonBytes, &categoryBytes)
	if err == sql.ErrNoRows {
		return false, nil
	}
	if err != nil {
		return false, fmt.Errorf("read remote catalog snapshot blob sizes: %w", err)
	}
	return addonBytes > 0 && categoryBytes > 0, nil
}

func saveRemoteAddonRows(tx *sql.Tx, rows []DBRemoteAddon) error {
	existing, err := loadRemoteAddonRows(tx)
	if err != nil {
		return err
	}
	seen := make(map[string]struct{}, len(rows))
	stmt, err := tx.Prepare(
		`INSERT INTO remote_addons (
		   uid, category_id, ui_name, ui_author_name, uidate, ui_version, uidirs_json,
		   ui_file_info_url, uidownload_total, uidownload_monthly, ui_favorite_total,
		   ui_img_thumbs_json, ui_im_gs_json, compatabilities_json, siblings_json
		 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		 ON CONFLICT(uid) DO UPDATE SET
		   category_id = excluded.category_id,
		   ui_name = excluded.ui_name,
		   ui_author_name = excluded.ui_author_name,
		   uidate = excluded.uidate,
		   ui_version = excluded.ui_version,
		   uidirs_json = excluded.uidirs_json,
		   ui_file_info_url = excluded.ui_file_info_url,
		   uidownload_total = excluded.uidownload_total,
		   uidownload_monthly = excluded.uidownload_monthly,
		   ui_favorite_total = excluded.ui_favorite_total,
		   ui_img_thumbs_json = excluded.ui_img_thumbs_json,
		   ui_im_gs_json = excluded.ui_im_gs_json,
		   compatabilities_json = excluded.compatabilities_json,
		   siblings_json = excluded.siblings_json`,
	)
	if err != nil {
		return fmt.Errorf("prepare remote addon upsert: %w", err)
	}
	defer stmt.Close()

	for _, row := range rows {
		seen[row.UID] = struct{}{}
		if sameRemoteAddonRow(existing[row.UID], row) {
			continue
		}
		if _, err := stmt.Exec(
			row.UID,
			row.CategoryID,
			row.UIName,
			row.UIAuthorName,
			row.UIDate,
			row.UIVersion,
			row.UIDirsJSON,
			row.UIFileInfoURL,
			row.UIDownloadTotal,
			row.UIDownloadMonthly,
			row.UIFavoriteTotal,
			row.UIIMGThumbsJSON,
			row.UIIMGsJSON,
			row.CompatabilitiesJSON,
			row.SiblingsJSON,
		); err != nil {
			return fmt.Errorf("upsert remote addon %q: %w", row.UID, err)
		}
	}
	for uid := range existing {
		if _, ok := seen[uid]; ok {
			continue
		}
		if _, err := tx.Exec("DELETE FROM remote_addons WHERE uid = ?", uid); err != nil {
			return fmt.Errorf("delete stale remote addon %q: %w", uid, err)
		}
	}
	return nil
}

func saveCategoryRows(tx *sql.Tx, rows []DBCategory) error {
	existing, err := loadCategoryRows(tx)
	if err != nil {
		return err
	}
	seen := make(map[string]struct{}, len(rows))
	stmt, err := tx.Prepare(
		`INSERT INTO categories (id, name, icon_url, parent_id, parent_ids_json, count)
		 VALUES (?, ?, ?, ?, ?, ?)
		 ON CONFLICT(id) DO UPDATE SET
		   name = excluded.name,
		   icon_url = excluded.icon_url,
		   parent_id = excluded.parent_id,
		   parent_ids_json = excluded.parent_ids_json,
		   count = excluded.count`,
	)
	if err != nil {
		return fmt.Errorf("prepare category upsert: %w", err)
	}
	defer stmt.Close()

	for _, row := range rows {
		seen[row.ID] = struct{}{}
		if sameCategoryRow(existing[row.ID], row) {
			continue
		}
		if _, err := stmt.Exec(row.ID, row.Name, row.IconURL, row.ParentID, row.ParentIDsJSON, row.Count); err != nil {
			return fmt.Errorf("upsert category %q: %w", row.ID, err)
		}
	}
	for id := range existing {
		if _, ok := seen[id]; ok {
			continue
		}
		if _, err := tx.Exec("DELETE FROM categories WHERE id = ?", id); err != nil {
			return fmt.Errorf("delete stale category %q: %w", id, err)
		}
	}
	return nil
}

func loadRemoteAddonRows(tx *sql.Tx) (map[string]DBRemoteAddon, error) {
	rows, err := tx.Query(
		`SELECT uid, category_id, ui_name, ui_author_name, uidate, ui_version, uidirs_json,
		        ui_file_info_url, uidownload_total, uidownload_monthly, ui_favorite_total,
		        ui_img_thumbs_json, ui_im_gs_json, compatabilities_json, siblings_json
		   FROM remote_addons`,
	)
	if err != nil {
		return nil, fmt.Errorf("load existing remote addon rows: %w", err)
	}
	defer rows.Close()

	out := map[string]DBRemoteAddon{}
	for rows.Next() {
		var row DBRemoteAddon
		if err := rows.Scan(
			&row.UID,
			&row.CategoryID,
			&row.UIName,
			&row.UIAuthorName,
			&row.UIDate,
			&row.UIVersion,
			&row.UIDirsJSON,
			&row.UIFileInfoURL,
			&row.UIDownloadTotal,
			&row.UIDownloadMonthly,
			&row.UIFavoriteTotal,
			&row.UIIMGThumbsJSON,
			&row.UIIMGsJSON,
			&row.CompatabilitiesJSON,
			&row.SiblingsJSON,
		); err != nil {
			return nil, fmt.Errorf("scan remote addon row: %w", err)
		}
		out[row.UID] = row
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate remote addon rows: %w", err)
	}
	return out, nil
}

func loadCategoryRows(tx *sql.Tx) (map[string]DBCategory, error) {
	rows, err := tx.Query("SELECT id, name, icon_url, parent_id, parent_ids_json, count FROM categories")
	if err != nil {
		return nil, fmt.Errorf("load existing category rows: %w", err)
	}
	defer rows.Close()

	out := map[string]DBCategory{}
	for rows.Next() {
		var row DBCategory
		if err := rows.Scan(&row.ID, &row.Name, &row.IconURL, &row.ParentID, &row.ParentIDsJSON, &row.Count); err != nil {
			return nil, fmt.Errorf("scan category row: %w", err)
		}
		out[row.ID] = row
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate category rows: %w", err)
	}
	return out, nil
}

func sortRemoteAddonRows(rows []DBRemoteAddon) {
	sort.Slice(rows, func(i, j int) bool {
		return rows[i].UID < rows[j].UID
	})
}

func sortCategoryRows(rows []DBCategory) {
	sort.Slice(rows, func(i, j int) bool {
		return rows[i].ID < rows[j].ID
	})
}

func sameRemoteAddonRow(a, b DBRemoteAddon) bool {
	return a.UID == b.UID &&
		a.CategoryID == b.CategoryID &&
		a.UIName == b.UIName &&
		a.UIAuthorName == b.UIAuthorName &&
		a.UIDate == b.UIDate &&
		a.UIVersion == b.UIVersion &&
		a.UIDirsJSON == b.UIDirsJSON &&
		a.UIFileInfoURL == b.UIFileInfoURL &&
		a.UIDownloadTotal == b.UIDownloadTotal &&
		a.UIDownloadMonthly == b.UIDownloadMonthly &&
		a.UIFavoriteTotal == b.UIFavoriteTotal &&
		a.UIIMGThumbsJSON == b.UIIMGThumbsJSON &&
		a.UIIMGsJSON == b.UIIMGsJSON &&
		a.CompatabilitiesJSON == b.CompatabilitiesJSON &&
		a.SiblingsJSON == b.SiblingsJSON
}

func sameCategoryRow(a, b DBCategory) bool {
	return a.ID == b.ID &&
		a.Name == b.Name &&
		a.IconURL == b.IconURL &&
		a.ParentID == b.ParentID &&
		a.ParentIDsJSON == b.ParentIDsJSON &&
		a.Count == b.Count
}
