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
terminal emulator's CPU, GPU, font rendering, and memory. No energy or native
screensaver measurement has been made.

## Reproduce

```sh
cargo run --release --locked -p stillterm-engine --example measure
cargo build --release --locked
python3 scripts/measure-terminal.py
python3 scripts/measure-terminal.py --columns 240 --rows 80
```

The Python script uses only the standard library and runs on macOS/Linux. Run each
size in a separate process to isolate peak child memory. Use a longer duration
with `--seconds 60` when comparing changes. Profile the terminal emulator separately
before attributing whole-system resource use to the animation engine.
