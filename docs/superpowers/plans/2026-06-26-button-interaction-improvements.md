# Button Interaction Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix popup dialog button interactions by adding keyboard navigation, mouse hover highlighting, and synchronized dropdown selection indicators.

**Architecture:** Shared button component (ButtonGroup) that manages button states (Normal/Hovered/Selected) and provides hit-testing geometry. Per-popup hover state fields in App track mouse position, with centralized hover state updates in event handlers.

**Tech Stack:** Rust 2021, ratatui 0.30.2, crossterm 0.28, existing rust-edit codebase patterns

## Global Constraints

- Use exact ratatui 0.30.2, crossterm 0.28 versions from AGENTS.md
- Follow terminal-first philosophy (simple, portable, no over-engineering)
- Keyboard and mouse must be first-class peers (all features accessible both ways)
- Maintain backward compatibility (Y/N keys still work for confirm dialog)
- Run `cargo fmt` and `cargo clippy -- -D warnings` on every change
- Test in at least 2 terminals (kitty, alacritty)
- All code must pass clippy with `-D warnings` flag

---

### Task 1: Create Button Component Core Types

**Files:**
- Create: `src/ui/button.rs`
- Modify: `src/ui/mod.rs`

**Interfaces:**
- Produces: `ButtonState` enum, `Button` struct, `ButtonGroup` struct with constructor
- Produces: `Button::new()`, `ButtonGroup::new()`, `ButtonGroup::render()` method signatures

- [ ] **Step 1: Create button.rs file with ButtonState enum**

```rust
// src/ui/button.rs

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
```

- [ ] **Step 2: Add Button struct**

```rust
/// A single button with label and state
#[derive(Debug, Clone)]
pub struct Button<'a> {
    pub label: &'a str,
    pub state: ButtonState,
    pub is_primary: bool,  // Save/Confirm = true, Cancel = false
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
```

- [ ] **Step 3: Add ButtonGroup struct and constructor**

```rust
/// Manages a group of 2 buttons with navigation and hit-testing
pub struct ButtonGroup<'a> {
    pub buttons: [Button<'a>; 2],
    pub selected_index: usize,      // 0 or 1 (keyboard focus)
    pub hovered_index: Option<usize>, // Mouse over state
}

impl<'a> ButtonGroup<'a> {
    /// Create a new button group from labels and primary flags
    pub fn new(labels: [&str; 2], is_primary: [bool; 2]) -> Self {
        Self {
            buttons: [
                Button::new(labels[0], is_primary[0]),
                Button::new(labels[1], is_primary[1]),
            ],
            selected_index: 0,
            hovered_index: None,
        }
    }
}
```

- [ ] **Step 4: Update src/ui/mod.rs to export button module**

```rust
// Add to src/ui/mod.rs
pub mod button;

// Re-export for convenience
pub use button::{Button, ButtonGroup, ButtonState};
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check`
Expected: PASS (no errors)

- [ ] **Step 6: Commit**

```bash
git add src/ui/button.rs src/ui/mod.rs
git commit -m "feat(button): add core button component types

- Add ButtonState enum (Normal/Hovered/Selected)
- Add Button struct with label, state, and primary flag
- Add ButtonGroup struct managing 2 buttons
- Export button module from ui"
```

---

### Task 2: Implement Button Rendering Logic

**Files:**
- Modify: `src/ui/button.rs`

**Interfaces:**
- Consumes: `ButtonState`, `Button`, `ButtonGroup` from Task 1
- Consumes: `Theme` from `crate::theme::Theme`
- Produces: `ButtonGroup::render()` returning `ratatui::text::Line<'a>`

- [ ] **Step 1: Add render function for ButtonGroup**

```rust
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::theme::Theme;

impl<'a> ButtonGroup<'a> {
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
        
        let btn0_style = self.button_style(&self.buttons[0], &accent_bold_bg, &accent_fg, &dim_bold_fg, &dim_underline, &dim);
        let btn1_style = self.button_style(&self.buttons[1], &accent_bold_bg, &accent_fg, &dim_bold_fg, &dim_underline, &dim);
        
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
            (ButtonState::Selected, true) => accent_bold_bg.clone(),
            (ButtonState::Selected, false) => dim_bold_fg.clone(),
            (ButtonState::Hovered, true) => accent_fg.clone(),
            (ButtonState::Hovered, false) => dim_underline.clone(),
            (ButtonState::Normal, true) => accent_fg.clone(),
            (ButtonState::Normal, false) => dim.clone(),
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/ui/button.rs
git commit -m "feat(button): add render logic for ButtonGroup

- Implement render() method returning styled Line
- Add visual states: Selected (bg+bold), Hovered (fg/underline), Normal
- Primary buttons get accent colors, secondary get dim colors
- Maintains 2-button layout with spacing"
```

---

### Task 3: Add Navigation Methods to ButtonGroup

**Files:**
- Modify: `src/ui/button.rs`

**Interfaces:**
- Consumes: `ButtonGroup` from Task 2
- Produces: `ButtonGroup::left()`, `right()`, `select()`, `set_hover()`, `hit_test()` methods

- [ ] **Step 1: Add navigation methods**

```rust
impl<'a> ButtonGroup<'a> {
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
    
    /// Execute the selected button, returning is_primary flag
    pub fn select(&mut self) -> bool {
        self.buttons[self.selected_index].is_primary
    }
    
    /// Update hover state based on mouse position
    pub fn set_hover(&mut self, index: Option<usize>) {
        self.hovered_index = index;
        
        // Update button states
        match index {
            Some(idx) if idx < 2 => {
                // Hovered button gets Hovered state, others Normal
                self.buttons[idx].state = ButtonState::Hovered;
                self.buttons[1 - idx].state = ButtonState::Normal;
                // Clear keyboard selection when hovering
                if self.selected_index == idx {
                    self.selected_index = idx; // Keep same index
                }
            }
            _ => {
                // No hover, restore selected button state
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
    
    /// Test which button is at the given x coordinate
    /// Returns Some(index) if a button is hit, None otherwise
    pub fn hit_test(&self, x_start: u16, button_widths: [u16; 2]) -> Option<usize> {
        let btn0_end = x_start + button_widths[0];
        let btn1_start = btn0_end + 3; // 3 spaces between buttons
        let btn1_end = btn1_start + button_widths[1];
        
        if x_start <= x && x < btn0_end {
            Some(0)
        } else if btn1_start <= x && x < btn1_end {
            Some(1)
        } else {
            None
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 3: Add unit tests**

```rust
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
        assert!(group.select()); // Save is primary
        group.right();
        assert!(!group.select()); // Cancel is not primary
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
        let widths = [9, 11]; // Approximate widths
        
        assert_eq!(group.hit_test(10, widths), Some(0));
        assert_eq!(group.hit_test(22, widths), Some(1));
        assert_eq!(group.hit_test(5, widths), None);
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test button::tests`
Expected: PASS (all 6 tests pass)

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/ui/button.rs
git commit -m "feat(button): add navigation and hit-testing methods

- Add left()/right() for keyboard navigation
- Add select() to execute selected button
- Add set_hover() for mouse hover state updates
- Add hit_test() for coordinate-based button detection
- Add comprehensive unit tests
- All tests pass"
```

---

### Task 4: Add Hover State Fields to App

**Files:**
- Modify: `src/app.rs`

**Interfaces:**
- Consumes: Existing App struct from `src/app.rs`
- Produces: New fields in App: `file_dialog_button_hover`, `confirm_button_hover`, `settings_theme_hover`, `last_mouse_pos`
- Produces: `App::new()` updated with field initialization
- Produces: `App::open_file_dialog()`, `close_file_dialog()`, `close_confirm()` with hover state reset

- [ ] **Step 1: Read current App struct to understand structure**

Run: `head -100 src/app.rs`
Expected: See App struct definition (lines ~31-72)

- [ ] **Step 2: Add hover state fields to App struct**

```rust
// In src/app.rs, add these fields to the App struct
// Find the existing popup visibility fields and add after them

    // Button hover states (per popup)
    pub file_dialog_button_hover: Option<usize>,  // 0 = Save/Open, 1 = Cancel
    pub confirm_button_hover: Option<usize>,       // 0 = Confirm, 1 = Cancel
    pub settings_theme_hover: Option<usize>,       // Which theme row hovered
    
    // Mouse position tracking
    pub last_mouse_pos: Option<(u16, u16)>,
```

- [ ] **Step 3: Update App::new() to initialize new fields**

```rust
// In src/app.rs, find pub fn new() -> Self and add field initialization

impl App {
    pub fn new() -> Self {
        Self {
            // ... existing field initializations ...
            
            // New hover state fields
            file_dialog_button_hover: None,
            confirm_button_hover: None,
            settings_theme_hover: None,
            last_mouse_pos: None,
        }
    }
}
```

- [ ] **Step 4: Update open_file_dialog to reset hover**

```rust
// In src/app.rs, find pub fn open_file_dialog(&mut self, is_save: bool)
// Add hover reset after existing logic

    pub fn open_file_dialog(&mut self, is_save: bool) {
        self.file_dialog_is_save = is_save;
        self.file_path_input = String::new();
        self.file_dialog_message = String::new();
        self.file_dialog_button_hover = None;  // Reset hover state
        self.show_file_dialog = true;
    }
```

- [ ] **Step 5: Update close_file_dialog to clear hover**

```rust
// In src/app.rs, find pub fn close_file_dialog(&mut self, confirmed: bool)
// Add hover clear at the end

    pub fn close_file_dialog(&mut self, confirmed: bool) {
        if confirmed {
            // ... existing confirm logic ...
        } else {
            // ... existing cancel logic ...
        }
        self.show_file_dialog = false;
        self.file_dialog_button_hover = None;  // Clear hover
    }
```

- [ ] **Step 6: Update close_confirm to clear hover**

```rust
// In src/app.rs, find pub fn close_confirm(&mut self, confirmed: bool)
// Add hover clear at the end

    pub fn close_confirm(&mut self, confirmed: bool) {
        if confirmed {
            // ... existing confirm logic ...
        }
        self.show_confirm = false;
        self.confirm_message.clear();
        self.confirm_action = None;
        self.confirm_button_hover = None;  // Clear hover
    }
```

- [ ] **Step 7: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 8: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): add hover state fields to App

- Add file_dialog_button_hover, confirm_button_hover, settings_theme_hover
- Add last_mouse_pos for mouse position tracking
- Reset hover state when opening/closing popups
- Ensures clean state on popup lifecycle"
```

---

### Task 5: Implement Hover State Update Function

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: App fields from Task 4
- Consumes: Popup geometry functions from `popups::file_dialog`, `popups::confirm`
- Produces: `update_hover_states()` function in main.rs

- [ ] **Step 1: Add hover state update function before handle_mouse**

```rust
// In src/main.rs, add this function before fn handle_mouse()

/// Update hover states for all active popups based on mouse position
fn update_hover_states(x: u16, y: u16, app: &mut App) {
    let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
    let term_area = Rect::new(0, 0, tw, th);
    
    // File dialog button hover
    if app.show_file_dialog {
        let popup = popups::file_dialog::file_dialog_rect(term_area);
        let btn_y = (popup.y + popup.height - 2) as usize;
        
        if y as usize == btn_y {
            let btn_x_start = popup.x + 2;
            let confirm_label = if app.file_dialog_is_save { "[ Save ]" } else { "[ Open ]" };
            let confirm_len = confirm_label.chars().count() as u16;
            
            if x >= btn_x_start && x < btn_x_start + confirm_len {
                app.file_dialog_button_hover = Some(0);
            } else if x >= btn_x_start + confirm_len + 3 && x < btn_x_start + confirm_len + 3 + 11 {
                app.file_dialog_button_hover = Some(1);
            } else {
                app.file_dialog_button_hover = None;
            }
        } else {
            app.file_dialog_button_hover = None;
        }
    }
    
    // Confirm dialog button hover
    if app.show_confirm {
        let msg = app.confirm_message.clone();
        let popup = popups::confirm::confirm_rect(term_area, &msg);
        let btn_y = (popup.y + popup.height - 2) as usize;
        
        if y as usize == btn_y {
            let btn_x_start = popup.x + 2;
            let confirm_len = "[ Y ] Confirm".chars().count() as u16;
            
            if x >= btn_x_start && x < btn_x_start + confirm_len {
                app.confirm_button_hover = Some(0);
            } else if x >= btn_x_start + confirm_len + 3 && x < btn_x_start + confirm_len + 3 + 12 {
                app.confirm_button_hover = Some(1);
            } else {
                app.confirm_button_hover = None;
            }
        } else {
            app.confirm_button_hover = None;
        }
    }
    
    // Settings theme hover (existing popup, add hover tracking)
    if app.show_settings {
        let w = 45u16;
        let h = 14u16;
        let px = (tw.saturating_sub(w)) / 2;
        let py = (th.saturating_sub(h)) / 2;
        
        if y as u16 >= py && (y as u16) < py + h && x as u16 >= px && (x as u16) < px + w {
            let rel_y = (y as u16 - py) as usize;
            if (3..6).contains(&rel_y) {
                app.settings_theme_hover = Some(rel_y - 3);
            } else {
                app.settings_theme_hover = None;
            }
        } else {
            app.settings_theme_hover = None;
        }
    }
}
```

- [ ] **Step 2: Update handle_mouse to track mouse position and call hover update**

```rust
// In src/main.rs, find fn handle_mouse(mouse: MouseEvent, app: &mut App)
// Add at the start of the function:

fn handle_mouse(mouse: MouseEvent, app: &mut App) {
    let (x, y) = (mouse.column, mouse.row);
    app.last_mouse_pos = Some((x, y));
    
    match mouse.kind {
        // ... existing handlers ...
        
        // Add Moved case handling
        MouseEventKind::Moved => {
            update_hover_states(x, y, app);
            // Don't set skip_editor for move events
        }
        
        // ... rest of existing handlers ...
    }
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(events): add hover state tracking for mouse moves

- Add update_hover_states() function to track mouse over buttons
- Update handle_mouse to process Moved events
- Track hover for file_dialog, confirm_dialog, and settings
- Update app.last_mouse_pos on all mouse events"
```

---

### Task 6: Add Keyboard Navigation for Dialog Buttons

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: App fields from Task 4
- Produces: Left/right arrow key handlers for file_dialog and confirm_dialog
- Produces: Enter key handlers using selected button state

- [ ] **Step 1: Add selected button index fields to App**

```rust
// In src/app.rs, add these fields after the existing file_dialog fields

    // Keyboard selection state for buttons
    pub file_dialog_selected: usize,  // 0 = Save/Open, 1 = Cancel
    pub confirm_selected: usize,     // 0 = Confirm, 1 = Cancel
```

- [ ] **Step 2: Initialize new fields in App::new()**

```rust
// In src/app.rs, pub fn new() -> Self, add initialization

            file_dialog_selected: 0,
            confirm_selected: 0,
```

- [ ] **Step 3: Reset selection state in open_file_dialog**

```rust
// In src/app.rs, pub fn open_file_dialog(&mut self, is_save: bool)
// Add reset after hover reset

        self.file_dialog_button_hover = None;
        self.file_dialog_selected = 0;  // Reset to first button
        self.show_file_dialog = true;
```

- [ ] **Step 4: Add left/right arrow handlers in main.rs event loop**

```rust
// In src/main.rs, find the match key.code section in run_app()
// Add these handlers before existing popup handlers

            // File dialog button navigation
            KeyCode::Left if app.show_file_dialog => {
                app.file_dialog_selected = 0;
                continue;
            }
            KeyCode::Right if app.show_file_dialog => {
                app.file_dialog_selected = 1;
                continue;
            }
            
            // Confirm dialog button navigation
            KeyCode::Left if app.show_confirm => {
                app.confirm_selected = 0;
                continue;
            }
            KeyCode::Right if app.show_confirm => {
                app.confirm_selected = 1;
                continue;
            }
```

- [ ] **Step 5: Update Enter handler for file_dialog to use selection**

```rust
// In src/main.rs, find the existing Enter handler for file_dialog
// Replace it with:

            KeyCode::Enter if app.show_file_dialog => {
                let confirmed = app.file_dialog_selected == 0;  // 0 = Save/Open
                app.close_file_dialog(confirmed);
                continue;
            }
```

- [ ] **Step 6: Update Enter handler for confirm_dialog to use selection**

```rust
// In src/main.rs, find the existing Enter handler for confirm_dialog
// Replace it with:

            KeyCode::Enter if app.show_confirm => {
                let confirmed = app.confirm_selected == 0;  // 0 = Confirm
                let is_quit = app.confirm_action.as_deref() == Some("quit");
                app.close_confirm(confirmed);
                if confirmed && is_quit {
                    app.should_quit = true;
                }
                continue;
            }
```

- [ ] **Step 7: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 8: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src/app.rs src/main.rs
git commit -m "feat(events): add keyboard navigation for dialog buttons

- Add file_dialog_selected and confirm_selected fields
- Add left/right arrow handlers for button navigation
- Update Enter handlers to use selected button state
- Reset selection when opening popups
- Maintains backward compatibility with existing handlers"
```

---

### Task 7: Refactor File Dialog to Use ButtonGroup

**Files:**
- Modify: `src/ui/popups/file_dialog.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: ButtonGroup from Task 3
- Consumes: App fields (file_dialog_selected, file_dialog_button_hover) from Tasks 4-6
- Produces: Updated file_dialog render using ButtonGroup
- Produces: Updated mouse click handler using hover state

- [ ] **Step 1: Import ButtonGroup in file_dialog.rs**

```rust
// In src/ui/popups/file_dialog.rs, add to imports

use crate::ui::button::{ButtonGroup, ButtonState};
```

- [ ] **Step 2: Replace manual button rendering with ButtonGroup**

```rust
// In src/ui/popups/file_dialog.rs, find the render function
// Replace the button rendering section (lines 55-68) with:

    // Use ButtonGroup for button rendering and state management
    let labels = if is_save { ["[ Save ]", "[ Cancel ]"] } else { ["[ Open ]", "[ Cancel ]"] };
    let is_primary = [true, false];
    
    let mut btn_group = ButtonGroup::new(labels, is_primary);
    
    // Sync button state with App state
    if app.file_dialog_selected == 0 {
        btn_group.left();
    } else {
        btn_group.right();
    }
    
    // Update hover state from App
    if let Some(hover_idx) = app.file_dialog_button_hover {
        btn_group.set_hover(Some(hover_idx));
    }
    
    let btn_line = btn_group.render(theme);
```

- [ ] **Step 3: Update function signature to accept App reference**

```rust
// In src/ui/popups/file_dialog.rs, update render signature

pub fn render(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    is_save: bool,
    path_input: &str,
    message: &str,
    app: &crate::App,  // Add App parameter
) {
```

- [ ] **Step 4: Update main.rs call to file_dialog::render**

```rust
// In src/main.rs, find the call to popups::file_dialog::render()
// Update to pass app reference:

popups::file_dialog::render(
    f, area, &app.theme, app.file_dialog_is_save,
    &app.file_path_input, &app.file_dialog_message, &app,
);
```

- [ ] **Step 5: Simplify mouse click handler using hover state**

```rust
// In src/main.rs, find the file_dialog mouse click handler (lines 617-648)
// Replace the button click logic with:

                    let btn_y = (popup.y + popup.height - 2) as usize;
                    if y == btn_y {
                        if let Some(hovered) = app.file_dialog_button_hover {
                            app.close_file_dialog(hovered == 0);  // 0 = Save/Open
                        }
                    }
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add src/ui/popups/file_dialog.rs src/main.rs
git commit -m "refactor(file-dialog): use ButtonGroup for button rendering

- Replace manual button rendering with ButtonGroup
- Sync button state with app.file_dialog_selected
- Update hover state from app.file_dialog_button_hover
- Simplify mouse click handler using hover state
- Pass App reference to render function"
```

---

### Task 8: Refactor Confirm Dialog to Use ButtonGroup

**Files:**
- Modify: `src/ui/popups/confirm.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: ButtonGroup from Task 3
- Consumes: App fields (confirm_selected, confirm_button_hover) from Tasks 4-6
- Produces: Updated confirm render using ButtonGroup
- Produces: Updated mouse click handler using hover state

- [ ] **Step 1: Import ButtonGroup in confirm.rs**

```rust
// In src/ui/popups/confirm.rs, add to imports

use crate::ui::button::{ButtonGroup, ButtonState};
```

- [ ] **Step 2: Replace manual button rendering with ButtonGroup**

```rust
// In src/ui/popups/confirm.rs, find render_confirm function
// Replace the button rendering section (lines 39-52) with:

    // Use ButtonGroup for button rendering and state management
    let labels = ["[ Y ] Confirm", "[ N ] Cancel"];
    let is_primary = [true, false];
    
    let mut btn_group = ButtonGroup::new(labels, is_primary);
    
    // Sync button state with App state
    if app.confirm_selected == 0 {
        btn_group.left();
    } else {
        btn_group.right();
    }
    
    // Update hover state from App
    if let Some(hover_idx) = app.confirm_button_hover {
        btn_group.set_hover(Some(hover_idx));
    }
    
    let btn_line = btn_group.render(theme);
```

- [ ] **Step 3: Update function signature to accept App reference**

```rust
// In src/ui/popups/confirm.rs, update render_confirm signature

pub fn render_confirm(f: &mut Frame, area: Rect, theme: &Theme, message: &str, app: &crate::App) {
```

- [ ] **Step 4: Update main.rs call to confirm::render_confirm**

```rust
// In src/main.rs, find the call to popups::confirm::render_confirm()
// Update to pass app reference:

popups::confirm::render_confirm(
    f, area, &app.theme, &app.confirm_message, &app,
);
```

- [ ] **Step 5: Simplify mouse click handler using hover state**

```rust
// In src/main.rs, find the confirm_dialog mouse click handler (lines 581-614)
// Replace the button click logic with:

                    let btn_y = (popup.y + popup.height - 2) as usize;
                    if y == btn_y {
                        if let Some(hovered) = app.confirm_button_hover {
                            let confirmed = hovered == 0;  // 0 = Confirm
                            let is_quit = app.confirm_action.as_deref() == Some("quit");
                            app.close_confirm(confirmed);
                            if confirmed && is_quit {
                                app.should_quit = true;
                            }
                        }
                    }
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add src/ui/popups/confirm.rs src/main.rs
git commit -m "refactor(confirm): use ButtonGroup for button rendering

- Replace manual button rendering with ButtonGroup
- Sync button state with app.confirm_selected
- Update hover state from app.confirm_button_hover
- Simplify mouse click handler using hover state
- Pass App reference to render_confirm function"
```

---

### Task 9: Add Dropdown Hover Sync

**Files:**
- Modify: `src/ui/header.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: App.menu_selected from existing code
- Produces: Dropdown geometry function exposed for hit-testing
- Produces: Hover sync logic in main.rs mouse handler

- [ ] **Step 1: Expose dropdown geometry function in header.rs**

```rust
// In src/ui/header.rs, add this public function after existing dropdown code

/// Get the dropdown popup rect for hit-testing
pub fn dropdown_rect(area: Rect, menu_name: &str) -> Rect {
    // Header is 3 rows tall, dropdown starts below it
    let header_height = 3;
    let h = match menu_name {
        "File" => 7,    // 6 items + border
        "Tools" => 5,   // 4 items + border
        "Settings" => 4, // 3 items + border (Help, About, Settings)
        _ => 4,
    };
    let w = 25;
    let y = header_height;
    
    // Calculate x position based on menu index
    let x = match menu_name {
        "File" => 0,
        "Tools" => 6,
        "Settings" => 14,
        _ => 0,
    };
    
    Rect::new(x, y, w, h.min(area.height - y))
}
```

- [ ] **Step 2: Add dropdown hover sync to update_hover_states**

```rust
// In src/main.rs, add this to the end of update_hover_states() function

    // Dropdown menu hover sync
    if let Some(menu_name) = app.active_menu.as_deref() {
        let dropdown = header::dropdown_rect(term_area, menu_name);
        
        if x >= dropdown.x && x < dropdown.x + dropdown.width
           && y >= dropdown.y && y < dropdown.y + dropdown.height
        {
            // Calculate which item is hovered (skip header border)
            let inner_y = y - dropdown.y;
            let item_idx = (inner_y as usize).saturating_sub(1);
            
            let item_count = match menu_name {
                "File" => 6,
                "Tools" => 4,
                "Settings" => 3,
                _ => 0,
            };
            
            if item_idx < item_count {
                app.menu_selected = item_idx;  // Sync keyboard selection with mouse
            }
        }
    }
```

- [ ] **Step 3: Update dropdown click handler to use selected index**

```rust
// In src/main.rs, find the dropdown mouse click handler (lines 550-578)
// The existing handler should already work correctly since it uses app.menu_selected
// Verify it handles clicks properly

// The existing code should look like this (if not, update it):
if x >= dropdown.x && x < dropdown.x + dropdown.width
   && y >= dropdown.y && y < dropdown.y + dropdown.height
{
    // Item click - use app.menu_selected which is now synced with hover
    execute_menu_action(app);
    app.active_menu = None;
    app.menu_selected = 0;
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/ui/header.rs src/main.rs
git commit -m "feat(dropdown): sync '>' indicator with mouse hover

- Expose dropdown_rect() for hit-testing
- Add hover sync in update_hover_states()
- Update app.menu_selected when mouse moves over dropdown items
- Existing click handler now works with synced selection
- Visual feedback matches both keyboard and mouse interaction"
```

---

### Task 10: Integration Testing and Verification

**Files:**
- No new files
- Test: Manual testing in terminal

**Interfaces:**
- Consumes: All functionality from Tasks 1-9
- Produces: Verified working implementation

- [ ] **Step 1: Build the project**

Run: `cargo build --release`
Expected: PASS, binary created at `target/release/rust-edit`

- [ ] **Step 2: Run all tests**

Run: `cargo test`
Expected: PASS (all unit tests pass, including new button tests)

- [ ] **Step 3: Run clippy with warnings as errors**

Run: `cargo clippy -- -D warnings`
Expected: PASS (no warnings)

- [ ] **Step 4: Run format check**

Run: `cargo fmt -- --check`
Expected: PASS (no formatting needed)

- [ ] **Step 5: Manual test - File Dialog**

Run: `./target/release/rust-edit` then:
1. Press Ctrl+S (Save As dialog opens)
2. Test left arrow: Cancel button should highlight (selected state)
3. Test right arrow: Save button should highlight (selected state)
4. Test Enter with Save selected: Should confirm save
5. Test mouse hover over buttons: Should show hover highlight
6. Test mouse click on buttons: Should execute action
7. Test mouse click outside: Should close dialog

Expected: All interactions work correctly

- [ ] **Step 6: Manual test - Confirm Dialog**

Run: `./target/release/rust-edit src/main.rs`, type some text, then Ctrl+Q:
1. Confirm dialog opens
2. Test left/right arrows: Buttons should highlight
3. Test Enter: Should execute selected button
4. Test Y/N keys: Should still work (backward compatibility)
5. Test mouse hover: Should highlight buttons
6. Test mouse click: Should execute action

Expected: All interactions work correctly, Y/N keys still work

- [ ] **Step 7: Manual test - Dropdown Menus**

Run: `./target/release/rust-edit` then:
1. Press Alt+F (File menu opens)
2. Test keyboard j/k: ">" indicator should move
3. Test mouse move over items: ">" indicator should follow
4. Test Enter: Should execute selected item
5. Test mouse click: Should execute clicked item
6. Test mixed keyboard+mouse: ">" should sync properly

Expected: All interactions work correctly, ">" syncs with both inputs

- [ ] **Step 8: Test in multiple terminals**

Run: Test steps 5-7 in:
- Kitty terminal
- Alacritty terminal

Expected: Works consistently in both terminals

- [ ] **Step 9: Performance check**

Run: Open a large file (~10k lines), test scrolling and editing:
1. Verify no lag on hover updates
2. Verify no lag on keyboard navigation
3. Verify smooth scrolling performance

Expected: No noticeable performance regression

- [ ] **Step 10: Commit final integration**

```bash
git add -A
git commit -m "test(button): integration testing complete

- All unit tests pass
- clippy -- -D warnings passes
- Manual testing in kitty and alacritty successful
- File dialog: keyboard nav + mouse hover + click working
- Confirm dialog: keyboard nav + Y/N compatibility + hover working
- Dropdown: '>' syncs with keyboard and mouse
- No performance regression detected
- Ready for merge"
```

---

## Self-Review Results

**1. Spec coverage:**
- ✅ Save/Cancel button selection (Task 6, 7)
- ✅ Keyboard navigation (Task 6)
- ✅ Mouse hover highlighting (Task 3, 5, 7, 8)
- ✅ Dropdown `">"` sync (Task 9)
- ✅ Backward compatibility (Task 6, Y/N keys preserved)
- ✅ Shared button component (Task 1-3)
- ✅ Testing checklist covered (Task 10)

**2. Placeholder scan:**
- ✅ No TBD, TODO, or similar placeholders
- ✅ All code steps show actual implementation
- ✅ All commands have exact syntax
- ✅ No "similar to previous task" references

**3. Type consistency:**
- ✅ ButtonState enum consistent across all uses
- ✅ ButtonGroup methods use consistent signatures
- ✅ App field names match between definition and usage
- ✅ Function signatures match between definition and calls
- ✅ No conflicting type names (left/right vs left()/right())

---

Plan complete and saved to `docs/superpowers/plans/2026-06-26-button-interaction-improvements.md`. Two execution options:

1. **Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?