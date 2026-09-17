use serde::{Deserialize, Serialize};

use crate::paths::{DEFAULT_SYNC_SECS, MIN_SYNC_SECS};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default)]
    pub watch: WatchConfig,
    #[serde(default)]
    pub keys: KeyConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    #[serde(default = "icloud")]
    pub provider: String,
    #[serde(default)]
    pub apple_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatchConfig {
    #[serde(default = "on")]
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyConfig {
    #[serde(default = "k_open")]
    pub open_panel: String,
    #[serde(default = "k_search")]
    pub search: String,
    #[serde(default = "k_esc")]
    pub escape: String,
    #[serde(default = "k_up")]
    pub up: String,
    #[serde(default = "k_down")]
    pub down: String,
    #[serde(default = "k_up_alt")]
    pub up_alt: String,
    #[serde(default = "k_down_alt")]
    pub down_alt: String,
    #[serde(default = "k_enter")]
    pub open: String,
    #[serde(default = "k_new")]
    pub new: String,
    #[serde(default = "k_delete")]
    pub delete: String,
    #[serde(default = "k_edit")]
    pub edit: String,
    #[serde(default = "k_sync")]
    pub sync: String,
    #[serde(default = "k_import")]
    pub import: String,
    #[serde(default = "k_export")]
    pub export: String,
    #[serde(default = "k_save")]
    pub save: String,
    #[serde(default = "k_help")]
    pub help: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sync: SyncConfig::default(),
            watch: WatchConfig::default(),
            keys: KeyConfig::default(),
        }
    }
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_secs: DEFAULT_SYNC_SECS,
            provider: "icloud".into(),
            apple_id: String::new(),
        }
    }
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            open_panel: k_open(),
            search: k_search(),
            escape: k_esc(),
            up: k_up(),
            down: k_down(),
            up_alt: k_up_alt(),
            down_alt: k_down_alt(),
            open: k_enter(),
            new: k_new(),
            delete: k_delete(),
            edit: k_edit(),
            sync: k_sync(),
            import: k_import(),
            export: k_export(),
            save: k_save(),
            help: k_help(),
        }
    }
}

impl Config {
    pub fn clamp(&mut self) {
        if self.sync.interval_secs < MIN_SYNC_SECS {
            self.sync.interval_secs = MIN_SYNC_SECS;
        }
        if self.sync.provider != "icloud" {
            self.sync.provider = "icloud".into();
        }
        if self.sync.apple_id.contains('\n') || self.sync.apple_id.contains('\r') {
            self.sync.apple_id.clear();
        }
        self.sync.apple_id = self.sync.apple_id.trim().to_string();
    }

    pub fn from_toml(text: &str) -> Self {
        let mut cfg: Config = toml::from_str(text).unwrap_or_default();
        cfg.clamp();
        cfg
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }
}

fn default_interval() -> u64 {
    DEFAULT_SYNC_SECS
}
fn icloud() -> String {
    "icloud".into()
}
fn on() -> bool {
    true
}
fn k_open() -> String {
    "Super+C".into()
}
fn k_search() -> String {
    "/".into()
}
fn k_esc() -> String {
    "Esc".into()
}
fn k_up() -> String {
    "Up".into()
}
fn k_down() -> String {
    "Down".into()
}
fn k_up_alt() -> String {
    "k".into()
}
fn k_down_alt() -> String {
    "j".into()
}
fn k_enter() -> String {
    "Enter".into()
}
fn k_new() -> String {
    "n".into()
}
fn k_delete() -> String {
    "d".into()
}
fn k_edit() -> String {
    "e".into()
}
fn k_sync() -> String {
    "s".into()
}
fn k_import() -> String {
    "Ctrl+I".into()
}
fn k_export() -> String {
    "Ctrl+E".into()
}
fn k_save() -> String {
    "Ctrl+S".into()
}
fn k_help() -> String {
    "?".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_floor() {
        let mut c = Config::default();
        c.sync.interval_secs = 30;
        c.clamp();
        assert_eq!(c.sync.interval_secs, MIN_SYNC_SECS);
    }
}
