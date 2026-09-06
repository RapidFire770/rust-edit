// Status bar.
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

    // Everything after the path is fixed-width content that must stay visible.
    // Truncate the path from the left when it would push these off-screen.
    let fixed_rest = format!(
        "  |  {}  |  Ln {}, Col {}  |  {} lines  |  Ctrl+Q quit  |  F1 help ",
        lang,
        row + 1,
        col + 1,
        buf.line_count()
    );
    let fixed_width = 1 + modified.chars().count() + fixed_rest.chars().count();
    let available = (area.width as usize).saturating_sub(fixed_width);
    let path_display = truncate_left(&path, available);

    let text = format!(" {}{}{}", path_display, modified, fixed_rest);
    let status = Paragraph::new(text).style(app.current_theme().status_style());
    f.render_widget(status, area);
}

/// Shorten `s` from the left with an ellipsis so it fits in `max_chars`
/// columns (char count, matching the wrap module's width convention).
/// The tail is kept so the file name stays visible.
fn truncate_left(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars {
        return s.to_string();
    }
    if max_chars < 3 {
        return s.chars().skip(n - max_chars).collect();
    }
    let keep = max_chars - 3;
    let suffix: String = s.chars().skip(n - keep).collect();
    format!("...{}", suffix)
}

#[cfg(test)]
mod tests {
    use super::truncate_left;

    #[test]
    fn short_string_unchanged() {
        assert_eq!(truncate_left("abc", 5), "abc");
    }

    #[test]
    fn exact_fit_unchanged() {
        assert_eq!(truncate_left("abc", 3), "abc");
    }

    #[test]
    fn long_string_truncated_with_ellipsis() {
        // 28 chars; keep last 7 after the ellipsis.
        let out = truncate_left("/home/user/projects/file.txt", 10);
        assert_eq!(out, "...ile.txt");
        assert_eq!(out.chars().count(), 10);
    }

    #[test]
    fn tiny_width_has_no_room_for_ellipsis() {
        let out = truncate_left("abcdef", 2);
        assert_eq!(out, "ef");
    }

    #[test]
    fn zero_width_is_empty() {
        assert_eq!(truncate_left("abcdef", 0), "");
    }

    #[test]
    fn multibyte_chars_counted_per_char() {
        // 6 chars (a,b,c,é,中,x); keep the last 2 after the ellipsis.
        // Byte-based counting would break this, so it must be char-based.
        let out = truncate_left("abcé中x", 5);
        assert_eq!(out, "...中x");
        assert_eq!(out.chars().count(), 5);
    }
}
