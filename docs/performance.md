# Performance

Stillterm sleeps between input and frame deadlines. Presentation defaults to 30
FPS; simulation runs at 30 fixed steps per second. Missed presentation deadlines
are skipped, and simulation catch-up is limited to 250 ms of elapsed time.

The engine reuses its frame and stream storage. Inspection of its step/render
paths shows no allocations after construction or resize. The terminal renderer
retains its previous frame and output buffer, compares quantized cells, and writes
only changed runs. Buffer growth can still allocate as a new high-water mark is
reached; allocations were not instrumented in the measurements below.

## Initial baseline

Measured on 2026-09-30 at commit `e7ed24c`: Apple M3, macOS 14.6.1, Rust 1.97.1,
release profile, default settings, seed 42. Each CLI run lasted approximately
10 seconds at 30 FPS. The later Windows encoding fix has not been re-benchmarked.
CPU percentage is relative to one core. These are local observations, not portable
performance guarantees.

| Grid | Engine step + frame, mean | CLI CPU | CLI peak RSS | Terminal output |
| --- | ---: | ---: | ---: | ---: |
| 120×40 | 4.32 µs | 0.385% | 3.41 MiB | 24,498 bytes/s |
| 240×80 | 9.84 µs | 0.688% | 3.64 MiB | 49,178 bytes/s |

Engine timing averages 20,000 iterations after 300 warm-up steps. CLI measurements
use a drained Unix pseudo-terminal and include process startup. They exclude the
terminal emulator's CPU, GPU, font rendering, and memory. These baseline figures do not include energy use or native screensaver rendering.

## Dense Matrix development measurements

The first dense preset (build 10) was measured on 2026-10-18 at `10e2e70`: Apple M3,
macOS 27.0.1 (26A434), Rust 1.88.0, release profile, seed 42, Dense Matrix defaults
(speed `1.2`, density `0.95`, intensity `1.0`). These short local measurements
are observations, not hardware requirements or installed-screensaver guarantees.

| Engine grid | Step + frame, mean over 20,000 iterations |
| --- | ---: |
| 120×40 | 14.90 µs |
| 240×80 | 44.43 µs |

The actual native bundle in a 1440×877-point Retina preview used a 206×63 grid,
with about 62% of cells visible. After two seconds of warm-up, five-second samples
measured 23–28% of one CPU core at 30 draws/second. The host's physical footprint
was about 171 MiB while active; peak RSS was 81–85 MiB. Physical footprint includes
resources that RSS does not, so these values measure different things. This
includes AppKit/Core Text and the test loop; it excludes WindowServer and energy.

Batching glyphs by font and exact color reduced an earlier local sample from 113%
of one core at 24 draws/second. A native image comparison against per-cell Core
Text drawing found no lit pixels differing by more than 16/255 in any color
channel. Unusual glyphs that cross cell edges retain the clipped drawing path.
Both paths release drawing buffers and glyph caches on stop or resize.

The retained-view test at 1600×900 points settled at 25.3 MiB after 12 dense
start/stop cycles, with 0.1 MiB growth from the warm baseline. Asynchronous graphics
cleanup caused temporary growth of about 194 MiB; settled usage and transient
peaks must be distinguished. Longer sessions, installed full-screen playback,
additional displays, Windows rendering, and energy use still need measurement.

### Layered Mac rendering (build 11)

The revised Mac preset uses three independently seeded grids. At base size 10,
font sizes are 6.5, 8.5, and 11.5 points; distant layers move more slowly and use
lower brightness. The near layer owns the user's words. One Retina canvas
composites a shared background bitmap at logical resolution, bounded to four
million pixels, with the two distant layers refreshed at up to 15 FPS.

On the same M3/macOS setup, a 1432×849-point preview measured 35.2% of one core at
30 draws/second over five seconds after two seconds of warm-up. The near grid was
179×61; its visible-cell percentage describes only that grid, not the overlapping
background. The background updated 10 times/second and used a 4.6 MiB bitmap.
The process footprint was 174.8 MiB and peak RSS 93.8 MiB. Seed was 42, words `FREE`,
frequency Frequent. A drawing comparison found 0.24% of lit pixels differed by
more than 16/255 between batched and per-cell drawing, within the 5% allowance
for antialiasing and overlapping halos.

After 12 layered start/stop cycles at 1600×900 points, footprint settled at
33.1 MiB with no growth from the warm baseline; transient growth reached about
149 MiB during asynchronous graphics cleanup. Tests also check independent grids,
resize through zero dimensions, sleep/wake, and release of the shared bitmap.
Installed long sessions, other displays, and energy use remain unmeasured. These
figures replace build 10's rendering figures for the current Mac preset; Windows
and terminal Dense Matrix still use one grid.

## Reproduce

```sh
cargo run --release --locked -p stillterm-engine --example measure
cargo run --release --locked -p stillterm-engine --example measure -- dense-matrix
cargo build --release --locked
python3 scripts/measure-terminal.py
python3 scripts/measure-terminal.py --columns 240 --rows 80
```

The Python script uses only the standard library and runs on macOS/Linux. Run each
size in a separate process to isolate peak child memory. Use a longer duration
with `--seconds 60` when comparing changes. Profile the terminal emulator separately
before attributing whole-system resource use to the animation engine.

On macOS, `scripts/build-macos.sh` followed by `scripts/check-macos.sh` runs the
native dense measurement, drawing comparison, and retained-view checks. See
[the macOS guide](macos.md#checks-and-limits) for running the dense measurement
alone. Native measurements open a window; keep it visible and avoid other heavy
work while comparing samples.
