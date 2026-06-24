# AGENTS.md — Coding Agent Guidelines for Rust-Edit

**Project:** Rust-Edit — Feature-rich terminal text & code editor (TUI)  
**Primary Framework:** Ratatui v0.30+  
**Status:** Early development (MVP focus)  
**Last Updated:** June 24, 2026

This file defines how coding agents (Grok Build, Cursor, Claude, human contributors, etc.) should work on the project. It exists to keep changes consistent, high-quality, and aligned with the terminal-first philosophy.

**Mandatory first step for any task:** Read the full Design Specification (`docs/Rust-Edit-Design-Specification-2026-06-24.md` or the version provided with this task). It contains the prioritized scope, UI rules, tradeoffs, and data model. Do not implement anything that contradicts it.

---

## 1. Core Principles (Non-Negotiable)

1. **Terminal reality first**
   - Every feature must work well in common Linux terminal emulators (kitty, wezterm, alacritty, foot, gnome-terminal, etc.).
   - No reliance on non-portable extensions (Kitty graphics protocol for "font sizes", sixel for rich text, etc.) unless explicitly approved and feature-gated.
   - When in doubt: choose the simpler, more compatible path and document the limitation.
2. **Keyboard-first, mouse optional**
   - All primary actions must be fully keyboard accessible with discoverable shortcuts.
   - Mouse is a convenience (tab switching, cursor placement, scrolling), never the only way.
3. **Incremental & shippable > perfect**
   - MVP scope is deliberately limited (see Design Spec §5.1). Do not add Phase 2+ features (full LSP, split panes, git integration, plugins, session restore) without explicit approval.
   - Every change should leave the app in a runnable, testable state.
4. **Explain tradeoffs**
   - When you have a choice (crate X vs Y, approach A vs B), briefly state the pros/cons in your reasoning or commit message. Cite terminal constraints, performance, complexity, or maintenance burden.
5. **Respect the spec**
   - Rich text = source editing + markdown formatting toolbar + styled preview (no WYSIWYG font sizes).
   - LSP is Phase 2+ only.
   - Printing is future/export-only.
   - If the spec is unclear, ask before coding.

---

## 2. Tech Stack & Pinned Dependencies (June 2026)

Use these exact versions (or compatible patch releases) unless a compelling reason exists:

```toml
[dependencies]
ratatui = "0.30.2"
crossterm = "0.28"
ropey = "1.6.1"
syntect = "5.3.0"
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
directories = "5.0"
pulldown-cmark = "0.12"
arboard = "3.4"
anyhow = "1.0"
regex = "1.10"
```

**Optional / evaluation only (do not add to Cargo.toml without approval):**

- `ratatui-code-editor = "0.0.4"` — promising but very new (0.0.x). Test thoroughly in isolation before considering integration.

**Dev dependencies (recommended):**

```toml
[dev-dependencies]
pretty_assertions = "1.4"
# insta = "1"          # only if we decide on snapshot testing for pure logic
```

Run `cargo fmt` and `cargo clippy -- -D warnings` on every change.

---

## 3. Recommended Project Structure

```
rust-edit/
├── Cargo.toml
├── README.md
├── AGENTS.md                  # this file
├── docs/
│   ├── Rust-Edit-Design-Specification-2026-06-24.md
│   └── ui-design-sample-2.jpg
├── src/
│   ├── main.rs
│   ├── app.rs                 # Top-level App state, event loop, high-level commands
│   ├── buffer.rs              # Buffer struct + Ropey wrapper + core editing ops (insert, delete, undo, indent, etc.)
│   ├── config.rs              # Config loading/saving (toml + XDG)
│   ├── theme.rs               # Theme struct + color roles + built-in themes + serde
│   ├── syntax.rs              # Syntect wrapper, language detection, highlight cache
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── layout.rs          # Ratatui Layout helpers
│   │   ├── header.rs          # Menu bar + file tabs + [+] button
│   │   ├── editor.rs          # Main editing view, line numbers, cursor, selection, syntax spans
│   │   ├── status.rs          # Status bar
│   │   ├── find.rs            # Find / Replace bar or popup logic
│   │   └── popups/
│   │       ├── mod.rs
│   │       ├── file_dialog.rs # Open / Save As
│   │       ├── settings.rs    # Settings form + theme switcher
│   │       ├── confirm.rs     # Unsaved changes, etc.
│   │       └── word_count.rs
│   └── utils/
│       └── clipboard.rs       # arboard thin wrapper
├── assets/                    # (future) bundled syntect syntax/theme packs if needed
└── tests/                     # Integration or property tests (optional early)
```

**Module rules:**

- Keep `ui/` focused on presentation only. Business logic (buffer mutations, find logic, formatting) lives in `buffer.rs`, `app.rs`, or dedicated modules.
- One popup = one file in `popups/`.
- No god objects. App should delegate to Buffer, Theme, SyntaxHighlighter, etc.

---

## 4. Coding Standards & Style

### Rust

- Edition 2021.
- `cargo fmt` (default rustfmt config is fine; add `rustfmt.toml` only if we need specific rules later).
- `cargo clippy -- -D warnings` must pass. Allow only with `#[allow(...)]` + comment explaining why.
- Error handling:
  - User-facing errors → `anyhow::Result` + context + nice popup message.
  - Internal recoverable → `Result<T, MyError>` with `thiserror` (only if we grow a small lib).
  - Never `unwrap()` or `expect()` in production paths without a very good reason.
- Async: Only introduce `tokio` when we actually need it (e.g., future LSP client). Keep the main event loop synchronous with `crossterm::event::read()` for now.
- Logging: Use `log` + `env_logger` (or `tracing`) for debug info. Never print directly to stdout/stderr except for final binary output.

### UI / Ratatui Specific

- All colors and styles come from the current `Theme`. Never hard-code `Color::Red` etc. outside the default theme definition.
- Popups: Use a centered `Block` + `Clear` widget pattern. Dismiss on `Esc`, click outside (if mouse), or explicit button.
- Layout: Prefer `Layout` constraints (`Length`, `Min`, `Percentage`) over manual math.
- Text rendering in editor: Build `Vec<Line<'static>>` or use a custom widget that applies pre-computed syntax `Span`s. Do not re-highlight the entire buffer on every frame.
- Scroll & viewport: Track `scroll_offset` (row, col) per buffer. Only render visible lines.

### Theming & Config

- Theme changes must apply live (no restart) where possible.
- All user-configurable values live in `config.toml` under clear sections (`[ui]`, `[editor]`, `[theme]`).
- Provide sensible defaults; never require a config file on first run.

### Markdown Preview (when implemented)

- Source is always plain text + markdown syntax.
- Preview renderer maps pulldown-cmark events to Ratatui `Span` / `Line` with theme colors (headings = bold + accent color, code blocks = bg color + inner syntax if possible, etc.).
- Formatting toolbar inserts syntax into the source buffer (never mutates a "rich" representation).

---

## 5. Workflow & Change Process

### Before Writing Code

1. Read the relevant section of the Design Specification.
2. Check the priority (MVP vs Phase 1 vs Future). If it's Future, stop and confirm with the user.
3. Think about terminal constraints and the "keyboard-first" rule.
4. If adding a new dependency, justify it (why not std or existing crate? size? maintenance? terminal impact?).

### Making Changes

- Small, focused commits or PRs. One logical change per PR when possible.
- For UI changes: also update or add to the manual test checklist (see §7).
- When touching the editor core (cursor, rope ops, selection, indent): add or update unit tests in `buffer.rs` or `tests/`.
- For new popups or complex UI: implement the logic first (stateless or with clear state machine), then the rendering.

### Commit / PR Messages

Good example:

```
feat(find): add incremental search with match highlighting

- Opens bottom bar on Ctrl+F
- Highlights all matches in visible area using theme accent
- Basic regex + case toggle (Phase 1 scope only)
- No Replace yet (see Design Spec §5.2)

Tested in: kitty, wezterm (mouse + keyboard)
Tradeoff: chose simple regex crate over full fancy engine for MVP size
```

Bad: "added find stuff", "fix bug", "wip".

### When to Ask Questions

Ask early if:

- The spec is ambiguous on behavior or priority.
- A terminal limitation makes the requested feature painful.
- You see a significantly better architectural approach (with clear pros/cons).
- Adding a new external dependency or major crate.

Do **not** ask for every tiny decision. Propose and note the reasoning.

---

## 6. Specific Implementation Patterns

### Adding a New Popup (example flow)

1. Create `src/ui/popups/my_popup.rs`.
2. Define a state struct (e.g., `MyPopupState { input: String, ... }`).
3. In `app.rs` or a UI coordinator, handle `Event` for that popup (key handling, mouse).
4. Render with a helper that draws a centered bordered block + inner widgets.
5. Wire dismissal (Esc → close, Enter on action → execute + close).
6. Add keyboard shortcut or menu entry that opens it.
7. Update help text / keybinding list.

### Smart Indent on Enter

- In `buffer.rs`: on newline, copy leading whitespace from previous line.
- Then apply simple language-aware rules (increase after `{`, `:` for Python, etc.).
- Use the detected `Language` enum. Keep rules small and testable.
- Do **not** implement full tree-sitter indent queries in MVP unless the `ratatui-code-editor` route is taken.

### Syntax Highlighting

- Cache highlighted `Vec<Span>` for the current visible window only.
- Recompute on edit or scroll into new lines (dirty-line tracking is a plus).
- Fall back gracefully if syntect fails for a language.

### Theme Switching

- `Theme` is plain data (colors as `ratatui::style::Color` or hex strings + conversion).
- Changing theme in Settings immediately rebuilds the current UI draw (no restart).
- Provide 4–5 built-in themes matching the spec (Dark Red default, Light, High Contrast, etc.).

---

## 7. Testing & Verification

### Automated

- `cargo test` for all pure logic (buffer operations, config serde roundtrip, syntax language detection, find/replace engine, etc.).
- `cargo clippy -- -D warnings && cargo fmt -- --check`.

### Manual (required for any UI change)

Test checklist (run in at least 2 different terminals, e.g. kitty + alacritty):

- [ ] App starts and shows header + empty buffer matching the red/dark aesthetic.
- [ ] Open file from CLI and from menu; tabs appear and switch correctly.
- [ ] Typing, cursor movement (arrows, Home/End, word jumps), selection, copy/paste all work.
- [ ] Line numbers visible and correct.
- [ ] Syntax highlighting appears for supported languages (test .rs, .py, .md, .json at minimum).
- [ ] Smart indent on Enter behaves reasonably for code.
- [ ] Find bar opens, incremental highlight works, basic replace functions.
- [ ] Theme switch in Settings applies immediately and looks good in both dark/light.
- [ ] Markdown file shows formatting toolbar (when implemented); preview toggle renders headings/lists/code nicely.
- [ ] Unsaved changes confirmation appears on close/quit.
- [ ] Mouse works for tab switching, cursor placement, scrolling (where supported).
- [ ] No obvious layout breakage on resize (small vs wide terminal).
- [ ] Help (`?` or F1) shows accurate keybindings.
- [ ] Large file (e.g. 50k+ lines) remains responsive (test scrolling & editing).

Document any terminal-specific quirks you discover.

### Performance

- Editing and scrolling a 10k+ line file should feel instant.
- Highlighting should not cause visible lag on normal typing.
- If you introduce slowness, profile and explain before merging.

---

## 8. What NOT to Do (Yet)

- Do not implement full LSP client, diagnostics from rust-analyzer, completions, etc.
- Do not add "font size" or arbitrary rich-text WYSIWYG editing.
- Do not add built-in printing / PDF generation unless the user explicitly moves it to Phase 1.
- Do not introduce heavy GUI crates (egui, iced, etc.) or webview.
- Do not change the core data model (Rope + per-buffer state) without strong justification.
- Do not hard-code colors or layout numbers outside the Theme / Layout system.
- Avoid "clever" global singletons or excessive use of `Rc<RefCell<>>`. Prefer ownership and message passing where possible.

---

## 9. Communication Style Expected from the Agent

- Direct and concise.
- Always cite the Design Specification section when relevant.
- When proposing an alternative: "Option A (current spec) vs Option B (my suggestion). Tradeoffs: X, Y, Z. Recommendation: B because..."
- For UI work: describe what the user will see and how to test it.
- If something in the spec seems suboptimal for implementation or UX, say so with evidence (complexity, performance, terminal limits) rather than silently working around it.

---

## 10. Getting Started Checklist (for a fresh clone)

```bash
git clone ... && cd rust-edit
cargo build --release
./target/release/rust-edit                    # empty buffer
./target/release/rust-edit src/main.rs        # open a file
```

Then explore the code, run `cargo clippy`, and pick a small MVP task from the Design Spec roadmap.

---

**Remember:** The goal is a delightful, fast, keyboard-centric terminal editor that respects the constraints of the medium. Ship working slices often, document tradeoffs, and stay aligned with the Design Specification.

If this file conflicts with new decisions, update it and note the change.

---

*This AGENTS.md is intended to be placed in the root of the rust-edit repository.*