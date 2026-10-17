# macOS screensaver

Stillterm builds a native `Stillterm.saver` with the same Rust rain engine as the
CLI. It includes Monochrome, Matrix, Dense Matrix, an experimental 3D corridor, and
a native options sheet. This is a source-build preview, not a signed public release.

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

The panel includes **Words / phrases**, one per line, and a **Word frequency** selector.
Empty text disables phrases. Up to 16 phrases of 64 printable, single-column
characters are supported. Theme changes keep your phrases and frequency.
Letters stay fixed vertically in their column. The passing stream head
reveals them one by one, holding each bright until the entire word is lit. Then
letters fade from top to bottom. The first attempt is after roughly 3–5 seconds;
actual timing also depends on rain speed and density. Phrases taller than the
preview are skipped; try full-screen preview or a shorter word. Save applies the
change; Cancel preserves the previous settings.
The text stays in local preferences.

Build 11 gives **Dense Matrix** three overlapping depth layers. Small, slower,
softer characters sit behind larger, brighter streams and glowing heads. Trails
remain fuller near the top and fade into black toward the bottom. Words appear
only in the near layer, using the same progressive reveal and ordered fade.

Selecting Dense Matrix sets base character size `10`; another theme restores `18`.
Adjust size after choosing a theme. Phrases and frequency are preserved. The Mac
renderer batches glyphs by font and color, retaining clipping for unusual fallback
characters. All three grids share one canvas; the two distant layers share a
bounded bitmap refreshed at up to 15 FPS. See [performance measurements](performance.md).

The current build is for local evaluation. The disk image submitted for
notarization on 2026-10-17 is the preserved build 5, without phrases; the current build needs
its own submission before public distribution.

Preferences use Apple's `ScreenSaverDefaults`, under
`io.github.saeedet.Stillterm`, independently of CLI TOML files. Each native view
owns its engine and refreshes the ScreenSaverDefaults cache before loading
preferences on its next start. Changes in a different
host process take effect when that screensaver instance restarts.

## Experimental 3D corridor

Build 12 adds **Matrix 3D (experimental)** to Options. It renders a procedural
corridor using Rust/wgpu and a WGSL shader, presented through a native CAMetalLayer.
Four independently seeded rain sheets cover the walls, floor, and ceiling. Slow
camera movement creates perspective and parallax; distance fog and a small glow
kernel soften the code. Custom glyphs are rasterized into an atlas using Core Text;
words reveal on the two walls with the shared engine's progressive timing.

Choose the mode and Save to keep it. A temporary demo preserves saved preferences:

```sh
# Build first, then run scripts/check-macos.sh to create the preview host.
"target/macos/Stillterm Preview.app/Contents/MacOS/preview-host" \
  target/macos/Stillterm.saver --corridor
```

The demo uses seed 42 and `FREE` for repeatable comparison. Speed controls rain and
camera movement; character size changes surface spacing. Brightness and density
retain their usual controls. Rendering resolution is capped near 2.1 million GPU
pixels and scaled to the view; the default is 30 FPS. The windowed prototype has
been checked locally on Apple M3. Installed full-screen/idle activation, additional
GPU hardware, long sessions, and energy use still need testing. Windows and the
terminal keep their existing rain modes.

GPU resources are released explicitly on stop, sleep, hidden/detached views, and
zero-sized surfaces. An initialization or rendering failure falls back to Dense
Matrix with a short on-screen message. A diagnostic check renders a GPU image,
checks animation, remote-host visibility, resize, sleep/wake, repeated cleanup,
and fallback. It reports a skipped GPU check if Metal is unavailable, while still
checking that fallback draws rain. Normal animation never reads GPU pixels back.

```sh
"target/macos/Stillterm Preview.app/Contents/MacOS/preview-host" \
  target/macos/Stillterm.saver --check-corridor target/macos/corridor-preview.png
```

The snapshot comes directly from the renderer; screen-recording permission is not
needed. See [measurements](performance.md#gpu-corridor-prototype-build-12).

## Checks and limits

```sh
scripts/build-macos.sh
scripts/check-macos.sh
```

Checks load the actual bundle into a temporary native window and exercise drawing,
resizing (including an empty surface), independent instances, repeated start/stop,
late callbacks, retained views after dismissal, remote-host window visibility,
hidden views, simulated sleep/wake notifications, and the options sheet. A separate
memory check retains 12 stopped, layer-backed views at 1600×900 points and verifies
that the process footprint stays bounded after warm-up and a two-second settling
interval for asynchronous graphics cleanup. Transient peaks are reported separately.
These checks run in CI.
Preference tests use disposable, isolated domains and a separate writer process
to check switching from Matrix to Monochrome and back. The pixel check verifies
that the Matrix theme produces green glyphs through Core Text. A native frame is
saved at `target/macos/preview.png` for inspection. Dense checks also compare the
batched image with ordinary Core Text drawing, verify three independent grids,
resize and sleep cleanup, report large-window CPU/frame rate,
and repeat the retained-view memory check using Dense Matrix. These are standalone
host measurements; installed full-screen and longer-session validation remain pending.

To repeat only the dense measurement after building and running the checks:

```sh
"target/macos/Stillterm Preview.app/Contents/MacOS/preview-host" \
  target/macos/Stillterm.saver --measure-dense target/macos/dense-preview.png
```

Use `--dense` instead for a temporary interactive demo. It changes only that
preview instance; select **Options → Dense Matrix → Save** to keep it as your
screensaver theme.

Local functional validation on 2026-10-01: Apple M3, macOS 14.6.1 (23G93),
Xcode 15.4, Rust 1.88.0. Installed-screensaver checks covered installation,
System Settings and full-screen previews, options and theme switching, idle
activation, two physical displays, sleep/wake, and repeated dismissal. After the
final repeated-preview test on build 4, the helper measured 0.0% CPU across three
samples, with logs confirming that retained instances stopped rendering. This is
an after-dismissal measurement, not an active-rendering performance claim.

A longer-session check on 2026-10-02 found a 673 MiB idle host footprint despite
0% CPU and matching animation start/stop events. `vmmap` attributed about 595 MiB
to IOSurface display buffers and 41 MiB to Core Animation; ordinary heap allocations
were about 8 MiB. Most retained writable memory was reported in the swapped category.
This is consistent with the legacy host retaining drawing surfaces after dismissal.
Stopping animation does not establish that the host released those surfaces.
Build 5 moves drawing into a removable child view and gives the outer view a
bitmap-free black background. Stop, sleep, hiding, or detachment removes the
child along with the engine resources. The next active frame recreates it. This
uses AppKit's layer update path without modifying the host window or terminating
its process. See [Apple's layer drawing guidance](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/CoreAnimation_guide/SettingUpLayerObjects/SettingUpLayerObjects.html).

On the same M3 / macOS 14.6.1 on 2026-10-03, the retained-view check reproduced
build 4's growth to about 885 MiB across 12 stopped views. Build 5 stayed near
228 MiB during the check and settled near 184 MiB. The measurement is the whole
test process footprint, including AppKit and display buffers, at 2× backing scale;
it is not the installed helper's memory usage. Drawing/lifecycle checks and the
memory check passed for arm64 and x86_64 (under Rosetta).
Installed build 5 was then exercised through three full-screen previews on the
same two-display Mac. Logs confirmed 12 starts and 12 stops across retained
instances. After System Settings closed, the existing helper held at 117.2 MiB
and 0.0% CPU across three samples, without restarting the process. `vmmap` showed
one 45.6 MiB IOSurface allocation, versus 22 totaling about 595 MiB in the earlier
long-session report. Its peak footprint was still 850.3 MiB during this session;
macOS restarts multiple retained instances on later previews. This short test
supports improved cleanup after dismissal, not a sustained active-memory bound.
**Long-session memory and active-rendering performance validation remain pending.**

The bundle contains arm64 and x86_64 code. Both slices passed the native host
checks locally (the Intel slice under Rosetta); physical Intel hardware remains
untested. The deployment target is macOS 11, not a claim of testing every release
since then. CI builds both slices and runs the native checks on its macOS runner.

Remaining release validation:

- Physical Intel hardware and additional named macOS versions.
- Broader display configurations, scaling, and display connection changes.
- Sustained CPU/memory at full resolution over longer sessions.
- Download, Gatekeeper, install, upgrade, and uninstall on a clean Mac using the
  final signed and notarized package.

Standalone host checks do not replace these installed-system checks. Apple documents
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

Developer ID signing and Keychain-based notarization credentials are configured
for the first macOS prerelease. Source builds and contributions do not require a
paid account.

On 2026-10-17, the universal 0.1.0 / build 5 bundle and disk image passed Developer
ID signature verification. Native drawing, settings, lifecycle, and retained-view
memory checks passed on arm64 with macOS 27.0.1 (26A434) and Xcode 27.0. The disk
image has been submitted to Apple; notarization, stapling, and final downloaded
installation checks are pending. Nothing has been published as a GitHub Release.

Local builds are ad-hoc signed by default. They are suitable for development,
not a substitute for Developer ID signing and notarization. Do not remove quarantine or disable
Gatekeeper to distribute an unnotarized download.

The intended download is `Stillterm.dmg` on GitHub Releases. Users can install
it without Rust or Xcode. Source builds remain available independently.

For the signed download, a maintainer needs Apple Developer Program membership,
a **Developer ID Application** certificate with its private key in the local
Keychain, and notarization credentials stored in a Keychain profile. Keep private
keys and credentials out of Git. See [Apple membership](https://developer.apple.com/support/compare-memberships/)
and [Developer ID](https://developer.apple.com/developer-id/).

To configure notarization, create an app-specific password in your Apple Account
and store it directly in Keychain. Substitute the enrolled account email and Team
ID below. The command prompts securely for the app-specific password; keep it out
of shell arguments, repository files, and chat.

```sh
xcrun notarytool store-credentials stillterm-notary \
  --apple-id 'YOUR_APPLE_ACCOUNT_EMAIL' --team-id YOUR_TEAM_ID
```

With the signing identity installed, build and package:

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
installation is tested. Credentials stay outside the repository. Signing and native
host checks are verified locally; Apple approval and final downloaded installation are separate checks. Then create
a versioned GitHub Release with the verified disk image, its SHA-256
checksum, installation instructions, and the exact tested macOS versions. Start
with a prerelease while broader compatibility testing is pending. CI artifacts
are development builds and are not automatically published as GitHub Releases.
[Apple distribution guidance](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).

If a preview stays black after an update, confirm macOS loaded the new build.
Stillterm logs its bundle build number at start and reports when its first frame
is ready. These messages can be found in Console by searching for `Stillterm`.
Close System Settings before replacing the bundle. If it keeps loading an older
build, log out and back in to clear the retained screensaver host.
