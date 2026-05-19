package main

import (
	"context"
	"testing"
	"time"

	"Scribe/internal/scanner"
)

func TestGetInstalledAddonsReturnsCachedStateWithoutStartingScan(t *testing.T) {
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

	time.Sleep(50 * time.Millisecond)
	if got := app.scanner.GetAddons(); len(got) != 0 {
		t.Fatalf("GetInstalledAddons started an implicit scan; got %#v", got)
	}
	app.scanMu.RLock()
	scanStarted := app.scanStartedAt
	scanReady := app.scanReadyAt
	scanInFlight := app.scanInFlight
	app.scanMu.RUnlock()
	if !scanStarted.IsZero() || !scanReady.IsZero() || scanInFlight {
		t.Fatalf("implicit scan state changed: started=%v ready=%v inFlight=%v", scanStarted, scanReady, scanInFlight)
	}
}

func TestRefreshInstalledAddonsScansAndUpdatesCachedState(t *testing.T) {
	root := t.TempDir()
	writeManifest(t, root, "AddonOne", "## Title: Addon One\n")

	app := NewApp()
	app.scanner = scanner.New(root)
	app.shutdownCtx, app.shutdownCancel = context.WithCancel(context.Background())
	defer app.shutdownCancel()

	addons, err := app.RefreshInstalledAddons()
	if err != nil {
		t.Fatalf("RefreshInstalledAddons: %v", err)
	}
	if len(addons) != 1 {
		t.Fatalf("RefreshInstalledAddons returned %d addons, want 1", len(addons))
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

func TestShouldEmitInstalledScanCompleteOnlyForUserVisibleScans(t *testing.T) {
	emitReasons := []string{"startup", "set-addon-path", "save-settings", "manual-refresh"}
	for _, reason := range emitReasons {
		if !shouldEmitInstalledScanComplete(reason) {
			t.Fatalf("shouldEmitInstalledScanComplete(%q) = false, want true", reason)
		}
	}
	queryReasons := []string{"get-installed", "matched-addons", "check-updates", "missing-dependencies"}
	for _, reason := range queryReasons {
		if shouldEmitInstalledScanComplete(reason) {
			t.Fatalf("shouldEmitInstalledScanComplete(%q) = true, want false", reason)
		}
	}
}
