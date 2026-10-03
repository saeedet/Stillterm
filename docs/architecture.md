# Architecture

Stillterm has four crates. The dependency flows from presentation to simulation:

```text
stillterm (CLI, timing, configuration files, terminal renderer)
    └── stillterm-engine (settings, effect state, character frames)

macOS ScreenSaverView (Objective-C, Core Text, native preferences)
    └── stillterm-macos-bridge (owned C handle, copied frames, panic containment)
        └── stillterm-engine

stillterm-windows (.scr modes, Win32 windows, GDI, native preferences)
    └── stillterm-engine

Hyprland adapter (Python standard library, scoped Foot windows)
    └── stillterm CLI
        └── stillterm-engine
```

## Engine

`Engine` owns one `Effect` and a reusable `Frame`. `Engine::rain` constructs the
built-in effect with an explicit ChaCha8 seed. `Engine::new` accepts another
effect without changing the scheduler or renderer.

`Frame` stores row-major `Cell` values: a validated `Glyph`, an intensity byte,
and an emphasis hint identifying a stream head. Monochrome ignores that hint;
the Matrix palette uses it for pale leading characters. Shared `Theme` presets
provide effect defaults and ideal RGB values without accessing display APIs.
Its dimensions and storage length cannot disagree through the public API.
`GridSize` limits allocation to 262,144 cells; zero-sized grids are valid.
Out-of-bounds `Frame::set` calls are clipped. Native renderers map cells to font
metrics and pixel coordinates without changing the animation.

`EffectConfig` validates settings and quantizes numeric values once. Rain uses
integer movement and hashing thereafter. Effects receive randomness explicitly,
and rendering never consumes it. Existing columns survive ordinary resizing;
removed columns are discarded, and new columns consume the next seeded values.
An empty grid discards streams and does not advance the rain state.

## Timing and determinism

The engine never reads a clock. Each `step` advances 1/30 second of simulation.
The shared `StepClock` accumulates integer elapsed nanoseconds. Terminal and native
adapters can present at 10–60 FPS independently. Excess elapsed time beyond 250 ms per update is discarded
after a stall or suspension. Resizing discards fractional pending time.

The same engine version, validated settings, seed, steps, and resize history
produce the same cells. Changing presentation frequency does not consume extra
randomness. Live sessions with different stalls or resize histories need not
match, and platform fonts/palettes need not produce identical pixels.

Reference hashes cover glyph scalars and intensity bytes at known ticks, using an
explicit byte order and hash. They are a regression contract within this version,
not a promise that future artistic changes will preserve every seed forever.

## Terminal boundary

The terminal crate parses optional TOML, applies CLI overrides, then constructs
validated engine settings. It checks terminal availability before entering raw
mode and the alternate screen. A session guard and panic hook restore the screen,
cursor, colors, and raw mode. Signal handlers request exit through an atomic flag;
terminal cleanup happens on the main thread.

Rendering compares quantized presented cells, groups adjacent changes, and emits
one buffered write/flush per changed frame. Initial display and resize repaint
the screen. The bottom-right cell is reserved to avoid terminal auto-wrap scroll.
Polling waits for input or the next presentation deadline; signal checks are at
most 50 ms apart while the output device is accepting writes.

Ordinary runs do not capture the mouse. The opt-in `--exit-on-input` mode captures
mouse buttons/scroll and exits on any pressed key; all capture modes are restored
on cleanup. The desktop adapter handles global pointer movement. There is no job
suspension binding. Forced kills, process aborts,
and a terminal disappearing cannot guarantee cleanup. If an external failure
leaves a Unix terminal unusable, `stty sane` or `reset` can restore it.

## macOS boundary

`stillterm-macos-bridge` builds a static library. `StilltermBridge.h` defines a
private C ABI rebuilt with the bundle: validated numeric options, an opaque owned
engine handle, and eight-byte cells with a Unicode scalar and RGB/visibility bytes.
The caller owns the output buffer; Rust copies input characters and frame data.
Handles must remain live and calls must be serialized. Raw pointers are confined
to this crate, with explicit safety contracts. Unwinding panics poison a handle
and return failure; invalid pointers, aborts, and allocation failure are not caught.

Each `StilltermView` owns one handle, buffers, and a Core Text glyph cache. The
native view advances using a monotonic clock, invalidates changed rows, and draws
only dirty cells in a removable child view. The outer view uses a black layer
background without a drawing bitmap. Stop/sleep removes the child so macOS can
release its backing surface even if the host retains the outer view. The system
owns the animation timer. Start/stop are guarded; late callbacks do no work. Hidden/detached
views suspend rendering. The settings sheet validates before persisting through
`ScreenSaverDefaults`; it does not read CLI files or alter system lock preferences.

## Future adapters

Each adapter owns its lifecycle and configuration persistence. OS-specific unsafe
code belongs in its own adapter crate, with documented invariants.

No universal renderer trait or plug-in loader is needed yet. Effects are ordinary
Rust implementations compiled into the program. Runtime effect discovery and
a shared native text renderer can follow actual requirements.

## Windows boundary

`stillterm-windows` keeps mode parsing and settings in a safe, platform-independent
library. Its executable isolates Win32 calls under `native`. Each monitor or preview
owns an engine, fixed-step clock, and GDI back buffer. Changed cells repaint offscreen;
window timers schedule presentation and the message loop blocks between events.
Boxed state outlives its windows, and callbacks contain unwind panics. Windows
controls idle activation and authentication. See [Windows behavior](windows.md).

## Linux boundary

`platforms/linux/stillterm-hyprland` is a transient process controller using Python's
standard library. Reusing Foot and the Rust CLI avoids another rendering stack.
The controller talks to Hyprland's Unix IPC socket and owns one standalone Foot
process per non-mirrored monitor. It identifies windows by child PID and app ID,
sets only their properties, and never executes forwarded CLI arguments as shell
code. Private session sockets and an advisory lock handle stop and duplicate starts.

Keyboard and mouse-button events go through the Rust CLI. Bounded compositor
queries handle global pointer movement, focus loss, locking, and layout changes.
A single child exit or IPC failure tears down the run. The existing idle manager
owns activation and authentication; the controller does not infer or reschedule
lock deadlines. Automatic Omarchy 4 substitution is deliberately unsupported until
that desktop exposes a suitable lifecycle interface. See [Linux limits](linux.md).
