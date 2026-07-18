// Theme struct + color roles + built-in themes + serde. Sprint 1.
// Default "dark-red" must match docs/ui-design-sample-2.jpg aesthetic (red header bar + dark editor).
// All UI colors MUST come from Theme (AGENTS.md rule).

#![allow(dead_code)]

use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    // Core UI roles
    pub header_bg: ColorDef,
    pub header_fg: ColorDef,
    pub editor_bg: ColorDef,
    pub editor_fg: ColorDef,
    pub status_bg: ColorDef,
    pub status_fg: ColorDef,
    pub line_number_fg: ColorDef,
    pub selection_bg: ColorDef,
    pub cursor_fg: ColorDef, // used for caret in some cases

    // Syntax / accent roles (approximate names)
    pub keyword: ColorDef,
    pub string: ColorDef,
    pub comment: ColorDef,
    pub function: ColorDef,
    pub number: ColorDef,
    pub type_name: ColorDef,
    pub accent: ColorDef, // for find highlights, active tab etc.
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ColorDef {
    /// RGB tuple
    Rgb(u8, u8, u8),
    /// Named for a few common ones (fallback)
    Named(String),
}

impl From<ColorDef> for Color {
    fn from(def: ColorDef) -> Self {
        match def {
            ColorDef::Rgb(r, g, b) => Color::Rgb(r, g, b),
            ColorDef::Named(name) => match name.to_ascii_lowercase().as_str() {
                "black" => Color::Black,
                "red" => Color::Red,
                "green" => Color::Green,
                "yellow" => Color::Yellow,
                "blue" => Color::Blue,
                "magenta" => Color::Magenta,
                "cyan" => Color::Cyan,
                "gray" | "grey" => Color::Gray,
                "darkgray" | "dark_gray" => Color::DarkGray,
                "lightred" | "light_red" => Color::LightRed,
                "lightgreen" | "light_green" => Color::LightGreen,
                "lightyellow" | "light_yellow" => Color::LightYellow,
                "lightblue" | "light_blue" => Color::LightBlue,
                "lightmagenta" | "light_magenta" => Color::LightMagenta,
                "lightcyan" | "light_cyan" => Color::LightCyan,
                "white" => Color::White,
                "darkred" | "dark_red" => Color::Rgb(139, 0, 0),
                _ => Color::White,
            },
        }
    }
}

impl From<&ColorDef> for Color {
    fn from(def: &ColorDef) -> Self {
        Color::from(def.clone())
    }
}

impl ColorDef {
    /// Parse a user-typed color string as accepted by the Themes editor.
    ///
    /// Accepts (case-insensitive):
    ///   * `#rgb` / `#rrggbb` hex
    ///   * `rgb(r,g,b)` with each channel 0–255
    ///   * any ratatui named color (with either spaces, underscores, or no
    ///     separator — e.g. `brightblue`, `bright_blue`, `bright blue`)
    ///
    /// Returns the canonical `ColorDef`: parsed RGB tuples become
    /// [`ColorDef::Rgb`]; recognized names become [`ColorDef::Named`] using the
    /// canonical underscore form (so round-tripping through TOML stays clean);
    /// unrecognized input is rejected with a message.
    pub fn parse_input(s: &str) -> Result<ColorDef, String> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err("empty color".to_string());
        }

        // Hex forms.
        if let Some(rest) = trimmed.strip_prefix('#') {
            let rest = rest.trim();
            if rest.len() == 3 && rest.chars().all(|c| c.is_ascii_hexdigit()) {
                let mut vals = [0u8; 3];
                for (i, b) in rest.bytes().enumerate() {
                    let d = (b as char).to_digit(16).unwrap() as u8;
                    vals[i] = d * 16 + d;
                }
                return Ok(ColorDef::Rgb(vals[0], vals[1], vals[2]));
            }
            if rest.len() == 6 && rest.chars().all(|c| c.is_ascii_hexdigit()) {
                let r = u8::from_str_radix(&rest[0..2], 16).map_err(|e| e.to_string())?;
                let g = u8::from_str_radix(&rest[2..4], 16).map_err(|e| e.to_string())?;
                let b = u8::from_str_radix(&rest[4..6], 16).map_err(|e| e.to_string())?;
                return Ok(ColorDef::Rgb(r, g, b));
            }
            return Err(format!("invalid hex color: '{trimmed}'"));
        }

        // rgb(r,g,b) form.
        let lower = trimmed.to_ascii_lowercase();
        if let Some(inner) = lower.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
            let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
            if parts.len() == 3 {
                let parse =
                    |p: &str| -> Result<u8, String> { p.parse::<u8>().map_err(|e| e.to_string()) };
                return Ok(ColorDef::Rgb(
                    parse(parts[0])?,
                    parse(parts[1])?,
                    parse(parts[2])?,
                ));
            }
            return Err(format!("invalid rgb() color: '{trimmed}'"));
        }

        // Named color. Try the most-readable spelling first (raw lowercased
        // input, then space→underscore, then fully stripped of separators) so
        // `light_blue`, `light blue`, and `LightBlue` all resolve but the
        // stored form preserves what the user typed where possible.
        let underscored = lower.replace(' ', "_");
        let stripped: String = lower
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        for candidate in [lower.as_str(), &underscored, stripped.as_str()] {
            if ColorDef::is_named(candidate) {
                return Ok(ColorDef::Named(candidate.to_string()));
            }
        }
        Err(format!("unknown color: '{trimmed}'"))
    }

    /// Is `name` (already lowercased + separator-normalized) a recognized
    /// ratatui named color? Mirrors the lookup table in [`From<ColorDef>`].
    /// Accepts both the no-separator (`brightblue`) and underscore
    /// (`bright_blue`) spellings — `parse_input` strips separators before
    /// calling this, so any of these spellings resolves here.
    fn is_named(name: &str) -> bool {
        matches!(
            name,
            "black"
                | "red"
                | "green"
                | "yellow"
                | "blue"
                | "magenta"
                | "cyan"
                | "gray"
                | "grey"
                | "darkgray"
                | "dark_gray"
                | "lightred"
                | "light_red"
                | "lightgreen"
                | "light_green"
                | "lightyellow"
                | "light_yellow"
                | "lightblue"
                | "light_blue"
                | "lightmagenta"
                | "light_magenta"
                | "lightcyan"
                | "light_cyan"
                | "white"
                | "darkred"
                | "dark_red"
        )
    }
}

impl Theme {
    pub fn dark_red() -> Self {
        // Matches the spirit of ui-design-sample-2.jpg: deep red header, near-black editor
        Self {
            name: "dark-red".to_string(),
            header_bg: ColorDef::Rgb(178, 34, 34), // firebrick-ish red
            header_fg: ColorDef::Rgb(255, 255, 255),
            editor_bg: ColorDef::Rgb(18, 18, 20),
            editor_fg: ColorDef::Rgb(220, 220, 220),
            status_bg: ColorDef::Rgb(35, 35, 38),
            status_fg: ColorDef::Rgb(180, 180, 180),
            line_number_fg: ColorDef::Rgb(120, 120, 125),
            selection_bg: ColorDef::Rgb(60, 80, 120),
            cursor_fg: ColorDef::Rgb(255, 200, 100),
            keyword: ColorDef::Rgb(255, 120, 120),
            string: ColorDef::Rgb(120, 200, 150),
            comment: ColorDef::Rgb(110, 140, 110),
            function: ColorDef::Rgb(200, 180, 255),
            number: ColorDef::Rgb(180, 210, 255),
            type_name: ColorDef::Rgb(130, 220, 255),
            accent: ColorDef::Rgb(255, 100, 80),
        }
    }

    pub fn light() -> Self {
        Self {
            name: "light".to_string(),
            header_bg: ColorDef::Rgb(180, 60, 60),
            header_fg: ColorDef::Rgb(255, 255, 255),
            editor_bg: ColorDef::Rgb(250, 250, 248),
            editor_fg: ColorDef::Rgb(30, 30, 30),
            status_bg: ColorDef::Rgb(235, 235, 233),
            status_fg: ColorDef::Rgb(80, 80, 80),
            line_number_fg: ColorDef::Rgb(150, 150, 150),
            selection_bg: ColorDef::Rgb(180, 200, 230),
            cursor_fg: ColorDef::Rgb(30, 30, 30),
            keyword: ColorDef::Rgb(180, 40, 40),
            string: ColorDef::Rgb(30, 120, 70),
            comment: ColorDef::Rgb(100, 120, 100),
            function: ColorDef::Rgb(80, 60, 160),
            number: ColorDef::Rgb(50, 80, 150),
            type_name: ColorDef::Rgb(30, 110, 140),
            accent: ColorDef::Rgb(200, 70, 50),
        }
    }

    pub fn high_contrast() -> Self {
        Self {
            name: "high-contrast".to_string(),
            header_bg: ColorDef::Rgb(200, 0, 0),
            header_fg: ColorDef::Rgb(255, 255, 0),
            editor_bg: ColorDef::Rgb(0, 0, 0),
            editor_fg: ColorDef::Rgb(255, 255, 255),
            status_bg: ColorDef::Rgb(20, 20, 20),
            status_fg: ColorDef::Rgb(255, 255, 0),
            line_number_fg: ColorDef::Rgb(180, 180, 180),
            selection_bg: ColorDef::Rgb(0, 120, 215),
            cursor_fg: ColorDef::Rgb(255, 255, 0),
            keyword: ColorDef::Rgb(255, 80, 80),
            string: ColorDef::Rgb(80, 220, 120),
            comment: ColorDef::Rgb(140, 140, 140),
            function: ColorDef::Rgb(150, 150, 255),
            number: ColorDef::Rgb(120, 200, 255),
            type_name: ColorDef::Rgb(100, 220, 255),
            accent: ColorDef::Rgb(255, 220, 0),
        }
    }

    /// Solarized-dark-inspired palette. Base colors from the canonical
    /// Solarized spec (Ethan Schoonover); syntax accents map to Solarized's
    /// blue/green/magenta/orange roles.
    pub fn solarized() -> Self {
        Self {
            name: "solarized".to_string(),
            header_bg: ColorDef::Rgb(7, 54, 66),         // base02
            header_fg: ColorDef::Rgb(238, 232, 213),     // base3
            editor_bg: ColorDef::Rgb(0, 43, 54),         // base03
            editor_fg: ColorDef::Rgb(131, 148, 150),     // base0
            status_bg: ColorDef::Rgb(7, 54, 66),         // base02
            status_fg: ColorDef::Rgb(147, 161, 161),     // base01
            line_number_fg: ColorDef::Rgb(88, 110, 117), // base01 dim
            selection_bg: ColorDef::Rgb(27, 61, 76),     // base02 lighter
            cursor_fg: ColorDef::Rgb(147, 161, 161),     // base01
            keyword: ColorDef::Rgb(133, 153, 0),         // green
            string: ColorDef::Rgb(42, 161, 152),         // cyan
            comment: ColorDef::Rgb(101, 123, 131),       // base1
            function: ColorDef::Rgb(38, 139, 210),       // blue
            number: ColorDef::Rgb(203, 75, 22),          // orange
            type_name: ColorDef::Rgb(181, 137, 0),       // yellow
            accent: ColorDef::Rgb(108, 113, 196),        // violet
        }
    }

    /// Grayscale-only theme. A single accent (bright white + bold) keeps UI
    /// affordances legible without any chroma — useful for monochrome displays
    /// and for users sensitive to color.
    pub fn monochrome() -> Self {
        Self {
            name: "monochrome".to_string(),
            header_bg: ColorDef::Rgb(40, 40, 40),
            header_fg: ColorDef::Rgb(235, 235, 235),
            editor_bg: ColorDef::Rgb(12, 12, 12),
            editor_fg: ColorDef::Rgb(204, 204, 204),
            status_bg: ColorDef::Rgb(28, 28, 28),
            status_fg: ColorDef::Rgb(160, 160, 160),
            line_number_fg: ColorDef::Rgb(96, 96, 96),
            selection_bg: ColorDef::Rgb(70, 70, 70),
            cursor_fg: ColorDef::Rgb(250, 250, 250),
            keyword: ColorDef::Rgb(245, 245, 245),
            string: ColorDef::Rgb(190, 190, 190),
            comment: ColorDef::Rgb(120, 120, 120),
            function: ColorDef::Rgb(225, 225, 225),
            number: ColorDef::Rgb(210, 210, 210),
            type_name: ColorDef::Rgb(232, 232, 232),
            accent: ColorDef::Rgb(255, 255, 255),
        }
    }

    /// Classic CRT phosphor-green look: black background, green foreground
    /// family. Comments dim green, strings/numbers brighter green, keywords
    /// bright green+bold.
    pub fn terminal_green() -> Self {
        Self {
            name: "terminal-green".to_string(),
            header_bg: ColorDef::Rgb(0, 40, 0),
            header_fg: ColorDef::Rgb(120, 255, 120),
            editor_bg: ColorDef::Rgb(0, 0, 0),
            editor_fg: ColorDef::Rgb(0, 200, 0),
            status_bg: ColorDef::Rgb(0, 30, 0),
            status_fg: ColorDef::Rgb(0, 180, 0),
            line_number_fg: ColorDef::Rgb(0, 110, 0),
            selection_bg: ColorDef::Rgb(0, 90, 0),
            cursor_fg: ColorDef::Rgb(180, 255, 180),
            keyword: ColorDef::Rgb(120, 255, 120),
            string: ColorDef::Rgb(80, 230, 80),
            comment: ColorDef::Rgb(0, 140, 0),
            function: ColorDef::Rgb(160, 255, 160),
            number: ColorDef::Rgb(100, 240, 100),
            type_name: ColorDef::Rgb(140, 255, 140),
            accent: ColorDef::Rgb(0, 255, 0),
        }
    }

    /// Return the six built-in themes (for the Themes popup picker).
    pub fn built_ins() -> Vec<Theme> {
        vec![
            Self::dark_red(),
            Self::light(),
            Self::high_contrast(),
            Self::solarized(),
            Self::monochrome(),
            Self::terminal_green(),
        ]
    }

    /// Style a role name. Unknown roles fall back to editor_fg on editor_bg.
    pub fn style_for(&self, role: &str) -> Style {
        let fg: Color = match role {
            "header_fg" => Color::from(&self.header_fg),
            "keyword" => Color::from(&self.keyword),
            "string" => Color::from(&self.string),
            "comment" => Color::from(&self.comment),
            "function" => Color::from(&self.function),
            "number" => Color::from(&self.number),
            "type" | "type_name" => Color::from(&self.type_name),
            "accent" => Color::from(&self.accent),
            "line_number" => Color::from(&self.line_number_fg),
            _ => Color::from(&self.editor_fg),
        };
        let bg: Option<Color> = match role {
            "header_bg" => Some(Color::from(&self.header_bg)),
            "status_bg" => Some(Color::from(&self.status_bg)),
            "selection" => Some(Color::from(&self.selection_bg)),
            _ => None,
        };
        let mut s = Style::default().fg(fg);
        if let Some(b) = bg {
            s = s.bg(b);
        }
        if role == "keyword" || role == "function" || role == "type" {
            s = s.add_modifier(Modifier::BOLD);
        }
        s
    }

    /// Get header style (bg + fg).
    pub fn header_style(&self) -> Style {
        Style::default()
            .bg(Color::from(&self.header_bg))
            .fg(Color::from(&self.header_fg))
            .add_modifier(Modifier::BOLD)
    }

    pub fn editor_bg_style(&self) -> Style {
        Style::default()
            .bg(Color::from(&self.editor_bg))
            .fg(Color::from(&self.editor_fg))
    }

    pub fn status_style(&self) -> Style {
        Style::default()
            .bg(Color::from(&self.status_bg))
            .fg(Color::from(&self.status_fg))
    }

    pub fn line_number_style(&self) -> Style {
        Style::default().fg(Color::from(&self.line_number_fg))
    }

    pub fn selection_style(&self) -> Style {
        Style::default().bg(Color::from(&self.selection_bg))
    }

    pub fn accent_style(&self) -> Style {
        Style::default().fg(Color::from(&self.accent))
    }

    /// Style for the cursor (block/reverse style). Uses cursor_fg as the highlight color.
    pub fn cursor_style(&self) -> Style {
        Style::default()
            .bg(Color::from(&self.cursor_fg))
            .fg(Color::from(&self.editor_bg))
            .add_modifier(Modifier::REVERSED)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark_red()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_ins_count_and_unique_names() {
        let themes = Theme::built_ins();
        assert_eq!(themes.len(), 6, "expected 6 built-in themes");
        let names: Vec<&str> = themes.iter().map(|t| t.name.as_str()).collect();
        for (i, n) in names.iter().enumerate() {
            assert!(
                names.iter().rposition(|x| x == n) == Some(i),
                "duplicate theme name '{n}'"
            );
        }
    }

    #[test]
    fn built_ins_serde_roundtrip() {
        // Each built-in must survive a TOML round-trip so custom-theme files
        // (which use the same serde path) stay valid.
        for theme in Theme::built_ins() {
            let toml_str = toml::to_string(&theme).expect("serialize theme");
            let back: Theme = toml::from_str(&toml_str).expect("deserialize theme");
            assert_eq!(back.name, theme.name, "name survived roundtrip");
        }
    }

    #[test]
    fn parse_input_hex_short_long() {
        assert_eq!(
            ColorDef::parse_input("#abc").unwrap(),
            ColorDef::Rgb(0xaa, 0xbb, 0xcc)
        );
        assert_eq!(
            ColorDef::parse_input("#1F2E3D").unwrap(),
            ColorDef::Rgb(0x1f, 0x2e, 0x3d)
        );
    }

    #[test]
    fn parse_input_rgb_tuple() {
        assert_eq!(
            ColorDef::parse_input("rgb(1, 2, 3)").unwrap(),
            ColorDef::Rgb(1, 2, 3)
        );
        assert!(ColorDef::parse_input("rgb(1,2)").is_err());
        assert!(ColorDef::parse_input("rgb(300,0,0)").is_err());
    }

    #[test]
    fn parse_input_named_canonicalized() {
        // Various spellings collapse to a canonical no-separator form.
        assert_eq!(
            ColorDef::parse_input("LightBlue").unwrap(),
            ColorDef::Named("lightblue".to_string())
        );
        assert_eq!(
            ColorDef::parse_input("light blue").unwrap(),
            ColorDef::Named("light_blue".to_string())
        );
        assert_eq!(
            ColorDef::parse_input("light_blue").unwrap(),
            ColorDef::Named("light_blue".to_string())
        );
        assert_eq!(
            ColorDef::parse_input("red").unwrap(),
            ColorDef::Named("red".to_string())
        );
    }

    #[test]
    fn parse_input_rejects_garbage() {
        assert!(ColorDef::parse_input("").is_err());
        assert!(ColorDef::parse_input("   ").is_err());
        assert!(ColorDef::parse_input("#zzz").is_err());
        assert!(ColorDef::parse_input("#12345").is_err());
        assert!(ColorDef::parse_input("purplish").is_err());
    }

    #[test]
    fn named_conversion_covers_all_documented_names() {
        // Every name `is_named` accepts must convert to a non-White ratatui
        // color (White is our fallback for unknowns, so seeing it for a "known"
        // name would mean the lookup table and the recognizer drifted apart).
        for name in [
            "black",
            "red",
            "green",
            "yellow",
            "blue",
            "magenta",
            "cyan",
            "gray",
            "darkgray",
            "lightred",
            "lightgreen",
            "lightyellow",
            "lightblue",
            "lightmagenta",
            "lightcyan",
        ] {
            let c = Color::from(ColorDef::Named(name.to_string()));
            assert_ne!(c, Color::White, "name '{name}' must not fall back to White");
        }
    }
}
