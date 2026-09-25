#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
WORKDIR=$(mktemp -d)
trap 'rm -rf "$WORKDIR"' EXIT
export HOME=$WORKDIR/home
export XDG_DATA_HOME=$HOME/.local/share
export XDG_CONFIG_HOME=$HOME/.config
mkdir -p "$HOME"
SYSTEM_BIN=$WORKDIR/system-bin
mkdir -p "$SYSTEM_BIN"
export RUST_EDIT_SYSTEM_BIN=$SYSTEM_BIN
FAKE_BIN=$WORKDIR/fake-bin
mkdir -p "$FAKE_BIN"
cat > "$FAKE_BIN/sudo" <<'EOF'
#!/bin/sh
# Test double. The real sudo would ask for a password and search a
# secure_path that does not include ~/.local/bin.
exec "$@"
EOF
chmod 755 "$FAKE_BIN/sudo"
export PATH="$FAKE_BIN:$PATH"

fail() { echo "FAIL: $*" >&2; exit 1; }

make_bundle() {
  ver=$1
  dir=$WORKDIR/bundle-$ver
  rm -rf "$dir"
  mkdir -p "$dir"
  cp "$ROOT/scripts/install.sh" "$dir/install.sh"
  chmod 755 "$dir/install.sh"
  printf '%s\n' "$ver" > "$dir/VERSION"
  printf 'logo\n' > "$dir/logo.svg"
  printf 'readme\n' > "$dir/README.md"
  printf 'license\n' > "$dir/LICENSE"
  # Unquoted delimiter so $ver is baked in. \${1:-} stays literal.
  cat > "$dir/rust-edit" <<EOF
#!/bin/sh
if [ "\${1:-}" = "--version" ]; then
  printf 'rust-edit %s\\n' "$ver"
  exit 0
fi
echo "editor should not start" >&2
exit 1
EOF
  chmod 755 "$dir/rust-edit"
  printf '%s\n' "$dir"
}

installed_version() {
  "$HOME/.local/bin/rust-edit" --version | awk '{ print $2 }'
}

bundle=$(make_bundle 0.1.0)
"$bundle/install.sh" </dev/null
[ "$(installed_version)" = "0.1.0" ] || fail "fresh install version"
[ ! -e "$SYSTEM_BIN/rust-edit" ] || fail "EOF install must not link into system bin"
desktop=$XDG_DATA_HOME/applications/rust-edit.desktop
grep -q '^Terminal=true$' "$desktop" || fail "Terminal=true"
grep -q '^Icon=rust-edit$' "$desktop" || fail "Icon"
grep -q '^Name=Rust-Edit$' "$desktop" || fail "Name"
grep -qx "Exec=$HOME/.local/bin/rust-edit %F" "$desktop" || fail "Exec"
[ -f "$XDG_DATA_HOME/icons/hicolor/scalable/apps/rust-edit.svg" ] || fail "icon"
grep -q '^logo$' "$XDG_DATA_HOME/icons/hicolor/scalable/apps/rust-edit.svg" || fail "icon bytes"
[ -f "$XDG_DATA_HOME/doc/rust-edit/README.md" ] || fail "readme"
[ -f "$XDG_DATA_HOME/doc/rust-edit/LICENSE" ] || fail "license"

bundle=$(make_bundle 0.2.0)
printf 'n\n' | "$bundle/install.sh"
[ "$(installed_version)" = "0.1.0" ] || fail "declined update"

printf 'y\n' | "$bundle/install.sh"
[ "$(installed_version)" = "0.2.0" ] || fail "accepted update"

printf '\n' | "$bundle/install.sh"
[ "$(installed_version)" = "0.2.0" ] || fail "declined reinstall"

bundle=$(make_bundle 0.1.0)
printf '\n' | "$bundle/install.sh"
[ "$(installed_version)" = "0.2.0" ] || fail "declined downgrade"

bundle=$(make_bundle 0.3.0)
"$bundle/install.sh" --yes </dev/null
[ "$(installed_version)" = "0.3.0" ] || fail "--yes update"
[ -L "$SYSTEM_BIN/rust-edit" ] || fail "yes install should symlink system bin"
[ "$(readlink "$SYSTEM_BIN/rust-edit")" = "$HOME/.local/bin/rust-edit" ] || fail "symlink target"

[ -x "$HOME/.local/bin/rust-edit-uninstall" ] || fail "uninstaller missing"
[ -x "$XDG_DATA_HOME/doc/rust-edit/install.sh" ] || fail "installed install.sh"

mkdir -p "$XDG_CONFIG_HOME/rust-edit"
printf 'keep\n' > "$XDG_CONFIG_HOME/rust-edit/config.toml"
printf 'n\n' | "$HOME/.local/bin/rust-edit-uninstall"
[ "$(installed_version)" = "0.3.0" ] || fail "declined uninstall"
printf 'y\ny\n' | "$HOME/.local/bin/rust-edit-uninstall"
[ ! -e "$HOME/.local/bin/rust-edit" ] || fail "binary removed"
[ ! -e "$HOME/.local/bin/rust-edit-uninstall" ] || fail "uninstaller removed"
[ ! -e "$XDG_DATA_HOME/doc/rust-edit/install.sh" ] || fail "installed install.sh removed"
[ ! -e "$desktop" ] || fail "desktop removed"
[ ! -e "$XDG_DATA_HOME/icons/hicolor/scalable/apps/rust-edit.svg" ] || fail "icon removed"
grep -q '^keep$' "$XDG_CONFIG_HOME/rust-edit/config.toml" || fail "config preserved"
[ ! -e "$SYSTEM_BIN/rust-edit" ] || fail "system link removed"

empty=$WORKDIR/empty
mkdir -p "$empty"
cp "$ROOT/scripts/install.sh" "$empty/install.sh"
if "$empty/install.sh" </dev/null; then
  fail "missing payload should fail"
fi

trashed=$WORKDIR/fake/.local/share/Trash/files/bundle
mkdir -p "$trashed"
cp "$ROOT/scripts/install.sh" "$trashed/install.sh"
chmod 755 "$trashed/install.sh"
printf '0.1.0\n' > "$trashed/VERSION"
printf 'logo\n' > "$trashed/logo.svg"
printf 'readme\n' > "$trashed/README.md"
printf 'license\n' > "$trashed/LICENSE"
printf '#!/bin/sh\nprintf "rust-edit 0.1.0\\n"\n' > "$trashed/rust-edit"
chmod 755 "$trashed/rust-edit"
if "$trashed/install.sh" --yes </dev/null; then
  fail "installer in the trash should fail"
fi

bundle=$(make_bundle 0.4.0)
printf 'other\n' > "$SYSTEM_BIN/rust-edit"
"$bundle/install.sh" --yes </dev/null
[ "$(installed_version)" = "0.4.0" ] || fail "home install continues when system path is occupied"
[ ! -L "$SYSTEM_BIN/rust-edit" ] || fail "must not replace a real file"
grep -q '^other$' "$SYSTEM_BIN/rust-edit" || fail "foreign file changed"
rm -f "$SYSTEM_BIN/rust-edit"
printf 'y\n' | "$HOME/.local/bin/rust-edit-uninstall"

echo "install_bundle ok"
