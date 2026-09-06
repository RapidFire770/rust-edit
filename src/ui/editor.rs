// Main editing view, line numbers, cursor, selection, syntax spans. Sprint 1/2.
// Presentation only. Receives state from App/Buffer + highlighter.

#![allow(dead_code)]

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::buffer::Buffer;
use crate::theme::Theme;
use crate::ui::spans::paint_line_spans;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let buf = app.active_buffer();
    let theme = app.current_theme();
    // Single source of truth: `ui()` already applied the predicate and
    // shrank `visible_lines`. Recomputing from `buf.visible_lines` here
    // would double-subtract.
    let show_hbar = app.editor_hscrollbar_visible.get();

    let (main_area, hbar_area) = if show_hbar {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let inner = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(main_area);
    let content_area = inner[0];
    let vbar_area = inner[1];

    let visible_lines = content_area.height as usize;
    let cache = app.cached_visible_spans();
    let mut lines: Vec<Line<'static>> = if app.config.word_wrap {
        render_wrapped(buf, app, visible_lines, cache)
    } else {
        render_unwrapped(buf, app, visible_lines, cache)
    };
    while lines.len() < visible_lines {
        lines.push(Line::from(""));
    }

    f.render_widget(
        Paragraph::new(lines)
            .style(theme.editor_bg_style())
            .block(Block::default().borders(Borders::NONE)),
        content_area,
    );

    // vertical bar (always) — height is content rows, not including h-bar
    let vis = (vbar_area.height as usize).max(1);
    let (total_rows, scroll_pos) = if app.config.word_wrap {
        let total =
            crate::wrap::total_screen_rows(&buf.rope, buf.visible_width, app.config.tab_size);
        (total, buf.scroll_offset.0.min(total.saturating_sub(1)))
    } else {
        (buf.line_count(), buf.scroll_offset.0)
    };
    crate::ui::scrollbar::render_always(
        f,
        vbar_area,
        crate::ui::scrollbar::OverflowBar {
            total: total_rows,
            position: scroll_pos,
            viewport: vis,
        },
        theme,
    );

    if let Some(hbar_row) = hbar_area {
        // Full width, including the vertical-bar column, so `┘` meets `↓`
        // instead of leaving a dead corner hole on the right.
        f.render_widget(Paragraph::new("").style(theme.editor_bg_style()), hbar_row);
        crate::ui::scrollbar::render_horizontal(
            f,
            hbar_row,
            crate::ui::scrollbar::OverflowBar {
                // +1 so max_position == max_hscroll (extra EOL caret column).
                total: buf.max_line_width.saturating_add(1),
                position: buf.scroll_offset.1,
                viewport: buf.visible_width.max(1),
            },
            theme,
        );
    }
}

/// Line-number prefix for one screen row.
///
/// Shown: `"   12| "` (6 ASCII cells) on the first sub-row of a logical line,
/// or 6 spaces on wrap continuation rows so text columns line up.
/// Hidden: empty string so content starts at column 0. Never use U+2502
/// (`│`) — East-Asian-Width Ambiguous, desyncs the hardware cursor (AGENTS §1).
fn gutter_prefix(show: bool, line_1based: usize, continuation: bool) -> String {
    if !show {
        String::new()
    } else if continuation {
        " ".repeat(Buffer::GUTTER_WIDTH)
    } else {
        let s = format!("{:>4}| ", line_1based);
        debug_assert_eq!(s.chars().count(), Buffer::GUTTER_WIDTH);
        s
    }
}

/// Original 1-line-per-row render path. Unchanged from the pre-wrap version.
fn render_unwrapped(
    buf: &Buffer,
    app: &App,
    visible_lines: usize,
    cache: &crate::highlight_cache::HighlightCache,
) -> Vec<Line<'static>> {
    let theme = app.current_theme();
    let start_line = buf.scroll_offset.0;
    let n_lines = buf.line_count();

    let mut lines: Vec<Line<'static>> = Vec::new();
    let hscroll = buf.scroll_offset.1;
    for ii in 0..visible_lines.max(1) {
        let line_idx = start_line + ii;
        if line_idx >= n_lines {
            break;
        }
        // Gutter MUST be pure ASCII: `│` (U+2502) is East-Asian-Width Ambiguous
        // and renders as 2 cells in some terminals/fonts, which shifts content
        // right by 1 vs GUTTER_WIDTH and desyncs the hardware cursor. AGENTS §1.
        let gutter = gutter_prefix(app.config.show_line_numbers, line_idx + 1, false);
        let Some(hline) = cache.line_spans(line_idx) else {
            debug_assert!(false, "highlight cache must cover visible line {line_idx}");
            // still emit gutter so the row exists; do NOT Span::raw the whole line
            lines.push(Line::from(vec![Span::styled(
                gutter,
                theme.line_number_style(),
            )]));
            continue;
        };
        let line_start_byte = buf.rope.line_to_byte(line_idx);
        let mut spans = vec![Span::styled(gutter, theme.line_number_style())];
        spans.extend(paint_line_spans(
            hline,
            line_start_byte,
            hscroll,
            buf.selection.as_ref(),
            theme.selection_style(),
            None,
        ));
        lines.push(Line::from(spans));
    }
    lines
}

/// Wrapped render path. Emits one `Line` per screen row. The first sub-row of
/// each logical line carries the gutter; continuation rows carry 6 spaces.
/// Hscroll is ignored (wrap replaces horizontal scrolling).
fn render_wrapped(
    buf: &Buffer,
    app: &App,
    visible_lines: usize,
    cache: &crate::highlight_cache::HighlightCache,
) -> Vec<Line<'static>> {
    let theme = app.current_theme();
    let n_lines = buf.line_count();
    let width = buf.visible_width.max(1);
    let tab = app.config.tab_size;
    let scroll_screen_row = buf.scroll_offset.0; // under wrap, this is a SCREEN ROW

    // NOTE: full-buffer layout = O(buffer bytes) per frame. Acceptable for normal
    // editing; a 50k+ line file with many long lines may stutter. Future: window
    // the compute to the visible logical-line range (spec §9 Approach C).
    let full_layout = crate::wrap::compute_layout(&buf.rope, 0, n_lines, width, tab);
    if full_layout.total_screen_rows == 0 {
        return Vec::new();
    }

    // Prefix sum: prefix[lidx] = first global screen row of logical line `lidx`.
    let mut prefix: Vec<usize> = Vec::with_capacity(full_layout.breaks.len() + 1);
    prefix.push(0);
    let mut prev = 0usize;
    for line_breaks in &full_layout.breaks {
        prev += line_breaks.len() + 1;
        prefix.push(prev);
    }

    let start_line =
        crate::wrap::first_logical_line_for_screen_row(&full_layout, scroll_screen_row);

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(visible_lines);

    'outer: for (lidx, line_breaks) in full_layout.breaks.iter().enumerate().skip(start_line) {
        let rows_in_line = line_breaks.len() + 1;
        let line_start_byte = buf.rope.line_to_byte(lidx);
        let line_end_byte = if lidx + 1 < n_lines {
            buf.rope.line_to_byte(lidx + 1)
        } else {
            buf.rope.len_bytes()
        };

        // Sub-row byte boundaries. sub_starts[0] = line_start_byte; sub_starts[k]
        // = breaks[k-1]. sub_ends[k] = breaks[k] (or line_end_byte for the last).
        let mut sub_starts: Vec<usize> = Vec::with_capacity(rows_in_line);
        sub_starts.push(line_start_byte);
        for &brk in line_breaks.iter() {
            sub_starts.push(brk);
        }
        let mut sub_ends: Vec<usize> = line_breaks.clone();
        sub_ends.push(line_end_byte);

        let line_first_global_row = prefix[lidx];
        for sub in 0..rows_in_line {
            let global_row = line_first_global_row + sub;
            if global_row < scroll_screen_row {
                continue;
            }
            if lines.len() >= visible_lines {
                break 'outer;
            }
            let sub_start_byte = sub_starts[sub];
            let sub_end_byte = sub_ends[sub];
            let is_first_sub = sub == 0;
            let gutter = Span::styled(
                gutter_prefix(app.config.show_line_numbers, lidx + 1, !is_first_sub),
                if is_first_sub && app.config.show_line_numbers {
                    theme.line_number_style()
                } else {
                    Style::default()
                },
            );
            let Some(hline) = cache.line_spans(lidx) else {
                debug_assert!(false, "highlight cache must cover visible line {lidx}");
                lines.push(Line::from(vec![gutter]));
                continue;
            };
            let mut spans = vec![gutter];
            spans.extend(paint_line_spans(
                hline,
                line_start_byte,
                0,
                buf.selection.as_ref(),
                theme.selection_style(),
                Some(sub_start_byte..sub_end_byte),
            ));
            lines.push(Line::from(spans));
        }
    }

    lines
}

/// For future: accept precomputed highlights or scroll info for better decoupling.
pub fn render_with_highlights(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    highlighted: &[Vec<Span<'static>>],
    cursor_line: usize,
    cursor_col: usize,
    start_line: usize,
) {
    // Simplified version if needed later.
    let mut lines: Vec<Line> = Vec::new();
    for (i, hline) in highlighted.iter().enumerate() {
        let line_idx = start_line + i;
        // ASCII gutter — see note above (U+2502 width ambiguity shifts cursor).
        let gutter = gutter_prefix(true, line_idx + 1, false);
        let mut spans = vec![Span::styled(gutter, theme.line_number_style())];
        let is_cursor = line_idx == cursor_line;
        let mut char_count = 0usize;
        for sp in hline {
            let txt = sp.content.clone();
            if is_cursor {
                let mut chars: Vec<char> = txt.chars().collect();
                for (j, ch) in chars.drain(..).enumerate() {
                    if char_count + j == cursor_col {
                        spans.push(Span::styled(
                            ch.to_string(),
                            sp.style.patch(theme.selection_style()),
                        ));
                    } else {
                        spans.push(Span::styled(ch.to_string(), sp.style));
                    }
                }
                char_count += txt.chars().count();
            } else {
                spans.push(sp.clone());
            }
        }
        lines.push(Line::from(spans));
    }
    while lines.len() < area.height as usize {
        lines.push(Line::from(""));
    }
    let editor = Paragraph::new(lines)
        .style(theme.editor_bg_style())
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(editor, area);
}
