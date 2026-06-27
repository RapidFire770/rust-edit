# Mouse Interaction Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix mouse hover/click offset in popup buttons and unify button styling so both primary and non-primary buttons use accent color on hover/select.

**Architecture:** Add helper functions to each popup module that compute the button row from the paragraph line count + block border offset. Update mouse handlers to use these helpers. Simplify the button styling to remove the primary/non-primary distinction.

**Tech Stack:** Rust, Ratatui 0.30+, Crossterm 0.28

## Global Constraints

- Edition 2021. `cargo fmt` and `cargo clippy -- -D warnings` must pass.
- All colors and styles come from the current `Theme`. Never hard-code `Color::Red` etc. outside the default theme definition.
- One popup = one file in `popups/`.
- Keep `ui/` focused on presentation only. Business logic lives in `app.rs` or dedicated modules.

---

### Task 1: Add button row helper to confirm popup

**Files:**
- Modify: `src/ui/popups/confirm.rs:12-20`

**Interfaces:**
- Consumes: `ratatui::layout::Rect`
- Produces: `pub fn confirm_button_row(popup: Rect) -> u16`

**Context:** The confirm popup renders a paragraph with 6 lines (indices 0–5). The button is at line index 5. The paragraph is rendered inside a Block with `Borders::ALL`, which adds a 1-cell border offset.

**Steps:**

- [ ] **Step 1: Add the helper function**

Add this function to `src/ui/popups/confirm.rs` after the existing `confirm_rect` function (around line 21):

```rust
/// Returns the screen y-coordinate of the button row in the confirm popup.
/// The paragraph has 6 lines (indices 0–5), button is at index 5.
/// The Block border adds +1 to the y offset.
pub fn confirm_button_row(popup: Rect) -> u16 {
    popup.y + 1 + 5
}
```

- [ ] **Step 2: Verify the file compiles**

Run: `cargo check`

Expected: No errors. The new function should compile cleanly.

- [ ] **Step 3: Commit**

```bash
git add src/ui/popups/confirm.rs
git commit -m "feat(confirm): add confirm_button_row helper for accurate mouse hit-testing"
```

---

### Task 2: Add button row helper to file dialog popup

**Files:**
- Modify: `src/ui/popups/file_dialog.rs:12-21`

**Interfaces:**
- Consumes: `ratatui::layout::Rect`
- Produces: `pub fn file_dialog_button_row(popup: Rect) -> u16`

**Context:** The file dialog popup renders a paragraph with 7 lines (indices 0–6). The button is at line index 6. The paragraph is rendered inside a Block with `Borders::ALL`, which adds a 1-cell border offset.

**Steps:**

- [ ] **Step 1: Add the helper function**

Add this function to `src/ui/popups/file_dialog.rs` after the existing `file_dialog_rect` function (around line 22):

```rust
/// Returns the screen y-coordinate of the button row in the file dialog popup.
/// The paragraph has 7 lines (indices 0–6), button is at index 6.
/// The Block border adds +1 to the y offset.
pub fn file_dialog_button_row(popup: Rect) -> u16 {
    popup.y + 1 + 6
}
```

- [ ] **Step 2: Verify the file compiles**

Run: `cargo check`

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add src/ui/popups/file_dialog.rs
git commit -m "feat(file-dialog): add file_dialog_button_row helper for accurate mouse hit-testing"
```

---

### Task 3: Update mouse handlers to use button row helpers

**Files:**
- Modify: `src/main.rs:568-660` (update_hover_states function)
- Modify: `src/main.rs:704-731` (confirm popup click handler)
- Modify: `src/main.rs:733-754` (file dialog click handler)

**Interfaces:**
- Consumes: `popups::file_dialog::file_dialog_button_row(popup)`, `popups::confirm::confirm_button_row(popup)`
- Produces: Updated mouse hover and click handling with correct button y-positions

**Context:** The current code uses `popup.y + popup.height - 2` to calculate the button row. This needs to be replaced with calls to the new helper functions.

**Steps:**

- [ ] **Step 1: Update update_hover_states for file dialog**

In `src/main.rs`, find the file dialog hover section (around line 573-596) and replace:

```rust
let btn_y = (popup.y + popup.height - 2) as usize;
```

with:

```rust
let btn_y = popups::file_dialog::file_dialog_button_row(popup) as usize;
```

The full section should look like:

```rust
if app.show_file_dialog {
    let popup = popups::file_dialog::file_dialog_rect(term_area);
    let btn_y = popups::file_dialog::file_dialog_button_row(popup) as usize;

    if y as usize == btn_y {
        let btn_x_start = popup.x + 2;
        let confirm_label = if app.file_dialog_is_save {
            "[ Save ]"
        } else {
            "[ Open ]"
        };
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
```

- [ ] **Step 2: Update update_hover_states for confirm popup**

In `src/main.rs`, find the confirm popup hover section (around line 598-617) and replace:

```rust
let btn_y = (popup.y + popup.height - 2) as usize;
```

with:

```rust
let btn_y = popups::confirm::confirm_button_row(popup) as usize;
```

The full section should look like:

```rust
if app.show_confirm {
    let msg = app.confirm_message.clone();
    let popup = popups::confirm::confirm_rect(term_area, &msg);
    let btn_y = popups::confirm::confirm_button_row(popup) as usize;

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
```

- [ ] **Step 3: Update confirm popup click handler**

In `src/main.rs`, find the confirm popup click section (around line 715) and replace:

```rust
let btn_y = (popup.y + popup.height - 2) as usize;
```

with:

```rust
let btn_y = popups::confirm::confirm_button_row(popup) as usize;
```

- [ ] **Step 4: Update file dialog click handler**

In `src/main.rs`, find the file dialog click section (around line 743) and replace:

```rust
let btn_y = (popup.y + popup.height - 2) as usize;
```

with:

```rust
let btn_y = popups::file_dialog::file_dialog_button_row(popup) as usize;
```

- [ ] **Step 5: Verify the file compiles**

Run: `cargo check`

Expected: No errors. The mouse handlers should now use the correct button y-positions.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "fix(mouse): use button row helpers for accurate hover and click detection"
```

---

### Task 4: Unify button styling (Option A)

**Files:**
- Modify: `src/ui/button.rs:58-120` (render and button_style functions)

**Interfaces:**
- Consumes: `ButtonState`, `Theme`
- Produces: Unified styling where both primary and non-primary buttons use accent color on hover/select

**Context:** The current `button_style` function distinguishes between primary and non-primary buttons. Per Option A, both should use the same style: accent color text on hover/select, no background fill.

**Steps:**

- [ ] **Step 1: Simplify button_style function**

Replace the `button_style` function in `src/ui/button.rs` (lines 103-120) with:

```rust
fn button_style<'b>(
    &self,
    _button: &Button<'a>,
    accent_bold: &'b Style,  // accent fg + bold (used for both hover and select)
    dim: &'b Style,          // default fg (used when no hover/select)
) -> Style {
    match self.buttons[0].state {
        ButtonState::Selected | ButtonState::Hovered => *accent_bold,
        ButtonState::Normal => *dim,
    }
}
```

Wait, that's wrong. Let me look at the actual function signature again. The function takes `&self` and `button: &Button<'a>`. Let me fix:

```rust
fn button_style<'b>(
    &self,
    button: &Button<'a>,
    accent_bold: &'b Style,  // accent fg + bold (used for both hover and select)
    dim: &'b Style,          // default fg (used when no hover/select)
) -> Style {
    match button.state {
        ButtonState::Selected | ButtonState::Hovered => *accent_bold,
        ButtonState::Normal => *dim,
    }
}
```

- [ ] **Step 2: Update render function to pass only 3 style arguments**

In the `render` function (lines 58-101), the current code passes 6 style arguments to `button_style`. Replace with 3:

```rust
let accent_bold = Style::default()
    .fg(Color::from(&theme.accent))
    .add_modifier(Modifier::BOLD);

let dim = Style::default().fg(Color::from(&theme.editor_fg));

let btn0_style = self.button_style(
    &self.buttons[0],
    &accent_bold,
    &dim,
);
let btn1_style = self.button_style(
    &self.buttons[1],
    &accent_bold,
    &dim,
);
```

Remove the `accent_fg`, `dim_bold_fg`, `dim_underline` style definitions since they're no longer needed.

The full `render` function should look like:

```rust
pub fn render(&self, theme: &Theme) -> Line<'a> {
    let accent_bold = Style::default()
        .fg(Color::from(&theme.accent))
        .add_modifier(Modifier::BOLD);

    let dim = Style::default().fg(Color::from(&theme.editor_fg));

    let btn0_style = self.button_style(
        &self.buttons[0],
        &accent_bold,
        &dim,
    );
    let btn1_style = self.button_style(
        &self.buttons[1],
        &accent_bold,
        &dim,
    );

    Line::from(vec![
        Span::raw("  "),
        Span::styled(self.buttons[0].label, btn0_style),
        Span::raw("   "),
        Span::styled(self.buttons[1].label, btn1_style),
    ])
}
```

- [ ] **Step 3: Keep is_primary field but mark as unused (optional)**

The `is_primary` field in the `Button` struct is no longer used by `button_style`, but removing it would require updating all call sites. For now, keep it but add an allow directive:

```rust
#[derive(Debug, Clone)]
pub struct Button<'a> {
    pub label: &'a str,
    pub state: ButtonState,
    #[allow(dead_code)]
    pub is_primary: bool,
}
```

- [ ] **Step 4: Verify the file compiles and tests pass**

Run: `cargo test`

Expected: All tests pass. The existing tests should still pass since they don't test the visual output, only the state transitions.

- [ ] **Step 5: Commit**

```bash
git add src/ui/button.rs
git commit -m "refactor(button): unify styling for primary and non-primary buttons (Option A)"
```

---

### Task 5: Run full test suite and manual verification

**Files:**
- Run: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`
- Manual testing per checklist

**Steps:**

- [ ] **Step 1: Run automated checks**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

Expected: All pass with no warnings.

- [ ] **Step 2: Manual testing — Confirm popup**

Build and run the app:
```bash
cargo run --release
```

Test:
- Open a file with unsaved changes
- Trigger the confirm dialog (e.g., try to quit)
- Hover over [Y] Confirm → should highlight with accent color
- Hover over [N] Cancel → should highlight with accent color
- Click on [Y] Confirm → should confirm action
- Click on [N] Cancel → should cancel
- Verify hover/click works when mouse is directly on the button text (not below)
- Test keyboard navigation (Left/Right arrows) still works
- Test Enter key still activates the selected button

- [ ] **Step 3: Manual testing — File dialog**

Test:
- Open the file dialog (Ctrl+O or Ctrl+Shift+S)
- Hover over [Save]/[Open] → should highlight with accent color
- Hover over [Cancel] → should highlight with accent color
- Click on [Save]/[Open] → should trigger save/open
- Click on [Cancel] → should cancel
- Verify hover/click works when mouse is directly on the button text (not below)
- Test keyboard navigation (Left/Right arrows) still works
- Test Enter key still activates the selected button

- [ ] **Step 4: Test in multiple terminals**

Repeat steps 2-3 in at least one other terminal emulator (e.g., if you tested in kitty, also test in alacritty or gnome-terminal).

- [ ] **Step 5: Commit any final adjustments**

If manual testing reveals issues, fix them and commit:

```bash
git add -A
git commit -m "fix: adjust button styling/hit-testing based on manual testing"
```

---

## Self-Review Checklist

**Spec coverage:**
- [x] Fix button y-position miscalculation → Tasks 1-3
- [x] Unify button styling (Option A) → Task 4
- [x] Testing checklist → Task 5

**Placeholder scan:**
- [x] No "TBD", "TODO", or "implement later"
- [x] No "similar to Task N" references
- [x] All code snippets are complete

**Type consistency:**
- [x] `confirm_button_row` and `file_dialog_button_row` both return `u16`
- [x] Mouse handlers cast to `usize` consistently
- [x] `button_style` signature updated consistently in both `render` and the function definition

**Edge cases:**
- [x] What if paragraph line counts change in the future? → The constants (5, 6) are documented with comments linking them to the line counts. If line counts change, update the constant and the comment.
- [x] What if the Block border style changes? → The +1 offset is hardcoded based on `Borders::ALL`. If the border style changes, update the constant.

---

Plan complete and saved to `docs/superpowers/plans/2026-06-26-mouse-interaction-fixes.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
