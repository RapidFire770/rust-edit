// Config loading/saving (toml + XDG). Sprint 1.
// All user-configurable values (AGENTS.md).

#![allow(dead_code)]

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Maximum number of entries kept in the recent-files list (MRU order).
pub const RECENT_FILES_MAX: usize = 10;

/// Maximum number of project folders kept in the recent-projects list.
pub const RECENT_PROJECTS_MAX: usize = 10;

/// Missing keys in `config.toml` fall back to [`Config::default`] per field,
/// so an older or partial file keeps every setting it does contain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub tab_size: u8,
    pub theme: String,
    pub show_line_numbers: bool,
    pub word_wrap: bool,

    /// Recently opened/saved file paths, most-recent-first. Capped at
    /// [`RECENT_FILES_MAX`] by [`Config::record_recent_file`]. Stored as
    /// strings (not `PathBuf`) so the TOML stays human-readable and survives
    /// serde round-trips cleanly across platforms.
    pub recent_files: Vec<String>,

    /// Recently opened project folders, most-recent-first. Capped at
    /// [`RECENT_PROJECTS_MAX`] by [`Config::record_recent_project`].
    /// Directories only. A missing key in an older `config.toml` stays empty.
    pub recent_projects: Vec<String>,

    /// When true, clean buffers reload from disk without prompting.
    /// Dirty buffers still prompt. Default false (ask per file).
    pub auto_reload_all: bool,

    /// Underline misspellings and offer dictionary suggestions. Default off.
    pub spellcheck: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_size: 4,
            theme: "dark-red".to_string(),
            show_line_numbers: true,
            word_wrap: false,

            recent_files: Vec::new(),
            recent_projects: Vec::new(),
            auto_reload_all: false,
            spellcheck: false,
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

    /// Set tab size (clamped to 1–8) and persist. Mirrors `set_theme`.
    pub fn set_tab_size(&mut self, n: u8) {
        self.tab_size = n.clamp(1, 8);
        self.save();
    }

    /// Toggle line-number visibility and persist.
    pub fn set_show_line_numbers(&mut self, on: bool) {
        self.show_line_numbers = on;
        self.save();
    }

    /// Toggle auto-reload of all clean files and persist (except in tests).
    pub fn set_auto_reload_all(&mut self, on: bool) {
        self.auto_reload_all = on;
        #[cfg(not(test))]
        self.save();
    }

    /// Toggle spellcheck and persist (except in tests).
    pub fn set_spellcheck(&mut self, on: bool) {
        self.spellcheck = on;
        #[cfg(not(test))]
        self.save();
    }

    /// Record a file as just-opened/saved: move it to the front of
    /// `recent_files`, dedupe by canonicalized path, and cap the list at
    /// [`RECENT_FILES_MAX`]. Persists immediately. No-op if the path doesn't
    /// point at an existing regular file (so directories and misses are not
    /// recorded).
    pub fn record_recent_file(&mut self, path: &Path) {
        if self.record_recent_file_inmemory(path) {
            // App unit tests load the user's real XDG config via App::new.
            // Skip persist so opening temp files cannot rewrite config.toml.
            #[cfg(not(test))]
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

    /// Remove `path` from `recent_files` if present. Persists when the list
    /// changed. Returns whether anything was removed.
    pub fn remove_recent_file(&mut self, path: &Path) -> bool {
        let changed = self.remove_recent_file_inmemory(path);
        #[cfg(not(test))]
        if changed {
            self.save();
        }
        changed
    }

    fn remove_recent_file_inmemory(&mut self, path: &Path) -> bool {
        let key = path.to_string_lossy().to_string();
        let canonical = fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| key.clone());
        let before = self.recent_files.len();
        self.recent_files.retain(|s| s != &key && s != &canonical);
        before != self.recent_files.len()
    }

    /// Empty `recent_files`. Persists when the list was non-empty.
    pub fn clear_recent_files(&mut self) -> bool {
        let changed = self.clear_recent_files_inmemory();
        #[cfg(not(test))]
        if changed {
            self.save();
        }
        changed
    }

    fn clear_recent_files_inmemory(&mut self) -> bool {
        if self.recent_files.is_empty() {
            return false;
        }
        self.recent_files.clear();
        true
    }

    /// Drop recents that are gone or are no longer regular files. Persists
    /// when the list changed. Other IO errors (e.g. permission denied) keep
    /// the entry so a temporarily unreadable path is not forgotten.
    pub fn prune_missing_recent_files(&mut self) -> bool {
        let changed = self.prune_missing_recent_files_inmemory();
        #[cfg(not(test))]
        if changed {
            self.save();
        }
        changed
    }

    fn prune_missing_recent_files_inmemory(&mut self) -> bool {
        let before = self.recent_files.len();
        self.recent_files
            .retain(|s| recent_path_is_regular_file(Path::new(s)));
        before != self.recent_files.len()
    }

    /// Record a project folder: move it to the front of `recent_projects`,
    /// dedupe by canonical path, and cap the list. Persists immediately
    /// except under `cfg(test)`. No-op unless `path` is an existing directory.
    pub fn record_recent_project(&mut self, path: &Path) {
        if self.record_recent_project_inmemory(path) {
            #[cfg(not(test))]
            self.save();
        }
    }

    fn record_recent_project_inmemory(&mut self, path: &Path) -> bool {
        if !path.is_dir() {
            return false;
        }
        let canonical = match fs::canonicalize(path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => path.to_string_lossy().to_string(),
        };
        if self.recent_projects.first() == Some(&canonical) {
            return false;
        }
        self.recent_projects.retain(|s| s != &canonical);
        self.recent_projects.insert(0, canonical);
        if self.recent_projects.len() > RECENT_PROJECTS_MAX {
            self.recent_projects.truncate(RECENT_PROJECTS_MAX);
        }
        true
    }

    /// Remove `path` from `recent_projects` if present. Persists when the
    /// list changed, except under `cfg(test)`.
    pub fn remove_recent_project(&mut self, path: &Path) -> bool {
        let changed = self.remove_recent_project_inmemory(path);
        #[cfg(not(test))]
        if changed {
            self.save();
        }
        changed
    }

    fn remove_recent_project_inmemory(&mut self, path: &Path) -> bool {
        let key = path.to_string_lossy().to_string();
        let canonical = fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| key.clone());
        let before = self.recent_projects.len();
        self.recent_projects
            .retain(|s| s != &key && s != &canonical);
        before != self.recent_projects.len()
    }

    /// Empty `recent_projects`. Persists when the list was non-empty,
    /// except under `cfg(test)`.
    pub fn clear_recent_projects(&mut self) -> bool {
        let changed = self.clear_recent_projects_inmemory();
        #[cfg(not(test))]
        if changed {
            self.save();
        }
        changed
    }

    fn clear_recent_projects_inmemory(&mut self) -> bool {
        if self.recent_projects.is_empty() {
            return false;
        }
        self.recent_projects.clear();
        true
    }

    /// Drop project recents that are gone or are no longer directories.
    /// A permission error keeps the entry. Persists when the list changed,
    /// except under `cfg(test)`.
    pub fn prune_missing_recent_projects(&mut self) -> bool {
        let changed = self.prune_missing_recent_projects_inmemory();
        #[cfg(not(test))]
        if changed {
            self.save();
        }
        changed
    }

    fn prune_missing_recent_projects_inmemory(&mut self) -> bool {
        let before = self.recent_projects.len();
        self.recent_projects
            .retain(|s| recent_path_is_directory(Path::new(s)));
        before != self.recent_projects.len()
    }
}

/// True when `path` is a regular file, or when metadata fails for a reason
/// other than NotFound (keep unreadable-but-possibly-still-there paths).
fn recent_path_is_regular_file(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(m) => m.is_file(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => true,
    }
}

/// True when `path` is a directory (symlinks followed), or when metadata
/// fails for a reason other than NotFound.
fn recent_path_is_directory(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(m) => m.is_dir(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => true,
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
    fn auto_reload_all_defaults_false_and_roundtrips() {
        let cfg = Config::default();
        assert!(!cfg.auto_reload_all);
        let mut cfg = cfg;
        cfg.auto_reload_all = true;
        let s = toml::to_string(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert!(back.auto_reload_all);
    }

    #[test]
    fn missing_auto_reload_all_in_toml_defaults_false() {
        let cfg: Config = toml::from_str(
            "tab_size = 4\ntheme = \"dark-red\"\nshow_line_numbers = true\nword_wrap = false\n",
        )
        .unwrap();
        assert!(!cfg.auto_reload_all);
        assert!(!cfg.spellcheck);
    }

    #[test]
    fn spellcheck_defaults_false_and_roundtrips() {
        let cfg = Config::default();
        assert!(!cfg.spellcheck);
        let mut cfg = cfg;
        cfg.spellcheck = true;
        let saved = toml::to_string(&cfg).unwrap();
        let back: Config = toml::from_str(&saved).unwrap();
        assert!(back.spellcheck);
    }

    #[test]
    fn partial_toml_keeps_present_fields_and_defaults_the_rest() {
        let cfg: Config =
            toml::from_str("theme = \"light\"\nrecent_files = [\"/tmp/a.rs\"]\n").unwrap();
        let def = Config::default();
        assert_eq!(cfg.theme, "light");
        assert_eq!(cfg.recent_files, vec!["/tmp/a.rs".to_string()]);
        assert_eq!(cfg.tab_size, def.tab_size);
        assert_eq!(cfg.show_line_numbers, def.show_line_numbers);
        assert_eq!(cfg.word_wrap, def.word_wrap);
        assert_eq!(cfg.auto_reload_all, def.auto_reload_all);
        assert_eq!(cfg.spellcheck, def.spellcheck);
    }

    #[test]
    fn record_recent_file_ignores_nonexistent_and_dirs() {
        let mut cfg = Config::default();
        assert!(!cfg.record_recent_file_inmemory(Path::new("/this/does/not/exist/anywhere/x")));
        assert!(!cfg.record_recent_file_inmemory(&std::env::temp_dir())); // a directory
        assert!(cfg.recent_files.is_empty());
    }

    #[test]
    fn remove_recent_file_drops_matching_path_and_keeps_others() {
        let mut cfg = Config::default();
        let a = tmpfile("rm-a");
        let b = tmpfile("rm-b");
        assert!(cfg.record_recent_file_inmemory(&a));
        assert!(cfg.record_recent_file_inmemory(&b));
        assert!(cfg.remove_recent_file_inmemory(&b));
        assert_eq!(cfg.recent_files.len(), 1);
        assert!(cfg.recent_files[0].contains("rust-edit-cfg-rm-a"));
        assert!(!cfg.remove_recent_file_inmemory(&b));
    }

    #[test]
    fn remove_recent_file_matches_stored_string_even_if_file_is_gone() {
        let mut cfg = Config::default();
        let gone = "/tmp/rust-edit-recent-gone-does-not-exist.rs".to_string();
        cfg.recent_files.push(gone.clone());
        assert!(cfg.remove_recent_file_inmemory(Path::new(&gone)));
        assert!(cfg.recent_files.is_empty());
    }

    #[test]
    fn clear_recent_files_empties_list() {
        let mut cfg = Config::default();
        let a = tmpfile("clr-a");
        assert!(cfg.record_recent_file_inmemory(&a));
        assert!(cfg.clear_recent_files_inmemory());
        assert!(cfg.recent_files.is_empty());
        assert!(!cfg.clear_recent_files_inmemory());
    }

    #[test]
    fn record_recent_project_moves_an_existing_directory_to_front() {
        let dir = std::env::temp_dir().join(format!("rust-edit-proj-{}", std::process::id()));
        let a = dir.join("a");
        let b = dir.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let mut cfg = Config::default();
        assert!(cfg.record_recent_project_inmemory(&a));
        assert!(cfg.record_recent_project_inmemory(&b));
        assert!(cfg.recent_projects[0].contains("b"));
        assert!(cfg.recent_projects[1].contains("a"));
        assert!(!cfg.record_recent_project_inmemory(&b));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn record_recent_project_rejects_a_file_and_caps_at_ten() {
        let dir = std::env::temp_dir().join(format!("rust-edit-proj-cap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("note.txt");
        std::fs::write(&file, "x").unwrap();
        let mut cfg = Config::default();
        assert!(!cfg.record_recent_project_inmemory(&file));
        for i in 0..RECENT_PROJECTS_MAX + 2 {
            let sub = dir.join(format!("p{i}"));
            std::fs::create_dir_all(&sub).unwrap();
            cfg.record_recent_project_inmemory(&sub);
        }
        assert_eq!(cfg.recent_projects.len(), RECENT_PROJECTS_MAX);
        assert!(cfg.recent_projects[0].contains(&format!("p{}", RECENT_PROJECTS_MAX + 1)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_recent_projects_drops_missing_and_files_only() {
        let mut cfg = Config::default();
        cfg.recent_projects
            .push("/tmp/rust-edit-proj-missing".into());
        cfg.recent_projects.push("/etc/hosts".into());
        let kept = std::env::temp_dir();
        cfg.recent_projects.push(kept.display().to_string());
        assert!(cfg.prune_missing_recent_projects_inmemory());
        assert_eq!(cfg.recent_projects.len(), 1);
        assert!(cfg.recent_projects[0].contains("tmp"));
    }

    #[test]
    fn old_config_without_recent_projects_still_loads() {
        let cfg: Config =
            toml::from_str("theme = \"light\"\nrecent_files = [\"/tmp/a.rs\"]\n").unwrap();
        assert!(cfg.recent_projects.is_empty());
        assert_eq!(cfg.recent_files, vec!["/tmp/a.rs".to_string()]);
    }

    #[test]
    fn prune_missing_recent_files_drops_gone_paths_and_dirs() {
        let mut cfg = Config::default();
        let keep = tmpfile("prune-keep");
        let gone_later = tmpfile("prune-gone");
        assert!(cfg.record_recent_file_inmemory(&keep));
        assert!(cfg.record_recent_file_inmemory(&gone_later));
        fs::remove_file(&gone_later).unwrap();
        cfg.recent_files
            .push(std::env::temp_dir().to_string_lossy().into_owned());
        cfg.recent_files
            .push("/tmp/rust-edit-recent-never-existed.rs".into());
        assert!(cfg.prune_missing_recent_files_inmemory());
        assert_eq!(cfg.recent_files.len(), 1);
        assert!(cfg.recent_files[0].contains("rust-edit-cfg-prune-keep"));
        assert!(!cfg.prune_missing_recent_files_inmemory());
    }
}
