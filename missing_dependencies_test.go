package main

import (
	"os"
	"path/filepath"
	"testing"

	"Scribe/internal/esoui"
	"Scribe/internal/scanner"
)

func TestGetMissingDependenciesResolution(t *testing.T) {
	root := t.TempDir()
	writeManifest(t, root, "RequiredAddon", "## DependsOn: LibShared>=1.0 LibInstalled\n")
	writeManifest(t, root, "OptionalAddon", "## OptionalDependsOn: LibShared LibOptional<=2 LibUnknown\n")
	writeManifest(t, root, "LibInstalled", "## Title: Installed Library\n")

	app := &App{
		scanner: scanner.New(root),
		remoteList: []esoui.RemoteAddon{
			{UID: "shared-uid", UIName: "Shared Library", UIDirs: []string{"LibShared"}},
			{UID: "optional-uid", UIName: "Optional Library", UIDirs: []string{"OtherDir", "LibOptional"}},
		},
	}

	missing, err := app.GetMissingDependencies()
	if err != nil {
		t.Fatalf("GetMissingDependencies: %v", err)
	}
	byFolder := make(map[string]esoui.MissingDepInfo, len(missing))
	for _, dep := range missing {
		byFolder[dep.DepFolderName] = dep
	}

	shared := byFolder["libshared"]
	if shared.Optional {
		t.Fatal("LibShared Optional = true, want false because required takes precedence")
	}
	if !shared.CanInstall || shared.RemoteUID != "shared-uid" || shared.RemoteName != "Shared Library" {
		t.Fatalf("LibShared remote mapping = %+v, want installable shared-uid", shared)
	}
	if !containsString(shared.RequiredBy, "RequiredAddon") || !containsString(shared.RequiredBy, "OptionalAddon") {
		t.Fatalf("LibShared RequiredBy = %#v, want both requiring addons", shared.RequiredBy)
	}

	optional := byFolder["liboptional"]
	if !optional.Optional {
		t.Fatal("LibOptional Optional = false, want true")
	}
	if !optional.CanInstall || optional.RemoteUID != "optional-uid" {
		t.Fatalf("LibOptional remote mapping = %+v, want installable optional-uid", optional)
	}

	unknown := byFolder["libunknown"]
	if !unknown.Optional {
		t.Fatal("LibUnknown Optional = false, want true")
	}
	if unknown.CanInstall || unknown.RemoteUID != "" {
		t.Fatalf("LibUnknown remote mapping = %+v, want unresolved not installable", unknown)
	}

	if _, ok := byFolder["libinstalled"]; ok {
		t.Fatal("installed dependency LibInstalled should not be reported missing")
	}
}

func writeManifest(t *testing.T, root, folderName, content string) {
	t.Helper()

	dir := filepath.Join(root, folderName)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatalf("mkdir %s: %v", folderName, err)
	}
	if err := os.WriteFile(filepath.Join(dir, folderName+".txt"), []byte(content), 0o644); err != nil {
		t.Fatalf("write manifest %s: %v", folderName, err)
	}
}

func containsString(items []string, want string) bool {
	for _, item := range items {
		if item == want {
			return true
		}
	}
	return false
}
