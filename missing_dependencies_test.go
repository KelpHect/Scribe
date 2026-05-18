package main

import (
	"os"
	"path/filepath"
	"testing"

	"Scribe/internal/addon"
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
	if shared.PlanState != "installable" || shared.PlanReason == "" {
		t.Fatalf("LibShared plan = %+v, want installable with reason", shared)
	}
	if !containsString(shared.VersionConstraints, ">=1.0") {
		t.Fatalf("LibShared VersionConstraints = %#v, want >=1.0", shared.VersionConstraints)
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
	if unknown.PlanState != "unresolved" || unknown.PlanReason == "" {
		t.Fatalf("LibUnknown plan = %+v, want unresolved with reason", unknown)
	}

	if _, ok := byFolder["libinstalled"]; ok {
		t.Fatal("installed dependency LibInstalled should not be reported missing")
	}
}

func TestFindMissingDependenciesPureHelper(t *testing.T) {
	locals := []*addon.Addon{
		{FolderName: "RootAddon", DependsOn: []string{"LibRequired>=1.0"}, OptionalDependsOn: []string{"LibOptional", "LibInstalled"}},
		{FolderName: "OtherAddon", OptionalDependsOn: []string{"LibRequired<=2.0"}},
		{FolderName: "LibInstalled"},
	}
	remotes := []esoui.RemoteAddon{
		{UID: "required-uid", UIName: "Required Library", UIDirs: []string{"LibRequired"}},
		{UID: "optional-uid", UIName: "Optional Library", UIDirs: []string{"Nested", "LibOptional"}},
	}

	missing := findMissingDependencies(locals, remotes)
	byFolder := make(map[string]esoui.MissingDepInfo, len(missing))
	for _, dep := range missing {
		byFolder[dep.DepFolderName] = dep
	}

	required := byFolder["librequired"]
	if required.Optional {
		t.Fatal("LibRequired Optional = true, want false because a required dependency wins")
	}
	if required.RemoteUID != "required-uid" || !required.CanInstall {
		t.Fatalf("LibRequired remote mapping = %+v, want installable required-uid", required)
	}
	if !containsString(required.RequiredBy, "RootAddon") || !containsString(required.RequiredBy, "OtherAddon") {
		t.Fatalf("LibRequired RequiredBy = %#v, want both addons", required.RequiredBy)
	}
	if !containsString(required.VersionConstraints, ">=1.0") || !containsString(required.VersionConstraints, "<=2.0") {
		t.Fatalf("LibRequired VersionConstraints = %#v, want both version constraints", required.VersionConstraints)
	}

	optional := byFolder["liboptional"]
	if !optional.Optional || optional.RemoteUID != "optional-uid" || !optional.CanInstall {
		t.Fatalf("LibOptional = %+v, want optional installable optional-uid", optional)
	}
	if _, ok := byFolder["libinstalled"]; ok {
		t.Fatal("installed dependency should not be reported missing")
	}
}

func TestFindMissingDependenciesUsesLatestCanonicalRemoteForDuplicateDirs(t *testing.T) {
	locals := []*addon.Addon{
		{FolderName: "RootAddon", DependsOn: []string{"LibRequired<=1.0"}},
	}
	remotes := []esoui.RemoteAddon{
		{
			UID:               "latest-lib",
			UIName:            "Required Library",
			UIVersion:         "3.0",
			UIDate:            "2026-01-02",
			UIDirs:            []string{"LibRequired"},
			UIDownloadTotal:   1000,
			UIDownloadMonthly: 200,
		},
		{
			UID:               "old-bundle",
			UIName:            "Old Bundle",
			UIVersion:         "1.0",
			UIDate:            "2020-01-02",
			UIDirs:            []string{"OtherBundledDir", "LibRequired"},
			UIDownloadTotal:   10,
			UIDownloadMonthly: 1,
		},
	}

	missing := findMissingDependencies(locals, remotes)
	if len(missing) != 1 {
		t.Fatalf("missing dependencies = %#v, want one dependency", missing)
	}
	dep := missing[0]
	if dep.RemoteUID != "latest-lib" || dep.RemoteName != "Required Library" {
		t.Fatalf("Remote mapping = %+v, want latest canonical dependency addon", dep)
	}
	if !containsString(dep.VersionConstraints, "<=1.0") {
		t.Fatalf("VersionConstraints = %#v, want original requested constraint preserved for display", dep.VersionConstraints)
	}
	if dep.PlanReason == "" {
		t.Fatal("PlanReason is empty")
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
