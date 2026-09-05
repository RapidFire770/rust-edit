# rust-edit

A fast, keyboard-and-mouse text and code editor for the Linux terminal, built with [Ratatui](https://ratatui.rs/).

**Status:** Phase 1 complete (June 2026).

Inspired by the UI mockup in `docs/ui-design-sample-2.jpg`.

## Features

- **Tabbed multi-file editing** — open many files as tabs, switch with the mouse, `Alt+1..9`, or the tab bar.
- **Rope-backed buffer** — efficient editing of large files (powered by [ropey](https://github.com/cessen/ropey)).
- **Syntax highlighting** for Rust, Python, Markdown, JSON, TOML, YAML, Bash/Shell, C, C++, JavaScript, and TypeScript (via [syntect](https://github.com/trishume/syntect)), with parse-state checkpoints and an overscanned span window so large files scroll smoothly without re-parsing the viewport on every scroll.
- **Smart indentation** on Enter (copies leading whitespace, adds a level after `{`, `[`, `(`, `:`).
- **Bidirectional selection** — extend a selection in either direction with `Shift+arrows`/`Home`/`End`, or click-and-drag with the mouse.
- **Find & Replace** with case-sensitive, regex, and whole-word toggles; a three-state focus model (query → replace → commands) so you can type any letter (including `n`) and still navigate matches.
- **Copy / cut / paste** via the system clipboard.
- **Undo / redo** with full state snapshots.
- **Theming** — Dark Red (default), Light, and High Contrast, switchable live from the Settings popup; colors come from the theme, never hard-coded.
- **Configurable** — TOML config under your XDG config dir (tab size, theme, line numbers).
- **Word count**, **format document** (shells out to `rustfmt`/`black`/`prettier` when present).
- **Horizontal and vertical scrolling** — auto-follows the cursor on long lines; manual scroll via wheel, `Alt+H`/`Alt+L`, and `PgUp`/`PgDn`; a bottom scrollbar appears when wrap is off and a line is longer than the window.
- **Full mouse support** — tabs, menus, cursor placement, text selection, scrolling, and clickable buttons in every popup. Mouse is on par with the keyboard, not an afterthought.
- **Tabbed Help popup** (`?` or `F1`) with categorized shortcuts, supported languages, and about.

> Markdown files get source syntax highlighting. (A rendered split-preview pane was evaluated and removed; see AGENTS.md for the rationale.)

## Build & Run

> **Note on the** `cargo` **wrapper:** in some environments the system `cargo` proxy is
> broken. If `cargo --version` fails with `unknown proxy name`, use the real
> toolchain binary directly:
>
> ```bash
> export CARGO=/home/$USER/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo
> # discover it with:  ls ~/.rustup/toolchains/*/bin/cargo
> "$CARGO" --version
> ```
>
> Below, replace `cargo` with `"$CARGO"` if you hit this. (See
> `docs/cargo toolchain location.md`.)

```bash
cargo build --release
./target/release/rust-edit                    # empty buffer
./target/release/rust-edit src/main.rs        # open one or more files
```

## Keyboard & Mouse

See the in-app help (`?` or `F1`) for the full, categorized list. Highlights:

**File** — `Ctrl+N` new · `Ctrl+O` open · `Ctrl+S` save · `Ctrl+Shift+S` save as · `Ctrl+W` close tab · `Ctrl+Q` quit
**Edit** — `Enter` newline+indent · `Tab` insert at cursor · `Shift+Tab` dedent · `Ctrl+Z`/`Ctrl+Y` undo/redo · `Ctrl+X`/`C`/`V` cut/copy/paste
**Find** — `Ctrl+F` open bar · `Tab` cycle query→replace→commands · (in commands) `n`/`N` next/prev · `r`/`Enter` replace · `Ctrl+A` all · `c`/`x`/`w` case/regex/word
**Navigation** — arrows/Home/End · `Shift+arrows` select · `PgUp`/`PgDn` · `Alt+H`/`Alt+L` scroll left/right · `Alt+F`/`T`/`S` menus · `Alt+1..9` tabs
**View/Tools** — `?`/`F1` help · `Shift+W` word count · `Ctrl+Shift+F` format
**Mouse** — click tabs/menus/[+] · click+drag to select · scroll wheel (vertical; Shift = horizontal) · click any `[ button ]` in popups

## Requirements

A modern terminal with UTF-8 and mouse support (kitty, wezterm, alacritty, foot, gnome-terminal, …). Truecolor is recommended for accurate theme colors.

## Development

See [AGENTS.md](AGENTS.md) and the full [Design Specification](docs/Rust-Edit-Design-Specification-2026-06-24.md).

Run on every change:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```