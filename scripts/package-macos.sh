#!/bin/bash
# Package the already-built universal bundle; never install or publish it.
set -euo pipefail
cd "$(dirname "$0")/.."
bundle="$PWD/target/macos/Stillterm.saver"
if [[ ! -d "$bundle" ]]; then echo "Build first with scripts/build-macos.sh" >&2; exit 1; fi
xcrun lipo "$bundle/Contents/MacOS/Stillterm" -verify_arch arm64 x86_64
codesign --verify --strict "$bundle"
staging="$(mktemp -d "${TMPDIR:-/tmp}/stillterm-package.XXXXXX")"
trap 'rm -rf "$staging"' EXIT
ditto "$bundle" "$staging/Stillterm.saver"
cat > "$staging/Read Me.txt" <<'TEXT'
Stillterm — native character rain

Double-click Stillterm.saver to install for your user account, then select it in
System Settings. Use Options to choose Monochrome or Matrix and adjust the rain.

This screensaver provides visuals. macOS controls locking and authentication.

Development builds are not notarized public releases. Distribution status and
source: https://github.com/saeedet/Stillterm

To uninstall, select another screensaver and move
~/Library/Screen Savers/Stillterm.saver to Trash.
TEXT
image="$PWD/target/macos/Stillterm.dmg"
hdiutil create -ov -volname Stillterm -srcfolder "$staging" -format UDZO "$image"
if [[ -n "${STILLTERM_SIGN_IDENTITY:-}" && "$STILLTERM_SIGN_IDENTITY" != - ]]; then
  codesign --force --timestamp --sign "$STILLTERM_SIGN_IDENTITY" "$image"
fi
hdiutil verify "$image"
echo "Packaged $image (notarization and publishing are separate steps)"
