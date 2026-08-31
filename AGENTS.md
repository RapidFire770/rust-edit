# AGENTS.md — Coding Agent Guidelines for Rust-Edit

**Project:** Rust-Edit — Feature-rich terminal text & code editor (TUI)  
**Primary Framework:** Ratatui v0.30+  
**Status:** Phase 1 complete (June 2026). MVP + high-value additions shipped.   
**Last Updated:** August 31, 2026

This file defines how coding agents (Grok Build, Cursor, Claude, human contributors, etc.) should work on the project. It exists to keep changes consistent, high-quality, and aligned with the terminal-first philosophy.

**IMPORTANT:**

- Always check available skills and invoke them when relevant to the user's request.
- For smaller tasks, research, code exploration use a subagent that will investigate and report their findings back to you. Preform small changes on the the main git branch instead of using a worktree. Use a code reviewer subagent to review the work afterward. Wait for subagents to complete and create a commit once work is validated.
- For large tasks always use the subagent-driven-development skill, if a git worktree is not setup, configure one in a .worktrees folder before handoff to the subagent.
- Keep this AGENTS.md file updated with information about the project that other AI agents would need to know to keep the project consistent from session to session. (Project layout/structure, technology stack, tool chain, coding standards & style, testing, pitfalls and conflicts to avoid, features, etc.)
- When a bug is fixed or feature is implemented check it off the users `docs/Todo-List.md`

**Mandatory first step for any task:** Read the full Design Specification (`docs/Rust-Edit-Design-Specification-2026-06-24.md` or the version provided with this task). It contains the prioritized scope, UI rules, tradeoffs, and data model.

---

## 1. Core Principles (Non-Negotiable)

1. **Terminal reality first**
   - Every feature must work well in common Linux terminal emulators (kitty, wezterm, alacritty, foot, gnome-terminal, etc.).
   - No reliance on non-portable extensions (Kitty graphics protocol for "font sizes", sixel for rich text, etc.) unless explicitly approved and feature-gated.
   - When in doubt: choose the simpler, more compatible path and document the limitation.
2. **Keyboard and mouse are first-class peers**
   - Every action must be reachable both ways: full keyboard shortcuts *and* full mouse interaction (click tabs, menus, cursor placement, text selection, scrolling, and clickable buttons in every popup).
   - Neither input method is "secondary" or "optional." When adding a feature, wire up both the keybinding and the mouse path.
3. **Incremental & shippable > perfect**
   - Phase 1 is complete. Phase 2 features (full LSP, split panes, git integration, plugins, session restore — see Design Spec §5.3) require explicit approval before starting.
   - Every change should leave the app in a runnable, testable state.
   - If a major refactor would be the best choice for the future of the application give the user the option to approve or deny refactoring vs the small change.
4. **Explain tradeoffs**
   - When you have a choice (crate X vs Y, approach A vs B), briefly state the pros/cons in your reasoning or commit message. Cite terminal constraints, performance, complexity, or maintenance burden.
5. **Respect the spec — with documented deviations**
   - The Design Spec is the historical source of intent; this AGENTS.md reflects the *current* agreed scope. Where they differ, AGENTS.md wins:
     - **Markdown rendered preview was removed.** Markdown files keep source syntax highlighting and the `Ctrl+B/I/\`` formatting inserters, but the pulldown-cmark split-preview pane was deleted (the rendering was incomplete and editing in the source pane was awkward). Re-adding a preview requires a new design decision.
     - **Single Find/Replace shortcut (**`Ctrl+F`**)** — the bar has query/replace fields reachable via Tab, so a separate `Ctrl+H` was dropped.
   - LSP is Phase 2+ only.
   - Printing/PDF export is future-only.
   - If the spec is unclear, ask before coding.

---

## 2. Tech Stack & Pinned Dependencies

Use these exact versions (or compatible patch releases) unless a compelling reason exists:

```toml
[dependencies]
ratatui = "0.30.2"
crossterm = "0.29"
ropey = "1.6.1"
syntect = "5.3.0"
clap = { version = "4.6.6", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
toml = "1.1.4"
directories = "6.0.0"
arboard = "3.6.1"
anyhow = "1.0"
regex = "1.13.1"
```

> `pulldown-cmark` was removed when the markdown preview feature was deleted.

**Optional / evaluation only (do not add to Cargo.toml without approval):**

- `ratatui-code-editor = "0.0.4"` — promising but very new (0.0.x). Test thoroughly in isolation before considering integration.

**Dev dependencies (recommended):**

```toml
[dev-dependencies]
pretty_assertions = "1.4"
# insta = "1"          # only if we decide on snapshot testing for pure logic
```

Run `cargo fmt` and `cargo clippy -- -D warnings` on every change.

### Toolchain / cargo path (important for agents)

**For use in most environments** : Cargo is located at `/usr/bin/cargo`.

**For use in Zcode Appimage** : The system `cargo` proxy is broken in this environment (it fails with
`error: unknown proxy name: 'zcode'`). Use the real toolchain binary instead:

```bash
CARGO="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo"
"$CARGO" --version     # sanity check
```

Discover the path with `ls ~/.rustup/toolchains/*/bin/cargo`. It is also recorded
in `docs/zcode cargo toolchain location.md`. All `cargo` commands in this file assume
you've set this `CARGO` variable (or that the proxy is fixed).

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
│   ├── app.rs                 # Top-level App state, event loop, high-level commands, highlight cache
│   ├── buffer.rs              # Buffer struct + Ropey wrapper + editing ops + cursor/selection/scroll
│   ├── config.rs              # Config loading/saving (toml + XDG)
│   ├── theme.rs               # Theme struct + color roles + built-in themes + serde
│   ├── syntax.rs              # Syntect wrapper, highlight_window (one HighlightLines per window)
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── layout.rs          # Ratatui Layout helpers + HEADER_ROWS constant
│   │   ├── header.rs          # Menu bar + file tabs + [+] button + separator rule + dropdown
│   │   ├── editor.rs          # Main editing view, line numbers, cursor, selection, hscroll
│   │   ├── status.rs          # Status bar
│   │   ├── find.rs            # Find/Replace bar (FindState + FindFocus 3-state model)
│   │   └── popups/
│   │       ├── mod.rs
│   │       ├── confirm.rs     # Unsaved changes (clickable [Y]/[N] buttons)
│   │       ├── file_dialog.rs # Open / Save As (scalable, scrollable path input)
│   │       ├── help.rs        # Tabbed help (Shortcuts / Languages / About)
│   │       ├── settings.rs    # Settings form + theme switcher
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

**Key types / established patterns (read before editing these areas):**

- `Buffer` fields: `cursor` (byte offset), `selection` (normalized `Range`) + `selection_anchor` (fixed anchor for bidirectional extend), `scroll_offset` (row, col), `visible_lines`/`visible_width` (synced each frame), `generation` (invalidation key for the highlight cache), `is_initial` (true only for the auto-created untitled placeholder at startup or after closing the last tab; `[+]` tabs stay false). Auto-scroll lives in `adjust_scroll` (vertical+horizontal edge clamping) and `ensure_cursor_*`. File open goes through `App::open_file_in_tab` (duplicate-focus, then in-place placeholder replace, else append).
- `FindState.focus: FindFocus` — a 3-state model (Query → Replace → Command) so letters like `n`/`N` type normally into the boxes and navigate only in Command mode.
- Syntax highlighting: `app.refresh_highlight_cache()` recomputes a visible window only on edit/scroll/language change; `editor.rs` reads the cache. One `HighlightLines` per window (multi-line constructs stay correct).
- Event coalescing: `run_app` drains queued events and sums consecutive mouse-wheel deltas into one scroll → one redraw per burst.
- Popup focus: at most one centered popup owns input (`App::popup_focus`). Other popups cannot open. Global shortcuts are swallowed except Ctrl+Q. Quit confirm paints last and stacks on top of the current popup without dismissing it. File browser may stack only on the file dialog (Browse…). Mouse and wheel follow `popup_focus`, not independent `show_*` flags.

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
- [ ] Markdown file shows syntax highlighting and `Ctrl+B/I/\`` formatting inserters.
- [ ] Unsaved changes confirmation appears on close/quit; Y/N works via keyboard and clickable buttons.
- [ ] Mouse works for tab switching, cursor placement, scrolling, and clicking popup buttons.
- [ ] No obvious layout breakage on resize (small vs wide terminal).
- [ ] Help (`?` or F1) shows accurate keybindings.
- [ ] Large file (e.g. 50k+ lines) remains responsive (test scrolling & editing).

Document any terminal-specific quirks you discover.

### Performance

- Editing and scrolling a 10k+ line file should feel instant.
- Highlighting should not cause visible lag on normal typing.
- If you introduce slowness, profile and explain before merging.

---

## 8. What NOT to Do (Phase 2 — not started; needs approval)

- Do not implement full LSP client, diagnostics from rust-analyzer, completions, etc.
- Do not add session restore, split panes, a sidebar file tree, git integration, or a plugin system.
- Do not re-add a markdown rendered preview without a fresh design decision (the previous one was removed — see §1 "Respect the spec — with documented deviations").
- Do not add "font size" or arbitrary rich-text WYSIWYG editing.
- Do not add built-in printing / PDF generation unless the user explicitly moves it into scope.
- Do not introduce heavy GUI crates (egui, iced, etc.) or webview.
- Do not change the core data model (Rope + per-buffer state) without strong justification.
- Do not hard-code colors or layout numbers outside the Theme / Layout system.
- Avoid "clever" global singletons or excessive use of `Rc<RefCell<>>`. Prefer ownership and message passing where possible.
- Do not add a feature with keyboard input but no mouse path (or vice versa) — see Core Principle 2.

---

## 9. Communication Style Expected from the Agent

- Direct and concise.
- Always cite the Design Specification section when relevant.
- When proposing an alternative: "Option A (current spec) vs Option B (my suggestion). Tradeoffs: X, Y, Z. Recommendation: B because..."
- For UI work: describe what the user will see and how to test it.
- If something in the spec seems suboptimal for implementation or UX, say so with evidence (complexity, performance, terminal limits) rather than silently working around it.

---

## 10. Getting Started Checklist (for a fresh clone)

> The system `cargo` proxy may be broken — see §2 "Toolchain / cargo path".
> Set `CARGO="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo"`
> and use `"$CARGO"` in place of `cargo` below if needed.

```bash
git clone ... && cd rust-edit
cargo build --release
./target/release/rust-edit                    # empty buffer
./target/release/rust-edit src/main.rs        # open a file
```

Then explore the code, run `cargo fmt && cargo clippy -- -D warnings && cargo test`,
and review `docs/Todo-List.md` for open Phase 2 ideas.

---

**Remember:** The goal is a delightful, fast, keyboard-centric terminal editor that respects the constraints of the medium. Ship working slices often, document tradeoffs, and stay aligned with the Design Specification.

If this file conflicts with new decisions, update it and note the change.

---

*This AGENTS.md is intended to be placed in the root of the rust-edit repository.*
