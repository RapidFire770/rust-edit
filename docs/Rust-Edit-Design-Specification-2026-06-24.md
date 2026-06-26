# Rust-Edit: Feature-Rich Terminal Text & Code Editor

**Design Specification v0.2**  
**Date:** June 24, 2026  
**Target Platform:** Linux Terminal (TUI)  
**Implementation Language:** Rust  
**Primary Framework:** Ratatui v0.30+

---

## 1. Executive Summary

Rust-Edit is a fast, keyboard-first (with mouse support) text and code editor that runs entirely inside Linux terminal emulators. It uses Ratatui to deliver a modern, tabbed, menu-driven interface with syntax highlighting, smart indentation, find/replace, markdown preview, theming, and clipboard integration — all without leaving the terminal.

The design is directly inspired by the provided UI mockup image (`ui-design-sample-2.jpg`): a single red header bar combining menus and file tabs, dark editing area with line numbers, clean syntax-colored Python code example, and overall dark aesthetic with red accent.

**Core Philosophy**

- Prioritize terminal compatibility and performance over pixel-perfect GUI features.
- Make common actions discoverable via menus while remaining fast for keyboard users.
- Phase features ruthlessly so a working, useful editor ships quickly.
- Use mature, well-maintained crates to minimize reinventing wheels.

---

## 2. Critical Evaluation of Original Design Ideas

The original concept is solid in intent but contains several areas where terminal realities create important tradeoffs. Here is a direct, honest assessment:

### 2.1 Rich Text Editing (Fonts, Sizes, Bold, Italic, etc.)

**Original Vision:** "Rich text editing features for text files... fonts, font sizes, bold, underlining, italics... These will appear on a bar below the file and tabs menu bar when on a normal text document file."

**Problems & Tradeoffs:**

- Standard terminal emulators (xterm, gnome-terminal, alacritty, kitty, wezterm, etc.) use a fixed grid of cells. Each cell has one foreground color, one background color, and a small set of style flags (bold, italic, underline, strikethrough, reverse).
- Arbitrary font families or per-character font sizes are **not possible** in a portable way. Even advanced emulators that support images or custom fonts do not expose this as a general text attribute.
- Attempting to fake "font sizes" (e.g., by using Unicode block characters or multiple lines) produces poor results and breaks accessibility/copy-paste.

**Recommended Scope Adjustment (adopted in this spec):**

- **Code files:** Excellent syntax highlighting using colors + limited styles (bold/italic for keywords, comments, etc.). This is the highest-value "rich" feature in a terminal.
- **Markdown / plain text files:**
  - Always edit the **source** (plain text + markdown syntax).
  - Provide a **contextual Formatting Toolbar** (or command palette) that inserts markdown syntax at the cursor or wraps the current selection (`**bold**`, `*italic*`, `` `code` ``, headings, lists, links).
  - Add a **toggleable Preview pane** (split view) that renders the markdown nicely using Ratatui's styling capabilities (headings become bold + colored, code blocks get background, emphasis uses italic/bold spans, etc.).
- **Result:** Files remain portable plain text. No loss of compatibility. The "rich text bar" becomes a **markdown formatting helper**, which is actually more useful for most users than trying to do WYSIWYG in a terminal.

This is the single biggest clarification needed before implementation begins.

### 2.2 Language Server Protocol (LSP) Support

**Original Vision:** "Ability to use installed coding language servers."

**Assessment:**

- Full LSP (diagnostics, completions, hover info, go-to-definition, refactoring) is a **large, complex subsystem**. It requires async I/O (tokio), proper JSON-RPC handling, server process lifecycle management, capability negotiation, and tight integration with the buffer/cursor model.
- While crates like `lsp-types` and `tower` exist, building a robust client is non-trivial and often the source of bugs and maintenance burden in editors.
- The brand-new `ratatui-code-editor` crate (v0.0.4, May 2026) uses Tree-sitter for highlighting but does **not** include LSP.

**Recommendation:**

- **MVP / Phase 1:** Do **not** implement LSP. Rely on high-quality static syntax highlighting (syntect or tree-sitter) + smart indentation rules. This still delivers 80% of the "code intelligence" feel for most editing tasks.
- **Phase 2+:** Re-evaluate. Either:
  - Add a lightweight diagnostics layer (shell out to `cargo check` / `ruff` / `eslint` and parse output), or
  - Integrate the `ratatui-code-editor` widget once it matures and exposes extension points, or
  - Build a minimal LSP client only for 1-2 priority languages (e.g., Rust + Python).
- **Rationale:** Shipping a reliable core editor is far more valuable than a half-finished LSP implementation. Many excellent terminal editors (helix, kakoune, micro) started without full LSP and added it later.

### 2.3 Printer Support

**Original Vision:** "Printer support for the user to print out their files."

**Assessment:**

- True WYSIWYG printing from a TUI is surprisingly difficult. You need page size detection, font metrics, margin handling, header/footer, syntax-colored output, etc. Most of this lives in GUI toolkits.
- Common terminal-editor pattern: "Export to PDF/text" then let the user print with system tools (`lp`, `lpr`, CUPS web UI, or GUI viewer).

**Recommendation (adopted):**

- De-prioritize built-in printing to **Future / Nice-to-have**.
- For v1: Implement **Export** → "Plain Text" and "Monospace PDF" (using a lightweight PDF crate or shelling out).
- Document clearly: "For formatted printing, export and use your system's print tools."
- This avoids heavy dependencies and edge cases (no printer configured, weird page sizes, color management) while still giving users a practical path.

### 2.4 Other Notable Tradeoffs & Mitigations

| Original Idea                | Risk / Limitation                                | Mitigation in This Spec                                                        | Priority |
|------------------------------|--------------------------------------------------|--------------------------------------------------------------------------------|----------|
| Auto-format on every change  | Expensive, disruptive, language detection errors | Explicit "Format Document" command + optional on-save toggle                   | Phase 1  |
| Full mouse-driven UI         | Terminal mouse support varies; fat-finger errors | Full keyboard shortcuts + optional mouse (click tabs, menus, cursor placement) | MVP      |
| Many simultaneous open files | Memory + UI clutter                              | Reasonable default limit (e.g. 20), easy close, session restore later          | MVP      |
| Complex dropdown menus       | TUI popup positioning & focus management         | Simple bordered List popups; Esc/click outside to dismiss                      | MVP      |

**Overall Feasibility Verdict:**  
The original idea is **achievable and worthwhile**, but only if we accept the terminal constraints and prioritize ruthlessly. By clarifying rich-text scope, deferring full LSP, and treating printing as export, we create a realistic path to a genuinely useful tool in a reasonable timeframe. Using **Ropey** for the text buffer and **Ratatui 0.30** + **syntect** (or the experimental `ratatui-code-editor`) gives us a strong technical foundation.

---

## 3. Technical Stack & Recommended Dependencies (June 2026)

All versions are the latest stable as of June 2026. Pin to these (or compatible `^` ranges) for reproducibility.

**Core TUI & Editing**

- `ratatui = "0.30.2"` — Layout, widgets (Paragraph, List, Tabs, Block, Scrollbar, Table), styling, event handling, mouse, multiple backends.
- `crossterm = "0.28"` (or latest compatible) — Recommended backend for Linux (excellent mouse, colors, raw mode, resize handling).
- `ropey = "1.6.1"` — **Critical**. Efficient UTF-8 rope data structure for the text buffer. Enables fast insert/delete in the middle of large files, cheap cloning for undo, line/col access.
- `syntect = "5.3.0"` — Mature syntax highlighting using Sublime Text grammars. Supports 100+ languages out of the box. Easy to bundle default assets. Alternative/complement: evaluate `ratatui-code-editor = "0.0.4"` (very new — test stability before heavy dependence).

**CLI, Config & Persistence**

- `clap = { version = "4.5", features = ["derive"] }` — CLI argument parsing (`rust-edit file.rs`).
- `serde = { version = "1.0", features = ["derive"] }`
- `toml = "0.8"` — Human-readable config (`~/.config/rust-edit/config.toml`).
- `directories = "5.0"` — Cross-platform (XDG) config/data directory resolution.

**Markdown & Preview**

- `pulldown-cmark = "0.12"` (or latest) — Fast, correct Markdown parser. Feed events into a custom Ratatui renderer.

**Utilities**

- `arboard = "3.4"` — Cross-platform clipboard (copy/paste works in most terminal emulators).
- `anyhow = "1.0"` — Ergonomic error handling with user-friendly messages in popups.
- `regex = "1.10"` — For advanced find/replace (optional but recommended).

**Future / Phase 2+ Candidates**

- `notify` — Watch for external file changes.
- `printpdf` or shell out for PDF export (only if printing becomes priority).
- Tree-sitter ecosystem (if moving beyond syntect for better indentation/queries).

**Why these choices?**

- Ratatui 0.30 is current, actively developed, and has a rich widget ecosystem.
- Ropey is the de-facto standard for Rust text editors (used by helix, etc.).
- Syntect is extremely mature and low-maintenance for multi-language support.
- We deliberately avoid very new 0.0.x crates in the critical path unless they solve a massive problem (the new code-editor widget is worth *evaluating* but not yet a hard dependency).

**Minimum Rust Version:** 1.80+ (or whatever Ratatui 0.30 requires).

---

## 4. UI Layout & Visual Design

### 4.1 Reference Mockup Interpretation

The attached image (`ui-design-sample-2.jpg`) shows the target aesthetic:

- Single **red header bar** spanning the full width.
- Left: Menu labels (**File**, **Tools**, **Settings**).
- Then file tabs (**main.py** active with red background highlight, **config.json**, **README.md**).
- Far right: **[+]** new tab button.
- Dark editor background, light text, visible line numbers on the left gutter.
- Python syntax highlighting visible (keywords in distinct color).
- Clean, modern terminal look with good contrast.

### 4.2 Proposed Layout (Vertical Stack)

1. **Header Bar** (height: 1 row, red background by default)
   - Flex layout: Menus (left) | File Tabs (center, scrollable) | [+] (right)
   - Menu items are clickable and have Alt+letter mnemonics.
   - Clicking a menu opens a **dropdown popup** (bordered list) directly below it.
   - Active tab visually distinct (different background or underline + bold).
   - Modified files show `filename•` or similar indicator.
   - Tab close: middle-click, or small `[x]` affordance, or `Ctrl+W`.
2. **Editing Area** (flex height, main content)
   - **Left gutter** (fixed ~5 columns): Line numbers (dim color, right-aligned).
   - **Content** (flex): Scrollable text with syntax coloring (or markdown source styling).
   - Cursor styles: Block (default) or bar; support terminal blink if available.
   - Selection: Distinct background highlight (themeable, e.g. blue or accent color).
   - Long lines: Horizontal scrolling (preferred for code) or optional soft-wrap (config).
   - Scrollbar on right when content overflows (Ratatui Scrollbar widget).
3. **Status Bar** (height: 1, recommended addition)
   - Left: Full or truncated file path + modified flag.
   - Center: Language / mode (e.g. "Rust • 4 spaces").
   - Right: `Ln 42, Col 17 | 1,284 lines | UTF-8`.
   - Different subtle background color for visual separation.
4. **Contextual Toolbars** (conditional, height 1)
   - When buffer is Markdown or plain text: **Formatting Toolbar** appears (or is toggleable via key). Contains quick actions: **B** Bold, *I* Italic, `` ` `` Code, headings, lists, link, etc. These insert markdown syntax into the source buffer.
   - **Find / Replace Bar** (slides up from bottom or appears as popup): Search input, replace input, match count, Next/Prev/Replace/Replace All buttons, options (case, regex, whole word).
5. **Popups & Modals** (centered or anchored, always on top)
   - File Open / Save dialogs (path input + simple directory listing or recent files).
   - Settings panel (form-style: theme selector, tab size, toggles).
   - Theme editor (list of color roles → hex/RGB input or swatches).
   - Confirmations (unsaved changes, etc.).
   - Word Count results.
   - Help / Keybindings cheatsheet.
   - All dismissible with `Esc`, click outside, or explicit close.

**Color Theme System**

- Default theme: "Dark Red" matching the mockup (header red accent, near-black editor bg, light text, carefully chosen syntax colors inspired by popular dark themes).
- 4–5 built-in themes (Dark, Light, High Contrast, Solarized-inspired, Monochrome).
- Full user customization via config file or Settings UI (expose all important color roles: `menu_bg`, `editor_bg`, `line_number_fg`, `keyword_fg`, `string_fg`, `comment_fg`, `selection_bg`, `cursor_fg`, etc.).
- Themes are plain data (serializable) so users can share `.toml` snippets.

**Accessibility & Terminal Compatibility**

- Support truecolor + 256-color fallbacks.
- High-contrast mode.
- Respect `NO_COLOR` env var and `COLORTERM`.
- Test on common emulators (kitty, wezterm, alacritty, gnome-terminal, foot, etc.).

---

## 5. Feature Specifications (Prioritized)

### 5.1 MVP (Core Usable Editor)

**Must-have for first working version:**

- CLI: `rust-edit [file...]` opens files in tabs (or empty buffer if none).
- Multi-buffer tab management (open, switch, close, reorder?).
- Basic editing: cursor movement (arrows, home/end, pgup/dn, word jumps), insert, delete, backspace, selection (shift+arrows or mouse drag), copy/paste/cut via clipboard + internal.
- Efficient text buffer powered by Ropey.
- Line numbers (toggleable).
- Syntax highlighting for popular languages (Rust, Python, JS/TS, JSON, TOML, YAML, Markdown, Bash, C/C++) via syntect.
- Smart indent on Enter (copy previous line indent + language-aware rules for braces, colons, etc.).
- File operations: New, Open (simple modal), Save, Save As, Close Tab, Quit (with unsaved guard).
- Theming with at least the default red/dark theme matching mockup.
- Persistent config (toml) for tab size, theme, show line numbers, etc.
- Keyboard shortcuts for everything important + Alt menu access.
- Mouse support for tab switching, menu opening, cursor placement, scrolling.
- Basic Find (incremental search with match highlighting).

**Explicitly out of MVP:**

- Full LSP, advanced auto-format on every keystroke, printing, session restore, split panes, sidebar file tree, plugins.

### 5.2 Phase 1 (High-Value Additions)

- Find & Replace (full bar with replace all, regex, preview of changes).
- Word Count (current buffer or selection) → nice popup.
- Markdown-aware features:
  - Auto-detect `.md` files.
  - Formatting toolbar/palette for quick syntax insertion.
  - Toggleable split preview pane (source | rendered) with good visual mapping of markdown elements to Ratatui styles.
- Settings popup with live theme switching and common options.
- Status bar (always visible info).
- Explicit "Format Document" command (detect language → shell out to `rustfmt`, `black`, `prettier`, etc. if present in PATH; graceful error if missing).
- Better popups and focus management.
- Help screen (`?` or F1) showing keybindings.

### 5.3 Phase 2+ (Stretch)

- Evaluate and possibly adopt `ratatui-code-editor` widget for the core editing component.
- Lightweight diagnostics (parse output from language linters/formatters).
- Session persistence (remember open files + cursor positions on restart).
- Fuzzy file open modal (or simple directory browser).
- Horizontal split or multiple editor panes.
- Git integration (status in status bar, diff highlights, etc.).
- Printer / PDF export.

---

## 6. Data Model Sketch (for implementers)

```rust
// High-level idea — not literal code
struct App {
    tabs: Vec<Buffer>,
    active: usize,
    config: Config,
    theme: Theme,
    current_popup: Option<PopupKind>,
    find_state: Option<FindState>,
    // ...
}

struct Buffer {
    path: Option<PathBuf>,
    rope: Rope,                    // from ropey crate
    language: Language,            // enum or detected from extension
    is_modified: bool,
    cursor: usize,                 // byte or (line, col) index
    selection: Option<Range<usize>>,
    scroll_offset: (u16, u16),     // row, col for viewport
    syntax_spans: Vec<...>,        // cached highlighted spans for visible area
}

enum PopupKind {
    FileOpen { path_input: String, ... },
    Settings { ... },
    FindReplace { search: String, replace: String, ... },
    Confirm { message: String, action: ... },
    // etc.
}
```

**Key Implementation Notes:**

- Never store the entire document in a `String` for editing. Use `Rope`.
- Highlight only visible lines (or dirty lines) for performance.
- Keep UI state (popups, find bar) separate from buffer state.
- All long-running or blocking work (formatting, file I/O) should be async or in a way that doesn't freeze the TUI event loop.

---

## 7. Proposed Keyboard Shortcuts (Initial)

**Global / Menu**

- `Alt+F` — Open File menu
- `Alt+T` — Open Tools menu
- `Alt+S` — Open Settings menu
- `Ctrl+Q` — Quit (with confirm if unsaved)
- `F1` or `?` — Help / Keybindings

**Tabs & Files**

- `Ctrl+Tab` / `Ctrl+Shift+Tab` — Next / Previous tab
- `Alt+1` … `Alt+9` — Switch to tab N
- `Ctrl+W` — Close current tab
- `Ctrl+N` — New empty tab
- `Ctrl+O` — Open file dialog
- `Ctrl+S` — Save current
- `Ctrl+Shift+S` — Save As

**Editing**

- Standard arrows, `Home`/`End`, `PgUp`/`PgDn`, `Ctrl+Left`/`Right` (word)
- `Ctrl+Z` / `Ctrl+Y` — Undo / Redo
- `Ctrl+C` / `Ctrl+V` / `Ctrl+X` — Copy / Paste / Cut
- `Ctrl+F` — Open Find bar
- `Ctrl+H` — Open Find & Replace
- `Ctrl+P` — Toggle Markdown preview (when applicable)
- `Tab` / `Shift+Tab` — Indent / Dedent (selection or line)
- `Enter` — New line + smart indent
- `Ctrl+K` (or similar) — Open command palette / formatting actions

All menu items should show their shortcut in the dropdown.

---

## 8. Implementation Roadmap (Recommended for Coding Agent)

**Sprint 0 – Project Skeleton (1–2 days)**

1. `cargo new rust-edit --bin`
2. Add core dependencies with exact versions above.
3. Basic Ratatui + crossterm event loop that draws a header + empty editor area.
4. Implement a minimal `Buffer` using Ropey that supports typing, cursor movement, and rendering as `Paragraph`.

**Sprint 1 – MVP Core (core loop working)**

- Tab management + switching.
- File open/save (CLI + menu).
- Line number gutter.
- Basic syntect syntax highlighting for 3–4 languages.
- Theming system + default red/dark theme.
- Simple Find with match highlighting.
- Smart indent on Enter (at least previous-line copy).
- Clipboard integration.
- Unsaved changes confirmation.
- Config persistence.

**Sprint 2 – Polish & Text Features**

- Find & Replace full implementation.
- Markdown detection + formatting toolbar + split preview renderer.
- Settings popup with theme switching.
- Status bar.
- "Format Document" command (at least for Rust + Python).
- Word count popup.
- Help screen.
- Comprehensive keyboard navigation + mouse parity.

**Sprint 3 – Hardening & Release Prep**

- Test on multiple terminals and with large files.
- Error handling & user-friendly messages.
- Documentation (README with screenshots, keybindings, config examples).
- Packaging (`cargo install`, optional .deb).
- Performance profiling (highlighting, rope operations, draw time).

**Evaluation Gate:** After Sprint 1, decide whether to adopt `ratatui-code-editor` for the editing widget or continue with custom Ropey + syntect implementation.

---

## 9. Open Questions & Assumptions

**Assumptions (please correct if wrong):**

- Primary user is on Linux with a modern terminal emulator that supports truecolor, mouse, and UTF-8.
- The editor is primarily for editing code and markdown; full rich-text document editing (ODT, DOCX, etc.) is out of scope.
- "Printer support" means practical export, not perfect WYSIWYG print preview.

**Questions for you (Josh):**

1. Do you want an optional **vim-like modal** layer (normal/insert/visual) later, or stay modeless like most GUI editors?
2. How important is **session restore** (re-open previous files on launch) for v1 vs later?
3. Any specific languages beyond the common ones that must have excellent syntax/indent support early?
4. Preference on config format: TOML (human friendly) vs JSON?

---

## 10. Why This Spec is Better for a Coding Agent

- **Clear priorities** — MVP is achievable; future phases are explicitly deferred.
- **Tradeoffs documented** — No surprises about terminal limitations.
- **Concrete crate versions** — No ambiguity; agent can `cargo add` exact pins.
- **UI described precisely** — References your mockup image and gives layout + interaction rules.
- **Data model sketch** — Gives implementers a starting mental model without over-specifying.
- **Roadmap with sprints** — Allows incremental delivery and early feedback.
- **Critical evaluation included** — Prevents wasted effort on impractical features (full rich text, early LSP, built-in printing).

This document turns your original idea list into a professional, actionable specification that a coding agent (Grok Build, another LLM, or human contributor) can use to produce working code with far less back-and-forth.

---

**Appendix: Original Document Reference**  
The content above expands and restructures the ideas from your `design ideas.md` while preserving the spirit and visual intent of the attached UI sample image. All major original features are addressed, with scope adjustments only where terminal constraints make the original literal interpretation impractical or low-value.

---

*End of Design Specification*