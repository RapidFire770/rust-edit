#!/bin/sh
# Install rust-edit into the current user's home folder.
set -eu

usage() {
  cat <<'EOF'
Usage: ./install.sh [--yes] [--uninstall]

Install rust-edit into ~/.local/bin and add a Rust-Edit application
menu entry. The desktop opens that entry in your default terminal.

  --yes        answer yes to update, reinstall, downgrade, and uninstall
  --uninstall  remove the program, menu entry, icon, and bundled docs

After installation, run rust-edit-uninstall from any directory. Settings
under ~/.config/rust-edit are left in place. Run this as your user,
without sudo.
EOF
}

YES=0
UNINSTALL=0
for arg in "$@"; do
  case "$arg" in
    --yes) YES=1 ;;
    --uninstall) UNINSTALL=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $arg" >&2; usage >&2; exit 1 ;;
  esac
done

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "This installer needs '$1'." >&2
    exit 1
  fi
}
need sort
need timeout
need setsid
if ! printf '1.0.0\n' | sort -V >/dev/null 2>&1; then
  echo "This installer needs GNU sort (sort -V)." >&2
  exit 1
fi

if [ "$(id -u)" -eq 0 ]; then
  echo "Run ./install.sh as your user, without sudo. It installs into your home folder." >&2
  exit 1
fi

ROOT=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
: "${HOME:?HOME is not set}"
BIN_DIR=$HOME/.local/bin
BIN=$BIN_DIR/rust-edit
UNINSTALLER=$BIN_DIR/rust-edit-uninstall
DATA=${XDG_DATA_HOME:-$HOME/.local/share}
APP_DIR=$DATA/applications
ICON_DIR=$DATA/icons/hicolor/scalable/apps
DOC_DIR=$DATA/doc/rust-edit
DESKTOP=$APP_DIR/rust-edit.desktop
ICON=$ICON_DIR/rust-edit.svg
INSTALLED_SCRIPT=$DOC_DIR/install.sh
CONFIG_DIR=${XDG_CONFIG_HOME:-$HOME/.config}/rust-edit

version_cmp() {
  if [ "$1" = "$2" ]; then
    printf '0\n'
    return
  fi
  higher=$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -n 1)
  if [ "$higher" = "$1" ]; then
    printf '1\n'
  else
    printf -- '-1\n'
  fi
}

probe_version() {
  timeout 2s setsid "$1" --version </dev/null 2>/dev/null || true
}

parse_version_line() {
  printf '%s\n' "$1" | awk '$1 == "rust-edit" && $2 ~ /^[0-9]+\.[0-9]+\.[0-9]+$/ { print $2; exit }'
}

confirm() {
  prompt=$1
  default_yes=$2
  if [ "$YES" -eq 1 ]; then
    return 0
  fi
  if [ -t 0 ]; then
    printf '%s ' "$prompt"
  fi
  if ! IFS= read -r answer; then
    echo "Re-run with --yes to answer: $prompt" >&2
    return 1
  fi
  case "$answer" in
    y|Y|yes|YES) return 0 ;;
    n|N|no|NO) return 1 ;;
    "")
      if [ "$default_yes" -eq 1 ]; then
        return 0
      fi
      return 1
      ;;
    *)
      echo "Please answer y or n." >&2
      return 1
      ;;
  esac
}

quote_exec_path() {
  case "$1" in
    *[[:space:]\"\\\$\`]*)
      printf '"%s"' "$(printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/`/\\`/g; s/\$/\\$/g')"
      ;;
    *)
      printf '%s' "$1"
      ;;
  esac
}

refresh_caches() {
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1 && [ -d "$DATA/icons/hicolor" ]; then
    gtk-update-icon-cache -f -t "$DATA/icons/hicolor" >/dev/null 2>&1 || true
  fi
}

uninstall() {
  if ! confirm "Remove rust-edit from your home folder? [y/N]" 0; then
    echo "Uninstall cancelled."
    exit 0
  fi
  rm -f "$BIN" "$UNINSTALLER" "$DESKTOP" "$ICON" \
    "$DOC_DIR/README.md" "$DOC_DIR/LICENSE" "$INSTALLED_SCRIPT"
  rmdir "$DOC_DIR" 2>/dev/null || true
  refresh_caches
  echo "Removed rust-edit. Settings in $CONFIG_DIR were left in place."
  exit 0
}

if [ "$UNINSTALL" -eq 1 ]; then
  uninstall
fi

for payload in rust-edit logo.svg README.md LICENSE VERSION; do
  if [ ! -f "$ROOT/$payload" ]; then
    echo "Missing $payload next to install.sh. Unpack the release archive and run ./install.sh from inside it." >&2
    exit 1
  fi
done

pkg=$(tr -d '[:space:]' < "$ROOT/VERSION")
printf '%s\n' "$pkg" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || {
  echo "VERSION must look like 1.2.3 (found '$pkg')." >&2
  exit 1
}

probed=$(parse_version_line "$(probe_version "$ROOT/rust-edit")")
if [ "$probed" != "$pkg" ]; then
  echo "This bundle's rust-edit did not report version $pkg." >&2
  echo "It may be built for a different system." >&2
  exit 1
fi

if [ -x "$BIN" ]; then
  old=$(parse_version_line "$(probe_version "$BIN")")
  if [ -z "$old" ]; then
    if ! confirm "A rust-edit binary is already in $BIN_DIR but it did not report a version. Replace it with $pkg? [y/N]" 0; then
      echo "Left the installed rust-edit in place."
      exit 0
    fi
  else
    cmp=$(version_cmp "$old" "$pkg")
    case "$cmp" in
      -1)
        if ! confirm "Installed rust-edit $old. This package is $pkg. Update? [Y/n]" 1; then
          echo "Left rust-edit $old in place."
          exit 0
        fi
        ;;
      0)
        if ! confirm "rust-edit $pkg is already installed. Reinstall? [y/N]" 0; then
          echo "Left rust-edit $pkg in place."
          exit 0
        fi
        ;;
      1)
        if ! confirm "Installed rust-edit $old is newer than this package ($pkg). Downgrade? [y/N]" 0; then
          echo "Left rust-edit $old in place."
          exit 0
        fi
        ;;
    esac
  fi
fi

install -d "$BIN_DIR" "$APP_DIR" "$ICON_DIR" "$DOC_DIR"
install -m 0755 "$ROOT/rust-edit" "$BIN"
install -m 0755 "$ROOT/install.sh" "$INSTALLED_SCRIPT"
install -m 0644 "$ROOT/logo.svg" "$ICON"
install -m 0644 "$ROOT/README.md" "$DOC_DIR/README.md"
install -m 0644 "$ROOT/LICENSE" "$DOC_DIR/LICENSE"
script_field=$(quote_exec_path "$INSTALLED_SCRIPT")
cat > "$UNINSTALLER" <<EOF
#!/bin/sh
exec $script_field --uninstall "\$@"
EOF
chmod 755 "$UNINSTALLER"

exec_field=$(quote_exec_path "$BIN")
cat > "$DESKTOP" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=Rust-Edit
GenericName=Text Editor
Comment=Terminal text and code editor
Exec=$exec_field %F
TryExec=$exec_field
Icon=rust-edit
Terminal=true
Categories=Utility;TextEditor;
Keywords=editor;text;code;
StartupNotify=false
EOF
chmod 644 "$DESKTOP"
refresh_caches

echo "Installed rust-edit $pkg"
echo "  Program: $BIN"
echo "  Remove:  rust-edit-uninstall"
echo "  Menu:    Rust-Edit (opens in your default terminal)"
echo "  Icon:    $ICON"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    echo "$BIN_DIR is not on PATH in this shell."
    echo "Add this line to ~/.profile, then open a new terminal:"
    echo '  export PATH="$HOME/.local/bin:$PATH"'
    ;;
esac
echo "If the menu entry does not appear, log out and back in."
