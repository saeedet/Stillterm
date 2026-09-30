# Architecture

Stillterm has two crates. The dependency flows from presentation to simulation:

```text
stillterm (CLI, timing, configuration files, terminal renderer)
    └── stillterm-engine (settings, effect state, character frames)

Future macOS / Windows / Linux adapters
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
The terminal adapter accumulates integer elapsed nanoseconds and can present at
10–60 FPS independently. Excess elapsed time beyond 250 ms per update is discarded
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

## Future adapters

Each adapter owns its lifecycle and configuration persistence. The macOS adapter
will use a small C ABI around Rust rather than exposing Rust layouts. Handles,
buffer ownership, and panic containment must be explicit. OS-specific unsafe code
belongs in its own adapter crate, with documented invariants.

No universal renderer trait or plug-in loader is needed yet. Effects are ordinary
Rust implementations compiled into the program. Colors, runtime effect discovery,
and a shared native text renderer can follow actual requirements.
