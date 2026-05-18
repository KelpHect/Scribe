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

func newTestManager(t *testing.T) *Manager {
	t.Helper()

	db, err := esoui.OpenDB(filepath.Join(t.TempDir(), "settings.db"))
	if err != nil {
		t.Fatalf("OpenDB: %v", err)
	}
	return NewManager(db)
}
