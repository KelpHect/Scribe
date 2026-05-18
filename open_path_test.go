package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestValidateOpenPathAllowsAddonRootAndChildDirectories(t *testing.T) {
	root := t.TempDir()
	addons := filepath.Join(root, "AddOns")
	addon := filepath.Join(addons, "ValidAddon")
	mustMkdir(t, addon)

	for _, path := range []string{addons, addon} {
		got, err := validateOpenPath(addons, path)
		if err != nil {
			t.Fatalf("validateOpenPath(%q) returned error: %v", path, err)
		}
		if got != path {
			t.Fatalf("validateOpenPath(%q) = %q, want %q", path, got, path)
		}
	}
}

func TestValidateOpenPathRejectsUnexpectedPaths(t *testing.T) {
	root := t.TempDir()
	addons := filepath.Join(root, "AddOns")
	addon := filepath.Join(addons, "ValidAddon")
	outside := filepath.Join(root, "Outside")
	siblingPrefix := filepath.Join(root, "AddOnsSibling")
	fileTarget := filepath.Join(addons, "file.txt")
	mustMkdir(t, addon)
	mustMkdir(t, outside)
	mustMkdir(t, siblingPrefix)
	if err := os.WriteFile(fileTarget, []byte("not a directory"), 0o644); err != nil {
		t.Fatalf("write target file: %v", err)
	}

	tests := []struct {
		name      string
		addonPath string
		path      string
		want      string
	}{
		{name: "empty addon path", addonPath: "", path: addon, want: "addon path is not configured"},
		{name: "empty target path", addonPath: addons, path: "", want: "addon path is not configured"},
		{name: "relative addon path", addonPath: "AddOns", path: addon, want: "path must be absolute"},
		{name: "relative target path", addonPath: addons, path: "ValidAddon", want: "path must be absolute"},
		{name: "outside directory", addonPath: addons, path: outside, want: "inside configured AddOns"},
		{name: "sibling prefix directory", addonPath: addons, path: siblingPrefix, want: "inside configured AddOns"},
		{name: "missing target", addonPath: addons, path: filepath.Join(addons, "Missing"), want: "stat open path"},
		{name: "file target", addonPath: addons, path: fileTarget, want: "not a directory"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if _, err := validateOpenPath(tt.addonPath, tt.path); err == nil || !strings.Contains(err.Error(), tt.want) {
				t.Fatalf("validateOpenPath() error = %v, want containing %q", err, tt.want)
			}
		})
	}
}

func TestValidateOpenPathRejectsSymlinkEscape(t *testing.T) {
	root := t.TempDir()
	addons := filepath.Join(root, "AddOns")
	outside := filepath.Join(root, "Outside")
	link := filepath.Join(addons, "LinkedOutside")
	mustMkdir(t, addons)
	mustMkdir(t, outside)

	if err := os.Symlink(outside, link); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}

	if _, err := validateOpenPath(addons, link); err == nil || !strings.Contains(err.Error(), "inside configured AddOns") {
		t.Fatalf("validateOpenPath() error = %v, want inside configured AddOns", err)
	}
}

func mustMkdir(t *testing.T, path string) {
	t.Helper()
	if err := os.MkdirAll(path, 0o755); err != nil {
		t.Fatalf("mkdir %s: %v", path, err)
	}
}
