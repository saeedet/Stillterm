#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
case "${1:---check}" in --check|--preview) mode="${1:---check}";; *) echo "Usage: $0 [--check|--preview]" >&2; exit 1;; esac
out="$PWD/target/macos"
bundle="$out/Stillterm.saver"
if [[ ! -d "$bundle" ]]; then echo "Build first with scripts/build-macos.sh" >&2; exit 1; fi
app="$out/Stillterm Preview.app"
mkdir -p "$app/Contents/MacOS"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>io.github.saeedet.Stillterm.Preview</string>
<key>CFBundleName</key><string>Stillterm Preview</string>
<key>CFBundleExecutable</key><string>preview-host</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
xcrun clang -fobjc-arc -fmodules -Wall -Wextra -Werror -O2 -mmacosx-version-min=11.0 \
  platforms/macos/PreviewHost.m -framework Cocoa -framework ScreenSaver -o "$app/Contents/MacOS/preview-host"
if [[ "$mode" == --preview ]]; then exec "$app/Contents/MacOS/preview-host" "$bundle"; fi
host="$(rustc -vV | sed -n 's/^host: //p')"
xcrun clang -fobjc-arc -fmodules -Wall -Wextra -Werror -O2 -mmacosx-version-min=11.0 \
  platforms/macos/SettingsTests.m platforms/macos/StilltermSettings.m \
  "target/$host/release/libstillterm_macos_bridge.a" -framework Cocoa -framework ScreenSaver -framework Security -liconv \
  -o "$out/settings-tests"
"$out/settings-tests"
xcrun clang -fobjc-arc -fmodules -Wall -Wextra -Werror -O2 -mmacosx-version-min=11.0 \
  platforms/macos/PreferenceProcessTests.m platforms/macos/StilltermSettings.m \
  "target/$host/release/libstillterm_macos_bridge.a" \
  -framework Cocoa -framework ScreenSaver -framework Security -liconv \
  -o "$out/preference-process-tests"
"$out/preference-process-tests"
"$app/Contents/MacOS/preview-host" "$bundle" --check "$out/preview.png"
