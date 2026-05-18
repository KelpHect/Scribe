package main

import (
	"testing"
	"time"
)

func TestRemoteRefreshInFlightGuard(t *testing.T) {
	app := NewApp()
	close(app.initDone)

	if !app.beginRemoteRefresh() {
		t.Fatal("first beginRemoteRefresh() = false, want true")
	}
	status := app.GetRemoteCatalogStatus()
	if !status.RefreshInFlight {
		t.Fatal("RefreshInFlight = false, want true while refresh is guarded")
	}
	if status.RefreshStartedAt == "" {
		t.Fatal("RefreshStartedAt is empty")
	}
	if _, err := time.Parse(time.RFC3339, status.RefreshStartedAt); err != nil {
		t.Fatalf("RefreshStartedAt = %q, want RFC3339: %v", status.RefreshStartedAt, err)
	}
	if app.beginRemoteRefresh() {
		t.Fatal("second beginRemoteRefresh() = true, want false while refresh is in flight")
	}

	app.endRemoteRefresh()
	status = app.GetRemoteCatalogStatus()
	if status.RefreshInFlight {
		t.Fatal("RefreshInFlight = true after endRemoteRefresh")
	}
	if !app.beginRemoteRefresh() {
		t.Fatal("beginRemoteRefresh() after end = false, want true")
	}
}
