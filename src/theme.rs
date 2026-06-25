// Theme struct + color roles + built-in themes + serde. Sprint 1.
// Default "dark-red" must match docs/ui-design-sample-2.jpg aesthetic (red header bar + dark editor).
// All UI colors MUST come from Theme (AGENTS.md rule).

#![allow(dead_code)]

use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
                "white" => Color::White,
                "black" => Color::Black,
                "gray" | "grey" => Color::Gray,
                "red" => Color::Red,
                "darkred" | "dark_red" => Color::Rgb(139, 0, 0),
                "green" => Color::Green,
                "blue" => Color::Blue,
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

    /// Return the four built-in themes (for Settings picker).
    pub fn built_ins() -> Vec<Theme> {
        vec![Self::dark_red(), Self::light(), Self::high_contrast()]
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
