package main

import (
	"errors"
	"strings"
	"testing"
)

func TestPrivacySafePersistenceErrorDoesNotExposePath(t *testing.T) {
	err := errors.New("open /home/tester/.config/Scribe/esoui_cache.db: permission denied")

	got := privacySafePersistenceError(err)
	if strings.Contains(got, "/home/tester") || strings.Contains(got, "esoui_cache.db") {
		t.Fatalf("privacySafePersistenceError() exposed local path: %q", got)
	}
	if !strings.Contains(got, "permissions") || !strings.Contains(got, "disk space") {
		t.Fatalf("privacySafePersistenceError() = %q, want actionable generic message", got)
	}
}

func TestDiagnosticsIncludesPersistenceStatus(t *testing.T) {
	app := NewApp()
	app.setPersistenceStatus("degraded", "settings and cache persistence are unavailable")

	snapshot := app.getDiagnosticsSnapshot()
	if snapshot.PersistenceStatus != "degraded" {
		t.Fatalf("PersistenceStatus = %q, want degraded", snapshot.PersistenceStatus)
	}
	if snapshot.PersistenceError == "" {
		t.Fatal("PersistenceError is empty, want degraded message")
	}
}
