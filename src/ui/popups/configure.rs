// "Configure" popup — editable non-theme settings (tab size, word wrap, line
// numbers, mouse input). Live-applied + persisted on every change.
//
// Layout follows the confirm-popup convention: state lives on `App`, this
// module only renders, and a public `configure_rect` helper is reused by the
// mouse handler so hit-testing agrees with the drawn geometry (AGENTS §6).

#![allow(dead_code)]

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;
use crate::App;

/// Number of editable rows. Kept in sync with the match arms in
/// `App::configure_adjust` and the render loop below.
pub const ROW_COUNT: usize = 4;

// Row indices — single source of truth so App + render + mouse agree.
pub const ROW_TAB_SIZE: usize = 0;
pub const ROW_WORD_WRAP: usize = 1;
pub const ROW_LINE_NUMBERS: usize = 2;
pub const ROW_MOUSE: usize = 3;

/// Inner content height of the popup (title + blank + 4 rows + blank + footer).
pub const POPUP_W: u16 = 46;
pub const POPUP_H: u16 = 11;

/// Centered popup rect. Exposed for the mouse handler.
pub fn configure_rect(area: Rect) -> Rect {
    let x = (area.width.saturating_sub(POPUP_W)) / 2;
    let y = (area.height.saturating_sub(POPUP_H)) / 2;
    Rect::new(x, y, POPUP_W.min(area.width), POPUP_H.min(area.height))
}

/// Footer button labels (used by both render and the mouse hit-test).
pub fn footer_labels() -> [&'static str; 2] {
    ["[ Enter=Toggle ]", "[ Esc=Close ]"]
}

/// Screen y of the footer button row, given the popup rect.
pub fn footer_row_y(popup: Rect) -> u16 {
    // Layout: border(1) + title(1) + blank(1) + 4 rows + blank(1) + footer(1)
    //       = popup.y + 1 + 1 + 1 + 4 + 1 = popup.y + 8
    popup.y + 8
}

/// Inner-row y positions (absolute screen coords) for each editable row, so the
/// mouse handler can hit-test without re-deriving the layout.
pub fn row_ys(popup: Rect) -> [u16; ROW_COUNT] {
    let start = popup.y + 1 + 1 + 1; // border + title + blank
    [start, start + 1, start + 2, start + 3]
}

pub fn render(f: &mut Frame, area: Rect, theme: &Theme, app: &App) {
    let popup = configure_rect(area);
    let block = Block::default()
        .title("Configure")
        .borders(Borders::ALL)
        .style(theme.editor_bg_style());

    let accent = Style::default()
        .fg(Color::from(&theme.accent))
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::from(&theme.editor_fg));
    let dim_italic = Style::default()
        .fg(Color::from(&theme.editor_fg))
        .add_modifier(Modifier::DIM);

    // Build the four rows. Each row: label (left), value (right). The selected
    // or hovered row is rendered with the accent style; the value cell is what
    // the mouse click handler targets for toggles/steppers.
    let cfg = &app.config;
    let rows: [(String, String); ROW_COUNT] = [
        ("Tab size".to_string(), format!("< {} >", cfg.tab_size)),
        (
            "Word wrap".to_string(),
            if cfg.word_wrap { "[x]" } else { "[ ]" }.to_string(),
        ),
        (
            "Line numbers".to_string(),
            if cfg.show_line_numbers { "[x]" } else { "[ ]" }.to_string(),
        ),
        (
            "Mouse input".to_string(),
            if cfg.mouse_enabled { "[x]" } else { "[ ]" }.to_string(),
        ),
    ];

    let mut lines: Vec<Line> = Vec::with_capacity(8);
    lines.push(Line::from(Span::styled("Settings (live)", dim_italic)));
    lines.push(Line::from(""));

    for (i, (label, value)) in rows.iter().enumerate() {
        let active = i == app.configure_selected || app.configure_row_hover == Some(i);
        let label_style = if active { accent } else { dim };
        let value_style = if active { accent } else { dim };
        // Pad label to align the value column at a fixed x.
        let label_padded = format!("{:<14}", label);
        lines.push(Line::from(vec![
            Span::styled(if active { "> " } else { "  " }, label_style),
            Span::styled(label_padded, label_style),
            Span::styled(value.clone(), value_style),
        ]));
    }

    lines.push(Line::from(""));

    // Footer button row.
    let labels = footer_labels();
    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::raw("  "));
    for (i, l) in labels.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        let hovered = app.configure_button_hover == Some(i);
        let style = if hovered { accent } else { dim };
        spans.push(Span::styled(*l, style));
    }
    lines.push(Line::from(spans));

    let para = Paragraph::new(lines).block(block).style(dim);

    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(para, popup);
}
