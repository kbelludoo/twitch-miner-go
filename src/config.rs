use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub accounts: Vec<AccountConfig>,
    pub settings: SettingsConfig,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountConfig {
    pub username: String,
    pub auth_token: String,
    pub streamers: Vec<String>,
    pub max_concurrent: usize,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SettingsConfig {
    #[serde(default = "default_interval")]
    pub check_interval_seconds: u64,
    #[serde(default)]
    pub auto_claim_bonus: bool,
    #[serde(default)]
    pub auto_join_raid: bool,
    #[serde(default)]
    pub follow_raids: bool,
    #[serde(default = "default_port")]
    pub web_dashboard_port: u16,
}

fn default_interval() -> u64 {
    60
}

fn default_port() -> u16 {
    3010
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            check_interval_seconds: 60,
            auto_claim_bonus: false,
            auto_join_raid: false,
            follow_raids: false,
            web_dashboard_port: 3010,
        }
    }
}

pub fn load_config(path: impl AsRef<Path>) -> Result<Config> {
    let data = std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("reading {}", path.as_ref().display()))?;
    let mut cfg: Config =
        serde_json::from_str(&data).context("parsing config.json")?;

    if cfg.settings.check_interval_seconds == 0 {
        cfg.settings.check_interval_seconds = 60;
    }
    if cfg.settings.web_dashboard_port == 0 {
        cfg.settings.web_dashboard_port = 3010;
    }

    Ok(cfg)
}
