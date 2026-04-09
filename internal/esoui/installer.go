package esoui

import (
	"archive/zip"
	"context"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const downloadTimeout = 5 * time.Minute

type ExtractionProgressFn func(extracted, total int)

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

		name := filepath.FromSlash(f.Name)
		destPath := filepath.Clean(filepath.Join(cleanDest, name))

		if !strings.HasPrefix(destPath, cleanDest+string(filepath.Separator)) && destPath != cleanDest {
			return fmt.Errorf("zip entry escapes destination: %s", f.Name)
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

func extractZip(zipPath, destDir string) error {
	return ExtractWithProgress(context.Background(), zipPath, destDir, nil)
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
	if strings.ContainsAny(folderName, `/\`) || folderName == ".." || folderName == "." || folderName == "" {
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
