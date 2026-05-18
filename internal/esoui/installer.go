package esoui

import (
	"archive/zip"
	"context"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

const downloadTimeout = 5 * time.Minute
const staleInstallArtifactAge = 1 * time.Hour

type ExtractionProgressFn func(extracted, total int)

type InstallPlanEntry struct {
	FolderName string `json:"folderName"`
	Action     string `json:"action"`
	Reason     string `json:"reason"`
}

type TempArtifactCleanupReport struct {
	Removed  []string
	Retained []string
	Errors   []string
}

func (r TempArtifactCleanupReport) RemovedCount() int {
	return len(r.Removed)
}

func (r TempArtifactCleanupReport) RetainedCount() int {
	return len(r.Retained)
}

func (r TempArtifactCleanupReport) Error() string {
	if len(r.Errors) == 0 {
		return ""
	}
	return strings.Join(r.Errors, "; ")
}

func CleanStaleInstallArtifacts(addonPath string, olderThan time.Duration) TempArtifactCleanupReport {
	if olderThan <= 0 {
		olderThan = staleInstallArtifactAge
	}

	var report TempArtifactCleanupReport
	cleanDest := filepath.Clean(addonPath)
	entries, err := os.ReadDir(cleanDest)
	if err != nil {
		if !os.IsNotExist(err) {
			report.Errors = append(report.Errors, fmt.Sprintf("read AddOns directory: %v", err))
		}
		return report
	}

	now := time.Now()
	for _, entry := range entries {
		name := entry.Name()
		if !isScribeInstallArtifactName(name) {
			continue
		}
		if !entry.IsDir() {
			report.Retained = append(report.Retained, name)
			continue
		}

		info, err := entry.Info()
		if err != nil {
			report.Errors = append(report.Errors, fmt.Sprintf("stat %s: %v", name, err))
			continue
		}
		if age := now.Sub(info.ModTime()); age < olderThan {
			report.Retained = append(report.Retained, name)
			continue
		}

		target := filepath.Join(cleanDest, name)
		if !pathInsideDir(cleanDest, target) {
			report.Errors = append(report.Errors, fmt.Sprintf("refusing cleanup outside AddOns: %s", name))
			continue
		}
		if err := os.RemoveAll(target); err != nil {
			report.Errors = append(report.Errors, fmt.Sprintf("remove %s: %v", name, err))
			continue
		}
		report.Removed = append(report.Removed, name)
	}

	sort.Strings(report.Removed)
	sort.Strings(report.Retained)
	sort.Strings(report.Errors)
	return report
}

func isScribeInstallArtifactName(name string) bool {
	return strings.HasPrefix(name, ".scribe-staging-") || strings.HasPrefix(name, ".scribe-backup-")
}

func DownloadAndInstall(downloadURL, destDir string) error {
	tmp, err := os.CreateTemp("", "scribeeso_*.zip")
	if err != nil {
		return fmt.Errorf("create temp file: %w", err)
	}
	tmpPath := tmp.Name()
	defer os.Remove(tmpPath)

	client := &http.Client{Timeout: downloadTimeout}
	resp, err := client.Get(downloadURL)
	if err != nil {
		tmp.Close()
		return fmt.Errorf("download %s: %w", downloadURL, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		tmp.Close()
		return fmt.Errorf("download %s returned %d", downloadURL, resp.StatusCode)
	}

	if _, err := io.Copy(tmp, resp.Body); err != nil {
		tmp.Close()
		return fmt.Errorf("write download: %w", err)
	}
	if err := tmp.Close(); err != nil {
		return fmt.Errorf("close temp file: %w", err)
	}

	return extractZip(tmpPath, destDir)
}

func ExtractWithProgress(ctx context.Context, zipPath, destDir string, fn ExtractionProgressFn) error {
	r, err := zip.OpenReader(zipPath)
	if err != nil {
		return fmt.Errorf("open zip: %w", err)
	}
	defer r.Close()

	cleanDest := filepath.Clean(destDir)
	totalFiles := len(r.File)

	zipTopDirs := make(map[string]struct{})
	for _, f := range r.File {
		parts := strings.SplitN(filepath.FromSlash(f.Name), string(filepath.Separator), 2)
		if len(parts) > 0 && parts[0] != "" {
			zipTopDirs[parts[0]] = struct{}{}
		}
	}
	existingDirs := make(map[string]struct{})
	for dir := range zipTopDirs {
		target := filepath.Join(cleanDest, dir)
		if info, statErr := os.Stat(target); statErr == nil && info.IsDir() {
			existingDirs[dir] = struct{}{}
		}
	}

	for i, f := range r.File {

		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		destPath, err := safeZipEntryDestination(cleanDest, f.Name)
		if err != nil {
			return err
		}

		if f.FileInfo().IsDir() {
			if err := os.MkdirAll(destPath, 0o755); err != nil {
				return fmt.Errorf("mkdir %s: %w", destPath, err)
			}
		} else {
			if err := os.MkdirAll(filepath.Dir(destPath), 0o755); err != nil {
				return fmt.Errorf("mkdir parent %s: %w", filepath.Dir(destPath), err)
			}
			if err := extractZipEntry(f, destPath); err != nil {
				return err
			}
		}

		if fn != nil {
			fn(i+1, totalFiles)
		}
	}

	return nil
}

func InstallArchiveWithProgress(ctx context.Context, zipPath, destDir string, expectedDirs []string, fn ExtractionProgressFn) ([]InstallPlanEntry, error) {
	plan, err := PlanInstallArchive(zipPath, destDir, expectedDirs)
	if err != nil {
		return nil, err
	}
	if err := installPlannedArchiveWithProgress(ctx, zipPath, destDir, plan, fn); err != nil {
		return plan, err
	}
	return plan, nil
}

func installPlannedArchiveWithProgress(ctx context.Context, zipPath, destDir string, plan []InstallPlanEntry, fn ExtractionProgressFn) error {
	stagingDir, err := os.MkdirTemp(filepath.Clean(destDir), ".scribe-staging-*")
	if err != nil {
		return fmt.Errorf("create staging directory: %w", err)
	}
	defer os.RemoveAll(stagingDir)

	if err := ExtractWithProgress(ctx, zipPath, stagingDir, fn); err != nil {
		return err
	}
	select {
	case <-ctx.Done():
		return ctx.Err()
	default:
	}

	if err := commitStagedInstall(stagingDir, destDir, plan); err != nil {
		return err
	}
	return nil
}

func PlanInstallArchive(zipPath, destDir string, expectedDirs []string) ([]InstallPlanEntry, error) {
	r, err := zip.OpenReader(zipPath)
	if err != nil {
		return nil, fmt.Errorf("open zip: %w", err)
	}
	defer r.Close()

	cleanDest := filepath.Clean(destDir)
	expected := make(map[string]struct{}, len(expectedDirs))
	for _, dir := range expectedDirs {
		name := strings.TrimSpace(dir)
		if name != "" {
			expected[strings.ToLower(name)] = struct{}{}
		}
	}

	type folderInfo struct {
		hasManifest bool
	}
	folders := make(map[string]*folderInfo)

	for _, f := range r.File {
		if _, err := safeZipEntryDestination(cleanDest, f.Name); err != nil {
			return nil, err
		}
		normalized := filepath.FromSlash(f.Name)
		parts := strings.Split(normalized, string(filepath.Separator))
		if len(parts) == 0 || strings.TrimSpace(parts[0]) == "" {
			continue
		}
		if len(parts) == 1 && !f.FileInfo().IsDir() {
			return nil, fmt.Errorf("archive contains root file outside addon folder: %s", f.Name)
		}

		folder := parts[0]
		if !isSafeAddonFolderName(folder) {
			return nil, fmt.Errorf("archive contains invalid addon folder name: %s", folder)
		}
		info := folders[folder]
		if info == nil {
			info = &folderInfo{}
			folders[folder] = info
		}

		if len(parts) == 2 {
			entryName := strings.ToLower(parts[1])
			folderLower := strings.ToLower(folder)
			if entryName == folderLower+".txt" || entryName == folderLower+".addon" {
				info.hasManifest = true
			}
		}
	}

	if len(folders) == 0 {
		return nil, fmt.Errorf("archive contains no addon folders")
	}

	plan := make([]InstallPlanEntry, 0, len(folders))
	for folder, info := range folders {
		if !info.hasManifest {
			return nil, fmt.Errorf("addon folder %s has no canonical manifest", folder)
		}
		if len(expected) > 0 {
			if _, ok := expected[strings.ToLower(folder)]; !ok {
				return nil, fmt.Errorf("archive folder %s is not listed by ESOUI metadata", folder)
			}
		}

		action := "add"
		reason := "folder is not installed"
		if stat, err := os.Stat(filepath.Join(cleanDest, folder)); err == nil && stat.IsDir() {
			action = "replace"
			reason = "folder already exists"
		} else if err != nil && !os.IsNotExist(err) {
			return nil, fmt.Errorf("stat addon folder %s: %w", folder, err)
		}

		plan = append(plan, InstallPlanEntry{
			FolderName: folder,
			Action:     action,
			Reason:     reason,
		})
	}

	sort.Slice(plan, func(i, j int) bool {
		return strings.ToLower(plan[i].FolderName) < strings.ToLower(plan[j].FolderName)
	})

	return plan, nil
}

func extractZip(zipPath, destDir string) error {
	_, err := InstallArchiveWithProgress(context.Background(), zipPath, destDir, nil, nil)
	return err
}

func commitStagedInstall(stagingDir, destDir string, plan []InstallPlanEntry) error {
	cleanDest := filepath.Clean(destDir)
	backupDir, err := os.MkdirTemp(cleanDest, ".scribe-backup-*")
	if err != nil {
		return fmt.Errorf("create backup directory: %w", err)
	}
	keepBackup := false
	defer func() {
		if !keepBackup {
			_ = os.RemoveAll(backupDir)
		}
	}()

	type movedFolder struct {
		name      string
		action    string
		dst       string
		backup    string
		installed bool
	}
	moved := make([]movedFolder, 0, len(plan))

	rollback := func() {
		for i := len(moved) - 1; i >= 0; i-- {
			item := moved[i]
			if item.installed {
				_ = os.RemoveAll(item.dst)
			}
			if item.action == "replace" && item.backup != "" {
				_ = os.Rename(item.backup, item.dst)
			}
		}
	}

	for _, item := range plan {
		src := filepath.Join(stagingDir, item.FolderName)
		dst := filepath.Join(cleanDest, item.FolderName)
		if _, err := os.Stat(src); err != nil {
			rollback()
			return fmt.Errorf("staged addon folder missing %s: %w", item.FolderName, err)
		}

		movedItem := movedFolder{name: item.FolderName, action: item.Action, dst: dst}
		if item.Action == "replace" {
			backup := filepath.Join(backupDir, item.FolderName)
			if err := os.Rename(dst, backup); err != nil {
				rollback()
				return fmt.Errorf("backup existing addon folder %s: %w", item.FolderName, err)
			}
			movedItem.backup = backup
		}

		if err := os.Rename(src, dst); err != nil {
			moved = append(moved, movedItem)
			rollback()
			return fmt.Errorf("install addon folder %s: %w", item.FolderName, err)
		}
		movedItem.installed = true
		moved = append(moved, movedItem)
	}

	return nil
}

func safeZipEntryDestination(cleanDest, entryName string) (string, error) {
	if strings.Contains(entryName, `\`) || strings.HasPrefix(entryName, `/`) || hasWindowsVolumePrefix(entryName) {
		return "", fmt.Errorf("zip entry escapes destination: %s", entryName)
	}

	name := filepath.FromSlash(entryName)
	destPath := filepath.Clean(filepath.Join(cleanDest, name))

	if !strings.HasPrefix(destPath, cleanDest+string(filepath.Separator)) && destPath != cleanDest {
		return "", fmt.Errorf("zip entry escapes destination: %s", entryName)
	}
	return destPath, nil
}

func pathInsideDir(cleanDir, target string) bool {
	cleanTarget := filepath.Clean(target)
	return strings.HasPrefix(cleanTarget, cleanDir+string(filepath.Separator)) || cleanTarget == cleanDir
}

func isSafeAddonFolderName(folderName string) bool {
	return folderName != "" &&
		folderName != "." &&
		folderName != ".." &&
		!strings.ContainsAny(folderName, `/\`) &&
		!hasWindowsVolumePrefix(folderName)
}

func hasWindowsVolumePrefix(name string) bool {
	return len(name) >= 2 && name[1] == ':' && ((name[0] >= 'A' && name[0] <= 'Z') || (name[0] >= 'a' && name[0] <= 'z'))
}

func extractZipEntry(f *zip.File, destPath string) error {
	rc, err := f.Open()
	if err != nil {
		return fmt.Errorf("open zip entry %s: %w", f.Name, err)
	}
	defer rc.Close()

	out, err := os.OpenFile(destPath, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, f.Mode())
	if err != nil {
		return fmt.Errorf("create file %s: %w", destPath, err)
	}
	defer out.Close()

	if _, err := io.Copy(out, rc); err != nil {
		return fmt.Errorf("write file %s: %w", destPath, err)
	}
	return nil
}

func RemoveAddonFolder(addonPath, folderName string) error {
	if !isSafeAddonFolderName(folderName) {
		return fmt.Errorf("invalid folder name: %q", folderName)
	}

	target := filepath.Join(filepath.Clean(addonPath), folderName)

	if !strings.HasPrefix(target, filepath.Clean(addonPath)+string(filepath.Separator)) {
		return fmt.Errorf("folder name escapes addon path: %q", folderName)
	}

	if _, err := os.Stat(target); os.IsNotExist(err) {
		return fmt.Errorf("addon folder not found: %s", folderName)
	}
	return os.RemoveAll(target)
}
