# macOS screensaver

Stillterm builds a native `Stillterm.saver` with the same Rust rain engine as the
CLI. It includes Monochrome and Matrix themes, a small preview, and an options
sheet. This is a source-build preview, not a signed public release.

## Build and try

Requires macOS, Rust via rustup, and Xcode or its command-line tools. From the
repository root:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
scripts/build-macos.sh
scripts/check-macos.sh --preview
```

The build produces `target/macos/Stillterm.saver`, containing both architectures.
For a quicker build on this Mac only, use `scripts/build-macos.sh --native`.
The preview runs in its own window; close it to quit. Its **Options…** button uses
the same preferences as the screensaver. Nothing changes your selected screensaver
or password settings automatically.

To install, open the bundle and choose the per-user installation when macOS asks:

```sh
open target/macos/Stillterm.saver
```

Select Stillterm in System Settings. The location depends on macOS: Screen Saver
on Sonoma/Sequoia, or the screen saver dialog under Wallpaper on Tahoe. Use the
system's preview and **Options** controls. Set password requirements separately
in macOS Lock Screen settings; Stillterm provides visuals, not authentication.

Per-user installation goes under `~/Library/Screen Savers/Stillterm.saver`.
To uninstall, select a different screensaver and move only that bundle to Trash.
After replacing an installed build, close and reopen System Settings; a running
screensaver host can retain the old code until logout/login.

## Options

The panel provides theme, speed, density, brightness, FPS, character size, seed,
and an optional character set. Changing themes resets the visual fields to that
theme's preset. Blank characters use the preset; custom characters follow the
CLI's single-column validation. Monochrome is the default.

Preferences use Apple's `ScreenSaverDefaults`, under
`io.github.saeedet.Stillterm`, independently of CLI TOML files. Each native view
owns its engine and refreshes the ScreenSaverDefaults cache before loading
preferences on its next start. Changes in a different
host process take effect when that screensaver instance restarts.

## Checks and limits

```sh
scripts/build-macos.sh
scripts/check-macos.sh
```

Checks load the actual bundle into a temporary native window and exercise drawing,
resizing (including an empty surface), independent instances, repeated start/stop,
late callbacks, retained views after dismissal, remote-host window visibility,
hidden views, simulated sleep/wake
notifications, and the options
sheet. Preference tests use disposable, isolated domains and a separate writer process
to check switching from Matrix to Monochrome and back. The pixel check verifies
that the Matrix theme produces green glyphs through Core Text. A native frame is
saved at `target/macos/preview.png` for inspection.

Local validation: Apple M3, macOS 14.6.1 (23G93), Xcode 15.4, Rust 1.88.0. The bundle
contains arm64 and x86_64 code. Both slices passed the native host checks locally
(the Intel slice under Rosetta); physical Intel hardware remains untested. The deployment target is macOS 11, not a claim of testing every
release since then. CI builds both slices and runs the native checks on its macOS
runner.

Before a public release, test the **installed system-hosted** screensaver on named
macOS builds and physical Intel/Apple Silicon machines:

- Install, System Settings preview, save/cancel options, uninstall and upgrade.
- Multiple physical displays, mixed scaling, display connection changes.
- Actual sleep/wake and repeated dismissal; confirm no ongoing rendering after exit.
- Sustained CPU/memory at full display resolution and Gatekeeper on a clean Mac.

The standalone host does not establish all of those behaviors. Apple documents
that callbacks can arrive after `stopAnimation`; Stillterm explicitly ignores them,
frees frame resources on stop/sleep, and avoids rendering hidden or detached views.
Window visibility and occlusion flags are not used to gate animation: a remote
host can display a surface even when its local window reports invisible. Apple's
forums also track host lifecycle and multi-display regressions.

On Sonoma, the host can retain full-screen views without calling `stopAnimation`,
leaving them drawing after dismissal. Stillterm also listens for the undocumented
`com.apple.screensaver.willstop` distributed notification to stop its own
full-screen animation and release frame resources. Settings previews keep running;
sleep/wake cannot restart dismissed instances. This is a best-effort compatibility
workaround, not a guaranteed API contract. It does not terminate the host or change
lock-screen behavior, and macOS can still retain inactive view objects.
The [Aerial minimal template](https://github.com/AerialScreensaver/ScreenSaverMinimal#about-sonoma)
documents the underlying host issue and dismissal signal.
[Lifecycle contract](https://developer.apple.com/documentation/screensaver),
[host reports](https://developer.apple.com/forums/thread/787444).

## Public distribution

Local builds are ad-hoc signed. They are suitable for development, not a substitute
for Developer ID signing and notarization. Do not remove quarantine or disable
Gatekeeper to distribute an unnotarized download.

A maintainer with a Developer ID Application identity can build and package:

```sh
export STILLTERM_SIGN_IDENTITY='Developer ID Application: Your Name (TEAMID)'
scripts/build-macos.sh
scripts/package-macos.sh
```

The packaging script creates a signed disk image when that identity is set,
otherwise an unsigned development disk image. For a public release, submit the
image using a previously configured Keychain profile, then staple it:

```sh
xcrun notarytool submit target/macos/Stillterm.dmg --keychain-profile stillterm-notary --wait
xcrun stapler staple target/macos/Stillterm.dmg
xcrun stapler validate target/macos/Stillterm.dmg
shasum -a 256 target/macos/Stillterm.dmg
```

Publish only after notarization reports **Accepted**, stapling passes, and clean-Mac
installation is tested. Credentials stay outside the repository. These distribution
steps remain unverified until a maintainer supplies a signing identity; CI artifacts
are development builds and are not automatically published as GitHub Releases.
[Apple distribution guidance](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).

If a preview stays black after an update, confirm macOS loaded the new build.
Stillterm logs its bundle build number at start and reports when its first frame
is ready. These messages can be found in Console by searching for `Stillterm`.
Close System Settings before replacing the bundle. If it keeps loading an older
build, log out and back in to clear the retained screensaver host.
