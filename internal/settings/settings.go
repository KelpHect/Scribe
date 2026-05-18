package settings

import (
	"fmt"
	"path/filepath"
	"strconv"
	"strings"

	"Scribe/internal/esoui"

	"gorm.io/gorm"
	"gorm.io/gorm/clause"
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
	db *gorm.DB
}

func NewManager(db *gorm.DB) *Manager {
	return &Manager{db: db}
}

const (
	keyAddonPath     = "addon_path"
	keyAutoUpdate    = "auto_update"
	keyMemoryLimitMB = "memory_limit_mb"
	keyTheme         = "theme"
)

func (m *Manager) GetSettings() (AppSettings, error) {
	var rows []esoui.DBSetting
	if err := m.db.Select("key", "value").Find(&rows).Error; err != nil {
		return AppSettings{}, fmt.Errorf("load settings: %w", err)
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
	return s, nil
}

func (m *Manager) SaveSettings(s AppSettings) error {
	normalizedPath, err := normalizeAddonPath(s.AddonPath)
	if err != nil {
		return err
	}
	s.AddonPath = normalizedPath
	s.AutoUpdate = false
	if s.MemoryLimitMB < 0 {
		s.MemoryLimitMB = defaults().MemoryLimitMB
	}
	s.Theme = normalizeTheme(s.Theme)
	rows := []esoui.DBSetting{
		{Key: keyAddonPath, Value: s.AddonPath},
		{Key: keyAutoUpdate, Value: strconv.FormatBool(s.AutoUpdate)},
		{Key: keyMemoryLimitMB, Value: strconv.Itoa(s.MemoryLimitMB)},
		{Key: keyTheme, Value: s.Theme},
	}
	if err := m.db.Clauses(clause.OnConflict{
		Columns:   []clause.Column{{Name: "key"}},
		DoUpdates: clause.AssignmentColumns([]string{"value"}),
	}).Create(&rows).Error; err != nil {
		return fmt.Errorf("save settings: %w", err)
	}
	return nil
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
