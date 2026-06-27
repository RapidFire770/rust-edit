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
        let accent_bold_bg = Style::default()
            .bg(Color::from(&theme.accent))
            .fg(Color::from(&theme.header_fg))
            .add_modifier(Modifier::BOLD);

        let accent_fg = Style::default()
            .fg(Color::from(&theme.accent))
            .add_modifier(Modifier::BOLD);

        let dim_bold_fg = Style::default()
            .fg(Color::from(&theme.editor_fg))
            .add_modifier(Modifier::BOLD);

        let dim_underline = Style::default()
            .fg(Color::from(&theme.editor_fg))
            .add_modifier(Modifier::UNDERLINED);

        let dim = Style::default().fg(Color::from(&theme.editor_fg));

        let btn0_style = self.button_style(
            &self.buttons[0],
            &accent_bold_bg,
            &accent_fg,
            &dim_bold_fg,
            &dim_underline,
            &dim,
        );
        let btn1_style = self.button_style(
            &self.buttons[1],
            &accent_bold_bg,
            &accent_fg,
            &dim_bold_fg,
            &dim_underline,
            &dim,
        );

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
        accent_bold_bg: &'b Style,
        accent_fg: &'b Style,
        dim_bold_fg: &'b Style,
        dim_underline: &'b Style,
        dim: &'b Style,
    ) -> Style {
        match (button.state, button.is_primary) {
            (ButtonState::Selected, true) => *accent_bold_bg,
            (ButtonState::Selected, false) => *dim_bold_fg,
            (ButtonState::Hovered, true) => *accent_fg,
            (ButtonState::Hovered, false) => *dim_underline,
            (ButtonState::Normal, true) => *accent_fg,
            (ButtonState::Normal, false) => *dim,
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

    #[test]
    fn test_hit_test() {
        let group = ButtonGroup::new(["[ Save ]", "[ Cancel ]"], [true, false]);
        let widths = [9, 11];

        assert_eq!(group.hit_test(10, 15, widths), Some(0));
        assert_eq!(group.hit_test(10, 22, widths), Some(1));
        assert_eq!(group.hit_test(10, 5, widths), None);
    }
}
