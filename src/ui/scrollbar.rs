// Shared overflow scrollbar (vertical + horizontal) for the editor and popups.
// Presentation helper: overflow math + themed Ratatui Scrollbar.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::theme::Theme;

/// Overflow scrollbar. `position` is the first visible row (vertical) or
/// column (horizontal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverflowBar {
    pub total: usize,
    pub position: usize,
    pub viewport: usize,
}

impl OverflowBar {
    pub fn overflowing(&self) -> bool {
        self.total > self.viewport && self.viewport > 0
    }

    /// Ratatui `ScrollbarState::new` content length — same formula as the editor.
    pub fn content_length(&self) -> usize {
        self.total.saturating_sub(self.viewport).saturating_add(1)
    }

    pub fn max_position(&self) -> usize {
        self.total.saturating_sub(self.viewport)
    }

    pub fn clamp_position(&self, pos: usize) -> usize {
        pos.min(self.max_position())
    }

    /// Map a screen row `y` on `area` to a top-row / index. `None` if `y`
    /// is outside the track. Skips the ↑/↓ end cells, matching
    /// `scrollbar_target_for_row` in `main.rs`.
    pub fn target_for_row(&self, y: u16, area: Rect) -> Option<usize> {
        if !area_contains_y(area, y) {
            return None;
        }
        let max = self.max_position();
        if max == 0 {
            return Some(0);
        }
        let vis = (area.height as usize).max(1);
        let track = vis.saturating_sub(2).max(1);
        let rel = (y.saturating_sub(area.y.saturating_add(1))) as usize;
        let rel = rel.min(track);
        let frac = rel as f64 / track as f64;
        Some((frac * max as f64).round() as usize)
    }

    /// Map a screen column `x` on `area` to a left-col. `None` if `x` is
    /// outside the track. Skips the ←/→ end cells.
    // Editor mouse wiring lands in a later task.
    #[allow(dead_code)]
    pub fn target_for_col(&self, x: u16, area: Rect) -> Option<usize> {
        if !area_contains_x(area, x) {
            return None;
        }
        let max = self.max_position();
        if max == 0 {
            return Some(0);
        }
        let vis = (area.width as usize).max(1);
        let track = vis.saturating_sub(2).max(1);
        let rel = (x.saturating_sub(area.x.saturating_add(1))) as usize;
        let rel = rel.min(track);
        let frac = rel as f64 / track as f64;
        Some((frac * max as f64).round() as usize)
    }
}

// Editor layout wiring lands in a later task.
#[allow(dead_code)]
pub fn needs_horizontal_scrollbar(
    word_wrap: bool,
    max_line_width: usize,
    content_w: usize,
    editor_rows: usize,
) -> bool {
    !word_wrap && max_line_width > content_w && editor_rows > 1
}

/// Paint the bar only when content overflows the viewport.
pub fn render(f: &mut Frame, area: Rect, bar: OverflowBar, theme: &Theme) {
    render_ex(f, area, bar, theme, false);
}

/// Paint the bar even when content fits (editor track is always visible).
pub fn render_always(f: &mut Frame, area: Rect, bar: OverflowBar, theme: &Theme) {
    render_ex(f, area, bar, theme, true);
}

fn render_ex(f: &mut Frame, area: Rect, bar: OverflowBar, theme: &Theme, always: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if !always && !bar.overflowing() {
        return;
    }
    let vis = (area.height as usize).max(1);
    let mut state = ScrollbarState::new(bar.content_length())
        .position(bar.clamp_position(bar.position))
        .viewport_content_length(vis);
    let fg = Style::default().fg(Color::from(&theme.editor_fg));
    let accent = Style::default().fg(Color::from(&theme.accent));
    let widget = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"))
        .track_symbol(Some("│"))
        .thumb_symbol("█")
        .begin_style(fg)
        .end_style(fg)
        .track_style(fg)
        .thumb_style(accent);
    f.render_stateful_widget(widget, area, &mut state);
}

fn area_contains_y(area: Rect, y: u16) -> bool {
    y >= area.y && y < area.y.saturating_add(area.height)
}

// Called from `target_for_col`; unused until that path is wired.
#[allow(dead_code)]
fn area_contains_x(area: Rect, x: u16) -> bool {
    x >= area.x && x < area.x.saturating_add(area.width)
}

// Editor paint wiring lands in a later task.
#[allow(dead_code)]
pub fn render_horizontal(f: &mut Frame, area: Rect, bar: OverflowBar, theme: &Theme) {
    if area.width == 0 || area.height == 0 || !bar.overflowing() {
        return;
    }
    let vis = (area.width as usize).max(1);
    let mut state = ScrollbarState::new(bar.content_length())
        .position(bar.clamp_position(bar.position))
        .viewport_content_length(vis);
    let fg = Style::default().fg(Color::from(&theme.editor_fg));
    let accent = Style::default().fg(Color::from(&theme.accent));
    let widget = Scrollbar::default()
        .orientation(ScrollbarOrientation::HorizontalBottom)
        .begin_symbol(Some("←"))
        .end_symbol(Some("→"))
        .track_symbol(Some("─"))
        .thumb_symbol("█")
        .begin_style(fg)
        .end_style(fg)
        .track_style(fg)
        .thumb_style(accent);
    f.render_stateful_widget(widget, area, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflowing_only_when_total_exceeds_viewport() {
        assert!(!OverflowBar {
            total: 10,
            position: 0,
            viewport: 10
        }
        .overflowing());
        assert!(OverflowBar {
            total: 11,
            position: 0,
            viewport: 10
        }
        .overflowing());
        assert!(!OverflowBar {
            total: 11,
            position: 0,
            viewport: 0
        }
        .overflowing());
    }

    #[test]
    fn clamp_position_caps_at_max() {
        let bar = OverflowBar {
            total: 30,
            position: 0,
            viewport: 10,
        };
        assert_eq!(bar.max_position(), 20);
        assert_eq!(bar.clamp_position(99), 20);
        assert_eq!(bar.clamp_position(0), 0);
    }

    #[test]
    fn target_for_row_skips_end_caps_and_maps_proportionally() {
        let bar = OverflowBar {
            total: 100,
            position: 0,
            viewport: 10,
        };
        let area = Rect::new(10, 5, 1, 10); // y=5..15, vis=10
        assert_eq!(bar.target_for_row(4, area), None); // above
        assert_eq!(bar.target_for_row(15, area), None); // below
        assert_eq!(bar.target_for_row(5, area), Some(0)); // ↑ cell → top
        let bottom = bar.target_for_row(14, area).unwrap();
        assert_eq!(bottom, bar.max_position());
    }

    #[test]
    fn target_for_row_none_when_area_empty() {
        let bar = OverflowBar {
            total: 10,
            position: 0,
            viewport: 5,
        };
        assert_eq!(bar.target_for_row(0, Rect::new(0, 0, 1, 0)), None);
    }

    #[test]
    fn needs_horizontal_scrollbar_rules() {
        assert!(!needs_horizontal_scrollbar(true, 200, 40, 20)); // wrap on
        assert!(!needs_horizontal_scrollbar(false, 40, 40, 20)); // fits exactly
        assert!(!needs_horizontal_scrollbar(false, 80, 40, 1)); // would leave 0 content rows
        assert!(needs_horizontal_scrollbar(false, 41, 40, 20));
    }

    #[test]
    fn target_for_col_skips_end_caps_and_maps_proportionally() {
        let bar = OverflowBar {
            total: 100,
            position: 0,
            viewport: 10,
        };
        let area = Rect::new(5, 10, 10, 1); // x=5..15
        assert_eq!(bar.target_for_col(4, area), None); // left of bar
        assert_eq!(bar.target_for_col(15, area), None); // right of bar
        assert_eq!(bar.target_for_col(5, area), Some(0)); // ← cell → start
        let right = bar.target_for_col(14, area).unwrap();
        assert_eq!(right, bar.max_position());
    }
}
