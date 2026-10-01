# Platform support

The terminal application and a macOS screensaver source-build preview are implemented.
Windows and Omarchy native integration remain planned.
Local validation used macOS 14.6.1 on Apple M3. Rust tests and PTY lifecycle checks
passed there; Windows/Linux compilation checks and native CI complement this but
do not replace interactive testing on those systems.

## macOS — Milestone 2

Apple's documented third-party mechanism is a `.saver` bundle containing a
`ScreenSaverView` subclass. Install per user under `~/Library/Screen Savers`, or
system-wide under `/Library/Screen Savers`. The plug-in architecture must match
the host. [Apple framework](https://developer.apple.com/documentation/screensaver).

Implemented: a small Objective-C view, AppKit/Core Text drawing, and a Rust static
library behind a narrow C ABI. Each view owns its engine and uses native preferences
independently of CLI configuration. The build produces arm64 and x86_64 slices in
a universal bundle. See [build instructions and validation limits](macos.md).
[Universal binaries](https://developer.apple.com/documentation/apple-silicon/building-a-universal-macos-binary).

Public releases need a Developer ID signing/notarization workflow and a tested
installation path. Credentials are a release-maintainer concern, not a contributor
requirement for engine or CLI work. [Apple distribution](https://developer.apple.com/developer-id/).

Apple's developer forums contain acknowledged Sonoma/Tahoe reports about duplicate
instances, preview behavior, and animations continuing after dismissal. These
reports require testing on named OS builds; they do not establish that every
current patch has the same bugs. [Apple discussion](https://developer.apple.com/forums/thread/787444).

Local acceptance completed on Apple M3 / macOS 14.6.1: installation, preview,
configuration and theme switching, two displays, sleep/wake, and repeated
dismissal. Build 4's full-screen helper returned to 0.0% CPU after dismissal.
Automated checks cover FFI ownership, panic containment, and both bundle slices
(the Intel slice locally under Rosetta). Physical Intel hardware, additional
macOS versions, and clean-Mac distribution checks remain pending; see the
[validation record](macos.md#checks-and-limits).

## Windows — Milestone 3

A `.scr` is a Windows executable implementing screensaver behavior. The required
user-facing modes are `/s` (fullscreen), `/c` (configuration), and `/p <HWND>`
(embedded preview). [Microsoft modes](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ms686421%28v%3Dvs.85%29).

Plan: a separate Rust Win32 application, initially with buffered GDI text. Parse
case/separator variants and optional parent handles, and default bare invocation
to configuration. Preview renders into a validated parent window; it must not
behave like fullscreen input dismissal. The conventional screen saver library is
a reference for window lifecycle and resources, rather than a dependency of the
engine. [Microsoft contract](https://learn.microsoft.com/en-us/windows/win32/lwef/screen-saver-library).

Acceptance: all invocation modes, parent destruction, invalid handles, monitor
changes, mixed DPI, input dismissal, Windows resume-authentication settings, and
install/uninstall on a clean system. Keep authentication with Windows. Evaluate
code signing before public binary distribution.

## Omarchy / Hyprland — Milestone 4

Omarchy 4 moved idle handling and locking from Hypridle/Hyprlock into its Quickshell
desktop shell. An integration that assumes Hypridle everywhere would miss current
Omarchy installations. [Omarchy 4 release](https://github.com/omacom/omarchy/releases/tag/v4.0.0).

The current launcher still opens terminal windows on individual monitors for its
visual screensaver. Reusing Stillterm's terminal renderer is the first integration
to evaluate. Shell documentation exposes separate visual and lock timers, but a
stable custom-command substitution point has not been verified.
[Launcher](https://github.com/omacom/omarchy/blob/quattro/bin/omarchy-launch-screensaver),
[shell documentation](https://github.com/omacom/omarchy/blob/quattro/docs/omarchy-shell.md).

Pin and test a supported Omarchy release before implementing installation. Do not
overwrite packaged scripts or introduce a second idle daemon. If a standalone
Wayland surface becomes necessary, isolate it in an adapter while preserving the
same engine. GNOME and KDE integration remain outside V1.

For standalone Hyprland or older Omarchy installations that use Hypridle, its
timeout/resume listeners can launch and stop a visualizer while the existing locker
retains responsibility for authentication and suspend locking.
[Hypridle](https://wiki.hypr.land/hypr-ecosystem/user/hypridle/).

Acceptance: activity dismissal, all monitors, hotplug, no idle inhibition, lock
handoff, sleep/wake, crash behavior, and preservation of user settings. A failed
visualizer must never disable or delay the independent lock schedule. Stop visual
animation when locking starts; do not draw over a secure lock surface.

## Releases

First provide CLI archives with checksums after CI and terminal testing pass.
Then add a signed/notarized macOS bundle, Windows `.scr` packaging, and a Linux
binary with version-specific integration instructions as their milestones land.
The macOS build includes development disk-image packaging. Release publishing,
Developer ID signing, and notarization are not automated in CI.

Research reviewed 2026-09-30. Moving branches describe upstream direction;
adapter implementation must record exact tested versions.
