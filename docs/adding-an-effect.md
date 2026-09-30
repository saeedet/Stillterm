# Adding an effect

An effect produces characters and intensity, without knowing where they appear.
Start by reading `crates/engine/src/effects/rain.rs`.

1. Add a module under `crates/engine/src/effects` and export the type.
2. Implement `Effect::resize`, `step`, and `render`.
3. Accept validated settings and a seeded generator in the constructor.
4. Add a constructor or factory choice and wire its name into the CLI's settings
   validation and `list` output.
5. Add focused tests and update the configuration documentation.

`step` advances one tick at `TICKS_PER_SECOND`. `render` receives a cleared frame
and must not advance state, read time, or consume randomness. It may be called
multiple times between steps. `resize` receives a validated `GridSize`, including
empty dimensions; preserve existing state when that makes visual sense.

Keep storage allocated across frames. Use `Frame::set` to write a validated glyph
and an intensity from 0 to 255. Palette selection and text drawing belong to the
adapter. Avoid terminal escape sequences, platform handles, and filesystem access.

Useful tests compare known frames, verify lifecycle transitions, and exercise
shrink/grow/empty surfaces. Check that extra render calls leave subsequent output
unchanged. Use a fixed seed and integer arithmetic when practical; explain any
weaker determinism guarantee if an effect needs floating-point math.

Preview locally with `cargo run --release -- --effect <name>`. Tune one effect
before adding another. No dynamic plug-in system is required.
