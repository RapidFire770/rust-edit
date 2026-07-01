// Button component for dialog popups. Provides consistent button rendering,
// hover states, and keyboard navigation support.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::Theme;

/// State of a button for visual feedback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// Default appearance
    Normal,
    /// Mouse over but not keyboard-selected
    Hovered,
    /// Keyboard-selected via arrows
    Selected,
}

/// A single button with label and state
#[derive(Debug, Clone)]
pub struct Button<'a> {
    pub label: &'a str,
    pub state: ButtonState,
    #[allow(dead_code)]
    pub is_primary: bool,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str, is_primary: bool) -> Self {
        Self {
            label,
            state: ButtonState::Normal,
            is_primary,
        }
    }
}

/// Manages a group of 2 buttons with navigation and hit-testing
pub struct ButtonGroup<'a> {
    pub buttons: [Button<'a>; 2],
    pub selected_index: usize,
    pub hovered_index: Option<usize>,
}

impl<'a> ButtonGroup<'a> {
    pub fn new(labels: [&'a str; 2], is_primary: [bool; 2]) -> Self {
        Self {
            buttons: [
                Button::new(labels[0], is_primary[0]),
                Button::new(labels[1], is_primary[1]),
            ],
            selected_index: 0,
            hovered_index: None,
        }
    }

    /// Render the button group as a single line with styled buttons
    pub fn render(&self, theme: &Theme) -> Line<'a> {
        let accent_bold = Style::default()
            .fg(Color::from(&theme.accent))
            .add_modifier(Modifier::BOLD);

        let dim = Style::default().fg(Color::from(&theme.editor_fg));

        let btn0_style = self.button_style(&self.buttons[0], &accent_bold, &dim);
        let btn1_style = self.button_style(&self.buttons[1], &accent_bold, &dim);

        Line::from(vec![
            Span::raw("  "),
            Span::styled(self.buttons[0].label, btn0_style),
            Span::raw("   "),
            Span::styled(self.buttons[1].label, btn1_style),
        ])
    }

    fn button_style<'b>(
        &self,
        button: &Button<'a>,
        accent_bold: &'b Style,
        dim: &'b Style,
    ) -> Style {
        match button.state {
            ButtonState::Selected | ButtonState::Hovered => *accent_bold,
            ButtonState::Normal => *dim,
        }
    }

    /// Select the left button (index 0)
    pub fn left(&mut self) {
        self.selected_index = 0;
        self.buttons[0].state = ButtonState::Selected;
        self.buttons[1].state = ButtonState::Normal;
    }

    /// Select the right button (index 1)
    pub fn right(&mut self) {
        self.selected_index = 1;
        self.buttons[0].state = ButtonState::Normal;
        self.buttons[1].state = ButtonState::Selected;
    }

    #[allow(dead_code)]
    /// Execute the selected button, returning is_primary flag
    pub fn select(&mut self) -> bool {
        self.buttons[self.selected_index].is_primary
    }

    /// Update hover state based on mouse position
    pub fn set_hover(&mut self, index: Option<usize>) {
        self.hovered_index = index;

        match index {
            Some(idx) if idx < 2 => {
                self.buttons[idx].state = ButtonState::Hovered;
                self.buttons[1 - idx].state = ButtonState::Normal;
                if self.selected_index == idx {
                    self.selected_index = idx;
                }
            }
            _ => {
                self.buttons[0].state = if self.selected_index == 0 {
                    ButtonState::Selected
                } else {
                    ButtonState::Normal
                };
                self.buttons[1].state = if self.selected_index == 1 {
                    ButtonState::Selected
                } else {
                    ButtonState::Normal
                };
            }
        }
    }

    #[allow(dead_code)]
    /// Test which button is at the given x coordinate
    pub fn hit_test(&self, x_start: u16, x: u16, button_widths: [u16; 2]) -> Option<usize> {
        let btn0_end = x_start + button_widths[0];
        let btn1_start = btn0_end + 3;
        let btn1_end = btn1_start + button_widths[1];

        if x >= x_start && x < btn0_end {
            Some(0)
        } else if x >= btn1_start && x < btn1_end {
            Some(1)
        } else {
            None
        }
    }
}

/// A button group of arbitrary length. Used by popups that need three or more
/// buttons (e.g. the file browser's `[ Up ] [ Recent ] [ Hidden ] [ Cancel ]`).
///
/// The existing two-button [`ButtonGroup`] is kept untouched so `confirm.rs`
/// and its tests remain stable.
pub struct ButtonRow<'a> {
    pub buttons: Vec<Button<'a>>,
    pub selected_index: usize,
    pub hovered_index: Option<usize>,
}

impl<'a> ButtonRow<'a> {
    /// Caller owns the slice lifetimes; internal `Vec` copies the `Button`s.
    pub fn new(labels: &[&'a str], is_primary: &[bool]) -> Self {
        assert!(
            labels.len() == is_primary.len() && !labels.is_empty(),
            "ButtonRow needs matching-length, non-empty label/is_primary slices"
        );
        let mut buttons: Vec<Button<'a>> = labels
            .iter()
            .zip(is_primary.iter())
            .map(|(l, &p)| Button::new(l, p))
            .collect();
        buttons[0].state = ButtonState::Selected;
        Self {
            buttons,
            selected_index: 0,
            hovered_index: None,
        }
    }

    /// Compute the `(start_x, end_x)` screen column range of each button,
    /// assuming the row starts at `x_start` and buttons are separated by the
    /// same spacing [`ButtonRow::render`] uses (2 leading spaces, 3 between).
    /// Used by mouse hit-testing so hover and click agree with the renderer.
    pub fn button_x_ranges(x_start: u16, labels: &[&str]) -> Vec<(u16, u16)> {
        let mut ranges = Vec::with_capacity(labels.len());
        let mut cursor = x_start.saturating_add(2); // leading "  "
        for (i, label) in labels.iter().enumerate() {
            if i > 0 {
                cursor = cursor.saturating_add(3); // gap "   "
            }
            let len = label.chars().count() as u16;
            ranges.push((cursor, cursor.saturating_add(len)));
            cursor = cursor.saturating_add(len);
        }
        ranges
    }

    /// Move the selection by `delta` positions, wrapping at both ends. Updates
    /// each button's `state` so a single `render` call reflects the change.
    /// (Currently exercised only by tests — kept as part of the public API for
    /// future popups that prefer arrow-key cycling over explicit `select`.)
    #[allow(dead_code)]
    pub fn move_cursor(&mut self, delta: i32) {
        let n = self.buttons.len() as i32;
        // Euclidean modulo so negative deltas wrap correctly.
        let new_idx = ((self.selected_index as i32 + delta) % n + n) % n;
        self.select(Some(new_idx as usize));
    }

    /// Select an explicit index. Hover state is preserved independently.
    pub fn select(&mut self, idx: Option<usize>) {
        self.selected_index = idx.unwrap_or(self.selected_index);
        for (i, b) in self.buttons.iter_mut().enumerate() {
            b.state = if i == self.selected_index {
                ButtonState::Selected
            } else {
                ButtonState::Normal
            };
        }
        // Re-apply hover last so it wins over selection visuals.
        if let Some(h) = self.hovered_index {
            if h < self.buttons.len() {
                self.buttons[h].state = ButtonState::Hovered;
            }
        }
    }

    /// Update hover state from a mouse position (or `None` when the cursor
    /// leaves all buttons). Selection is preserved. While a hover is active
    /// only the hovered button is highlighted (matching `ButtonGroup`).
    pub fn set_hover(&mut self, idx: Option<usize>) {
        self.hovered_index = idx;
        for (i, b) in self.buttons.iter_mut().enumerate() {
            b.state = match (idx, i == self.selected_index) {
                (Some(h), _) if h == i => ButtonState::Hovered,
                (Some(_), _) => ButtonState::Normal,
                (_, true) => ButtonState::Selected,
                _ => ButtonState::Normal,
            };
        }
    }

    /// Render the row as a single `Line` with two leading spaces and three
    /// spaces between each button. Selected/hovered buttons use accent+bold;
    /// the rest use the dim editor foreground.
    pub fn render(&self, theme: &Theme) -> Line<'a> {
        let accent_bold = Style::default()
            .fg(Color::from(&theme.accent))
            .add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(Color::from(&theme.editor_fg));

        let mut spans: Vec<Span<'a>> = Vec::with_capacity(self.buttons.len() * 2 + 1);
        spans.push(Span::raw("  "));
        for (i, b) in self.buttons.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("   "));
            }
            let style = match b.state {
                ButtonState::Selected | ButtonState::Hovered => accent_bold,
                ButtonState::Normal => dim,
            };
            spans.push(Span::styled(b.label, style));
        }
        Line::from(spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_group_creation() {
        let group = ButtonGroup::new(["Save", "Cancel"], [true, false]);
        assert_eq!(group.selected_index, 0);
        assert_eq!(group.hovered_index, None);
        assert_eq!(group.buttons[0].label, "Save");
        assert_eq!(group.buttons[1].label, "Cancel");
    }

    #[test]
    fn test_left_navigation() {
        let mut group = ButtonGroup::new(["Save", "Cancel"], [true, false]);
        group.right();
        group.left();
        assert_eq!(group.selected_index, 0);
        assert_eq!(group.buttons[0].state, ButtonState::Selected);
        assert_eq!(group.buttons[1].state, ButtonState::Normal);
    }

    #[test]
    fn test_right_navigation() {
        let mut group = ButtonGroup::new(["Save", "Cancel"], [true, false]);
        group.right();
        assert_eq!(group.selected_index, 1);
        assert_eq!(group.buttons[0].state, ButtonState::Normal);
        assert_eq!(group.buttons[1].state, ButtonState::Selected);
    }

    #[test]
    fn test_select_returns_primary() {
        let mut group = ButtonGroup::new(["Save", "Cancel"], [true, false]);
        assert!(group.select());
        group.right();
        assert!(!group.select());
    }

    #[test]
    fn test_hover_state() {
        let mut group = ButtonGroup::new(["Save", "Cancel"], [true, false]);
        group.set_hover(Some(0));
        assert_eq!(group.hovered_index, Some(0));
        assert_eq!(group.buttons[0].state, ButtonState::Hovered);
        assert_eq!(group.buttons[1].state, ButtonState::Normal);

        group.set_hover(None);
        assert_eq!(group.hovered_index, None);
        assert_eq!(group.buttons[0].state, ButtonState::Selected);
    }
}

#[cfg(test)]
mod button_row_tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn button_row_creation_and_default_selection() {
        let row = ButtonRow::new(&["A", "B", "C"], &[true, false, false]);
        assert_eq!(row.buttons.len(), 3);
        assert_eq!(row.selected_index, 0);
        assert_eq!(row.hovered_index, None);
        assert_eq!(row.buttons[0].state, ButtonState::Selected);
        assert_eq!(row.buttons[1].state, ButtonState::Normal);
    }

    #[test]
    fn button_row_move_left_right_wraps() {
        let mut row = ButtonRow::new(&["A", "B", "C"], &[true, false, false]);
        row.move_cursor(1);
        assert_eq!(row.selected_index, 1);
        row.move_cursor(1);
        assert_eq!(row.selected_index, 2);
        // Wrap past the end.
        row.move_cursor(1);
        assert_eq!(row.selected_index, 0);
        // Wrap before the start.
        row.move_cursor(-1);
        assert_eq!(row.selected_index, 2);
    }

    #[test]
    fn button_row_set_hover_clears_others() {
        let mut row = ButtonRow::new(&["A", "B", "C"], &[true, false, false]);
        row.set_hover(Some(2));
        assert_eq!(row.hovered_index, Some(2));
        assert_eq!(row.buttons[2].state, ButtonState::Hovered);
        // Non-hovered, non-selected -> Normal.
        assert_eq!(row.buttons[0].state, ButtonState::Normal);
        assert_eq!(row.buttons[1].state, ButtonState::Normal);

        row.set_hover(None);
        assert_eq!(row.buttons[0].state, ButtonState::Selected);
    }

    #[test]
    fn button_row_render_smoke() {
        // Just confirm it produces N styled spans + separators without panic.
        let mut row = ButtonRow::new(&["[ A ]", "[ B ]", "[ C ]"], &[true, false, false]);
        row.set_hover(Some(1));
        let _line = row.render(&Theme::dark_red());
    }
}
