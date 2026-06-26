// Status bar. Sprint 1/2.
// Presentation only.

use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let buf = app.active_buffer();
    let (row, col) = buf.cursor_position();
    let path = buf
        .path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "[No Name]".into());
    let modified = if buf.is_modified { "•" } else { "" };
    let lang = format!("{:?}", buf.language);
    let text = format!(
        " {}{}  |  {}  |  Ln {}, Col {}  |  {} lines  |  Ctrl+Q quit  |  F1 help ",
        path,
        modified,
        lang,
        row + 1,
        col + 1,
        buf.line_count()
    );
    let status = Paragraph::new(text).style(app.current_theme().status_style());
    f.render_widget(status, area);
}
