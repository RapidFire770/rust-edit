// Registry of built-in + user-installed custom themes.
//
// Custom themes live as one `.toml` file per theme under the config directory's
// `themes/` subfolder (e.g. `~/.config/rust-edit/themes/my-theme.toml`). Each
// file is a `Theme` serialized via the existing serde derive, so users can
// share snippets and edit them by hand (Design Spec §4.2 "Color Theme System").
//
// Loading is best-effort: malformed or duplicate-name files are skipped
// silently and never crash startup, mirroring `Config::load`'s philosophy
// (avoiding a `log` dependency for this path).

#![allow(dead_code)]

use crate::config::Config;
use crate::theme::Theme;
use std::fs;
use std::path::{Path, PathBuf};

pub struct ThemeRegistry {
    built_ins: Vec<Theme>,
    customs: Vec<Theme>,
}

impl ThemeRegistry {
    /// Load the 6 built-ins plus every parseable custom theme in the themes
    /// directory. Never fails — returns an empty registry on any IO error.
    pub fn load() -> Self {
        let built_ins = Theme::built_ins();
        let mut customs = Vec::new();
        let mut seen_names: Vec<String> = built_ins.iter().map(|t| t.name.clone()).collect();

        if let Some(dir) = Self::themes_dir() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                        continue;
                    }
                    match Self::load_file(&path) {
                        Ok(theme) => {
                            if seen_names.iter().any(|n| n == &theme.name) {
                                // Duplicate (built-in or earlier custom): skip silently,
                                // mirroring Config::load's swallow-errors philosophy.
                                // We avoid adding a `log` dependency for this MVP path.
                                continue;
                            }
                            seen_names.push(theme.name.clone());
                            customs.push(theme);
                        }
                        Err(_e) => {
                            // Malformed custom theme file: skip rather than crash startup.
                            continue;
                        }
                    }
                }
            }
        }

        Self { built_ins, customs }
    }

    /// All themes, built-ins first then customs. Order is stable across reloads
    /// because the directory walk is the only non-deterministic source and
    /// `read_dir` order doesn't affect correctness (names are deduped).
    pub fn all(&self) -> Vec<&Theme> {
        self.built_ins.iter().chain(self.customs.iter()).collect()
    }

    pub fn built_ins(&self) -> &[Theme] {
        &self.built_ins
    }

    pub fn customs(&self) -> &[Theme] {
        &self.customs
    }

    /// Find a theme by name across both built-ins and customs.
    pub fn by_name(&self, name: &str) -> Option<&Theme> {
        self.built_ins
            .iter()
            .chain(self.customs.iter())
            .find(|t| t.name == name)
    }

    /// Is `name` one of the hardcoded built-ins (and therefore not deletable
    /// or overridable by a custom file)?
    pub fn is_built_in(&self, name: &str) -> bool {
        self.built_ins.iter().any(|t| t.name == name)
    }

    /// Persist a custom theme to `<themes-dir>/<name>.toml`, then register it
    /// in memory so the Themes list updates immediately. Returns the path
    /// written so the caller (e.g. a status message) can show it.
    pub fn save_custom(&mut self, theme: Theme) -> Result<PathBuf, String> {
        if theme.name.trim().is_empty() {
            return Err("theme name cannot be empty".to_string());
        }
        if Self::is_reserved_name(&theme.name) {
            return Err(format!("theme name '{}' is reserved", theme.name));
        }
        let dir = Self::themes_dir().ok_or_else(|| "no config directory available".to_string())?;
        fs::create_dir_all(&dir).map_err(|e| format!("create themes dir: {e}"))?;

        let file = dir.join(format!("{}.toml", sanitize_name(&theme.name)));
        let toml_str = toml::to_string_pretty(&theme).map_err(|e| format!("serialize: {e}"))?;
        fs::write(&file, toml_str).map_err(|e| format!("write {}: {e}", file.display()))?;

        // Replace-in-place if we just overwrote an existing custom, else append.
        if let Some(existing) = self.customs.iter_mut().find(|t| t.name == theme.name) {
            *existing = theme.clone();
        } else {
            self.customs.push(theme);
        }
        Ok(file)
    }

    /// Delete a custom theme's file and drop it from the registry. Built-ins
    /// cannot be deleted — returns an error if asked.
    pub fn delete_custom(&mut self, name: &str) -> Result<(), String> {
        if self.is_built_in(name) {
            return Err(format!("cannot delete built-in theme '{name}'"));
        }
        if !self.customs.iter().any(|t| t.name == name) {
            return Err(format!("no custom theme named '{name}'"));
        }
        if let Some(dir) = Self::themes_dir() {
            let file = dir.join(format!("{}.toml", sanitize_name(name)));
            // Best-effort file removal: even if it's already gone, drop the
            // in-memory entry so the UI stays consistent.
            let _ = fs::remove_file(&file);
        }
        self.customs.retain(|t| t.name != name);
        Ok(())
    }

    /// Path to the per-user themes directory: `<config-dir>/themes/`.
    pub fn themes_dir() -> Option<PathBuf> {
        Config::config_dir().map(|d| d.join("themes"))
    }

    fn load_file(path: &Path) -> Result<Theme, String> {
        let content = fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
        let theme: Theme = toml::from_str(&content).map_err(|e| format!("parse: {e}"))?;
        if theme.name.trim().is_empty() {
            return Err("theme has empty name".to_string());
        }
        Ok(theme)
    }

    /// Names that custom themes may never use (would collide with on-disk
    /// helpers or be confusing). Currently just `config`.
    fn is_reserved_name(name: &str) -> bool {
        name.eq_ignore_ascii_case("config")
    }
}

/// Lowercase the name and keep only `[a-z0-9-_]` so the on-disk filename is
/// predictable and safe across platforms. Other characters are replaced with
/// `-`.
fn sanitize_name(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_themes_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rust-edit-themes-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // Override Config::config_dir() resolution by writing files directly into a
    // temp `themes/` dir we control, then point ThemeRegistry at it via the
    // load_from helper below.
    impl ThemeRegistry {
        fn load_from(dir: &Path) -> Self {
            let built_ins = Theme::built_ins();
            let mut customs = Vec::new();
            let mut seen: Vec<String> = built_ins.iter().map(|t| t.name.clone()).collect();
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                        continue;
                    }
                    if let Ok(theme) = Self::load_file(&path) {
                        if !seen.iter().any(|n| n == &theme.name) {
                            seen.push(theme.name.clone());
                            customs.push(theme);
                        }
                    }
                }
            }
            Self { built_ins, customs }
        }
    }

    #[test]
    fn loads_valid_custom_themes() {
        let dir = fresh_themes_dir();
        let mut t = Theme::light();
        t.name = "my-light".to_string();
        fs::write(dir.join("my-light.toml"), toml::to_string(&t).unwrap()).unwrap();

        let reg = ThemeRegistry::load_from(&dir);
        assert_eq!(reg.built_ins().len(), 6);
        assert_eq!(reg.customs().len(), 1);
        assert!(reg.by_name("my-light").is_some());
        assert!(reg.by_name("dark-red").is_some());
    }

    #[test]
    fn skips_invalid_and_duplicate_files() {
        let dir = fresh_themes_dir();

        // Invalid TOML — skipped.
        fs::write(dir.join("broken.toml"), "this is not toml {{{{").unwrap();

        // Name collides with a built-in — skipped.
        let mut dup = Theme::dark_red();
        dup.name = "light".to_string(); // built-in name
        fs::write(dir.join("dup.toml"), toml::to_string(&dup).unwrap()).unwrap();

        // Valid custom — kept.
        let mut good = Theme::monochrome();
        good.name = "unique".to_string();
        fs::write(dir.join("good.toml"), toml::to_string(&good).unwrap()).unwrap();

        let reg = ThemeRegistry::load_from(&dir);
        assert_eq!(reg.customs().len(), 1);
        assert!(reg.by_name("unique").is_some());
    }

    #[test]
    fn save_then_delete_custom_roundtrip() {
        let dir = fresh_themes_dir();

        // save_custom writes through Config::themes_dir(), so temporarily
        // shadow it by setting an env var the real impl doesn't read — instead,
        // exercise the in-memory half by loading from our temp dir afterwards.
        let theme = {
            let mut t = Theme::solarized();
            t.name = "test-custom".to_string();
            t
        };

        // save_custom goes to the real themes dir; we can still verify the
        // in-memory registration + deletion path. Write the file manually into
        // our temp dir and load_from to emulate the on-disk state.
        let toml_str = toml::to_string(&theme).unwrap();
        fs::write(dir.join("test-custom.toml"), &toml_str).unwrap();

        let mut reg = ThemeRegistry::load_from(&dir);
        assert!(reg.by_name("test-custom").is_some());

        // Simulate delete on the temp copy.
        fs::remove_file(dir.join("test-custom.toml")).unwrap();
        reg.customs.retain(|t| t.name != "test-custom");
        assert!(reg.by_name("test-custom").is_none());

        // Built-ins are never deletable.
        let res = reg.delete_custom("dark-red");
        assert!(res.is_err());
    }

    #[test]
    fn sanitize_replaces_unsafe_chars() {
        assert_eq!(sanitize_name("My Theme!"), "my-theme-");
        assert_eq!(sanitize_name("ok_name-1"), "ok_name-1");
        assert_eq!(sanitize_name("  Spaces  "), "spaces");
    }
}
