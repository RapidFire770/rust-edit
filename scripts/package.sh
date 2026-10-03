#!/bin/sh
# Build a home-folder release bundle for this machine's architecture.
set -eu
cd "$(dirname "$0")/.."

if [ -n "${CARGO:-}" ] && [ -x "$CARGO" ]; then
  :
elif command -v cargo >/dev/null 2>&1; then
  CARGO=$(command -v cargo)
elif [ -x "$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo" ]; then
  CARGO=$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo
else
  echo "cargo not found. Set CARGO to the toolchain binary." >&2
  exit 1
fi

version=$(awk '
  /^\[package\]/ { in_pkg = 1; next }
  /^\[/ { in_pkg = 0 }
  in_pkg && /^version[[:space:]]*=/ {
    gsub(/"/, "", $3)
    print $3
    exit
  }
' Cargo.toml)

# `cargo rustc -vV` is rejected by cargo. `cargo -vV` prints the host triple.
triple=$("$CARGO" -vV | awk '/^host: / { print $2; exit }')
if [ -z "$triple" ]; then
  echo "Could not detect the rustc host triple from '$CARGO -vV'." >&2
  exit 1
fi
name="rust-edit-${version}-${triple}"
stage="dist/$name"
target_dir=${CARGO_TARGET_DIR:-target}

rm -rf "$stage"
mkdir -p "$stage"
"$CARGO" build --release
cp "$target_dir/release/rust-edit" "$stage/rust-edit"
chmod 755 "$stage/rust-edit"
cp docs/logo.svg "$stage/logo.svg"
cp README.md "$stage/README.md"
cp LICENSE "$stage/LICENSE"
cp assets/dictionaries/en.LICENSE "$stage/en.LICENSE"
cp scripts/install.sh "$stage/install.sh"
chmod 755 "$stage/install.sh"
printf '%s\n' "$version" > "$stage/VERSION"
tar -C dist -czf "dist/${name}.tar.gz" "$name"
echo "Wrote dist/${name}.tar.gz"
