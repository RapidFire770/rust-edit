# Mouse Interaction Fixes — Design Spec

**Date:** 2026-06-26  
**Status:** Draft — awaiting approval  
**Scope:** Fix mouse hover/click offset in popup buttons + unify button styling

---

## Problem Statement

Two related bugs in the popup button interactions:

1. **Hover/click offset:** The mouse must be positioned *below* a button (1–2 rows outside the button area) to trigger hover highlighting or click activation. The button hit-test zone is miscalculated.

2. **Inconsistent Cancel button styling:** The Save/Open (primary) buttons get accent-colored highlighting, but the Cancel (non-primary) button uses a different, less visible style (underlined/bold). Per user preference, both buttons should use the same accent-color-on-hover/select style with no background fill.

---

## Root Cause Analysis

### Issue 1: Button Y-Position Miscalculation

**Files involved:**
- `src/main.rs` — `update_hover_states()` (line 569), click handlers (lines 715, 743)
- `src/ui/popups/confirm.rs` — `render_confirm()` paragraph line count
- `src/ui/popups/file_dialog.rs` — `render()` paragraph line count

**The bug:**

Both popups render buttons inside a `Block` with `Borders::ALL`. The `Block` occupies the full popup rect, and the `Paragraph` widget renders into the Block's *inner* area, which is offset by `(+1, +1)` from the Block's top-left corner.

The current hover/click code calculates the button row as:
```rust
let btn_y = (popup.y + popup.height - 2) as usize;
```

This formula assumes the button sits 2 rows above the popup bottom, but doesn't account for the Block's top border offset. The actual button y-position depends on the paragraph's line count:

| Popup | Paragraph lines | Button line index | Block border offset | **Actual button y** | **Current calc** | **Delta** |
|-------|----------------|-------------------|--------------------|--------------------|--------------------|-----------|
| Confirm | 6 (indices 0–5) | 5 | +1 | `popup.y + 6` | `popup.y + 5` | −1 (too high) |
| File dialog | 7 (indices 0–6) | 6 | +1 | `popup.y + 7` | `popup.y + 8` | +1 (too low) |

The user reports having to click *below* the button, which matches: the detection zone is shifted downward relative to where the button actually renders.

### Issue 2: Styling Mismatch

**File involved:** `src/ui/button.rs` — `button_style()` (line 103)

The current code distinguishes primary vs. non-primary buttons:

```rust
match (button.state, button.is_primary) {
    (ButtonState::Selected, true) => *accent_bold_bg,  // red bg + bold
    (ButtonState::Selected, false) => *dim_bold_fg,     // bold only
    (ButtonState::Hovered, true) => *accent_fg,         // red text + bold
    (ButtonState::Hovered, false) => *dim_underline,    // underlined
    ...
}
```

Per Option A (user choice): both buttons use the same style — accent color text on hover/select, no background fill.

---

## Proposed Fix

### Fix 1: Correct Button Y-Position

**Approach:** Compute the button y-position from the actual paragraph line count + block border offset, rather than from the popup height.

**Implementation:**

Add a helper function to each popup module that returns the button row offset relative to the popup top:

```rust
// In confirm.rs
pub fn confirm_button_row(popup: Rect) -> u16 {
    // Block border (+1) + last paragraph line index (5 for confirm)
    popup.y + 1 + 5
}

// In file_dialog.rs
pub fn file_dialog_button_row(popup: Rect) -> u16 {
    // Block border (+1) + last paragraph line index (6 for file dialog)
    popup.y + 1 + 6
}
```

Then update `update_hover_states` and the click handlers to use these helpers instead of the hardcoded formula.

**Why this approach:**
- Mirrors the actual render logic (line count + block offset)
- Self-documenting: the magic numbers (5, 6) match the paragraph line counts in the render functions
- Easy to maintain: if line counts change, the helper stays correct as long as the constant is updated

**Alternative considered:** Calculate button y from `popup.height` minus the number of non-button rows (including borders). This is more flexible but adds complexity for minimal benefit since the line counts are fixed per popup.

### Fix 2: Unify Button Styling (Option A)

**Implementation:**

Simplify `button_style()` in `button.rs` to remove the primary/non-primary distinction:

```rust
fn button_style<'b>(
    &self,
    _button: &Button<'a>,
    accent_bold: &'b Style,  // accent fg + bold (used for both hover and select)
    dim: &'b Style,          // default fg (used when no hover/select)
) -> Style {
    match button.state {
        ButtonState::Selected | ButtonState::Hovered => *accent_bold,
        ButtonState::Normal => *dim,
    }
}
```

Remove the `is_primary` field from `Button` (or keep it but don't use it in styling). Update call sites that pass `is_primary` to still pass the value (for potential future use) but ignore it in styling.

**Why this approach:**
- Simpler code, fewer style branches
- Consistent visual feedback across all buttons
- Easier to reason about: hover/select always looks the same

**Tradeoff:** The keyboard-selected button previously had a distinct "Selected" vs "Hovered" visual (Selected = filled bg, Hovered = outlined). Per Option A, both now look identical. This is acceptable because:
- Mouse users see hover feedback
- Keyboard users see the same accent color
- The distinction between "mouse over" and "keyboard focused" is less critical in popup dialogs where focus is binary (popup is open/closed)

---

## Files to Modify

1. **`src/ui/popups/confirm.rs`**
   - Add `confirm_button_row(popup: Rect) -> u16` helper
   - No render changes needed

2. **`src/ui/popups/file_dialog.rs`**
   - Add `file_dialog_button_row(popup: Rect) -> u16` helper
   - No render changes needed

3. **`src/main.rs`**
   - Update `update_hover_states()` to use the new helpers
   - Update confirm click handler (line ~715) to use the new helper
   - Update file dialog click handler (line ~743) to use the new helper

4. **`src/ui/button.rs`**
   - Simplify `button_style()` to remove primary/non-primary distinction
   - Keep `is_primary` field for API compatibility (optional)

---

## Testing Checklist

After implementation, verify:

- [ ] Confirm popup: hover over [Y] Confirm → accent color highlight
- [ ] Confirm popup: hover over [N] Cancel → accent color highlight
- [ ] Confirm popup: click on [Y] Confirm → confirms action
- [ ] Confirm popup: click on [N] Cancel → cancels action
- [ ] Confirm popup: hover/click works when mouse is directly on the button text (not below)
- [ ] File dialog: hover over [Save]/[Open] → accent color highlight
- [ ] File dialog: hover over [Cancel] → accent color highlight
- [ ] File dialog: click on [Save]/[Open] → triggers save/open
- [ ] File dialog: click on [Cancel] → cancels
- [ ] File dialog: hover/click works when mouse is directly on the button text (not below)
- [ ] Keyboard navigation (Left/Right arrows) still works for button selection
- [ ] Enter key still activates the selected button
- [ ] Both popups work in at least 2 terminal emulators (kitty, alacritty, etc.)

---

## Risks & Mitigations

- **Risk:** Hardcoding the line index constants (5, 6) in the helpers could drift if paragraph line counts change.
  - **Mitigation:** Add a comment linking the constant to the paragraph line count. Consider extracting to a named constant (e.g., `CONFIRM_BUTTON_LINE_INDEX = 5`).

- **Risk:** Removing the primary/non-primary styling distinction might reduce visual hierarchy.
  - **Mitigation:** Option A was explicitly chosen by the user. The accent color on hover/select provides sufficient feedback. If needed in the future, a subtle "Selected = filled, Hovered = outlined" distinction can be re-added.

---

## Implementation Order

1. Fix button y-position helpers in `confirm.rs` and `file_dialog.rs`
2. Update mouse handlers in `main.rs` to use the helpers
3. Simplify `button_style()` in `button.rs`
4. Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test`
5. Manual testing per the checklist above
