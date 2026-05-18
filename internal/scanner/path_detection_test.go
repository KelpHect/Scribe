package scanner

import (
	"path/filepath"
	"testing"
)

func TestDetectAddonPathWindowsCandidatePrecedence(t *testing.T) {
	home := filepath.Join("C:", "Users", "Tester")
	live := filepath.Join(home, "Documents", "Elder Scrolls Online", "live", "AddOns")
	liveEU := filepath.Join(home, "Documents", "Elder Scrolls Online", "liveeu", "AddOns")
	oneDriveLive := filepath.Join(home, "OneDrive", "Documents", "Elder Scrolls Online", "live", "AddOns")

	got := detectAddonPath(home, "windows", fakeExists(live, liveEU, oneDriveLive), nilGlob)
	if got != live {
		t.Fatalf("DetectAddonPath windows = %q, want live candidate %q", got, live)
	}
}

func TestDetectAddonPathWindowsFallsBackToLiveEU(t *testing.T) {
	home := filepath.Join("C:", "Users", "Tester")
	liveEU := filepath.Join(home, "Documents", "Elder Scrolls Online", "liveeu", "AddOns")

	got := detectAddonPath(home, "windows", fakeExists(liveEU), nilGlob)
	if got != liveEU {
		t.Fatalf("DetectAddonPath windows = %q, want liveeu candidate %q", got, liveEU)
	}
}

func TestDetectAddonPathWindowsUsesOneDriveGlobMatches(t *testing.T) {
	home := filepath.Join("C:", "Users", "Tester")
	globbed := filepath.Join(home, "OneDrive - Guild", "Documents", "Elder Scrolls Online", "live", "AddOns")

	got := detectAddonPath(home, "windows", fakeExists(globbed), func(pattern string) []string {
		if pattern == filepath.Join(home, "OneDrive*", "Documents", "Elder Scrolls Online", "live", "AddOns") {
			return []string{globbed}
		}
		return nil
	})
	if got != globbed {
		t.Fatalf("DetectAddonPath windows glob = %q, want %q", got, globbed)
	}
}

func TestDetectAddonPathDarwinUsesDocumentsLive(t *testing.T) {
	home := filepath.Join("/", "Users", "tester")
	live := filepath.Join(home, "Documents", "Elder Scrolls Online", "live", "AddOns")
	liveEU := filepath.Join(home, "Documents", "Elder Scrolls Online", "liveeu", "AddOns")

	got := detectAddonPath(home, "darwin", fakeExists(live, liveEU), nilGlob)
	if got != live {
		t.Fatalf("DetectAddonPath darwin = %q, want live candidate %q", got, live)
	}
}

func TestDetectAddonPathLinuxPrefersSteamCompatDataThenDocuments(t *testing.T) {
	home := filepath.Join("/", "home", "tester")
	steam := filepath.Join(home, ".steam", "steam", "steamapps", "compatdata", "306130", "pfx", "drive_c", "users", "steamuser", "Documents", "Elder Scrolls Online", "live", "AddOns")
	documents := filepath.Join(home, "Documents", "Elder Scrolls Online", "live", "AddOns")

	got := detectAddonPath(home, "linux", fakeExists(steam, documents), nilGlob)
	if got != steam {
		t.Fatalf("DetectAddonPath linux = %q, want Steam candidate %q", got, steam)
	}

	got = detectAddonPath(home, "linux", fakeExists(documents), nilGlob)
	if got != documents {
		t.Fatalf("DetectAddonPath linux fallback = %q, want Documents candidate %q", got, documents)
	}
}

func TestDetectAddonPathUnsupportedOrMissingReturnsEmpty(t *testing.T) {
	home := filepath.Join("/", "home", "tester")
	if got := detectAddonPath(home, "plan9", fakeExists(), nilGlob); got != "" {
		t.Fatalf("DetectAddonPath unsupported = %q, want empty", got)
	}
	if got := detectAddonPath(home, "linux", fakeExists(), nilGlob); got != "" {
		t.Fatalf("DetectAddonPath missing = %q, want empty", got)
	}
}

func fakeExists(paths ...string) func(string) bool {
	existing := make(map[string]bool, len(paths))
	for _, path := range paths {
		existing[path] = true
	}
	return func(path string) bool {
		return existing[path]
	}
}

func nilGlob(string) []string {
	return nil
}
