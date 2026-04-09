package settings

import (
	"fmt"
	"strconv"

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
		s.AddonPath = v
	}
	if v, ok := kv[keyAutoUpdate]; ok {
		s.AutoUpdate = v == "true"
	}
	if v, ok := kv[keyMemoryLimitMB]; ok {
		if parsed, err := strconv.Atoi(v); err == nil && parsed >= 0 {
			s.MemoryLimitMB = parsed
		}
	}
	if v, ok := kv[keyTheme]; ok {
		switch v {
		case "scribe", "neutral", "dark":
			s.Theme = v
		}
	}
	return s, nil
}

func (m *Manager) SaveSettings(s AppSettings) error {
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
