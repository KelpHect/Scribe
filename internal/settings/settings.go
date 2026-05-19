package settings

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"Scribe/internal/esoui"

	"github.com/pelletier/go-toml/v2"
	"gorm.io/gorm"
)

type AppSettings struct {
	AddonPath     string `json:"addonPath"`
	AutoUpdate    bool   `json:"autoUpdate"`
	MemoryLimitMB int    `json:"memoryLimitMb"`
	Theme         string `json:"theme"`
}

func defaults() AppSettings {
	return AppSettings{
		AutoUpdate:    false,
		MemoryLimitMB: 150,
		Theme:         "scribe",
	}
}

type Manager struct {
	db           *gorm.DB
	settingsPath string
}

func NewManager(db *gorm.DB) *Manager {
	return &Manager{db: db, settingsPath: defaultSettingsPath()}
}

const (
	keyAddonPath     = "addon_path"
	keyAutoUpdate    = "auto_update"
	keyMemoryLimitMB = "memory_limit_mb"
	keyTheme         = "theme"
)

func (m *Manager) GetSettings() (AppSettings, error) {
	if m.settingsPath != "" {
		if s, ok, err := m.loadTOMLSettings(); ok {
			return s, err
		}
	}

	s, migrated, err := m.loadLegacyDBSettings()
	if err != nil {
		return AppSettings{}, err
	}
	if migrated && m.settingsPath != "" {
		_ = m.writeTOMLSettings(s)
	}
	return s, nil
}

func (m *Manager) SaveSettings(s AppSettings) error {
	normalized, err := normalizeSettings(s)
	if err != nil {
		return err
	}
	if m.settingsPath == "" {
		return fmt.Errorf("settings path is unavailable")
	}
	return m.writeTOMLSettings(normalized)
}

func (m *Manager) loadTOMLSettings() (AppSettings, bool, error) {
	data, err := os.ReadFile(m.settingsPath)
	if err != nil {
		if os.IsNotExist(err) {
			return AppSettings{}, false, nil
		}
		return defaults(), true, nil
	}

	var file settingsFile
	if err := toml.Unmarshal(data, &file); err != nil {
		return defaults(), true, nil
	}
	s, err := normalizeSettings(file.toAppSettings())
	if err != nil {
		return defaults(), true, nil
	}
	return s, true, nil
}

func (m *Manager) loadLegacyDBSettings() (AppSettings, bool, error) {
	if m.db == nil {
		return defaults(), false, nil
	}

	var rows []esoui.DBSetting
	if err := m.db.Select("key", "value").Find(&rows).Error; err != nil {
		return AppSettings{}, false, fmt.Errorf("load settings: %w", err)
	}
	if len(rows) == 0 {
		return defaults(), false, nil
	}

	kv := make(map[string]string, len(rows))
	for _, r := range rows {
		kv[r.Key] = r.Value
	}

	s := defaults()
	if v, ok := kv[keyAddonPath]; ok {
		if normalized, err := normalizeAddonPath(v); err == nil {
			s.AddonPath = normalized
		}
	}
	// Auto update has no safe worker yet; keep the stored preference inert until one exists.
	s.AutoUpdate = false
	if v, ok := kv[keyMemoryLimitMB]; ok {
		if parsed, err := strconv.Atoi(v); err == nil && parsed >= 0 {
			s.MemoryLimitMB = parsed
		}
	}
	if v, ok := kv[keyTheme]; ok {
		s.Theme = normalizeTheme(v)
	}
	return s, true, nil
}

func (m *Manager) writeTOMLSettings(s AppSettings) error {
	file := settingsFileFromAppSettings(s)
	data, err := toml.Marshal(file)
	if err != nil {
		return fmt.Errorf("encode settings toml: %w", err)
	}
	if err := atomicWriteFile(m.settingsPath, data, 0o600); err != nil {
		return fmt.Errorf("save settings: %w", err)
	}
	return nil
}

func normalizeSettings(s AppSettings) (AppSettings, error) {
	normalizedPath, err := normalizeAddonPath(s.AddonPath)
	if err != nil {
		return AppSettings{}, err
	}
	s.AddonPath = normalizedPath
	s.AutoUpdate = false
	if s.MemoryLimitMB < 0 {
		s.MemoryLimitMB = defaults().MemoryLimitMB
	}
	s.Theme = normalizeTheme(s.Theme)
	return s, nil
}

type settingsFile struct {
	AddonPath     string `toml:"addon_path"`
	AutoUpdate    bool   `toml:"auto_update"`
	MemoryLimitMB int    `toml:"memory_limit_mb"`
	Theme         string `toml:"theme"`
}

func settingsFileFromAppSettings(s AppSettings) settingsFile {
	return settingsFile{
		AddonPath:     s.AddonPath,
		AutoUpdate:    s.AutoUpdate,
		MemoryLimitMB: s.MemoryLimitMB,
		Theme:         s.Theme,
	}
}

func (f settingsFile) toAppSettings() AppSettings {
	return AppSettings{
		AddonPath:     f.AddonPath,
		AutoUpdate:    f.AutoUpdate,
		MemoryLimitMB: f.MemoryLimitMB,
		Theme:         f.Theme,
	}
}

var atomicWriteFile = writeFileAtomic

func writeFileAtomic(path string, data []byte, perm os.FileMode) error {
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	tmp, err := os.CreateTemp(dir, ".settings-*.tmp")
	if err != nil {
		return err
	}
	tmpName := tmp.Name()
	defer os.Remove(tmpName)

	if _, err := tmp.Write(data); err != nil {
		_ = tmp.Close()
		return err
	}
	if err := tmp.Chmod(perm); err != nil {
		_ = tmp.Close()
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	return os.Rename(tmpName, path)
}

func defaultSettingsPath() string {
	dir, err := os.UserConfigDir()
	if err != nil {
		return ""
	}
	return filepath.Join(dir, "Scribe", "settings.toml")
}

func normalizeAddonPath(path string) (string, error) {
	trimmed := strings.TrimSpace(path)
	if trimmed == "" {
		return "", nil
	}
	if strings.ContainsRune(trimmed, 0) {
		return "", fmt.Errorf("addon path contains invalid characters")
	}
	if !filepath.IsAbs(trimmed) {
		return "", fmt.Errorf("addon path must be absolute")
	}
	return filepath.Clean(trimmed), nil
}

func normalizeTheme(theme string) string {
	switch theme {
	case "scribe", "neutral", "dark":
		return theme
	default:
		return defaults().Theme
	}
}
