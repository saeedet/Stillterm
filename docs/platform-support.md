# Platform support

The terminal application and macOS/Windows screensaver development builds are
implemented. The experimental Hyprland adapter supports manual preview and optional
Hypridle integration; automatic Omarchy 4 integration remains unsupported. Paid signing and public
binary publishing are deferred; source development does not require them.
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
dismissal. Build 4's full-screen helper returned to 0.0% CPU after dismissal,
but longer use exposed retained host graphics memory. Build 5 releases drawing
surfaces and measured 117 MiB / 0% CPU after three installed previews; longer-session
and active-rendering performance validation remain open.
Automated checks cover FFI ownership, panic containment, and both bundle slices
(the Intel slice locally under Rosetta). Physical Intel hardware, additional
macOS versions, and clean-Mac distribution checks remain pending; see the
[validation record](macos.md#checks-and-limits).

## Windows — Milestone 3

A `.scr` is a Windows executable implementing screensaver behavior. The required
user-facing modes are `/s` (fullscreen), `/c` (configuration), and `/p <HWND>`
(embedded preview). [Microsoft modes](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ms686421%28v%3Dvs.85%29).

Implemented: a separate Rust Win32 application with buffered GDI text, theme and
visual settings, one full-screen window per monitor, and an embedded child preview.
It parses case/separator variants and optional parent handles; bare invocation
opens settings. Preview validates its parent and ignores fullscreen input dismissal.
Windows API calls stay in the adapter; the conventional screen saver library is a
reference for lifecycle behavior rather than an engine dependency.
See [Windows setup and validation](windows.md). [Microsoft contract](https://learn.microsoft.com/en-us/windows/win32/lwef/screen-saver-library).

Milestone 3 implementation and manual testing are complete. The tester reported
the checklist passing on Windows 10 on 2026-10-02 and Windows 11 on 2026-10-03,
including the settings-window fix. Exact OS builds and display configurations
were not recorded; these results do not imply universal Windows compatibility.
Automated checks complement manual validation of configuration, preview, full-screen
animation, input dismissal, displays, sleep/wake, resource use, and installation.
Windows retains responsibility for authentication. Signing and public binary
distribution remain deferred.

## Omarchy / Hyprland — Milestone 4

Implemented: an experimental adapter reusing the Rust CLI in separate Foot windows,
with per-monitor placement, input dismissal, scoped process cleanup, lock-state
checks, and exit on display changes. A Python standard-library controller exists
only while the visualizer runs. It does not add an idle daemon or edit settings.
See [installation, source pins, automated checks, and acceptance](linux.md).

Omarchy 4.0.4 (`c668141`) uses Quickshell for idle handling. Its idle service
hard-codes the visual launcher and cancels the lock timer when the last saver
window closes, including on failure. Automatic substitution is unsupported;
Stillterm offers a manual preview path without replacing Omarchy commands.
[Reviewed idle service](https://github.com/omacom/omarchy/blob/c668141e9c42b13c80c9ca4ea108e11708c5e8a5/shell/plugins/services/idle/Service.qml).

Standalone Hypridle users can add a visual-only listener while preserving their
independent lock schedule and suspend handling. A renderer failure must not delay
locking. Physical desktop tests, active resource measurements, and automatic
Omarchy integration remain outstanding, so Milestone 4 is not complete.

## Releases

First provide CLI archives with checksums after CI and terminal testing pass.
Then add a signed/notarized macOS bundle, Windows `.scr` packaging, and a Linux
binary with version-specific integration instructions as their milestones land.
The macOS build includes development disk-image packaging. Release publishing,
Developer ID signing, and notarization are not automated in CI.

Linux research updated 2026-10-03; other platform research reviewed 2026-09-30. Moving branches describe upstream direction;
adapter implementation must record exact tested versions.
