package esoui

import (
	"fmt"
	"path/filepath"
	"testing"

	"Scribe/internal/addon"
)

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
			UID:              fmt.Sprintf("%d", i),
			CategoryID:       "cat-ui",
			UIName:           fmt.Sprintf("Addon %04d", i),
			UIAuthorName:     "Bench Author",
			UIVersion:        fmt.Sprintf("1.%d", (i%9)+1),
			UIDirs:           []string{fmt.Sprintf("Addon%04d", i)},
			UIDownloadTotal:  int64(i * 10),
			UIFavoriteTotal:  int64(i),
			Compatabilities:  []GameVersion{{Name: "ESO", Version: "10.0.0"}},
			UIFileInfoURL:    "https://example.invalid/addon",
			UIIMGThumbs:      []string{"https://example.invalid/thumb.jpg"},
			UIIMGs:           []string{"https://example.invalid/image.jpg"},
		}
	}
	return locals, remotes
}
