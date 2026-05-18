package main

import "testing"

func TestRemoteRefreshInFlightGuard(t *testing.T) {
	app := &App{}

	if !app.beginRemoteRefresh() {
		t.Fatal("first beginRemoteRefresh() = false, want true")
	}
	if app.beginRemoteRefresh() {
		t.Fatal("second beginRemoteRefresh() = true, want false while refresh is in flight")
	}

	app.endRemoteRefresh()
	if !app.beginRemoteRefresh() {
		t.Fatal("beginRemoteRefresh() after end = false, want true")
	}
}
