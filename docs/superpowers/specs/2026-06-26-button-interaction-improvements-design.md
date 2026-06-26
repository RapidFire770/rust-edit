# Button Interaction Improvements Design Specification

**Date:** 2026-06-26  
**Status:** Approved  
**Priority:** High  
**Phase:** 1.5 (UI Polish - Post-MVP fixes)

---

## Problem Statement

Three critical UI issues prevent proper interaction with popup dialogs and menus:

1. **Save As Dialog buttons cannot be selected** - Neither mouse clicks nor keyboard input work for Save/Cancel buttons
2. **No keyboard navigation for dialog buttons** - Left/right arrow keys don't work to navigate between Save/Open and Cancel options
3. **No hover feedback** - Buttons and menu items don't highlight on mouse hover, reducing user feedback
4. **Dropdown selection indicator out of sync** - The `">"` indicator only follows keyboard navigation, not mouse hover

These issues make the editor feel unresponsive and break the "keyboard and mouse are first-class peers" principle from AGENTS.md.

---

## Goals

1. Make all dialog buttons (Save/Cancel, Confirm/Cancel) selectable via keyboard (left/right arrows + Enter)
2. Fix mouse click handling for all dialog buttons
3. Add hover highlighting for all buttons and menu items
4. Sync dropdown `">"` indicator with mouse hover position
5. Maintain backward compatibility (Y/N keys still work for confirm dialog)
6. Keep implementation minimal and focused (no over-engineering)

---

## Design Approach: Shared Button Component

We will introduce a reusable button system that fixes all issues while improving code quality through DRY principles.

### Option Selection

We evaluated three approaches:
- **Option A:** Targeted fixes only (minimal scope, but leads to code duplication)
- **Option B:** Shared button component (balanced scope, reusable, clean architecture) ✅ **CHOSEN**
- **Option C:** Full UI component system (overkill for current needs)

**Rationale for Option B:**
- Fixes all reported issues
- Reusable for future popups (Phase 2 features)
- Consistent button behavior everywhere
- Follows existing `confirm_button_rects()` pattern
- Balanced complexity (3-4 days estimated)
- Maintains terminal-first philosophy

---

## Architecture

### New Components

```
src/ui/
└── button.rs                 # NEW FILE
    ├── ButtonState (enum)
    ├── Button (struct)
    └── ButtonGroup (struct)
```

### Modified Files

```
src/
├── app.rs                    # Add hover state fields
├── main.rs                   # Add hover tracking, left/right nav, dropdown sync
├── ui/
│   ├── popups/
│   │   ├── file_dialog.rs    # Use ButtonGroup
│   │   └── confirm.rs        # Use ButtonGroup
│   └── header.rs             # Expose dropdown geometry for hover testing
```

### State Flow

```
User Input (Mouse/Keyboard)
       ↓
Event Handler (main.rs)
       ↓
Update App State (hover, selection)
       ↓
Render (popup files use ButtonGroup)
       ↓
Visual Output (highlighted buttons, synced dropdown)
```

---

## Component Design

### 1. ButtonState Enum

```rust
pub enum ButtonState {
    Normal,    // Default appearance
    Hovered,   // Mouse over but not keyboard-selected
    Selected,  // Keyboard-selected via arrows
}
```

**Visual behavior:**
- `Selected`: Background highlight + bold (primary action priority)
- `Hovered`: Background highlight or underline (feedback without action)
- `Normal`: Dim styling for secondary, accent color for primary

### 2. Button Struct

```rust
pub struct Button<'a> {
    label: &'a str,
    state: ButtonState,
    is_primary: bool,  // Save/Confirm = true, Cancel = false
}
```

### 3. ButtonGroup Struct

```rust
pub struct ButtonGroup<'a> {
    buttons: [Button<'a>; 2],
    selected_index: usize,      // 0 or 1 (keyboard focus)
    hovered_index: Option<usize>, // Mouse over state
}
```

**Key methods:**
- `new()` - Create from labels and primary flags
- `left()` / `right()` - Keyboard navigation
- `select()` - Execute selected button, returns `is_primary`
- `set_hover()` - Update hover state from mouse
- `hit_test()` - Determine which button is at x coordinate
- `render()` - Return styled `Line` with both buttons

**Geometry sharing:**
- ButtonGroup computes hit regions during render
- Both render and mouse handler use the same logic
- Eliminates duplication from current hardcoded hit regions

---

## State Management

### App State Additions

```rust
pub struct App {
    // ... existing fields ...
    
    // Button hover states (per popup)
    pub file_dialog_button_hover: Option<usize>,  // 0 = Save/Open, 1 = Cancel
    pub confirm_button_hover: Option<usize>,       // 0 = Confirm, 1 = Cancel
    pub settings_theme_hover: Option<usize>,       // Which theme row hovered
    
    // Mouse position tracking
    pub last_mouse_pos: Option<(u16, u16)>,
}
```

### State Lifecycle

```rust
// On popup open
pub fn open_file_dialog(&mut self, is_save: bool) {
    self.file_dialog_button_hover = None;  // Reset
    self.show_file_dialog = true;
}

// On popup close
pub fn close_file_dialog(&mut self, confirmed: bool) {
    self.show_file_dialog = false;
    self.file_dialog_button_hover = None;  // Clear
    // ... rest of logic ...
}
```

**Why per-popup fields?**
- Simpler than global focus system
- Each popup is modal (only one active)
- Easy to reset on open/close
- Avoids complex focus precedence logic

---

## Event Handling

### Keyboard Events

**Left/right arrow navigation:**
```rust
KeyCode::Left | KeyCode::Right if app.show_file_dialog => {
    if key.code == KeyCode::Left {
        app.file_dialog_selected = 0;
    } else {
        app.file_dialog_selected = 1;
    }
    continue;  // Stop propagation
}

KeyCode::Enter if app.show_file_dialog => {
    let confirmed = app.file_dialog_selected == 0;
    app.close_file_dialog(confirmed);
    continue;
}
```

**Same pattern for:** Confirm dialog, any future 2-button popups

**Backward compatibility:**
- Confirm dialog still accepts Y/N keys
- Existing shortcuts unchanged

### Mouse Events - Hover Tracking

**New handler:**
```rust
fn update_hover_states(x: u16, y: u16, app: &mut App) {
    // File dialog
    if app.show_file_dialog {
        let popup = popups::file_dialog::file_dialog_rect(term_area);
        let btn_y = (popup.y + popup.height - 2) as usize;
        
        if y == btn_y as u16 {
            let btn_x_start = popup.x + 2;
            let confirm_len = if app.file_dialog_is_save { 8 } else { 7 };
            
            if x >= btn_x_start && x < btn_x_start + confirm_len as u16 {
                app.file_dialog_button_hover = Some(0);
            } else if x >= btn_x_start + confirm_len as u16 + 3 
                   && x < btn_x_start + confirm_len as u16 + 3 + 11 {
                app.file_dialog_button_hover = Some(1);
            } else {
                app.file_dialog_button_hover = None;
            }
        } else {
            app.file_dialog_button_hover = None;
        }
    }
    
    // Similar for confirm_dialog, settings, etc.
}
```

**Integration:**
```rust
fn handle_mouse(mouse: MouseEvent, app: &mut App) {
    match mouse.kind {
        MouseEventKind::Moved => {
            update_hover_states(mouse.column, mouse.row, app);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            // Use hover state for click handling
            if app.show_file_dialog && app.file_dialog_button_hover.is_some() {
                let confirmed = app.file_dialog_button_hover.unwrap() == 0;
                app.close_file_dialog(confirmed);
            }
        }
        // ... rest unchanged ...
    }
}
```

---

## Dropdown Menu Hover Sync

**Current problem:** `">"` only follows keyboard navigation

**Solution:** Sync `menu_selected` with mouse position

```rust
// In handle_mouse, for dropdown menus
if app.active_menu.is_some() {
    let dropdown = header::dropdown_rect(term_area, &app.active_menu.as_ref().unwrap());
    
    if x >= dropdown.x && x < dropdown.x + dropdown.width
       && y >= dropdown.y && y < dropdown.y + dropdown.height
    {
        let inner_y = y - dropdown.y - 1;  // Skip header border
        let item_idx = (inner_y as usize).saturating_sub(1);
        
        let item_count = match app.active_menu.as_deref() {
            Some("File") => 6,
            Some("Tools") => 4,
            Some("Settings") => 2,
            _ => 0,
        };
        
        if item_idx < item_count {
            app.menu_selected = item_idx;  // Sync keyboard selection with mouse
        }
    }
}
```

**Result:**
- Mouse move → `">"` follows cursor
- Keyboard arrows → still work
- Enter key → executes selected (keyboard or mouse-set) item
- Click → executes clicked item

---

## File Changes Summary

### New Files

1. **`src/ui/button.rs`** (NEW, ~150 lines)
   - `ButtonState` enum
   - `Button` struct
   - `ButtonGroup` struct with methods
   - Render functions

### Modified Files

1. **`src/app.rs`** (+20 lines)
   - Add hover state fields
   - Reset hover on popup open/close
   - Reset mouse position tracking

2. **`src/main.rs`** (+80 lines)
   - Add `update_hover_states()` function
   - Add left/right arrow handlers for file/confirm dialogs
   - Integrate hover tracking into mouse handler
   - Add dropdown hover sync logic

3. **`src/ui/popups/file_dialog.rs`** (REFACTOR, -30 +20 lines)
   - Remove manual button rendering
   - Use `ButtonGroup` for buttons
   - Simplify render function

4. **`src/ui/popups/confirm.rs`** (REFACTOR, -20 +15 lines)
   - Use `ButtonGroup` instead of manual rendering
   - Keep `confirm_button_rects()` for backward compatibility

5. **`src/ui/mod.rs`** (+2 lines)
   - Export `button` module

**Total estimated change:** +137 lines, -50 lines net: **+87 lines**

---

## Testing Checklist

### File Dialog
- [ ] Left/right arrows select Save/Cancel buttons (highlight changes)
- [ ] Enter executes selected button
- [ ] Mouse hover highlights button (background or underline)
- [ ] Mouse click on button works
- [ ] Mouse click outside popup closes it
- [ ] Opening popup resets hover state
- [ ] Long path input still scrolls correctly

### Confirm Dialog
- [ ] Left/right arrows select Confirm/Cancel buttons
- [ ] Y/N keys still work (backward compatible)
- [ ] Enter executes selected button
- [ ] Mouse hover highlights button
- [ ] Mouse click works
- [ ] Quit with unsaved still shows confirm dialog

### Dropdown Menus
- [ ] Mouse move over items moves `">"` indicator
- [ ] Keyboard arrows (j/k) still work
- [ ] Enter executes selected item
- [ ] Click executes clicked item
- [ ] Menu closes after selection
- [ ] All three menus (File, Tools, Settings) work correctly

### General
- [ ] No performance regression (hover updates don't cause lag)
- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo fmt -- --check` passes
- [ ] Works in kitty terminal
- [ ] Works in alacritty terminal

---

## Implementation Phases

### Phase 1: Button Component (1 day)
1. Create `src/ui/button.rs` with ButtonState, Button, ButtonGroup
2. Implement render functions
3. Write unit tests for ButtonGroup logic

### Phase 2: App State Integration (0.5 day)
1. Add hover state fields to App
2. Add reset methods for hover states
3. Test state lifecycle

### Phase 3: Event Handling (1 day)
1. Implement `update_hover_states()` function
2. Add left/right arrow handlers
3. Integrate hover tracking into mouse handler
4. Test keyboard navigation

### Phase 4: Popup Refactoring (1 day)
1. Update file_dialog.rs to use ButtonGroup
2. Update confirm.rs to use ButtonGroup
3. Fix button click handlers to use hover state
4. Test all popups

### Phase 5: Dropdown Sync (0.5 day)
1. Implement dropdown hover sync
2. Test dropdown mouse and keyboard interaction
3. Verify `">"` indicator follows mouse

### Phase 6: Integration & Polish (0.5 day)
1. Run full test suite
2. Manual testing in multiple terminals
3. Performance profiling
4. Final polish

**Total estimated time:** 3-4 days

---

## Trade-offs & Decisions

### Per-Popup Hover Fields vs Global Focus System
**Decision:** Per-popup fields
**Reasoning:**
- Simpler to implement and understand
- Each popup is modal (only one active)
- Avoids complex focus precedence logic
- Easier to reset state on open/close
- Sufficient for current needs

### Background Highlight vs Underline for Hover
**Decision:** Background highlight (primary), underline (secondary)
**Reasoning:**
- Background is more visible in terminal
- Matches existing selected state styling
- Underline alone may be hard to see in some terminals
- Background + bold for selected (higher priority)
- Background only for hovered (feedback without action)

### ButtonGroup for 2-Button Dialogs Only
**Decision:** Yes, not generalizing to N buttons yet
**Reasoning:**
- All current dialogs have exactly 2 buttons
- Future dialogs likely also 2 buttons (OK/Cancel pattern)
- Keeps implementation simple
- Can generalize later if needed

### Dropdown Sync Strategy
**Decision:** Direct modification of `menu_selected`
**Reasoning:**
- Simple and effective
- Reuses existing selection infrastructure
- Keyboard and mouse share same state
- No additional complexity

---

## Future Considerations

1. **Phase 2 features** may benefit from extending ButtonGroup:
   - N-button dialogs (Yes/No/Cancel)
   - Button groups in settings forms
   - Tab navigation between controls

2. **Focus system** could be added if we get more complex forms:
   - Tab key navigation between controls
   - Visual focus indicator
   - Focus precedence rules

3. **Performance**: If hover updates cause lag, consider:
   - Throttling hover state updates
   - Only re-render on state changes
   - Profiling and optimization

---

## Success Criteria

1. ✅ All three reported issues are fixed
2. ✅ Button interaction works via keyboard (left/right + Enter)
3. ✅ Button interaction works via mouse (hover + click)
4. ✅ Dropdown `">"` syncs with mouse hover
5. ✅ Backward compatibility maintained (Y/N keys work)
6. ✅ No performance regression
7. ✅ Code is clean, tested, and follows project conventions
8. ✅ Implementation is reusable for future popups

---

## References

- AGENTS.md - Core principles (keyboard/mouse parity, terminal-first)
- Design Specification - Phase 1+ scope and UI patterns
- Existing button implementations in `confirm.rs`, `file_dialog.rs`
- `confirm_button_rects()` pattern for shared geometry

---

**End of Design Specification**