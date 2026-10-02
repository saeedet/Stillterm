# Windows screensaver

Stillterm's native Windows adapter uses the shared Rust rain engine with buffered
GDI drawing. It includes Monochrome and Matrix themes, a settings window, and the
embedded preview expected by Windows Screen Saver Settings.

This is an unsigned x64 development build targeting Windows 10 version 1703 or
newer and Windows 11. Interactive validation on a physical Windows PC is pending;
CI checks do not establish compatibility with every display configuration.

## Build and install for testing

Requires Rust with the `x86_64-pc-windows-msvc` target and Microsoft's C++ build
tools. From a PowerShell terminal in the repository:

```powershell
./scripts/build-windows.ps1
./scripts/check-windows.ps1
```

The result is `target/windows/Stillterm.scr`. CI also provides a
`Stillterm-windows-development` artifact on successful Windows runs; it contains
a ZIP and checksum. These are temporary development artifacts, not public releases.

1. Extract the ZIP and keep `Stillterm.scr` in a permanent folder.
2. Right-click the file, choose **Show more options** if needed, then **Install**.
3. In Windows Screen Saver Settings, choose Stillterm and open **Settings**.
4. Choose a theme, save, then click **Preview**.
5. Configure the wait time and **On resume, display logon screen** in Windows.

The executable is unsigned and may trigger Windows download/reputation warnings.
Signing and public binary releases are deferred. Stillterm does not change
Windows lock, password, or idle settings itself.

To remove it, select another screensaver first and delete the `.scr` file.
To reset its preferences too, delete `%APPDATA%\Stillterm\screensaver.toml`.

## Modes and settings

```powershell
./Stillterm.scr /c       # Settings (also the default with no arguments)
./Stillterm.scr /s       # Full-screen animation on every attached monitor
./Stillterm.scr /p 1234  # Embedded preview; Windows supplies the live parent HWND
```

Switches are case-insensitive; `-` and `:` forms are accepted. `/c` accepts an
optional owner HWND. A missing, invalid, or destroyed preview parent exits;
preview never falls back to full screen. Keyboard and mouse input dismiss full
screen, but do not dismiss the embedded preview. Windows owns authentication.

Settings include theme, speed, density, brightness, FPS, character size, seed,
and custom characters. Choosing a theme resets its visual values to the preset.
Save validates every field and replaces the per-user TOML file; Cancel changes
nothing. A damaged file falls back to defaults during animation and shows an
explanation when Settings opens. Preferences are separate from the CLI and macOS.

## Rendering and lifecycle

Each monitor has an independent engine and GDI back buffer. Changed cells redraw
offscreen, then the buffer is copied during painting. A window timer caps rendering
at the requested rate; the message loop sleeps between events. Fonts scale with
DPI and small previews. Consolas and Windows font fallback determine glyph shapes.

Hidden, minimized, and suspended views release their drawing resources. Closing
or destroying the preview parent ends its process. Full-screen dismissal destroys
all windows and releases timers and GDI objects. Display-topology changes end the
current full-screen session; Windows can start a fresh session with the new layout.
Each display surface is limited to 16 million pixels and the engine's 262,144 cells.

All Windows API calls are isolated under `crates/windows/src/native`; callbacks
contain unwind panics and state outlives the windows that reference it. The core
engine and the Windows invocation/settings library forbid unsafe Rust.

## Validation

Automated checks cover invocation parsing, invalid settings, seed round trips,
GDI pixels, unchanged frames, resize, hidden views, simulated suspend/resume,
preview input, and parent destruction. A separate process test launches the actual
`.scr` in a disposable preview host without changing your selected screensaver.
Settings-window tests use a temporary preferences folder to check theme switching
across processes and Cancel without touching your saved settings. They also open
Settings with a separate owner window, as Control Panel does, across Windows DPI
awareness modes. Settings matches the owner's scaling context and can open
independently if Windows rejects the owner relationship.

Manual Windows checklist:

- Install, configure, cancel/save, and switch between both themes.
- Embedded preview and full-screen Preview render and animate.
- Keyboard, mouse buttons, and meaningful mouse movement dismiss full screen.
- Idle activation and Windows resume/sign-in behavior work as configured.
- All monitors render; test mixed scaling and connecting/disconnecting a display.
- Sleep/wake works, and repeated previews leave no Stillterm process after exit.
- Check CPU/memory while running; test upgrade and removal.

Record the Windows version, architecture, displays, and results before expanding
support claims. No paid signing setup is needed for source development.

Platform references: [Microsoft screensaver modes](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ms686421%28v%3Dvs.85%29),
[screen saver lifecycle](https://learn.microsoft.com/en-us/windows/win32/lwef/screen-saver-library),
and [DPI-aware desktop applications](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows).
