# Stillterm

Quiet character animation for your terminal, built in Rust.

Stillterm is being built as a small terminal application with a reusable animation
engine. Native screensaver adapters for macOS, Windows, and Omarchy/Hyprland follow
the terminal milestone. It does not lock your session.

## Development

Requires Rust 1.88 or newer.

```sh
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

The engine produces a character grid without accessing a terminal, window, or
clock. Effects advance in fixed steps; adapters will own timing and presentation.

