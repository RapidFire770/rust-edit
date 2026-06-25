# rust-edit

A fast, keyboard-first text and code editor for the Linux terminal, built with Ratatui.

**Status:** Early development (Phase 1 / MVP in progress)

Inspired by the UI mockup in `docs/ui-design-sample-2.jpg`.

## Features (Target for Phase 1)

- Tabbed multi-file editing
- Syntax highlighting (many languages via syntect)
- Smart indentation
- Find & replace
- Markdown source editing + formatting toolbar + preview
- Theming (Dark Red default + others) with live switch
- Mouse support (optional)
- Configurable (TOML + XDG)

## Build & Run

```bash
cargo build --release
./target/release/rust-edit                 # empty buffer
./target/release/rust-edit src/main.rs     # open file(s)
```

## Keyboard (MVP subset)

See in-app help (`?` or `F1`) once implemented. Core:

- `Ctrl+Q` quit, `Ctrl+S` save, `Ctrl+O` open, `Ctrl+N` new, `Ctrl+W` close tab
- `Ctrl+F` find, `Ctrl+H` find/replace
- Arrow / word / Home/End navigation + selection
- `Alt+F/T/S` menus

## Requirements

Modern terminal with UTF-8 + mouse (kitty, wezterm, alacritty, etc.). Truecolor recommended.

## Development

See [AGENTS.md](AGENTS.md) and the full [Design Specification](docs/Rust-Edit-Design-Specification-2026-06-24.md).

Run on every change:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```
