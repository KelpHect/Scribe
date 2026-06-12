package esoui

import (
	"archive/zip"
	"context"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"sort"
	"strings"
	"testing"
	"time"
)

func TestExtractWithProgressRejectsEscapingEntries(t *testing.T) {
	tests := []struct {
		name         string
		entries      map[string]string
		outsideCheck string
		skipWindows  bool
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
			skipWindows:  true,
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

			if !(runtime.GOOS == "windows" && tt.skipWindows) {
				if _, statErr := os.Stat(filepath.Join(base, tt.outsideCheck)); !os.IsNotExist(statErr) {
					t.Fatalf("outside path was created or stat failed unexpectedly: %v", statErr)
				}
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

func TestExtractWithProgressStopsAfterContextCancellation(t *testing.T) {
	base := t.TempDir()
	dest := filepath.Join(base, "AddOns")
	if err := os.MkdirAll(dest, 0o755); err != nil {
		t.Fatalf("create destination: %v", err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	zipPath := createTestZip(t, map[string]string{
		"Addon/01-first.txt":  "first",
		"Addon/02-second.txt": "second",
	})

	err := ExtractWithProgress(ctx, zipPath, dest, func(extracted, total int) {
		if total != 2 {
			t.Fatalf("total = %d, want 2", total)
		}
		if extracted == 1 {
			cancel()
		}
	})
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("ExtractWithProgress error = %v, want context.Canceled", err)
	}

	assertFileContent(t, filepath.Join(dest, "Addon", "01-first.txt"), "first")
	if _, err := os.Stat(filepath.Join(dest, "Addon", "02-second.txt")); !os.IsNotExist(err) {
		t.Fatalf("second file was extracted after cancellation or stat failed unexpectedly: %v", err)
	}
}

func TestPlanInstallArchiveClassifiesAddAndReplaceFolders(t *testing.T) {
	dest := t.TempDir()
	if err := os.MkdirAll(filepath.Join(dest, "ExistingAddon"), 0o755); err != nil {
		t.Fatalf("mkdir existing addon: %v", err)
	}
	zipPath := createTestZip(t, map[string]string{
		"ExistingAddon/ExistingAddon.txt": "## Title: Existing\n",
		"ExistingAddon/file.lua":          "existing",
		"NewAddon/NewAddon.addon":         "## Title: New\n",
		"NewAddon/file.lua":               "new",
	})

	plan, err := PlanInstallArchive(zipPath, dest, []string{"ExistingAddon", "NewAddon"})
	if err != nil {
		t.Fatalf("PlanInstallArchive: %v", err)
	}

	want := []InstallPlanEntry{
		{FolderName: "ExistingAddon", Action: "replace", Reason: "folder already exists"},
		{FolderName: "NewAddon", Action: "add", Reason: "folder is not installed"},
	}
	if !reflect.DeepEqual(plan, want) {
		t.Fatalf("plan = %#v, want %#v", plan, want)
	}

	if _, err := os.Stat(filepath.Join(dest, "NewAddon")); !os.IsNotExist(err) {
		t.Fatalf("preflight created NewAddon or stat failed unexpectedly: %v", err)
	}
}

func TestPlanInstallArchiveRejectsAmbiguousUnsafeArchives(t *testing.T) {
	tests := []struct {
		name    string
		entries map[string]string
		wantErr string
	}{
		{
			name: "root file",
			entries: map[string]string{
				"README.txt": "root",
			},
			wantErr: "root file",
		},
		{
			name: "missing canonical manifest",
			entries: map[string]string{
				"Addon/file.lua": "content",
			},
			wantErr: "no canonical manifest",
		},
		{
			name: "unexpected folder",
			entries: map[string]string{
				"OtherAddon/OtherAddon.txt": "## Title: Other\n",
			},
			wantErr: "not listed by ESOUI metadata",
		},
		{
			name: "escaping path",
			entries: map[string]string{
				"../Escape/Escape.txt": "bad",
			},
			wantErr: "escapes destination",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			zipPath := createTestZip(t, tt.entries)
			_, err := PlanInstallArchive(zipPath, t.TempDir(), []string{"Addon"})
			if err == nil {
				t.Fatal("PlanInstallArchive returned nil error")
			}
			if !strings.Contains(err.Error(), tt.wantErr) {
				t.Fatalf("PlanInstallArchive error = %q, want substring %q", err, tt.wantErr)
			}
		})
	}
}

func TestInstallArchiveWithProgressStagesAndReplacesAtomically(t *testing.T) {
	dest := t.TempDir()
	if err := os.MkdirAll(filepath.Join(dest, "ExistingAddon"), 0o755); err != nil {
		t.Fatalf("mkdir existing addon: %v", err)
	}
	if err := os.WriteFile(filepath.Join(dest, "ExistingAddon", "old.lua"), []byte("old"), 0o644); err != nil {
		t.Fatalf("write old addon: %v", err)
	}
	zipPath := createTestZip(t, map[string]string{
		"ExistingAddon/ExistingAddon.txt": "## Title: Existing\n",
		"ExistingAddon/new.lua":           "new",
		"NewAddon/NewAddon.txt":           "## Title: New\n",
		"NewAddon/new.lua":                "new",
	})

	plan, err := InstallArchiveWithProgress(context.Background(), zipPath, dest, []string{"ExistingAddon", "NewAddon"}, nil)
	if err != nil {
		t.Fatalf("InstallArchiveWithProgress: %v", err)
	}
	if len(plan) != 2 {
		t.Fatalf("plan length = %d, want 2", len(plan))
	}

	assertFileContent(t, filepath.Join(dest, "ExistingAddon", "new.lua"), "new")
	assertFileContent(t, filepath.Join(dest, "NewAddon", "new.lua"), "new")
	if _, err := os.Stat(filepath.Join(dest, "ExistingAddon", "old.lua")); !os.IsNotExist(err) {
		t.Fatalf("old file still exists or stat failed unexpectedly: %v", err)
	}
	assertNoTempInstallDirs(t, dest)
}

func TestInstallArchiveWithProgressCancellationDoesNotTouchDestination(t *testing.T) {
	dest := t.TempDir()
	if err := os.MkdirAll(filepath.Join(dest, "ExistingAddon"), 0o755); err != nil {
		t.Fatalf("mkdir existing addon: %v", err)
	}
	if err := os.WriteFile(filepath.Join(dest, "ExistingAddon", "old.lua"), []byte("old"), 0o644); err != nil {
		t.Fatalf("write old addon: %v", err)
	}
	zipPath := createTestZip(t, map[string]string{
		"ExistingAddon/01-new.lua":        "new",
		"ExistingAddon/ExistingAddon.txt": "## Title: Existing\n",
	})
	ctx, cancel := context.WithCancel(context.Background())

	_, err := InstallArchiveWithProgress(ctx, zipPath, dest, []string{"ExistingAddon"}, func(extracted, total int) {
		if extracted == 1 {
			cancel()
		}
	})
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("InstallArchiveWithProgress error = %v, want context.Canceled", err)
	}

	assertFileContent(t, filepath.Join(dest, "ExistingAddon", "old.lua"), "old")
	if _, err := os.Stat(filepath.Join(dest, "ExistingAddon", "01-new.lua")); !os.IsNotExist(err) {
		t.Fatalf("cancelled install wrote new file or stat failed unexpectedly: %v", err)
	}
	assertNoTempInstallDirs(t, dest)
}

func TestInstallArchiveWithProgressInvalidArchiveDoesNotTouchDestination(t *testing.T) {
	dest := t.TempDir()
	if err := os.MkdirAll(filepath.Join(dest, "ExistingAddon"), 0o755); err != nil {
		t.Fatalf("mkdir existing addon: %v", err)
	}
	if err := os.WriteFile(filepath.Join(dest, "ExistingAddon", "old.lua"), []byte("old"), 0o644); err != nil {
		t.Fatalf("write old addon: %v", err)
	}
	zipPath := createTestZip(t, map[string]string{
		"ExistingAddon/new.lua": "new",
	})

	if _, err := InstallArchiveWithProgress(context.Background(), zipPath, dest, []string{"ExistingAddon"}, nil); err == nil {
		t.Fatal("InstallArchiveWithProgress returned nil error for invalid archive")
	}

	assertFileContent(t, filepath.Join(dest, "ExistingAddon", "old.lua"), "old")
	if _, err := os.Stat(filepath.Join(dest, "ExistingAddon", "new.lua")); !os.IsNotExist(err) {
		t.Fatalf("invalid archive wrote new file or stat failed unexpectedly: %v", err)
	}
	assertNoTempInstallDirs(t, dest)
}

func TestInstallArchiveWithProgressRollsBackCommitFailure(t *testing.T) {
	dest := t.TempDir()
	for _, folder := range []string{"ExistingAddon", "NewAddon"} {
		if err := os.MkdirAll(filepath.Join(dest, folder), 0o755); err != nil {
			t.Fatalf("mkdir %s: %v", folder, err)
		}
	}
	if err := os.WriteFile(filepath.Join(dest, "ExistingAddon", "old.lua"), []byte("old"), 0o644); err != nil {
		t.Fatalf("write old addon: %v", err)
	}
	if err := os.WriteFile(filepath.Join(dest, "NewAddon", "old.lua"), []byte("old-new"), 0o644); err != nil {
		t.Fatalf("write old new addon: %v", err)
	}
	zipPath := createTestZip(t, map[string]string{
		"ExistingAddon/ExistingAddon.txt": "## Title: Existing\n",
		"ExistingAddon/new.lua":           "new",
		"NewAddon/NewAddon.txt":           "## Title: New\n",
		"NewAddon/new.lua":                "new",
	})
	plan := []InstallPlanEntry{
		{FolderName: "ExistingAddon", Action: "replace"},
		{FolderName: "NewAddon", Action: "add"},
	}

	err := installPlannedArchiveWithProgress(context.Background(), zipPath, dest, plan, nil)
	if err == nil {
		t.Fatal("installPlannedArchiveWithProgress returned nil error")
	}

	assertFileContent(t, filepath.Join(dest, "ExistingAddon", "old.lua"), "old")
	assertFileContent(t, filepath.Join(dest, "NewAddon", "old.lua"), "old-new")
	if _, err := os.Stat(filepath.Join(dest, "ExistingAddon", "new.lua")); !os.IsNotExist(err) {
		t.Fatalf("rollback left new existing addon file or stat failed unexpectedly: %v", err)
	}
	assertNoTempInstallDirs(t, dest)
}

func TestRemoveAddonFolderRejectsUnsafeFolderNames(t *testing.T) {
	tests := []struct {
		name       string
		folderName string
	}{
		{name: "empty", folderName: ""},
		{name: "dot", folderName: "."},
		{name: "dot dot", folderName: ".."},
		{name: "slash", folderName: "Nested/Addon"},
		{name: "backslash", folderName: `Nested\Addon`},
		{name: "parent traversal", folderName: "../OutsideAddon"},
		{name: "windows parent traversal", folderName: `..\OutsideAddon`},
		{name: "absolute slash path", folderName: "/tmp/OutsideAddon"},
		{name: "windows absolute backslash path", folderName: `C:\OutsideAddon`},
		{name: "windows absolute slash path", folderName: "C:/OutsideAddon"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			base := t.TempDir()
			addonPath := filepath.Join(base, "AddOns")
			mustMkdir(t, filepath.Join(addonPath, "SafeAddon"))
			mustMkdir(t, filepath.Join(addonPath, "SiblingAddon"))
			mustMkdir(t, filepath.Join(base, "OutsideAddon"))

			err := RemoveAddonFolder(addonPath, tt.folderName)
			if err == nil {
				t.Fatal("expected invalid folder name error")
			}
			if !strings.Contains(err.Error(), "invalid folder name") {
				t.Fatalf("expected invalid folder name error, got %v", err)
			}

			assertDirExists(t, addonPath)
			assertDirExists(t, filepath.Join(addonPath, "SafeAddon"))
			assertDirExists(t, filepath.Join(addonPath, "SiblingAddon"))
			assertDirExists(t, filepath.Join(base, "OutsideAddon"))
		})
	}
}

func assertNoTempInstallDirs(t *testing.T, dest string) {
	t.Helper()
	entries, err := os.ReadDir(dest)
	if err != nil {
		t.Fatalf("read dest: %v", err)
	}
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), ".scribe-staging-") || strings.HasPrefix(entry.Name(), ".scribe-backup-") {
			t.Fatalf("temporary install directory was not cleaned up: %s", entry.Name())
		}
	}
}

func TestCleanStaleInstallArtifactsRemovesOnlyOldScribeTempDirs(t *testing.T) {
	addonPath := t.TempDir()
	oldStaging := filepath.Join(addonPath, ".scribe-staging-old")
	oldBackup := filepath.Join(addonPath, ".scribe-backup-old")
	freshStaging := filepath.Join(addonPath, ".scribe-staging-fresh")
	normalAddon := filepath.Join(addonPath, "NormalAddon")

	for _, dir := range []string{oldStaging, oldBackup, freshStaging, normalAddon} {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatalf("mkdir %s: %v", dir, err)
		}
	}
	old := time.Now().Add(-2 * time.Hour)
	if err := os.Chtimes(oldStaging, old, old); err != nil {
		t.Fatalf("chtimes old staging: %v", err)
	}
	if err := os.Chtimes(oldBackup, old, old); err != nil {
		t.Fatalf("chtimes old backup: %v", err)
	}

	report := CleanStaleInstallArtifacts(addonPath, time.Hour)

	if report.RemovedCount() != 2 || report.RetainedCount() != 1 || report.Error() != "" {
		t.Fatalf("cleanup report = %+v", report)
	}
	assertDirMissing(t, oldStaging)
	assertDirMissing(t, oldBackup)
	assertDirExists(t, freshStaging)
	assertDirExists(t, normalAddon)
}

func TestRemoveAddonFolderMissingFolderLeavesAddOnsUntouched(t *testing.T) {
	base := t.TempDir()
	addonPath := filepath.Join(base, "AddOns")
	mustMkdir(t, filepath.Join(addonPath, "SiblingAddon"))
	mustMkdir(t, filepath.Join(base, "OutsideAddon"))

	err := RemoveAddonFolder(addonPath, "MissingAddon")
	if err == nil {
		t.Fatal("expected missing folder error")
	}
	if !strings.Contains(err.Error(), "addon folder not found") {
		t.Fatalf("expected missing folder error, got %v", err)
	}

	assertDirExists(t, addonPath)
	assertDirExists(t, filepath.Join(addonPath, "SiblingAddon"))
	assertDirExists(t, filepath.Join(base, "OutsideAddon"))
}

func TestRemoveAddonFolderRemovesOnlyNamedAddonFolder(t *testing.T) {
	base := t.TempDir()
	addonPath := filepath.Join(base, "AddOns")
	target := filepath.Join(addonPath, "ValidAddon")
	sibling := filepath.Join(addonPath, "SiblingAddon")
	outside := filepath.Join(base, "OutsideAddon")
	mustMkdir(t, filepath.Join(target, "nested"))
	mustMkdir(t, sibling)
	mustMkdir(t, outside)
	if err := os.WriteFile(filepath.Join(target, "ValidAddon.txt"), []byte("manifest"), 0o644); err != nil {
		t.Fatalf("write target manifest: %v", err)
	}

	if err := RemoveAddonFolder(addonPath, "ValidAddon"); err != nil {
		t.Fatalf("RemoveAddonFolder valid folder: %v", err)
	}

	if _, err := os.Stat(target); !os.IsNotExist(err) {
		t.Fatalf("target folder still exists or stat failed unexpectedly: %v", err)
	}
	assertDirExists(t, addonPath)
	assertDirExists(t, sibling)
	assertDirExists(t, outside)
}

func createTestZip(t *testing.T, entries map[string]string) string {
	t.Helper()

	zipFile, err := os.CreateTemp(t.TempDir(), "addon-*.zip")
	if err != nil {
		t.Fatalf("create zip: %v", err)
	}
	defer zipFile.Close()

	zw := zip.NewWriter(zipFile)
	names := make([]string, 0, len(entries))
	for name := range entries {
		names = append(names, name)
	}
	sort.Strings(names)

	for _, name := range names {
		content := entries[name]
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

func mustMkdir(t *testing.T, path string) {
	t.Helper()

	if err := os.MkdirAll(path, 0o755); err != nil {
		t.Fatalf("mkdir %s: %v", path, err)
	}
}

func assertDirExists(t *testing.T, path string) {
	t.Helper()

	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("stat %s: %v", path, err)
	}
	if !info.IsDir() {
		t.Fatalf("%s is not a directory", path)
	}
}

func assertDirMissing(t *testing.T, path string) {
	t.Helper()

	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("expected %s to be missing, stat err = %v", path, err)
	}
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
