package scanner

import (
	"os"
	"path/filepath"
	"testing"
)

func TestScanAddonDir_PrefersFolderNameManifest(t *testing.T) {
	t.Parallel()

	root := t.TempDir()
	addonDir := filepath.Join(root, "LibLazyCrafting")
	if err := os.Mkdir(addonDir, 0o755); err != nil {
		t.Fatalf("mkdir: %v", err)
	}

	stubContent := "## Version: 2.3\n\nLibLazyCrafting.lua\n"
	if err := os.WriteFile(filepath.Join(addonDir, "LLC.txt"), []byte(stubContent), 0o644); err != nil {
		t.Fatalf("write LLC.txt: %v", err)
	}

	canonicalContent := "## Title: LibLazyCrafting v4.035\n## Version: 4.035\n## Author: Dolgubon\n## IsLibrary: true\n"
	if err := os.WriteFile(filepath.Join(addonDir, "LibLazyCrafting.addon"), []byte(canonicalContent), 0o644); err != nil {
		t.Fatalf("write LibLazyCrafting.addon: %v", err)
	}

	s := New(root)
	addons, err := s.Scan()
	if err != nil {
		t.Fatalf("Scan() error: %v", err)
	}
	if len(addons) != 1 {
		t.Fatalf("expected 1 addon, got %d", len(addons))
	}
	a := addons[0]
	if a.Version != "4.035" {
		t.Errorf("Version = %q, want %q (stub LLC.txt was picked instead of canonical .addon)", a.Version, "4.035")
	}
	if a.Author != "Dolgubon" {
		t.Errorf("Author = %q, want %q", a.Author, "Dolgubon")
	}
}

func TestScanAddonDir_FallsBackToAlphabetical(t *testing.T) {
	t.Parallel()

	root := t.TempDir()
	addonDir := filepath.Join(root, "MyAddon")
	if err := os.Mkdir(addonDir, 0o755); err != nil {
		t.Fatalf("mkdir: %v", err)
	}

	content := "## Title: My Addon\n## Version: 1.0\n## Author: Someone\n"
	if err := os.WriteFile(filepath.Join(addonDir, "SomeOtherName.txt"), []byte(content), 0o644); err != nil {
		t.Fatalf("write SomeOtherName.txt: %v", err)
	}

	s := New(root)
	addons, err := s.Scan()
	if err != nil {
		t.Fatalf("Scan() error: %v", err)
	}
	if len(addons) != 1 {
		t.Fatalf("expected 1 addon via fallback, got %d", len(addons))
	}
	if addons[0].Version != "1.0" {
		t.Errorf("Version = %q, want %q", addons[0].Version, "1.0")
	}
}
