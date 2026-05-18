package settings

import (
	"path/filepath"
	"testing"

	"Scribe/internal/esoui"
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
	mgr := newTestManager(t)
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
}

func newTestManager(t *testing.T) *Manager {
	t.Helper()

	db, err := esoui.OpenDB(filepath.Join(t.TempDir(), "settings.db"))
	if err != nil {
		t.Fatalf("OpenDB: %v", err)
	}
	return NewManager(db)
}
