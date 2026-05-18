package scanner

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
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

func TestParseAddonFile_MetadataEdgeCases(t *testing.T) {
	t.Parallel()

	root := t.TempDir()
	addonDir := filepath.Join(root, "EdgeAddon")
	if err := os.Mkdir(addonDir, 0o755); err != nil {
		t.Fatalf("mkdir: %v", err)
	}
	manifest := filepath.Join(addonDir, "EdgeAddon.txt")
	content := strings.Join([]string{
		"## Title: |cFFAA00Colored Title|r",
		"## Version: |cFFFFFF1.2.3|r",
		"## Author: |c00FF00Author Name|r",
		"## Description: |c123456A description|r",
		"## DependsOn: LibRequired>=1.0 LibAnother<=2",
		"## PCDependsOn: LibPC LibPCVersion>=3",
		"## ConsoleDependsOn: ConsoleOnly",
		"## OptionalDependsOn: LibOptional LibOptionalVersion>=4",
		"## SavedVariables: EdgeSaved AccountWideSaved",
		"## APIVersion: 101046 101047",
		"## AddOnVersion: 42",
		"## IsLibrary: 1",
		"",
		"EdgeAddon.lua",
	}, "\n")
	if err := os.WriteFile(manifest, []byte(content), 0o644); err != nil {
		t.Fatalf("write manifest: %v", err)
	}

	a, err := ParseAddonFile(manifest)
	if err != nil {
		t.Fatalf("ParseAddonFile: %v", err)
	}

	if a.Title != "Colored Title" {
		t.Fatalf("Title = %q, want color-stripped title", a.Title)
	}
	if a.Version != "1.2.3" {
		t.Fatalf("Version = %q, want 1.2.3", a.Version)
	}
	if a.Author != "Author Name" {
		t.Fatalf("Author = %q, want Author Name", a.Author)
	}
	if a.Description != "A description" {
		t.Fatalf("Description = %q, want A description", a.Description)
	}
	wantRequired := []string{"LibRequired>=1.0", "LibAnother<=2", "LibPC", "LibPCVersion>=3"}
	if !reflect.DeepEqual(a.DependsOn, wantRequired) {
		t.Fatalf("DependsOn = %#v, want %#v", a.DependsOn, wantRequired)
	}
	wantOptional := []string{"LibOptional", "LibOptionalVersion>=4"}
	if !reflect.DeepEqual(a.OptionalDependsOn, wantOptional) {
		t.Fatalf("OptionalDependsOn = %#v, want %#v", a.OptionalDependsOn, wantOptional)
	}
	wantSaved := []string{"EdgeSaved", "AccountWideSaved"}
	if !reflect.DeepEqual(a.SavedVariables, wantSaved) {
		t.Fatalf("SavedVariables = %#v, want %#v", a.SavedVariables, wantSaved)
	}
	if a.APIVersion != "101046 101047" {
		t.Fatalf("APIVersion = %q, want both API versions", a.APIVersion)
	}
	if a.AddOnVersion != "42" {
		t.Fatalf("AddOnVersion = %q, want 42", a.AddOnVersion)
	}
	if !a.IsLibrary {
		t.Fatal("IsLibrary = false, want true for value 1")
	}
}

func TestParseAddonFile_FallbackTitleAndLibraryBooleans(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name      string
		value     string
		isLibrary bool
	}{
		{name: "true", value: "true", isLibrary: true},
		{name: "one", value: "1", isLibrary: true},
		{name: "false", value: "false", isLibrary: false},
		{name: "zero", value: "0", isLibrary: false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			root := t.TempDir()
			addonDir := filepath.Join(root, "FallbackAddon")
			if err := os.Mkdir(addonDir, 0o755); err != nil {
				t.Fatalf("mkdir: %v", err)
			}
			manifest := filepath.Join(addonDir, "FallbackAddon.txt")
			content := "## IsLibrary: " + tt.value + "\n"
			if err := os.WriteFile(manifest, []byte(content), 0o644); err != nil {
				t.Fatalf("write manifest: %v", err)
			}

			a, err := ParseAddonFile(manifest)
			if err != nil {
				t.Fatalf("ParseAddonFile: %v", err)
			}
			if a.Title != "FallbackAddon" {
				t.Fatalf("Title = %q, want folder-name fallback", a.Title)
			}
			if a.IsLibrary != tt.isLibrary {
				t.Fatalf("IsLibrary = %v, want %v", a.IsLibrary, tt.isLibrary)
			}
		})
	}
}
