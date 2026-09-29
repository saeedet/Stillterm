//! A dependency-free release benchmark; includes simulation and frame generation.
use std::{hint::black_box, time::Instant};
use stillterm_engine::{EffectConfig, Engine, GridSize};

fn main() {
    const ITERATIONS: u32 = 20_000;
    println!("grid,iterations,mean_microseconds");
    for (columns, rows) in [(120, 40), (240, 80)] {
        let mut engine = Engine::rain(
            GridSize::new(columns, rows).unwrap(),
            EffectConfig::default(),
            42,
        );
        for _ in 0..300 {
            engine.step();
            black_box(engine.frame());
        }
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            engine.step();
            black_box(engine.frame());
        }
        let micros = start.elapsed().as_secs_f64() * 1_000_000.0 / f64::from(ITERATIONS);
        println!("{columns}x{rows},{ITERATIONS},{micros:.2}");
    }
}
