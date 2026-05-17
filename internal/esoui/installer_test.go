package esoui

import (
	"archive/zip"
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestExtractWithProgressRejectsEscapingEntries(t *testing.T) {
	tests := []struct {
		name         string
		entries      map[string]string
		outsideCheck string
	}{
		{
			name: "parent traversal",
			entries: map[string]string{
				"../evil.txt": "evil",
			},
			outsideCheck: "evil.txt",
		},
		{
			name: "nested parent traversal",
			entries: map[string]string{
				"Addon/../../evil.txt": "evil",
			},
			outsideCheck: "evil.txt",
		},
		{
			name: "absolute slash path",
			entries: map[string]string{
				"/tmp/evil.txt": "evil",
			},
			outsideCheck: filepath.Join("tmp", "evil.txt"),
		},
		{
			name: "windows drive path",
			entries: map[string]string{
				"C:/Users/evil.txt": "evil",
			},
			outsideCheck: filepath.Join("C:", "Users", "evil.txt"),
		},
		{
			name: "backslash traversal",
			entries: map[string]string{
				`Addon\..\evil.txt`: "evil",
			},
			outsideCheck: "evil.txt",
		},
		{
			name: "destination prefix sibling",
			entries: map[string]string{
				"../AddOnsSibling/evil.txt": "evil",
			},
			outsideCheck: filepath.Join("AddOnsSibling", "evil.txt"),
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			base := t.TempDir()
			dest := filepath.Join(base, "AddOns")
			if err := os.MkdirAll(dest, 0o755); err != nil {
				t.Fatalf("create destination: %v", err)
			}

			zipPath := createTestZip(t, tt.entries)
			err := ExtractWithProgress(context.Background(), zipPath, dest, nil)
			if err == nil {
				t.Fatal("expected extraction error")
			}
			if !strings.Contains(err.Error(), "zip entry escapes destination") {
				t.Fatalf("expected escape error, got %v", err)
			}

			if _, statErr := os.Stat(filepath.Join(base, tt.outsideCheck)); !os.IsNotExist(statErr) {
				t.Fatalf("outside path was created or stat failed unexpectedly: %v", statErr)
			}
		})
	}
}

func TestExtractWithProgressExtractsValidNestedFilesUnderDestination(t *testing.T) {
	base := t.TempDir()
	dest := filepath.Join(base, "AddOns")
	if err := os.MkdirAll(dest, 0o755); err != nil {
		t.Fatalf("create destination: %v", err)
	}

	zipPath := createTestZip(t, map[string]string{
		"Addon/":                    "",
		"Addon/Addon.txt":           "manifest",
		"Addon/textures/icon.dds":   "icon",
		"Addon/lang/en/strings.lua": "strings",
	})

	var progress []int
	err := ExtractWithProgress(context.Background(), zipPath, dest, func(extracted, total int) {
		progress = append(progress, extracted)
		if total != 4 {
			t.Fatalf("total = %d, want 4", total)
		}
	})
	if err != nil {
		t.Fatalf("extract valid zip: %v", err)
	}

	assertFileContent(t, filepath.Join(dest, "Addon", "Addon.txt"), "manifest")
	assertFileContent(t, filepath.Join(dest, "Addon", "textures", "icon.dds"), "icon")
	assertFileContent(t, filepath.Join(dest, "Addon", "lang", "en", "strings.lua"), "strings")

	if _, err := os.Stat(filepath.Join(base, "Addon")); !os.IsNotExist(err) {
		t.Fatalf("addon was extracted outside destination or stat failed unexpectedly: %v", err)
	}
	if len(progress) != 4 || progress[len(progress)-1] != 4 {
		t.Fatalf("progress = %v, want four updates ending at 4", progress)
	}
}

func createTestZip(t *testing.T, entries map[string]string) string {
	t.Helper()

	zipFile, err := os.CreateTemp(t.TempDir(), "addon-*.zip")
	if err != nil {
		t.Fatalf("create zip: %v", err)
	}
	defer zipFile.Close()

	zw := zip.NewWriter(zipFile)
	for name, content := range entries {
		if strings.HasSuffix(name, "/") {
			if _, err := zw.Create(name); err != nil {
				t.Fatalf("create zip directory %q: %v", name, err)
			}
			continue
		}
		w, err := zw.Create(name)
		if err != nil {
			t.Fatalf("create zip entry %q: %v", name, err)
		}
		if _, err := w.Write([]byte(content)); err != nil {
			t.Fatalf("write zip entry %q: %v", name, err)
		}
	}
	if err := zw.Close(); err != nil {
		t.Fatalf("close zip writer: %v", err)
	}
	if err := zipFile.Close(); err != nil {
		t.Fatalf("close zip file: %v", err)
	}

	return zipFile.Name()
}

func assertFileContent(t *testing.T, path, want string) {
	t.Helper()

	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	if string(got) != want {
		t.Fatalf("%s = %q, want %q", path, string(got), want)
	}
}
