//! Reproduce the README image: cargo run -p stillterm-engine --example snapshot
use stillterm_engine::{EffectConfig, Engine, GridSize};

fn main() {
    let size = GridSize::new(96, 28).unwrap();
    let mut engine = Engine::rain(size, EffectConfig::default(), 42);
    for _ in 0..120 {
        engine.step();
    }
    println!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="544" viewBox="0 0 1000 544" role="img" aria-labelledby="title description">
  <title id="title">Stillterm rain</title>
  <desc id="description">A deterministic snapshot of sparse character streams with soft gray trails on a dark background. Seed 42, 120 simulation ticks.</desc>
  <rect width="1000" height="544" rx="12" fill="#101114"/>
  <g font-family="ui-monospace, SFMono-Regular, Consolas, monospace" font-size="15">"##
    );
    for (index, cell) in engine.frame().cells().iter().enumerate() {
        let level = u16::from(cell.intensity) * 15 / 255;
        if level == 0 {
            continue;
        }
        // Match the terminal renderer's xterm grayscale ramp.
        let value = 8 + (level * 23 / 15) * 10;
        let x = 20 + index % 96 * 10;
        let y = 32 + index / 96 * 18;
        let glyph = match cell.glyph.as_char() {
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '&' => "&amp;".to_string(),
            ch => ch.to_string(),
        };
        println!(
            r##"    <text x="{x}" y="{y}" fill="#{value:02x}{value:02x}{value:02x}">{glyph}</text>"##
        );
    }
    println!("  </g>\n</svg>");
}
