#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
WORKDIR=$(mktemp -d)
trap 'rm -rf "$WORKDIR"' EXIT
export HOME=$WORKDIR/home
export XDG_DATA_HOME=$HOME/.local/share
export XDG_CONFIG_HOME=$HOME/.config
mkdir -p "$HOME"

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

mkdir -p "$XDG_CONFIG_HOME/rust-edit"
printf 'keep\n' > "$XDG_CONFIG_HOME/rust-edit/config.toml"
printf 'y\n' | "$bundle/install.sh" --uninstall
[ ! -e "$HOME/.local/bin/rust-edit" ] || fail "binary removed"
[ ! -e "$desktop" ] || fail "desktop removed"
[ ! -e "$XDG_DATA_HOME/icons/hicolor/scalable/apps/rust-edit.svg" ] || fail "icon removed"
grep -q '^keep$' "$XDG_CONFIG_HOME/rust-edit/config.toml" || fail "config preserved"

empty=$WORKDIR/empty
mkdir -p "$empty"
cp "$ROOT/scripts/install.sh" "$empty/install.sh"
if "$empty/install.sh" </dev/null; then
  fail "missing payload should fail"
fi

echo "install_bundle ok"
