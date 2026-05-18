package scanner

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"

	"Scribe/internal/addon"
)

type Scanner struct {
	mu        sync.RWMutex
	addons    map[string]*addon.Addon
	addonPath string
}

func New(addonPath string) *Scanner {
	return &Scanner{
		addons:    make(map[string]*addon.Addon),
		addonPath: addonPath,
	}
}

func DetectAddonPath() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}

	return detectAddonPath(home, runtime.GOOS, dirExists, globPaths)
}

func detectAddonPath(home, goos string, exists func(string) bool, glob func(string) []string) string {
	var candidates []string
	switch goos {
	case "windows":
		candidates = []string{
			filepath.Join(home, "Documents", "Elder Scrolls Online", "live", "AddOns"),
			filepath.Join(home, "Documents", "Elder Scrolls Online", "liveeu", "AddOns"),
			filepath.Join(home, "OneDrive", "Documents", "Elder Scrolls Online", "live", "AddOns"),
			filepath.Join(home, "OneDrive", "Documents", "Elder Scrolls Online", "liveeu", "AddOns"),
		}
		candidates = append(candidates, glob(filepath.Join(home, "OneDrive*", "Documents", "Elder Scrolls Online", "live", "AddOns"))...)
		candidates = append(candidates, glob(filepath.Join(home, "OneDrive*", "Documents", "Elder Scrolls Online", "liveeu", "AddOns"))...)
	case "darwin":
		candidates = []string{
			filepath.Join(home, "Documents", "Elder Scrolls Online", "live", "AddOns"),
		}
	case "linux":
		configDir := filepath.Join(home, ".steam", "steam", "steamapps", "compatdata")
		candidates = []string{
			filepath.Join(configDir, "306130", "pfx", "drive_c", "users", "steamuser", "Documents", "Elder Scrolls Online", "live", "AddOns"),
			filepath.Join(home, "Documents", "Elder Scrolls Online", "live", "AddOns"),
		}
	default:
		return ""
	}

	for _, p := range candidates {
		if exists(p) {
			return p
		}
	}
	return ""
}

func dirExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.IsDir()
}

func globPaths(pattern string) []string {
	matches, _ := filepath.Glob(pattern)
	return matches
}

func (s *Scanner) Scan() ([]*addon.Addon, error) {
	s.mu.RLock()
	addonPath := s.addonPath
	s.mu.RUnlock()

	if addonPath == "" {
		return nil, fmt.Errorf("addon path not configured")
	}

	entries, err := os.ReadDir(addonPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read addon directory: %w", err)
	}

	var wg sync.WaitGroup
	results := make(chan *addon.Addon, len(entries))
	errors := make(chan error, len(entries))

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		wg.Add(1)
		go func(name string) {
			defer wg.Done()
			a, err := s.scanAddonDir(filepath.Join(addonPath, name))
			if err != nil {
				errors <- err
				return
			}
			if a != nil {
				results <- a
			}
		}(entry.Name())
	}

	wg.Wait()
	close(results)
	close(errors)

	newAddons := make(map[string]*addon.Addon)
	for a := range results {
		newAddons[a.ID] = a
	}

	s.mu.Lock()
	s.addons = newAddons
	s.mu.Unlock()

	list := make([]*addon.Addon, 0, len(newAddons))
	for _, a := range newAddons {
		list = append(list, a)
	}
	return list, nil
}

func (s *Scanner) scanAddonDir(dir string) (*addon.Addon, error) {
	folderName := filepath.Base(dir)

	for _, canonical := range []string{folderName + ".addon", folderName + ".txt"} {
		a, err := ParseAddonFile(filepath.Join(dir, canonical))
		if err == nil && a != nil && a.Title != "" {
			return a, nil
		}
	}

	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		name := entry.Name()
		lower := strings.ToLower(name)

		if !strings.HasSuffix(lower, ".txt") && !strings.HasSuffix(lower, ".addon") {
			continue
		}
		if strings.HasPrefix(name, ".") {
			continue
		}

		if strings.EqualFold(name, folderName+".addon") || strings.EqualFold(name, folderName+".txt") {
			continue
		}
		a, err := ParseAddonFile(filepath.Join(dir, name))
		if err != nil {
			continue
		}
		if a != nil && a.Title != "" {
			return a, nil
		}
	}

	return nil, nil
}

func (s *Scanner) GetAddons() []*addon.Addon {
	s.mu.RLock()
	defer s.mu.RUnlock()
	list := make([]*addon.Addon, 0, len(s.addons))
	for _, a := range s.addons {
		list = append(list, a)
	}
	return list
}

func (s *Scanner) SetAddonPath(path string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.addonPath = path
}

func (s *Scanner) GetAddonPath() string {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.addonPath
}
