# Changelog

User-facing development changes are collected here until the first release.
Version `0.1.0` in the source identifies development builds; no public release
has been published.

## Unreleased

### Changed

- Words accept commas, line breaks, or a TOML string array. Entries appear one at
  a time in input order, loop after the last entry, and use randomly chosen rain
  positions. Existing vertical reveal, glow, fade, and frequency controls remain.

- Removed the experimental 3D mode and GPU dependencies. Saved Mac 3D preferences
  now use Dense Matrix while preserving words and other settings.

### Added

- macOS Dense Matrix now composites three independent grids at different sizes,
  speeds, and brightness levels, with softer distant rain and subtle near-head
  glow. Words remain in the near layer. One shared canvas and a bounded background
  bitmap limit drawing resources. Windows and terminal rendering use one grid.

- Dense Matrix preset with long trails scaled to the screen height, dim background
  streams, and smaller native characters. Select it in Mac/Windows settings or
  use `--theme dense-matrix`; personal words and frequency are preserved.

- Optional words and phrases appear vertically within rain streams, with Rare,
  Occasional, and Frequent settings. Configure them in native Mac/Windows settings
  or terminal TOML/flags; existing preferences keep words disabled. Mac build 15 is
  available for local evaluation; manual Windows feature testing is pending.

- Deterministic rain animation with Monochrome and Matrix themes, configurable
  speed, density, brightness, characters, frame rate, and seed.
- Terminal application with TOML configuration, resizing, color fallbacks, and
  cleanup on dismissal, signals, and recoverable errors.
- Universal macOS screensaver with native options and per-display animation.
- Windows x64 screensaver with settings, embedded preview, and full-screen mode.
- Experimental Hyprland launcher with one Foot window per monitor, input
  dismissal, scoped cleanup, and optional standalone Hypridle setup.
- Automated platform checks and temporary CI development artifacts.
- Verified Developer ID signing for the universal macOS bundle and disk image;
  the first package has been submitted to Apple for notarization.

### Fixed

- Dense trails hold more body brightness near the top and recede into black near
  the bottom, addressing the flat, sparse appearance of the first dense preset.

- Dense Matrix on macOS batches drawing by font and color to reduce CPU use;
  stopping or resizing releases its glyph caches and buffers.

- Words stay fixed in a column, light up progressively as the head passes, and
  retain their glow until complete before fading from top to bottom.

- macOS disk-image packaging checks each required architecture separately for
  compatibility with the installed Xcode tools.
- macOS black previews and stale theme preferences across screensaver hosts.
- Drawing resources retained by stopped macOS views; build 5 releases its canvas
  and passes the retained-view memory regression check.
- Windows settings-window creation across differing owner DPI contexts.
- CLI file errors now identify the path and distinguish encoding, size, and TOML
  errors before entering terminal mode.
- Linux launcher failures now return an error, reject malformed compositor
  response shapes, and clean up remaining windows.

### Validation limits

- Windows 10 and 11 manual checks passed; exact builds/display setups were not
  recorded. macOS installed checks passed on Apple M3 / macOS 14.6.1.
- Longer macOS sessions, active-rendering performance, additional macOS versions,
  physical Intel Macs, and Linux desktop acceptance still need testing.
- Omarchy 4.0.4 has a manual preview path only; automatic idle replacement is
  unsupported. Stillterm does not implement session locking.
- macOS notarization and final downloaded installation checks remain pending.
  Windows signing and the first public binary release are also pending.

See [platform support](docs/platform-support.md) for the detailed validation record.
