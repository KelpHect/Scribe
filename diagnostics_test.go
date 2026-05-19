package main

import (
	"testing"
	"time"
)

func TestDiagnosticsStartupMSUsesReadyTimeNotUptime(t *testing.T) {
	app := NewApp()
	startedAt := time.Now().Add(-5 * time.Minute)

	app.perfMu.Lock()
	app.startedAt = startedAt
	app.frontendReadyAt = startedAt.Add(250 * time.Millisecond)
	app.perfMu.Unlock()

	snapshot := app.getDiagnosticsSnapshot()
	if snapshot.StartupMS < 240 || snapshot.StartupMS > 260 {
		t.Fatalf("StartupMS = %d, want about 250ms", snapshot.StartupMS)
	}
	if snapshot.UptimeMS < int64((4 * time.Minute).Milliseconds()) {
		t.Fatalf("UptimeMS = %d, want process uptime separate from startup duration", snapshot.UptimeMS)
	}
}
