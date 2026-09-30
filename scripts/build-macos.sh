#!/bin/bash
# Build a local .saver; signing identity is optional (ad-hoc by default).
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Darwin ]]; then echo "This build requires macOS and Xcode tools." >&2; exit 1; fi
case "${1:---universal}" in
  --native) targets=("$(rustc -vV | sed -n 's/^host: //p')");;
  --universal) targets=(aarch64-apple-darwin x86_64-apple-darwin);;
  *) echo "Usage: $0 [--universal|--native]" >&2; exit 1;;
esac
out="$PWD/target/macos"
bundle="$out/Stillterm.saver"
mkdir -p "$out" "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
# Keep all tool output in the workspace, including when CARGO_TARGET_DIR is set.
export CARGO_TARGET_DIR="$PWD/target"
export MACOSX_DEPLOYMENT_TARGET=11.0
sdk="$(xcrun --sdk macosx --show-sdk-path)"
binaries=()
for target in "${targets[@]}"; do
  case "$target" in
    aarch64-apple-darwin) arch=arm64;;
    x86_64-apple-darwin) arch=x86_64;;
    *) echo "Unsupported macOS target: $target" >&2; exit 1;;
  esac
  if ! rustup target list --installed | /usr/bin/grep -qx "$target"; then
    echo "Install the target first: rustup target add $target" >&2; exit 1
  fi
  cargo build --release --locked -p stillterm-macos-bridge --target "$target"
  xcrun clang -arch "$arch" -isysroot "$sdk" -mmacosx-version-min=11.0 \
    -fobjc-arc -fmodules -Wall -Wextra -Werror -O2 -bundle \
    platforms/macos/StilltermView.m platforms/macos/StilltermSettings.m \
    "target/$target/release/libstillterm_macos_bridge.a" \
    -framework Cocoa -framework ScreenSaver -framework CoreText -framework QuartzCore \
    -framework Security -liconv -o "$out/Stillterm-$arch"
  binaries+=("$out/Stillterm-$arch")
done
xcrun lipo -create "${binaries[@]}" -output "$bundle/Contents/MacOS/Stillterm"
cp platforms/macos/Info.plist "$bundle/Contents/Info.plist"
cp LICENSE "$bundle/Contents/Resources/LICENSE"
# Only Objective-C metadata and the small C ABI need visible symbols in this plug-in.
/usr/bin/strip -x "$bundle/Contents/MacOS/Stillterm"
identity="${STILLTERM_SIGN_IDENTITY:--}"
if [[ "$identity" == - ]]; then
  codesign --force --sign - "$bundle"
else
  codesign --force --options runtime --timestamp --sign "$identity" "$bundle"
fi
codesign --verify --strict "$bundle"
plutil -lint "$bundle/Contents/Info.plist"
lipo -archs "$bundle/Contents/MacOS/Stillterm"
echo "Built $bundle"
