// Config loading/saving (toml + XDG). Sprint 1.
// All user-configurable values (AGENTS.md).

#![allow(dead_code)]

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Maximum number of entries kept in the recent-files list (MRU order).
pub const RECENT_FILES_MAX: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub tab_size: u8,
    pub theme: String,
    pub show_line_numbers: bool,
    pub word_wrap: bool,
    pub mouse_enabled: bool,
    /// Recently opened/saved file paths, most-recent-first. Capped at
    /// [`RECENT_FILES_MAX`] by [`Config::record_recent_file`]. Stored as
    /// strings (not `PathBuf`) so the TOML stays human-readable and survives
    /// serde round-trips cleanly across platforms.
    #[serde(default)]
    pub recent_files: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_size: 4,
            theme: "dark-red".to_string(),
            show_line_numbers: true,
            word_wrap: false,
            mouse_enabled: true,
            recent_files: Vec::new(),
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

    /// Toggle word wrap and persist (live setting). Mirrors `set_theme`.
    pub fn set_word_wrap(&mut self, on: bool) {
        self.word_wrap = on;
        self.save();
    }

    /// Record a file as just-opened/saved: move it to the front of
    /// `recent_files`, dedupe by canonicalized path, and cap the list at
    /// [`RECENT_FILES_MAX`]. Persists immediately. No-op if the path doesn't
    /// point at an existing regular file (so directories and misses are not
    /// recorded).
    pub fn record_recent_file(&mut self, path: &Path) {
        if self.record_recent_file_inmemory(path) {
            self.save();
        }
    }

    /// In-memory half of [`record_recent_file`](Self::record_recent_file).
    /// Returns `true` if `recent_files` actually changed (so the caller can
    /// decide whether to persist). Pure — does not touch disk. Tested in
    /// isolation so the tests don't clobber the user's real config file.
    fn record_recent_file_inmemory(&mut self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }
        let canonical = match fs::canonicalize(path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => path.to_string_lossy().to_string(),
        };
        if self.recent_files.iter().any(|s| s == &canonical)
            && self.recent_files.first() == Some(&canonical)
        {
            return false; // already at front, no change
        }
        self.recent_files.retain(|s| s != &canonical);
        self.recent_files.insert(0, canonical);
        if self.recent_files.len() > RECENT_FILES_MAX {
            self.recent_files.truncate(RECENT_FILES_MAX);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpfile(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "rust-edit-cfg-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&p, "").unwrap();
        p
    }

    #[test]
    fn record_recent_file_moves_to_front_and_dedupes() {
        let mut cfg = Config::default();
        let a = tmpfile("a");
        let b = tmpfile("b");
        assert!(cfg.record_recent_file_inmemory(&a));
        assert!(cfg.record_recent_file_inmemory(&b));
        // Most-recent first.
        assert_eq!(cfg.recent_files.len(), 2);
        assert!(cfg.recent_files[0].contains("rust-edit-cfg-b"));
        assert!(cfg.recent_files[1].contains("rust-edit-cfg-a"));

        // Re-recording `a` dedupes and moves to front.
        assert!(cfg.record_recent_file_inmemory(&a));
        assert_eq!(cfg.recent_files.len(), 2);
        assert!(cfg.recent_files[0].contains("rust-edit-cfg-a"));
    }

    #[test]
    fn record_recent_file_caps_at_max() {
        let mut cfg = Config::default();
        let created: Vec<PathBuf> = (0..RECENT_FILES_MAX + 3)
            .map(|i| tmpfile(&format!("cap{}", i)))
            .collect();
        for p in &created {
            cfg.record_recent_file_inmemory(p);
        }
        assert_eq!(cfg.recent_files.len(), RECENT_FILES_MAX);
        // The last one recorded should be at the front; the three oldest dropped.
        assert!(cfg.recent_files[0].contains(&format!("cap{}", RECENT_FILES_MAX + 2)));
    }

    #[test]
    fn record_recent_file_ignores_nonexistent_and_dirs() {
        let mut cfg = Config::default();
        assert!(!cfg.record_recent_file_inmemory(Path::new("/this/does/not/exist/anywhere/x")));
        assert!(!cfg.record_recent_file_inmemory(&std::env::temp_dir())); // a directory
        assert!(cfg.recent_files.is_empty());
    }
}
