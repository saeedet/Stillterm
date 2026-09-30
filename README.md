# Stillterm

Quiet character animation for your terminal, built in Rust.

Sparse streams, fading trails, and a small reusable animation engine. Stillterm
runs directly in your terminal, with deterministic seeds and a 30 FPS default.
It has no background service or browser runtime.

![A reproducible snapshot of Stillterm's rain effect](docs/media/rain.svg)

*Engine snapshot: seed 42, four seconds, 96×28 cells. Font and brightness vary by
terminal. An animation recording is planned for the first release.*

**Status:** Milestone 1 — terminal application. Native screensaver adapters are
planned. Stillterm does not lock your session.

## Install

Requires [Rust](https://www.rust-lang.org/tools/install) 1.88 or newer and a system
linker. With rustup, this checkout selects the tested Rust 1.88.0 toolchain.

```sh
git clone https://github.com/saeedet/Stillterm.git
cd Stillterm
cargo install --path crates/terminal --locked
```

This installs `stillterm` into Cargo's binary directory. Ensure that directory is
on your `PATH`. Uninstall with `cargo uninstall stillterm`.

Prebuilt downloads and native installers are not available yet. There is no
published crates.io package at this stage; use the source installation above.

## Run

```sh
stillterm
stillterm run --fps 30 --seed 42
stillterm --speed 0.6 --density 0.12
stillterm --config examples/config.toml
stillterm list
```

Press **q**, **Escape**, or **Ctrl+C** to quit. Resize the terminal while it runs.
For development without installation, use `cargo run --release -- --seed 42`.
Use an ANSI-capable terminal; Windows Terminal is the intended Windows surface.

## Configure

Settings apply in this order: built-in defaults, an explicitly supplied TOML file,
then command-line overrides. No configuration file is written automatically.

| Setting | Default | Accepted values |
| --- | --- | --- |
| `effect` | `"rain"` | `rain` |
| `fps` | `30` | Integer, 10–60 |
| `speed` | `1.0` | 0.1–4.0 |
| `density` | `0.18` | 0.0–1.0; stream activation probability |
| `intensity` | `0.7` | 0.0–1.0 |
| `characters` | `"0123456789.:+*"` | 1–256 printable, single-column characters |
| `seed` | `42` | Unsigned 64-bit integer |

See [the example configuration](examples/config.toml) or `stillterm --help`.
ASCII is the most portable character set. Wide, combining, control, and
ambiguous-width characters are rejected. Nonempty `NO_COLOR` disables color;
other terminals use a detected grayscale or basic-color palette.

## Platforms

| Surface | Status |
| --- | --- |
| macOS terminal | Tested locally on Apple Silicon, including PTY lifecycle checks |
| Windows terminal | Cross-checked for compilation; native CI configured; interactive testing pending |
| Linux terminal | Cross-checked for compilation; native CI and PTY checks configured |
| macOS `.saver` | Planned — Milestone 2 |
| Windows `.scr` | Planned — Milestone 3 |
| Omarchy / Hyprland idle integration | Planned — Milestone 4; version-specific integration |

CI results become available when this repository is pushed to GitHub. Native
screensaver and lock-screen behavior is not provided by the terminal executable.
See [platform research and acceptance checks](docs/platform-support.md).

## How it works

`stillterm-engine` owns a character grid, seeded rain state, and fixed simulation
steps. `stillterm` owns the terminal, configuration loading, timing, and input.
The renderer compares presented cells and writes changed runs. The engine has no
OS or graphical-framework dependencies, and both crates forbid unsafe Rust.

On the initial Apple M3 benchmark, the CLI used less than 1% of one CPU core at
30 FPS in a drained pseudo-terminal. This excludes terminal-emulator rendering;
see [measurements and reproduction steps](docs/performance.md).

For the boundaries and determinism contract, read [architecture](docs/architecture.md).
To extend the engine, read [adding an effect](docs/adding-an-effect.md).

## Contribute

```sh
cargo test --workspace --all-targets --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[Contributing](CONTRIBUTING.md) covers terminal tests, visual changes, and bug reports.
Licensed under [MIT](LICENSE).
