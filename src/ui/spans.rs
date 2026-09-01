// Batch highlight tokens into ratatui spans (hscroll / selection / wrap clip).
// Splits only at those edges — never one Span per character unless styles differ.

use ratatui::style::Style;
use ratatui::text::Span;
use std::ops::Range;

/// Turn highlight token spans into batched ratatui spans.
/// Splits only at hscroll, selection edges, and optional absolute-byte clip
/// (wrapped sub-rows). Never one Span per character unless styles actually differ.
pub fn paint_line_spans(
    highlight: &[Span<'static>],
    line_start_byte: usize,
    hscroll_chars: usize,
    selection: Option<&Range<usize>>,
    selection_style: Style,
    byte_clip: Option<Range<usize>>,
) -> Vec<Span<'static>> {
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style: Option<Style> = None;
    let mut byte_in_line = 0usize;
    let mut char_in_line = 0usize;

    for sp in highlight {
        for ch in sp.content.chars() {
            let abs_byte = line_start_byte + byte_in_line;
            let visible = char_in_line >= hscroll_chars
                && byte_clip
                    .as_ref()
                    .is_none_or(|clip| clip.contains(&abs_byte));
            if visible {
                let selected = selection.is_some_and(|sel| sel.contains(&abs_byte));
                let style = if selected {
                    sp.style.patch(selection_style)
                } else {
                    sp.style
                };
                match run_style {
                    Some(prev) if prev != style => {
                        out.push(Span::styled(std::mem::take(&mut run), prev));
                        run.push(ch);
                        run_style = Some(style);
                    }
                    Some(_) => run.push(ch),
                    None => {
                        run.push(ch);
                        run_style = Some(style);
                    }
                }
            }
            byte_in_line += ch.len_utf8();
            char_in_line += 1;
        }
    }
    if let Some(style) = run_style {
        if !run.is_empty() {
            out.push(Span::styled(run, style));
        }
    }

    // Syntect yields `[Span { content: "" }]` for blank lines, so treat "no
    // visible chars produced" the same as an empty highlight: emit a space
    // when selection covers this line's start (skip wrapped continuation rows).
    if out.is_empty()
        && hscroll_chars == 0
        && selection.is_some_and(|sel| sel.contains(&line_start_byte))
        && byte_clip
            .as_ref()
            .is_none_or(|clip| clip.contains(&line_start_byte))
    {
        out.push(Span::styled(" ", selection_style));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::paint_line_spans;
    use ratatui::style::{Color, Style};
    use ratatui::text::Span;

    fn hl(text: &str, color: Color) -> Span<'static> {
        Span::styled(text.to_string(), Style::default().fg(color))
    }

    #[test]
    fn keeps_whole_span_when_no_selection_or_hscroll() {
        let out = paint_line_spans(
            &[hl("fn ", Color::Red), hl("main", Color::Blue)],
            0,
            0,
            None,
            Style::default(),
            None,
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].content, "fn ");
        assert_eq!(out[1].content, "main");
    }

    #[test]
    fn splits_only_at_selection_edges() {
        let sel = 3..7; // "main"
        let out = paint_line_spans(
            &[hl("fn main()", Color::White)],
            0,
            0,
            Some(&sel),
            Style::default().bg(Color::Blue),
            None,
        );
        let joined: String = out.iter().map(|s| s.content.clone().into_owned()).collect();
        assert_eq!(joined, "fn main()");
        assert!(out.len() >= 3, "unselected + selected + unselected");
        assert_eq!(out[1].content, "main");
    }

    #[test]
    fn hscroll_drops_prefix_without_per_char_spans() {
        let out = paint_line_spans(
            &[hl("abcdefghij", Color::White)],
            0,
            4,
            None,
            Style::default(),
            None,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].content, "efghij");
    }

    #[test]
    fn byte_clip_keeps_sub_row_run() {
        // line_start_byte=0, clip bytes 3..7 of "abcdefghij" => "defg" as one span
        let out = paint_line_spans(
            &[hl("abcdefghij", Color::White)],
            0,
            0,
            None,
            Style::default(),
            Some(3..7),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].content, "defg");
    }

    #[test]
    fn empty_highlight_emits_selection_space() {
        let sel = 0..1;
        let style = Style::default().bg(Color::Blue);
        let out = paint_line_spans(&[], 0, 0, Some(&sel), style, None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].content, " ");
        assert_eq!(out[0].style, style);
    }

    #[test]
    fn empty_content_span_still_emits_selection_space() {
        let sel = 0..1;
        let style = Style::default().bg(Color::Blue);
        let out = paint_line_spans(&[Span::raw("")], 0, 0, Some(&sel), style, None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].content, " ");
        assert_eq!(out[0].style, style);
    }

    #[test]
    fn empty_content_span_skips_selection_space_on_wrapped_continuation() {
        // Continuation rows: clip does not contain line_start_byte.
        let sel = 0..1;
        let style = Style::default().bg(Color::Blue);
        let out = paint_line_spans(&[Span::raw("")], 0, 0, Some(&sel), style, Some(1..5));
        assert!(out.is_empty());
    }

    #[test]
    fn empty_content_span_emits_when_clip_contains_line_start() {
        let sel = 0..1;
        let style = Style::default().bg(Color::Blue);
        let out = paint_line_spans(&[Span::raw("")], 0, 0, Some(&sel), style, Some(0..1));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].content, " ");
        assert_eq!(out[0].style, style);
    }
}
