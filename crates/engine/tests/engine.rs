use stillterm_engine::{Cell, EffectConfig, Engine, Frame, Glyph, GridSize, MAX_CELLS};

fn engine(seed: u64) -> Engine {
    Engine::rain(
        GridSize::new(80, 24).unwrap(),
        EffectConfig::default(),
        seed,
    )
}

#[test]
fn fixed_seed_and_resize_history_reproduce_frames() {
    let (mut a, mut b) = (engine(42), engine(42));
    for tick in 0..600 {
        if tick == 100 || tick == 400 {
            let size = GridSize::new(45, 12).unwrap();
            a.resize(size);
            b.resize(size);
        }
        if tick == 200 {
            let size = GridSize::new(100, 40).unwrap();
            a.resize(size);
            b.resize(size);
        }
        a.step();
        b.step();
        assert_eq!(a.frame(), b.frame());
    }
}

#[test]
fn different_seeds_change_the_composition() {
    assert_ne!(engine(42).frame(), engine(43).frame());
}

#[test]
fn reference_frames_preserve_the_seeded_visual_sequence() {
    // FNV-1a over explicit scalar and intensity bytes, independent of Rust's Hash.
    fn fingerprint(frame: &Frame) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for cell in frame.cells() {
            for byte in (cell.glyph.as_char() as u32)
                .to_le_bytes()
                .into_iter()
                .chain([cell.intensity])
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        hash
    }
    let mut engine = engine(42);
    let mut actual = Vec::new();
    for tick in 0..=900 {
        if [0, 1, 120, 900].contains(&tick) {
            actual.push(fingerprint(engine.frame()));
        }
        engine.step();
    }
    assert_eq!(
        actual,
        [
            1932281938483498771,
            11784908531410415146,
            6619540036231623766,
            11073333601853703345
        ]
    );
}

#[test]
fn rendering_frequency_does_not_change_simulation() {
    let (mut a, mut b) = (engine(7), engine(7));
    for tick in 0..300 {
        a.step();
        b.step();
        a.frame();
        a.frame();
        if tick % 10 == 0 {
            b.frame();
        }
    }
    assert_eq!(a.frame(), b.frame());
}

#[test]
fn rain_changes_and_remains_alive_after_many_stream_lifetimes() {
    let mut engine = engine(42);
    let initial = engine.frame().clone();
    for _ in 0..10_000 {
        engine.step();
    }
    assert_ne!(&initial, engine.frame());
    assert!(engine.frame().cells().iter().any(|c| c.intensity > 0));
}

#[test]
fn resizing_handles_empty_tiny_and_larger_surfaces() {
    let mut engine = engine(0);
    for (columns, rows) in [(0, 0), (0, 100), (100, 0), (1, 1), (240, 80), (80, 24)] {
        let size = GridSize::new(columns, rows).unwrap();
        engine.resize(size);
        for _ in 0..10 {
            engine.step();
        }
        assert_eq!(engine.frame().size(), size);
        assert_eq!(
            engine.frame().cells().len(),
            usize::from(columns) * usize::from(rows)
        );
    }
}

#[test]
fn grid_limits_and_cell_boundaries_are_checked() {
    assert!(GridSize::new(u16::MAX, u16::MAX).is_err());
    assert_eq!(GridSize::new(512, 512).unwrap().len(), MAX_CELLS);
    let mut frame = Frame::new(GridSize::new(2, 1).unwrap());
    let cell = Cell {
        glyph: Glyph::new('x').unwrap(),
        intensity: 200,
        emphasis: false,
    };
    frame.set(1, 0, cell);
    frame.set(2, 0, cell);
    frame.set(0, 1, cell);
    assert_eq!(frame.cells(), &[Cell::BLANK, cell]);
}

#[test]
fn invalid_configuration_is_rejected() {
    for speed in [0.0, -1.0, 4.1, f64::NAN, f64::INFINITY] {
        assert!(EffectConfig::new(speed, 0.2, 0.7, "abc").is_err());
    }
    for value in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
        assert!(EffectConfig::new(1.0, value, 0.7, "abc").is_err());
        assert!(EffectConfig::new(1.0, 0.2, value, "abc").is_err());
    }
    for chars in ["", "\x1b", "\n", "\u{0301}", "界", "😀", "\u{200b}", "·"] {
        assert!(
            EffectConfig::new(1.0, 0.2, 0.7, chars).is_err(),
            "{chars:?}"
        );
    }
    assert!(EffectConfig::new(1.0, 0.2, 0.7, &"x".repeat(257)).is_err());
}

#[test]
fn zero_density_and_intensity_have_no_visible_cells() {
    for (density, intensity) in [(0.0, 1.0), (1.0, 0.0)] {
        let config = EffectConfig::new(1.0, density, intensity, "01").unwrap();
        let mut engine = Engine::rain(GridSize::new(20, 10).unwrap(), config, 42);
        for _ in 0..300 {
            engine.step();
            assert!(engine.frame().cells().iter().all(|c| c.intensity == 0));
        }
    }
}
