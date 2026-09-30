# Architecture

Stillterm has three crates. The dependency flows from presentation to simulation:

```text
stillterm (CLI, timing, configuration files, terminal renderer)
    └── stillterm-engine (settings, effect state, character frames)

macOS ScreenSaverView (Objective-C, Core Text, native preferences)
    └── stillterm-macos-bridge (owned C handle, copied frames, panic containment)
        └── stillterm-engine

Future Windows / Linux adapters
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

There is no mouse capture or job suspension binding. Forced kills, process aborts,
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
only dirty cells. The system owns the animation timer. Start/stop are guarded;
stop and sleep release resources, and late callbacks do no work. Hidden/detached
views suspend rendering. The settings sheet validates before persisting through
`ScreenSaverDefaults`; it does not read CLI files or alter system lock preferences.

## Future adapters

Each adapter owns its lifecycle and configuration persistence. OS-specific unsafe
code belongs in its own adapter crate, with documented invariants.

No universal renderer trait or plug-in loader is needed yet. Effects are ordinary
Rust implementations compiled into the program. Runtime effect discovery and
a shared native text renderer can follow actual requirements.
