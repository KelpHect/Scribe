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
		wantState  string
	}{
		{name: "exact version", local: "1.2.3", remote: "1.2.3", wantUpdate: false, wantState: UpdateStateUpToDate},
		{name: "local older", local: "1.2.3", remote: "1.2.4", wantUpdate: true, wantState: UpdateStateRemoteNewer},
		{name: "local newer", local: "1.3.0", remote: "1.2.9", wantUpdate: false, wantState: UpdateStateLocalNewer},
		{name: "numeric suffix newer", local: "v2.4", remote: "Version 2.5b", wantUpdate: true, wantState: UpdateStateRemoteNewer},
		{name: "numeric suffix equal", local: "release-2.5b", remote: "Version 2.5", wantUpdate: false, wantState: UpdateStateUpToDate},
		{name: "empty local", local: "", remote: "2.0", wantUpdate: false, wantState: UpdateStateUnknownVersion},
		{name: "empty remote", local: "2.0", remote: "", wantUpdate: false, wantState: UpdateStateUnknownVersion},
		{name: "unparseable", local: "release", remote: "new", wantUpdate: false, wantState: UpdateStateUnknownVersion},
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
			if matches[0].UpdateState != tt.wantState {
				t.Fatalf("UpdateState = %q, want %q", matches[0].UpdateState, tt.wantState)
			}
			if matches[0].UpdateReason == "" {
				t.Fatal("UpdateReason is empty")
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

func TestMatchAddonsClassifiesUnmatchedLocalAddons(t *testing.T) {
	matches := MatchAddons(
		[]*addon.Addon{{FolderName: "LocalOnly", Version: "1.0"}},
		[]RemoteAddon{{UID: "1", UIName: "Other", UIVersion: "2.0", UIDirs: []string{"OtherAddon"}}},
	)
	if len(matches) != 1 {
		t.Fatalf("matches = %d, want 1 unmatched local", len(matches))
	}
	if matches[0].UpdateState != UpdateStateUnmatched || matches[0].Remote != nil || matches[0].UpdateAvailable {
		t.Fatalf("unmatched state = %+v", matches[0])
	}
}
