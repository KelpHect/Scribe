package esoui

import (
	"testing"

	"Scribe/internal/addon"
)

func TestMatchAddonsVersionUpdateDetection(t *testing.T) {
	tests := []struct {
		name       string
		local      string
		remote     string
		wantUpdate bool
	}{
		{name: "exact version", local: "1.2.3", remote: "1.2.3", wantUpdate: false},
		{name: "local older", local: "1.2.3", remote: "1.2.4", wantUpdate: true},
		{name: "local newer", local: "1.3.0", remote: "1.2.9", wantUpdate: false},
		{name: "numeric suffix newer", local: "v2.4", remote: "Version 2.5b", wantUpdate: true},
		{name: "numeric suffix equal", local: "release-2.5b", remote: "Version 2.5", wantUpdate: false},
		{name: "empty local", local: "", remote: "2.0", wantUpdate: false},
		{name: "empty remote", local: "2.0", remote: "", wantUpdate: false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			matches := MatchAddons(
				[]*addon.Addon{{FolderName: "MyAddon", Version: tt.local}},
				[]RemoteAddon{{UID: "1", UIName: "My Addon", UIVersion: tt.remote, UIDirs: []string{"MyAddon"}}},
			)
			if len(matches) != 1 {
				t.Fatalf("matches = %d, want 1", len(matches))
			}
			if matches[0].UpdateAvailable != tt.wantUpdate {
				t.Fatalf("UpdateAvailable = %v, want %v", matches[0].UpdateAvailable, tt.wantUpdate)
			}
		})
	}
}

func TestMatchAddonsSelectsMostSpecificRemoteDirectoryCandidate(t *testing.T) {
	matches := MatchAddons(
		[]*addon.Addon{{FolderName: "SharedLib", Version: "1.0"}},
		[]RemoteAddon{
			{UID: "bundle", UIName: "Bundle", UIVersion: "9.0", UIDirs: []string{"SharedLib", "OtherLib"}},
			{UID: "specific", UIName: "Shared Lib", UIVersion: "1.1", UIDirs: []string{"SharedLib"}},
		},
	)

	if len(matches) != 1 {
		t.Fatalf("matches = %d, want 1", len(matches))
	}
	if matches[0].Remote == nil || matches[0].Remote.UID != "specific" {
		t.Fatalf("Remote UID = %#v, want specific", matches[0].Remote)
	}
	if !matches[0].UpdateAvailable {
		t.Fatal("UpdateAvailable = false, want true against selected specific candidate")
	}
}

func TestMatchAddonsSkipsUnmatchedLocalAddons(t *testing.T) {
	matches := MatchAddons(
		[]*addon.Addon{{FolderName: "LocalOnly", Version: "1.0"}},
		[]RemoteAddon{{UID: "1", UIName: "Other", UIVersion: "2.0", UIDirs: []string{"OtherAddon"}}},
	)
	if len(matches) != 0 {
		t.Fatalf("matches = %d, want 0", len(matches))
	}
}
