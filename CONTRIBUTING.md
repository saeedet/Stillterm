# Contributing

Small, focused changes are welcome. Discuss new platform integrations or large
features in an issue before building them. Keep the core independent of terminal
and window APIs, and keep new effects deterministic and inexpensive.

## Local checks

Install Rust through rustup; the checkout selects Rust 1.88.0. CI also tests the
current stable compiler on macOS, Windows, and Linux.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo doc --workspace --no-deps --locked
```

For terminal changes, run these additional checks on macOS or Linux with Python 3:

```sh
cargo build --locked
python3 scripts/check-terminal.py --panic-check
python3 scripts/check-terminal.py --theme matrix
```

The PTY harness checks quit keys, signals, resizing, error recovery, and panic
cleanup. The panic test is ignored by the ordinary Rust test command because it
needs an isolated terminal and a parent handshake. Run it through the script.
Also try the application in a real terminal; PTYs cannot check font rendering or
Windows console behavior.

## Visual changes

Include the seed, dimensions, settings, and simulation tick when reporting a
visual difference. Reference-frame hashes deliberately detect output changes.
Update them only after inspecting and explaining the changed behavior.

Regenerate the README snapshot after intentional visual changes:

```sh
cargo run --locked -p stillterm-engine --example snapshot > docs/media/rain.svg
cargo run --locked -p stillterm-engine --example snapshot -- matrix > docs/media/matrix.svg
```

Check resource-sensitive changes using [the measurement tools](docs/performance.md).
Avoid introducing a framework or dependency without a concrete need.

## Pull requests and reports

Describe the user-visible problem, the change, and how you tested it. Use concise
commit subjects such as `fix(terminal): restore the cursor after a write error`.
Keep documentation and tests alongside the behavior they describe.

For terminal bugs, include OS, terminal emulator, Rust/Stillterm version, command,
and whether resizing or suspension was involved. Remove private paths and any
unrelated terminal contents from recordings.
