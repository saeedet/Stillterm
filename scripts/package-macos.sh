#!/bin/bash
# Package the already-built universal bundle; never install or publish it.
set -euo pipefail
if [[ $# -gt 1 ]]; then echo "Usage: $0 [output.dmg]" >&2; exit 1; fi
cd "$(dirname "$0")/.."
bundle="$PWD/target/macos/Stillterm.saver"
if [[ ! -d "$bundle" ]]; then echo "Build first with scripts/build-macos.sh" >&2; exit 1; fi
for arch in arm64 x86_64; do
  xcrun lipo "$bundle/Contents/MacOS/Stillterm" -verify_arch "$arch"
done
codesign --verify --strict "$bundle"
staging="$(mktemp -d "${TMPDIR:-/tmp}/stillterm-package.XXXXXX")"
trap 'rm -rf "$staging"' EXIT
ditto "$bundle" "$staging/Stillterm.saver"
cat > "$staging/Read Me.txt" <<'TEXT'
Stillterm — native character rain

Double-click Stillterm.saver to install for your user account, then select it in
System Settings. Use Options to choose Monochrome, Matrix, or Dense Matrix and
adjust the rain. In Words / phrases, separate entries with commas or line breaks:
FREE, YOUR, MIND
They appear vertically in that order at randomly chosen rain positions, then
repeat. Word frequency controls the pauses; leave the words blank to disable them.

This screensaver provides visuals. macOS controls locking and authentication.

Development builds are not notarized public releases. Distribution status and
source: https://github.com/saeedet/Stillterm

To uninstall, select another screensaver and move
~/Library/Screen Savers/Stillterm.saver to Trash.
TEXT
image="${1:-$PWD/target/macos/Stillterm.dmg}"
mkdir -p "$(dirname "$image")"
hdiutil create -ov -volname Stillterm -srcfolder "$staging" -format UDZO "$image"
if [[ -n "${STILLTERM_SIGN_IDENTITY:-}" && "$STILLTERM_SIGN_IDENTITY" != - ]]; then
  codesign --force --timestamp --sign "$STILLTERM_SIGN_IDENTITY" "$image"
fi
hdiutil verify "$image"
echo "Packaged $image (notarization and publishing are separate steps)"
