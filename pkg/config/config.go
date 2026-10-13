package config

import (
	"encoding/json"
	"fmt"
	"os"
)

type Config struct {
	Accounts []AccountConfig `json:"accounts"`
	Settings SettingsConfig  `json:"settings"`
}

type AccountConfig struct {
	Username      string   `json:"username"`
	AuthToken     string   `json:"authToken"` // OAuth token (do cookie auth-token)
	Streamers     []string `json:"streamers"`
	MaxConcurrent int      `json:"maxConcurrent"`
}

type SettingsConfig struct {
	CheckIntervalSeconds int  `json:"checkIntervalSeconds"`
	AutoClaimBonus       bool `json:"autoClaimBonus"`
	AutoJoinRaid         bool `json:"autoJoinRaid"`
	FollowRaids          bool `json:"followRaids"`
	WebDashboardPort     int  `json:"webDashboardPort"`
}

func LoadConfig(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("erro lendo %s: %w", path, err)
	}

	var cfg Config
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("erro parseando config.json: %w", err)
	}

	if cfg.Settings.CheckIntervalSeconds <= 0 {
		cfg.Settings.CheckIntervalSeconds = 60
	}
	if cfg.Settings.WebDashboardPort <= 0 {
		cfg.Settings.WebDashboardPort = 3010
	}

	return &cfg, nil
}
