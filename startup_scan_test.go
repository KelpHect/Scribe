package main

import (
	"context"
	"testing"
	"time"

	"Scribe/internal/scanner"
)

func TestGetInstalledAddonsReturnsCachedStateAndScansInBackground(t *testing.T) {
	root := t.TempDir()
	writeManifest(t, root, "AddonOne", "## Title: Addon One\n")

	app := NewApp()
	app.scanner = scanner.New(root)
	app.shutdownCtx, app.shutdownCancel = context.WithCancel(context.Background())
	defer app.shutdownCancel()

	addons, err := app.GetInstalledAddons()
	if err != nil {
		t.Fatalf("GetInstalledAddons: %v", err)
	}
	if len(addons) != 0 {
		t.Fatalf("GetInstalledAddons returned %d addons immediately, want cached empty state", len(addons))
	}

	deadline := time.Now().Add(2 * time.Second)
	for {
		if got := app.scanner.GetAddons(); len(got) == 1 {
			break
		}
		if time.Now().After(deadline) {
			t.Fatalf("background scan did not populate cached addons; got %#v", app.scanner.GetAddons())
		}
		time.Sleep(10 * time.Millisecond)
	}

	app.scanMu.RLock()
	scanStarted := app.scanStartedAt
	scanReady := app.scanReadyAt
	scanInFlight := app.scanInFlight
	app.scanMu.RUnlock()
	if scanStarted.IsZero() {
		t.Fatal("scanStartedAt is zero, want background scan start recorded")
	}
	if scanReady.IsZero() {
		t.Fatal("scanReadyAt is zero, want background scan completion recorded")
	}
	if scanInFlight {
		t.Fatal("scanInFlight = true after background scan completed")
	}
}
