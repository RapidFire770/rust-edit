// Help / keybindings popup. Tabbed: Shortcuts, Languages, About.
// Pure presentation; tab state lives on App.

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::scrollbar::{self, OverflowBar};

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
                ("Open Project Folder", "Pick a folder; recents are kept"),
                ("Project Browser", "Show or hide that folder's tree"),
                ("Ctrl+S", "Save"),
                ("Ctrl+Alt+S", "Save As"),
                ("Read-only", "Save warns when you cannot write the file"),
                ("Ctrl+W", "Close tab"),
                ("Ctrl+Q", "Quit"),
                (
                    "Disk change",
                    "Prompt to reload a file edited outside the editor",
                ),
                (
                    "[ Always ]",
                    "Reload this file silently for the rest of the session",
                ),
                ("[ Browse… ]", "Open the file browser from Open/Save As"),
                (
                    "[ Use folder ]",
                    "Save As browse: save into the current folder using Name",
                ),
                ("Name:", "Save As browse: type the new filename here"),
                ("↑/↓ Enter", "Browse: navigate / enter dir, or pick a file"),
                ("Backspace/←/h", "Directory browse: go up one directory"),
                ("Ctrl+H", "Hidden files: browse or project"),
                (
                    "Tab",
                    "Browse: cycle focus (Save As: list → Name → buttons)",
                ),
                (
                    "[x] / Del / Backspace",
                    "Recent files: remove the highlighted path",
                ),
                ("[ Clear ]", "Recent files: empty the whole list"),
            ],
        ),
        (
            "Edit",
            vec![
                ("Enter", "Newline + smart indent"),
                ("Tab", "Insert tab-size spaces at cursor"),
                ("Shift+Tab", "Dedent line / selection"),
                ("Ctrl+/", "Toggle comment"),
                ("Backspace / Delete", "Delete char"),
                ("Ctrl+Z / Ctrl+Y", "Undo / redo"),
                ("Ctrl+X / Ctrl+C / Ctrl+V", "Cut / copy / paste"),
                ("Ctrl+A", "Select all"),
                ("Esc", "Clear selection"),
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
                ("Shift+PgUp / PgDn", "Extend selection by a page"),
                ("PgUp / PgDn", "Page up / down"),
                ("Alt+H / Alt+L", "Scroll view left / right"),
                ("Alt+F / E / T / S", "Open File / Edit / Tools / Settings"),
                ("j / k, Up / Down", "Move selection in an open menu"),
                (
                    "Left / Right / Tab",
                    "Switch File / Edit / Tools / Settings",
                ),
                ("Enter / Esc", "Run the menu item / close the menu"),
                ("Alt+1..9", "Switch to tab N"),
                ("[<] / [>]", "Scroll overflow file tabs"),
            ],
        ),
        (
            "View & Tools",
            vec![
                ("F1", "Toggle this help"),
                ("Alt+W", "Word count"),
                ("Alt+Z", "Toggle word wrap"),
                ("F7", "Spelling suggestions at the cursor"),
                ("Enter", "Spelling: replace with the selected word"),
                ("a / i", "Spelling: add the word / ignore until quit"),
                ("Dictionary", "List of your added words"),
                ("a / e", "Text box: add or edit"),
                ("Del / [x]", "Remove the selected word"),
                ("Format Document", "Reformat the open file"),
                ("", "Installed formatter, else cleanup"),
                ("Function List", "Function panel, prefers left"),
                ("", "Enter or a click jumps to a name"),
                ("Variables List", "Variable panel, prefers right"),
                ("", "Enter or a click jumps to a name"),
                ("Project Browser", "Folder tree, 36 columns"),
                ("", "Enter opens a file or a folder"),
                ("", "Replaces the left panel"),
                ("x / [x]", "Hide the tree; the folder stays"),
                ("r", "Reload the open folders"),
                ("Left / Right", "Move the focused list to that side"),
                ("x", "Close the focused list"),
                ("Esc", "Leave the list open and return to the editor"),
                ("Alt+Left / Alt+Right", "Focus the open list on that side"),
                ("1 / 2 / 3", "Help tab: Shortcuts / Languages / About"),
            ],
        ),
        (
            "Configure & Themes",
            vec![
                ("Settings menu", "Configure... / Themes... / Help"),
                (
                    "Auto-reload all",
                    "Configure: reload every clean file without asking",
                ),
                ("Spellcheck", "Configure or Tools: underline misspellings"),
                (
                    "j / k, Up / Down",
                    "Move selection in Configure / Themes list",
                ),
                ("Enter", "Toggle or step up / Apply theme"),
                ("Left / Right", "Step tab size in Configure"),
                ("e / n / d", "List: edit / new / delete custom"),
                ("Tab", "Edit: Name → Roles → Palette → R/G/B → Hex"),
                (
                    "Arrows",
                    "Navigate focused widget (roles, swatches, sliders)",
                ),
                ("s / Enter", "Save (s ignored while typing name or hex)"),
                ("Esc", "Close list / cancel editor"),
            ],
        ),
        (
            "Mouse",
            vec![
                ("Click tab", "Switch buffer"),
                ("Middle-click tab", "Close tab"),
                ("Click menu / [+]", "Open menu / new tab"),
                ("[<] / [>] / wheel over tabs", "Scroll overflow file tabs"),
                ("Click + drag", "Place cursor / select text"),
                ("Scroll wheel", "Scroll vertically"),
                ("Shift+wheel", "Scroll horizontally"),
                ("Click / drag bars", "Vertical and horizontal bars"),
                ("Click buttons", "Click any [ button ] in popups"),
                ("Click < / >", "Step tab size in Configure"),
                (
                    "Click swatch or RGB bar",
                    "Set the selected theme role color",
                ),
            ],
        ),
    ]
}

fn languages_rows() -> Vec<Row<'static>> {
    vec![
        ("Bash / Shell", ".sh, .bash, .zsh, .fish"),
        ("Batch", ".bat, .cmd"),
        ("C", ".c, .h"),
        ("C#", ".cs, .csx"),
        ("C++", ".cpp, .cc, .cxx, .hpp"),
        ("Clojure", ".clj, .cljs, .cljc"),
        ("CSS", ".css"),
        ("D", ".d, .di"),
        ("Diff", ".diff, .patch"),
        ("Erlang", ".erl, .hrl"),
        ("Go", ".go"),
        ("Groovy", ".groovy, .gradle"),
        ("Haskell", ".hs"),
        ("HTML", ".html, .htm"),
        ("INI", ".ini, .cfg"),
        ("Java", ".java"),
        ("Java Properties", ".properties"),
        ("JavaScript", ".js, .mjs, .jsx"),
        ("JSON", ".json"),
        ("JSONC", ".jsonc"),
        ("LaTeX", ".tex, .ltx"),
        ("Lisp", ".lisp, .el, .scm"),
        ("Lua", ".lua"),
        ("Makefile", "Makefile, .mk"),
        ("Markdown", ".md, .markdown"),
        ("Objective-C", ".m"),
        ("Objective-C++", ".mm"),
        ("OCaml", ".ml, .mli"),
        ("Perl", ".pl, .pm, .pod"),
        ("PHP", ".php, .phtml"),
        ("Python", ".py, .pyw, .pyi"),
        ("R", ".r"),
        ("reStructuredText", ".rst, .rest"),
        ("Ruby", ".rb, .rake, Gemfile"),
        ("Rust", ".rs"),
        ("Scala", ".scala, .sbt"),
        ("SQL", ".sql, .ddl, .dml"),
        ("Tcl", ".tcl"),
        ("TeX", ".sty, .cls"),
        ("TOML", ".toml"),
        ("TypeScript", ".ts, .tsx"),
        ("XML", ".xml, .svg, .xsd"),
        ("YAML", ".yaml, .yml"),
        ("Zig", ".zig"),
        ("Plain Text", "(fallback)"),
    ]
}

/// Compute the centered popup rect for the help window, mirroring `render`.
/// Exposed so the mouse handler can hit-test against the same geometry.
pub fn help_rect(area: Rect) -> Rect {
    let w = (area.width * 9 / 10).clamp(60, 96);
    let h = (area.height * 9 / 10).min(34);
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}

/// Tab button hit-rects in popup-local coordinates (0 = left border).
/// Each entry: (label, tab, x_start, x_end). The tab strip sits at inner row y=1.
pub fn help_tab_rects() -> &'static [(&'static str, HelpTab, u16, u16)] {
    &[
        ("1 Shortcuts", HelpTab::Shortcuts, 1, 12),
        ("2 Languages", HelpTab::Languages, 15, 26),
        ("3 About", HelpTab::About, 29, 36),
    ]
}

/// Inner area inside the help border.
pub fn help_inner(popup: Rect) -> Rect {
    Rect::new(
        popup.x.saturating_add(1),
        popup.y.saturating_add(1),
        popup.width.saturating_sub(2),
        popup.height.saturating_sub(2),
    )
}

/// Scrollable body: skips the pinned tab strip + blank row, leaves the footer.
pub fn help_body_area(popup: Rect) -> Rect {
    let inner = help_inner(popup);
    let y = inner.y.saturating_add(2);
    let h = inner.height.saturating_sub(3);
    Rect::new(inner.x, y, inner.width, h)
}

/// Rightmost column of the help body (scrollbar track).
pub fn help_scrollbar_rect(popup: Rect) -> Rect {
    let body = help_body_area(popup);
    Rect::new(
        body.x.saturating_add(body.width.saturating_sub(1)),
        body.y,
        if body.width == 0 { 0 } else { 1 },
        body.height,
    )
}

pub fn help_overflow_bar(tab: HelpTab, popup: Rect, scroll_row: u16) -> OverflowBar {
    let body = help_body_area(popup);
    OverflowBar {
        total: help_body_line_count(tab),
        position: scroll_row as usize,
        viewport: body.height as usize,
    }
}

pub fn help_body_line_count(tab: HelpTab) -> usize {
    help_body_lines(tab, &Theme::default()).len()
}

/// True when `(x, y)` is on the help body overflow track.
pub fn help_click_is_scrollbar(
    x: usize,
    y: usize,
    popup: Rect,
    tab: HelpTab,
    scroll_row: u16,
) -> bool {
    let bar = help_overflow_bar(tab, popup, scroll_row);
    if !bar.overflowing() {
        return false;
    }
    let sb = help_scrollbar_rect(popup);
    sb.width > 0
        && x >= sb.x as usize
        && x < (sb.x + sb.width) as usize
        && y >= sb.y as usize
        && y < (sb.y + sb.height) as usize
}

pub fn clamp_help_scroll(scroll: u16, tab: HelpTab, popup: Rect) -> u16 {
    let vis = help_body_area(popup).height as usize;
    let total = help_body_line_count(tab);
    let max = total.saturating_sub(vis);
    (scroll as usize).min(max) as u16
}

fn help_body_lines(tab: HelpTab, theme: &Theme) -> Vec<Line<'static>> {
    let accent = Style::default()
        .fg(Color::from(&theme.accent))
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::from(&theme.editor_fg));
    let key_style = Style::default()
        .fg(Color::from(&theme.string))
        .add_modifier(Modifier::BOLD);

    let mut lines: Vec<Line> = Vec::new();
    match tab {
        HelpTab::Shortcuts => {
            for (category, rows) in shortcuts_rows() {
                lines.push(Line::from(Span::styled(category, accent)));
                for (k, d) in rows {
                    let mut row = vec![Span::raw("  "), Span::styled(k, key_style)];
                    let pad = 28usize.saturating_sub(k.chars().count());
                    row.push(Span::raw(" ".repeat(pad + 2)));
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
            let version = format!("Version {}", env!("CARGO_PKG_VERSION"));
            lines.push(Line::from(Span::styled("rust-edit", accent)));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(version, dim)));
            lines.push(Line::from(""));
            // Split so each line fits the narrowest help body (popup width 60 → inner 58).
            lines.push(Line::from(Span::styled(
                "A fast keyboard-and-mouse text and code editor",
                dim,
            )));
            lines.push(Line::from(Span::styled("for the Linux terminal.", dim)));
            lines.push(Line::from(""));
            lines.push(Line::from(
                "Tabs, syntax highlighting, themes, find and replace,",
            ));
            lines.push(Line::from("and full mouse support."));
            lines.push(Line::from(""));
            lines.push(Line::from("See README.md for more details."));
        }
    }
    lines
}

pub fn render(f: &mut Frame, area: Rect, theme: &Theme, tab: HelpTab, scroll: (u16, u16)) {
    let popup = help_rect(area);
    let inner = help_inner(popup);
    let body = help_body_area(popup);
    let scroll_y = clamp_help_scroll(scroll.0, tab, popup);
    let bar = help_overflow_bar(tab, popup, scroll_y);

    let accent = Style::default()
        .fg(Color::from(&theme.accent))
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::from(&theme.editor_fg));

    let title = format!(" Help — {:?} ", tab);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    f.render_widget(Clear, popup);
    f.render_widget(block, popup);

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
    let tab_area = Rect::new(inner.x, inner.y, inner.width, 1.min(inner.height));
    f.render_widget(Paragraph::new(Line::from(tab_spans)), tab_area);

    let content_w = if bar.overflowing() {
        body.width.saturating_sub(1)
    } else {
        body.width
    };
    let content = Rect::new(body.x, body.y, content_w, body.height);
    if content.height > 0 && content.width > 0 {
        let para = Paragraph::new(help_body_lines(tab, theme))
            .style(Style::default().fg(Color::from(&theme.editor_fg)))
            .scroll((scroll_y, 0));
        f.render_widget(para, content);
    }

    scrollbar::render(f, help_scrollbar_rect(popup), bar, theme);

    if inner.height > 0 {
        let footer_area = Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(1),
            inner.width,
            1,
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Press F1 or Esc to close. Tab or 1/2/3 to switch sections.",
                dim,
            ))),
            footer_area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typical_term() -> Rect {
        Rect::new(0, 0, 80, 24)
    }

    #[test]
    fn help_shortcuts_overflows_default_popup() {
        let popup = help_rect(typical_term());
        let vis = help_body_area(popup).height as usize;
        assert!(help_body_line_count(HelpTab::Shortcuts) > vis);
        assert!(help_overflow_bar(HelpTab::Shortcuts, popup, 0).overflowing());
    }

    #[test]
    fn help_scrollbar_hidden_when_about_fits() {
        let popup = help_rect(typical_term());
        assert!(!help_overflow_bar(HelpTab::About, popup, 0).overflowing());
    }

    #[test]
    fn clamp_help_scroll_caps_at_max() {
        let popup = help_rect(typical_term());
        let vis = help_body_area(popup).height as usize;
        let total = help_body_line_count(HelpTab::Shortcuts);
        let max = total.saturating_sub(vis) as u16;
        assert_eq!(clamp_help_scroll(0, HelpTab::Shortcuts, popup), 0);
        assert_eq!(clamp_help_scroll(max, HelpTab::Shortcuts, popup), max);
        assert_eq!(clamp_help_scroll(u16::MAX, HelpTab::Shortcuts, popup), max);
    }

    #[test]
    fn help_tab_strip_stays_on_inner_row_one() {
        let popup = help_rect(typical_term());
        // handle_help_click uses popup.y + 1 as the tab row.
        assert_eq!(help_inner(popup).y, popup.y + 1);
        let body = help_body_area(popup);
        assert!(body.y > popup.y + 1, "body must sit below the tab strip");
    }

    #[test]
    fn help_scrollbar_rect_is_rightmost_body_column() {
        let popup = help_rect(typical_term());
        let body = help_body_area(popup);
        let sb = help_scrollbar_rect(popup);
        assert_eq!(sb.y, body.y);
        assert_eq!(sb.height, body.height);
        assert_eq!(sb.width, 1);
        assert_eq!(sb.x, body.x + body.width - 1);
    }

    #[test]
    fn about_tab_shows_version_and_readme() {
        let lines = help_body_lines(HelpTab::About, &Theme::default());
        let text: String = lines
            .iter()
            .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
            .collect();
        assert!(text.contains(&format!("Version {}", env!("CARGO_PKG_VERSION"))));
        assert!(text.contains("README.md"));
        assert!(!text.contains("AGENTS.md"));
        assert!(!text.contains("design spec"));
        assert!(text.contains("A fast keyboard-and-mouse text and code editor"));
        assert!(text.contains("for the Linux terminal."));
        assert!(text.contains("Tabs, syntax highlighting, themes, find and replace,"));
        assert!(text.contains("and full mouse support."));
        for line in &lines {
            let width: usize = line
                .spans
                .iter()
                .map(|span| span.content.chars().count())
                .sum();
            assert!(
                width <= 58,
                "about line is {width} columns; min help body is 58"
            );
        }
    }

    #[test]
    fn language_rows_fit_the_narrow_help_body() {
        for (name, ext) in languages_rows() {
            assert!(
                name.chars().count() <= 16,
                "{name} is wider than the 16-column name"
            );
            let pad = 16usize.saturating_sub(name.chars().count());
            let width = 2 + name.chars().count() + pad + ext.chars().count();
            assert!(
                width <= 58,
                "{name} / {ext} is {width} columns; min help body is 58"
            );
        }
    }

    fn shortcuts_text() -> String {
        help_body_lines(HelpTab::Shortcuts, &Theme::default())
            .iter()
            .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
            .collect()
    }

    #[test]
    fn shortcuts_page_matches_keys_that_run() {
        let text = shortcuts_text();
        assert!(text.contains("Ctrl+/"), "{text}");
        assert!(text.contains("Toggle comment"), "{text}");
        assert!(text.contains("Left / Right / Tab"), "{text}");
        assert!(
            text.contains("Switch File / Edit / Tools / Settings"),
            "{text}"
        );
        assert!(text.contains("Alt+W"), "{text}");
        assert!(text.contains("Word count"), "{text}");
        assert!(text.contains("Format Document"), "{text}");
        assert!(text.contains("Reformat the open file"), "{text}");
        assert!(text.contains("Installed formatter, else cleanup"), "{text}");
        assert!(text.contains("Function panel, prefers left"), "{text}");
        assert!(text.contains("Variable panel, prefers right"), "{text}");
        assert!(text.contains("Enter or a click jumps to a name"), "{text}");
        assert!(text.contains("Alt+Left / Alt+Right"), "{text}");
        assert!(text.contains("Focus the open list on that side"), "{text}");
        assert!(text.contains("Open Project Folder"), "{text}");
        assert!(text.contains("Project Browser"), "{text}");
        assert!(text.contains("Hide the tree; the folder stays"), "{text}");
        assert!(text.contains("Hidden files: browse or project"), "{text}");
        // 80-column help body is 69 columns once the scrollbar is reserved.
        // Descriptions start at column 32, so each of these lines must be <= 37.
        for (key, desc) in [
            ("Format Document", "Reformat the open file"),
            ("", "Installed formatter, else cleanup"),
            ("Function List", "Function panel, prefers left"),
            ("", "Enter or a click jumps to a name"),
            ("Variables List", "Variable panel, prefers right"),
            ("Alt+Left / Alt+Right", "Focus the open list on that side"),
            ("Open Project Folder", "Pick a folder; recents are kept"),
            ("Project Browser", "Show or hide that folder's tree"),
            ("Project Browser", "Folder tree, 36 columns"),
            ("", "Enter opens a file or a folder"),
            ("", "Replaces the left panel"),
            ("x / [x]", "Hide the tree; the folder stays"),
            ("r", "Reload the open folders"),
            ("Ctrl+H", "Hidden files: browse or project"),
        ] {
            let pad = 28usize.saturating_sub(key.chars().count());
            let width = 2 + key.chars().count() + pad + 2 + desc.chars().count();
            assert!(
                width <= 69,
                "{key:?} / {desc} is {width} columns; 80-col help body is 69"
            );
        }
        assert!(text.contains("Clear selection"), "{text}");
        assert!(text.contains("Shift+PgUp / PgDn"), "{text}");
        assert!(text.contains("Shift+wheel"), "{text}");
        assert!(text.contains("Vertical and horizontal bars"), "{text}");
        assert!(!text.contains("Shift+W"), "{text}");
        assert!(!text.contains("Ctrl+B"), "{text}");
        assert!(!text.contains("Markdown bold"), "{text}");
        assert!(!text.contains("Ctrl+Shift+F"), "{text}");
        // Ctrl+H remains the file-browser hidden-files toggle.
        assert!(text.contains("Ctrl+H"), "{text}");
        assert!(text.contains("[x] / Del / Backspace"), "{text}");
        assert!(text.contains("[ Clear ]"), "{text}");
    }

    #[test]
    fn help_click_is_scrollbar_false_when_about_fits() {
        let popup = help_rect(typical_term());
        let sb = help_scrollbar_rect(popup);
        assert!(!help_click_is_scrollbar(
            sb.x as usize,
            sb.y as usize,
            popup,
            HelpTab::About,
            0
        ));
        assert!(help_click_is_scrollbar(
            sb.x as usize,
            sb.y as usize,
            popup,
            HelpTab::Shortcuts,
            0
        ));
    }
}
