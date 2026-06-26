// Help / keybindings popup. Tabbed: Shortcuts, Languages, About.
// Pure presentation; tab state lives on App.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;

/// Which help tab is shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HelpTab {
    #[default]
    Shortcuts,
    Languages,
    About,
}

impl HelpTab {
    pub fn next(self) -> Self {
        match self {
            HelpTab::Shortcuts => HelpTab::Languages,
            HelpTab::Languages => HelpTab::About,
            HelpTab::About => HelpTab::Shortcuts,
        }
    }
}

/// A row of the shortcuts table: (key, description).
type Row<'a> = (&'a str, &'a str);

fn shortcuts_rows() -> Vec<(&'static str, Vec<Row<'static>>)> {
    // (category, rows)
    vec![
        (
            "File",
            vec![
                ("Ctrl+N", "New tab"),
                ("Ctrl+O", "Open file"),
                ("Ctrl+S", "Save"),
                ("Ctrl+Alt+S", "Save As"),
                ("Ctrl+W", "Close tab"),
                ("Ctrl+Q", "Quit"),
            ],
        ),
        (
            "Edit",
            vec![
                ("Enter", "Newline + smart indent"),
                ("Tab / Shift+Tab", "Indent / dedent"),
                ("Backspace / Delete", "Delete char"),
                ("Ctrl+Z / Ctrl+Y", "Undo / redo"),
                ("Ctrl+X / Ctrl+C / Ctrl+V", "Cut / copy / paste"),
                ("Ctrl+B / I / `", "Markdown bold / italic / code (in .md)"),
            ],
        ),
        (
            "Find & Replace",
            vec![
                ("Ctrl+F", "Open Find/Replace bar"),
                ("Tab", "Cycle focus: query -> replace -> commands"),
                (
                    "c / x / w",
                    "Toggle case / regex / whole-word (in command mode)",
                ),
                ("n / N", "Next / previous match (in command mode)"),
                ("r / Enter", "Replace current match"),
                ("Ctrl+A", "Replace all"),
                ("Esc", "Close bar"),
            ],
        ),
        (
            "Navigation",
            vec![
                ("Arrows / Home / End", "Move cursor"),
                ("Shift+Arrows / Home / End", "Extend selection"),
                ("PgUp / PgDn", "Page up / down"),
                ("Alt+H / Alt+L", "Scroll view left / right"),
                ("Alt+F / T / S", "Open File / Tools / Settings menu"),
                ("Alt+1..9", "Switch to tab N"),
            ],
        ),
        (
            "View & Tools",
            vec![
                ("? / F1", "Toggle this help"),
                ("Shift+W", "Word count"),
                ("Ctrl+Shift+F", "Format document (rustfmt/black/prettier)"),
                ("1 / 2 / 3", "Help tab: Shortcuts / Languages / About"),
            ],
        ),
        (
            "Mouse",
            vec![
                ("Click tab", "Switch buffer"),
                ("Click menu / [+]", "Open menu / new tab"),
                ("Click + drag", "Place cursor / select text"),
                ("Scroll wheel", "Scroll vertically"),
                ("Click buttons", "Click any [ button ] in popups"),
            ],
        ),
    ]
}

fn languages_rows() -> Vec<Row<'static>> {
    vec![
        ("Rust", ".rs"),
        ("Python", ".py, .pyw"),
        ("Markdown", ".md, .markdown"),
        ("JSON", ".json"),
        ("TOML", ".toml"),
        ("YAML", ".yaml, .yml"),
        ("Bash / Shell", ".sh, .bash, .zsh"),
        ("C", ".c, .h"),
        ("C++", ".cpp, .cc, .cxx, .hpp"),
        ("JavaScript", ".js, .mjs"),
        ("TypeScript", ".ts"),
        ("Plain Text", "(fallback)"),
    ]
}

pub fn render(f: &mut Frame, area: Rect, theme: &Theme, tab: HelpTab) {
    let w = (area.width * 9 / 10).clamp(60, 96);
    let h = (area.height * 9 / 10).min(34);
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w.min(area.width), h.min(area.height));

    let accent = Style::default()
        .fg(Color::from(&theme.accent))
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::from(&theme.editor_fg));
    let key_style = Style::default()
        .fg(Color::from(&theme.string))
        .add_modifier(Modifier::BOLD);

    let title = format!(" Help — {:?} ", tab);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    // Tab strip.
    let tabs = [
        ("1 Shortcuts", HelpTab::Shortcuts),
        ("2 Languages", HelpTab::Languages),
        ("3 About", HelpTab::About),
    ];
    let mut tab_spans: Vec<Span> = Vec::new();
    for (i, (label, t)) in tabs.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::raw("   "));
        }
        let s = if *t == tab { accent } else { dim };
        tab_spans.push(Span::styled(*label, s));
    }

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(tab_spans));
    lines.push(Line::from(""));

    match tab {
        HelpTab::Shortcuts => {
            for (category, rows) in shortcuts_rows() {
                lines.push(Line::from(Span::styled(category, accent)));
                for (k, d) in rows {
                    let mut row = vec![Span::raw("  "), Span::styled(k, key_style)];
                    // pad key column to align descriptions
                    let pad = 22usize.saturating_sub(k.chars().count());
                    row.push(Span::raw(" ".repeat(pad)));
                    row.push(Span::styled(d, dim));
                    lines.push(Line::from(row));
                }
                lines.push(Line::from(""));
            }
        }
        HelpTab::Languages => {
            lines.push(Line::from(Span::styled(
                "Syntax-highlighted languages:",
                accent,
            )));
            lines.push(Line::from(""));
            for (lang, ext) in languages_rows() {
                let mut row = vec![Span::raw("  "), Span::styled(lang, key_style)];
                let pad = 16usize.saturating_sub(lang.chars().count());
                row.push(Span::raw(" ".repeat(pad)));
                row.push(Span::styled(ext, dim));
                lines.push(Line::from(row));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Unsupported extensions fall back to Plain Text.",
                dim,
            )));
        }
        HelpTab::About => {
            lines.push(Line::from(Span::styled("rust-edit", accent)));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "A fast, keyboard + mouse terminal text & code editor built with Ratatui.",
                dim,
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(
                "Rope-backed buffer, syntect highlighting, configurable themes.",
            ));
            lines.push(Line::from(""));
            lines.push(Line::from(
                "See AGENTS.md and the design spec in docs/ for details.",
            ));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press ? or Esc to close. Tab or 1/2/3 to switch sections.",
        dim,
    )));

    let para = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(Color::from(&theme.editor_fg)));

    f.render_widget(Clear, popup);
    f.render_widget(para, popup);
}
