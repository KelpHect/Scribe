package main

import (
	"context"
	"testing"
	"time"

	"Scribe/internal/esoui"
)

func TestApplyRemoteCatalogSkipsStateUpdateAfterShutdown(t *testing.T) {
	app := NewApp()
	ctx, cancel := context.WithCancel(context.Background())
	app.shutdownCtx = ctx
	app.remoteList = []esoui.RemoteAddon{{UID: "old", UIName: "Old"}}
	app.remoteCategories = []esoui.Category{{ID: "old-cat", Name: "Old"}}

	cancel()

	if app.applyRemoteCatalog(
		[]esoui.RemoteAddon{{UID: "new", UIName: "New"}},
		[]esoui.Category{{ID: "new-cat", Name: "New"}},
		nil,
	) {
		t.Fatal("applyRemoteCatalog returned true after shutdown")
	}

	got := app.getRemoteList()
	if len(got) != 1 || got[0].UID != "old" {
		t.Fatalf("remote list changed after shutdown: %+v", got)
	}
	if app.remoteCategories[0].ID != "old-cat" {
		t.Fatalf("remote categories changed after shutdown: %+v", app.remoteCategories)
	}
}

func TestAppShutdownIsIdempotentAndWaitsForBackgroundWork(t *testing.T) {
	app := NewApp()
	ctx, cancel := context.WithCancel(context.Background())
	app.shutdownCtx = ctx
	app.shutdownCancel = cancel

	started := make(chan struct{})
	released := make(chan struct{})
	app.refreshWg.Add(1)
	go func() {
		defer app.refreshWg.Done()
		close(started)
		<-ctx.Done()
		time.Sleep(10 * time.Millisecond)
		close(released)
	}()
	<-started

	done := make(chan struct{})
	go func() {
		app.shutdown(context.Background())
		app.shutdown(context.Background())
		close(done)
	}()

	select {
	case <-released:
	case <-time.After(500 * time.Millisecond):
		t.Fatal("shutdown did not cancel background work")
	}

	select {
	case <-done:
	case <-time.After(500 * time.Millisecond):
		t.Fatal("shutdown did not return after background work completed")
	}
}
