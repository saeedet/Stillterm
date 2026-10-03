#!/bin/bash
# Package the CLI and optional Hyprland adapter; never install desktop settings.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Linux ]]; then
  echo "Build this development archive on Linux." >&2
  exit 1
fi
export CARGO_TARGET_DIR="$PWD/target"
cargo build --release --locked -p stillterm
out="$PWD/target/linux"
mkdir -p "$out"
stage=$(mktemp -d "$out/staging.XXXXXX")
trap 'rm -rf "$stage"' EXIT
install -m755 target/release/stillterm "$stage/stillterm"
install -m755 platforms/linux/stillterm-hyprland "$stage/stillterm-hyprland"
cp LICENSE "$stage/LICENSE"
cp docs/linux.md "$stage/linux.md"
cat > "$stage/README.txt" <<'README'
Stillterm GNU/Linux development build

Requires Python 3.10+, Foot 1.27+, and Hyprland for the optional desktop adapter.
See BUILD-INFO.txt for architecture, compiler, and C library requirements.

From this extracted directory, install for the current user:
  install -Dm755 stillterm "$HOME/.local/bin/stillterm"
  install -Dm755 stillterm-hyprland "$HOME/.local/bin/stillterm-hyprland"
Ensure ~/.local/bin is on the graphical session's PATH. Then try:
  stillterm --theme matrix
  stillterm-hyprland doctor
  stillterm-hyprland start -- --theme matrix

Read linux.md for optional Hypridle setup, removal, and validation limits.
Omarchy 4.0.4: manual preview only; automatic idle replacement is unsupported.
No Linux desktop acceptance testing has been completed. This is not a release.
README
{
  git describe --always --dirty
  rustc --version
  uname -sm
  ldd --version 2>&1 | sed -n '1p'
} > "$stage/BUILD-INFO.txt"
archive="Stillterm-linux-$(uname -m)-development.tar.gz"
tar -czf "$out/$archive" -C "$stage" stillterm stillterm-hyprland LICENSE README.txt linux.md BUILD-INFO.txt
(cd "$out" && sha256sum "$archive" > SHA256SUMS.txt)
echo "Built $out/$archive"
