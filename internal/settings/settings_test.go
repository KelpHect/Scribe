package settings

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"Scribe/internal/esoui"

	"gorm.io/gorm"
)

func TestSaveSettingsRejectsInvalidAddonPath(t *testing.T) {
	mgr := newTestManager(t)

	err := mgr.SaveSettings(AppSettings{
		AddonPath:     filepath.Join("relative", "AddOns"),
		MemoryLimitMB: 150,
		Theme:         "scribe",
	})
	if err == nil {
		t.Fatal("expected invalid addon path error")
	}
}

func TestGetSettingsReturnsDefaults(t *testing.T) {
	mgr := newTestManager(t)

	got, err := mgr.GetSettings()
	if err != nil {
		t.Fatalf("GetSettings: %v", err)
	}
	if got.AddonPath != "" {
		t.Fatalf("AddonPath = %q, want empty", got.AddonPath)
	}
	if got.AutoUpdate {
		t.Fatal("AutoUpdate = true, want false")
	}
	if got.MemoryLimitMB != 150 {
		t.Fatalf("MemoryLimitMB = %d, want 150", got.MemoryLimitMB)
	}
	if got.Theme != "scribe" {
		t.Fatalf("Theme = %q, want scribe", got.Theme)
	}
}

func TestSaveSettingsNormalizesSettingsInputs(t *testing.T) {
	mgr := newTestManager(t)
	addonPath := filepath.Join(t.TempDir(), "AddOns")

	if err := mgr.SaveSettings(AppSettings{
		AddonPath:     addonPath,
		AutoUpdate:    true,
		MemoryLimitMB: -1,
		Theme:         "not-a-theme",
	}); err != nil {
		t.Fatalf("SaveSettings: %v", err)
	}

	got, err := mgr.GetSettings()
	if err != nil {
		t.Fatalf("GetSettings: %v", err)
	}
	if got.AddonPath != filepath.Clean(addonPath) {
		t.Fatalf("AddonPath = %q, want %q", got.AddonPath, filepath.Clean(addonPath))
	}
	if got.AutoUpdate {
		t.Fatal("AutoUpdate = true, want false until a safe worker exists")
	}
	if got.MemoryLimitMB != 150 {
		t.Fatalf("MemoryLimitMB = %d, want 150", got.MemoryLimitMB)
	}
	if got.Theme != "scribe" {
		t.Fatalf("Theme = %q, want scribe", got.Theme)
	}
}

func TestSaveSettingsRoundTripAndRepeatedSaveUpdatesRows(t *testing.T) {
	mgr, settingsPath := newTestManagerAndPath(t)
	firstPath := filepath.Join(t.TempDir(), "FirstAddOns")
	secondPath := filepath.Join(t.TempDir(), "SecondAddOns")

	if err := mgr.SaveSettings(AppSettings{
		AddonPath:     firstPath,
		MemoryLimitMB: 100,
		Theme:         "neutral",
	}); err != nil {
		t.Fatalf("first SaveSettings: %v", err)
	}
	if err := mgr.SaveSettings(AppSettings{
		AddonPath:     secondPath,
		MemoryLimitMB: 250,
		Theme:         "dark",
	}); err != nil {
		t.Fatalf("second SaveSettings: %v", err)
	}

	got, err := mgr.GetSettings()
	if err != nil {
		t.Fatalf("GetSettings: %v", err)
	}
	if got.AddonPath != filepath.Clean(secondPath) {
		t.Fatalf("AddonPath = %q, want %q", got.AddonPath, filepath.Clean(secondPath))
	}
	if got.MemoryLimitMB != 250 {
		t.Fatalf("MemoryLimitMB = %d, want 250", got.MemoryLimitMB)
	}
	if got.Theme != "dark" {
		t.Fatalf("Theme = %q, want dark", got.Theme)
	}

	data, err := os.ReadFile(settingsPath)
	if err != nil {
		t.Fatalf("ReadFile(settings.toml): %v", err)
	}
	text := string(data)
	if !strings.Contains(text, filepath.Clean(secondPath)) {
		t.Fatalf("settings.toml does not contain second addon path:\n%s", text)
	}
}

func newTestManager(t *testing.T) *Manager {
	t.Helper()
	mgr, _ := newTestManagerAndPath(t)
	return mgr
}

func TestGetSettingsMigratesLegacySQLiteSettingsToTOML(t *testing.T) {
	db := newTestDB(t)
	addonPath := filepath.Join(t.TempDir(), "AddOns")
	if err := db.Create(&[]esoui.DBSetting{
		{Key: keyAddonPath, Value: addonPath},
		{Key: keyAutoUpdate, Value: "true"},
		{Key: keyMemoryLimitMB, Value: "220"},
		{Key: keyTheme, Value: "dark"},
	}).Error; err != nil {
		t.Fatalf("seed legacy settings: %v", err)
	}
	settingsPath := filepath.Join(t.TempDir(), "settings.toml")
	mgr := newManagerWithPath(db, settingsPath)

	got, err := mgr.GetSettings()
	if err != nil {
		t.Fatalf("GetSettings: %v", err)
	}
	if got.AddonPath != filepath.Clean(addonPath) {
		t.Fatalf("AddonPath = %q, want %q", got.AddonPath, filepath.Clean(addonPath))
	}
	if got.AutoUpdate {
		t.Fatal("AutoUpdate = true, want inert false after migration")
	}
	if got.MemoryLimitMB != 220 || got.Theme != "dark" {
		t.Fatalf("migrated settings = %+v, want memory 220 and dark theme", got)
	}
	if _, err := os.Stat(settingsPath); err != nil {
		t.Fatalf("settings.toml was not written during migration: %v", err)
	}
}

func TestGetSettingsInvalidTOMLFallsBackToDefaults(t *testing.T) {
	mgr, settingsPath := newTestManagerAndPath(t)
	if err := os.WriteFile(settingsPath, []byte("addon_path = [not valid\n"), 0o600); err != nil {
		t.Fatalf("write invalid toml: %v", err)
	}

	got, err := mgr.GetSettings()
	if err != nil {
		t.Fatalf("GetSettings: %v", err)
	}
	if got != defaults() {
		t.Fatalf("settings = %+v, want defaults %+v", got, defaults())
	}
}

func TestSaveSettingsAtomicWriteFailureKeepsPreviousFile(t *testing.T) {
	mgr, settingsPath := newTestManagerAndPath(t)
	addonPath := filepath.Join(t.TempDir(), "AddOns")
	if err := mgr.SaveSettings(AppSettings{AddonPath: addonPath, MemoryLimitMB: 200, Theme: "dark"}); err != nil {
		t.Fatalf("initial SaveSettings: %v", err)
	}
	before, err := os.ReadFile(settingsPath)
	if err != nil {
		t.Fatalf("read original settings: %v", err)
	}

	originalAtomicWrite := atomicWriteFile
	atomicWriteFile = func(string, []byte, os.FileMode) error {
		return errors.New("injected atomic write failure")
	}
	defer func() {
		atomicWriteFile = originalAtomicWrite
	}()

	err = mgr.SaveSettings(AppSettings{AddonPath: addonPath, MemoryLimitMB: 250, Theme: "neutral"})
	if err == nil {
		t.Fatal("expected atomic write error")
	}
	after, err := os.ReadFile(settingsPath)
	if err != nil {
		t.Fatalf("read settings after failed write: %v", err)
	}
	if string(after) != string(before) {
		t.Fatalf("settings.toml changed after failed atomic write\nbefore:\n%s\nafter:\n%s", before, after)
	}
}

func newTestManagerAndPath(t *testing.T) (*Manager, string) {
	t.Helper()
	db := newTestDB(t)
	settingsPath := filepath.Join(t.TempDir(), "settings.toml")
	return newManagerWithPath(db, settingsPath), settingsPath
}

func newTestDB(t *testing.T) *gorm.DB {
	t.Helper()

	db, err := esoui.OpenDB(filepath.Join(t.TempDir(), "settings.db"))
	if err != nil {
		t.Fatalf("OpenDB: %v", err)
	}
	return db
}

func newManagerWithPath(db *gorm.DB, settingsPath string) *Manager {
	return &Manager{db: db, settingsPath: settingsPath}
}
