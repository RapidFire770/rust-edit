// Config loading/saving (toml + XDG). Sprint 1.
// All user-configurable values (AGENTS.md).

#![allow(dead_code)]

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub tab_size: u8,
    pub theme: String,
    pub show_line_numbers: bool,
    pub word_wrap: bool,
    pub mouse_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_size: 4,
            theme: "dark-red".to_string(),
            show_line_numbers: true,
            word_wrap: false,
            mouse_enabled: true,
        }
    }
}

impl Config {
    pub fn config_dir() -> Option<PathBuf> {
        ProjectDirs::from("rs", "", "rust-edit").map(|p| p.config_dir().to_path_buf())
    }

    pub fn config_file() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join("config.toml"))
    }

    /// Load from disk or return default. Never fails hard — always returns usable config.
    pub fn load() -> Self {
        if let Some(path) = Self::config_file() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<Config>(&content) {
                    return cfg;
                }
            }
        }
        Self::default()
    }

    /// Save to disk. Creates directories if needed. Errors are swallowed for MVP (user will see on next open).
    pub fn save(&self) {
        if let Some(dir) = Self::config_dir() {
            if fs::create_dir_all(&dir).is_err() {
                return;
            }
            if let Some(file) = Self::config_file() {
                if let Ok(toml_str) = toml::to_string_pretty(self) {
                    let _ = fs::write(file, toml_str);
                }
            }
        }
    }

    /// Update theme and persist (for live settings changes).
    pub fn set_theme(&mut self, name: &str) {
        self.theme = name.to_string();
        self.save();
    }
}
