package esoui

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"Scribe/internal/addon"
	"Scribe/internal/scanner"
)

func BenchmarkSQLiteOpenDB(b *testing.B) {
	dir := b.TempDir()

	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		path := filepath.Join(dir, fmt.Sprintf("open-%d.db", i))
		db, err := OpenDB(path)
		if err != nil {
			b.Fatal(err)
		}
		sqlDB, err := db.DB()
		if err != nil {
			b.Fatal(err)
		}
		if err := sqlDB.Close(); err != nil {
			b.Fatal(err)
		}
		_ = os.Remove(path)
		_ = os.Remove(path + "-wal")
		_ = os.Remove(path + "-shm")
	}
}

func BenchmarkMatchLargeCatalog(b *testing.B) {
	locals, remotes := benchmarkCatalog(1000, 7000)

	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = MatchAddons(locals, remotes)
	}
}

func BenchmarkCachedCatalogLoad(b *testing.B) {
	_, remotes := benchmarkCatalog(1000, 7000)
	categories := []Category{{ID: "cat-ui", Name: "User Interface"}}
	dbPath := filepath.Join(b.TempDir(), "cache.db")
	db, err := OpenDB(dbPath)
	if err != nil {
		b.Fatal(err)
	}
	cache := NewCacheFromDB(db)
	if err := cache.Set(APIFeeds{ListFiles: "fixture"}, remotes, categories); err != nil {
		b.Fatal(err)
	}

	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = NewCacheFromDB(db)
	}
	reportSQLiteFileSizes(b, dbPath)
}

func BenchmarkSQLiteSaveRemoteCatalog(b *testing.B) {
	_, remotes := benchmarkCatalog(1000, 7000)
	categories := []Category{{ID: "cat-ui", Name: "User Interface"}}
	dbPath := filepath.Join(b.TempDir(), "remote-cache.db")
	db, err := OpenDB(dbPath)
	if err != nil {
		b.Fatal(err)
	}
	cache := NewCacheFromDB(db)

	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if err := cache.Set(APIFeeds{ListFiles: "fixture"}, remotes, categories); err != nil {
			b.Fatal(err)
		}
	}
	reportSQLiteFileSizes(b, dbPath)
}

func BenchmarkSQLiteSaveScannerCache(b *testing.B) {
	dbPath := filepath.Join(b.TempDir(), "scanner-cache.db")
	db, err := OpenDB(dbPath)
	if err != nil {
		b.Fatal(err)
	}
	store := NewScannerCacheStore(db)
	entries := benchmarkScannerCacheEntries(1000)

	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if err := store.SaveScanCache("/tmp/AddOns", entries); err != nil {
			b.Fatal(err)
		}
	}
	reportSQLiteFileSizes(b, dbPath)
}

func BenchmarkSQLiteQueryInstallMD5s(b *testing.B) {
	dbPath := filepath.Join(b.TempDir(), "install-records.db")
	db, err := OpenDB(dbPath)
	if err != nil {
		b.Fatal(err)
	}
	uids := make([]string, 1000)
	for i := range uids {
		uids[i] = fmt.Sprintf("uid-%04d", i)
		if err := SaveInstallMD5(db, uids[i], fmt.Sprintf("md5-%04d", i)); err != nil {
			b.Fatal(err)
		}
	}

	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = GetInstallMD5s(db, uids)
	}
	reportSQLiteFileSizes(b, dbPath)
}

func BenchmarkRemoteSearchLargeCatalog(b *testing.B) {
	_, remotes := benchmarkCatalog(1000, 7000)

	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = SearchRemote(remotes, "addon 42")
	}
}

func benchmarkCatalog(localCount, remoteCount int) ([]*addon.Addon, []RemoteAddon) {
	locals := make([]*addon.Addon, localCount)
	for i := range locals {
		locals[i] = &addon.Addon{
			FolderName: fmt.Sprintf("Addon%04d", i),
			Title:      fmt.Sprintf("Addon %04d", i),
			Version:    fmt.Sprintf("1.%d", i%9),
		}
	}

	remotes := make([]RemoteAddon, remoteCount)
	for i := range remotes {
		remotes[i] = RemoteAddon{
			UID:             fmt.Sprintf("%d", i),
			CategoryID:      "cat-ui",
			UIName:          fmt.Sprintf("Addon %04d", i),
			UIAuthorName:    "Bench Author",
			UIVersion:       fmt.Sprintf("1.%d", (i%9)+1),
			UIDirs:          []string{fmt.Sprintf("Addon%04d", i)},
			UIDownloadTotal: int64(i * 10),
			UIFavoriteTotal: int64(i),
			Compatabilities: []GameVersion{{Name: "ESO", Version: "10.0.0"}},
			UIFileInfoURL:   "https://example.invalid/addon",
			UIIMGThumbs:     []string{"https://example.invalid/thumb.jpg"},
			UIIMGs:          []string{"https://example.invalid/image.jpg"},
		}
	}
	return locals, remotes
}

func benchmarkScannerCacheEntries(count int) []scanner.CachedAddon {
	entries := make([]scanner.CachedAddon, count)
	for i := range entries {
		folderName := fmt.Sprintf("Addon%04d", i)
		entries[i] = scanner.CachedAddon{
			FolderName:  folderName,
			Fingerprint: fmt.Sprintf("fingerprint-%04d", i),
			Addon: &addon.Addon{
				ID:                folderName,
				FolderName:        folderName,
				Title:             fmt.Sprintf("Addon %04d", i),
				Version:           fmt.Sprintf("1.%d", i%9),
				Author:            "Bench Author",
				DependsOn:         []string{"LibAddonMenu-2.0"},
				OptionalDependsOn: []string{"LibGPS"},
				SavedVariables:    []string{folderName + "_Saved"},
				APIVersion:        "101046",
				AddOnVersion:      fmt.Sprintf("%d", i),
				Enabled:           true,
			},
		}
	}
	return entries
}

func reportSQLiteFileSizes(b *testing.B, dbPath string) {
	b.Helper()
	b.StopTimer()
	defer b.StartTimer()

	for _, suffix := range []string{"", "-wal", "-shm"} {
		info, err := os.Stat(dbPath + suffix)
		if err == nil {
			label := "sqlite_db_bytes"
			switch suffix {
			case "-wal":
				label = "sqlite_wal_bytes"
			case "-shm":
				label = "sqlite_shm_bytes"
			}
			b.ReportMetric(float64(info.Size()), label)
		}
	}
}
