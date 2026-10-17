use stillterm_engine::{Engine, GridSize, Theme};

#[test]
fn matrix_glyphs_are_valid_and_seeded_frames_are_repeatable() {
    let size = GridSize::new(80, 24).unwrap();
    let mut a = Engine::rain(size, Theme::Matrix.rain_config(), 42);
    let mut b = Engine::rain(size, Theme::Matrix.rain_config(), 42);
    let mut saw_katakana = false;
    let mut saw_head = false;
    for _ in 0..120 {
        a.step();
        b.step();
        assert_eq!(a.frame(), b.frame());
        for cell in a.frame().cells() {
            if cell.intensity > 0 {
                saw_katakana |= ('ｦ'..='ﾝ').contains(&cell.glyph.as_char());
                saw_head |= cell.emphasis;
            }
        }
    }
    assert!(saw_katakana && saw_head);
}

#[test]
fn only_the_leading_visible_cell_of_a_stream_is_emphasized() {
    let size = GridSize::new(80, 24).unwrap();
    let mut engine = Engine::rain(size, Theme::Matrix.rain_config(), 42);
    for _ in 0..300 {
        engine.step();
        let frame = engine.frame();
        for col in 0..80 {
            let mut heads = 0;
            for row in 0..24 {
                if frame.cells()[row * 80 + col].emphasis {
                    heads += 1;
                    assert!((row + 1..24).all(|y| frame.cells()[y * 80 + col].intensity == 0));
                }
            }
            assert!(heads <= 1);
        }
    }
}

#[test]
fn matrix_heads_are_pale_trails_are_green_and_zero_is_black() {
    assert_eq!(Theme::Matrix.rgb(0, true), [0; 3]);
    assert_eq!(Theme::Matrix.rgb(0, false), [0; 3]);
    let head = Theme::Matrix.rgb(255, true);
    let trail = Theme::Matrix.rgb(255, false);
    assert!(head.iter().all(|component| *component >= 210));
    assert!(trail[1] > trail[0] && trail[1] > trail[2]);
}

#[test]
fn dense_matrix_fills_short_and_tall_grids_without_brightening_the_background() {
    for (columns, rows) in [(80, 24), (240, 80)] {
        let size = GridSize::new(columns, rows).unwrap();
        let mut dense = Engine::rain(size, Theme::DenseMatrix.rain_config(), 42);
        let mut repeat = Engine::rain(size, Theme::DenseMatrix.rain_config(), 42);
        let mut classic = Engine::rain(size, Theme::Matrix.rain_config(), 42);
        let mut filled = 0;
        let mut sparse = 0;
        for _ in 0..120 {
            dense.step();
            repeat.step();
            classic.step();
            assert_eq!(dense.frame(), repeat.frame());
            for cell in dense.frame().cells() {
                if cell.intensity > 0 {
                    filled += 1;
                    if !cell.emphasis {
                        assert!(cell.intensity <= 220, "background overpowers the heads");
                    }
                }
            }
            sparse += classic
                .frame()
                .cells()
                .iter()
                .filter(|c| c.intensity > 0)
                .count();
        }
        assert!(
            filled > size.len() * 120 / 2,
            "dense preset leaves most cells empty"
        );
        assert!(
            filled > sparse * 2,
            "dense preset is too close to ordinary Matrix"
        );
    }
}

#[test]
fn dense_trails_handle_extreme_dimensions_resize_and_zero_density() {
    let mut engine = Engine::rain(
        GridSize::new(1, u16::MAX).unwrap(),
        Theme::DenseMatrix.rain_config(),
        42,
    );
    for size in [(1, u16::MAX), (320, 100), (1, 1), (0, 0), (200, 80)] {
        engine.resize(GridSize::new(size.0, size.1).unwrap());
        for _ in 0..10 {
            engine.step();
        }
        assert_eq!(
            engine.frame().size(),
            GridSize::new(size.0, size.1).unwrap()
        );
    }
    let config = stillterm_engine::EffectConfig::new(1.2, 0.0, 1.0, "01")
        .unwrap()
        .with_theme_style(Theme::DenseMatrix);
    let mut engine = Engine::rain(GridSize::new(80, 24).unwrap(), config, 42);
    for _ in 0..300 {
        engine.step();
        assert!(engine.frame().cells().iter().all(|c| c.intensity == 0));
    }
    assert_eq!("dense-matrix".parse::<Theme>().unwrap(), Theme::DenseMatrix);
    assert_eq!(
        Theme::DenseMatrix.rgb(255, true),
        Theme::Matrix.rgb(255, true)
    );
}

#[test]
fn dense_sheets_are_fuller_at_the_top_and_fade_into_the_lower_screen() {
    let mut engine = Engine::rain(
        GridSize::new(200, 120).unwrap(),
        Theme::DenseMatrix.rain_config(),
        42,
    );
    let mut top = 0u64;
    let mut bottom = 0u64;
    for _ in 0..120 {
        engine.step();
        for (index, cell) in engine.frame().cells().iter().enumerate() {
            if cell.emphasis {
                continue;
            }
            let row = index / 200;
            if row < 40 {
                top += u64::from(cell.intensity);
            }
            if row >= 80 {
                bottom += u64::from(cell.intensity);
            }
        }
    }
    assert!(
        bottom > 0,
        "the lower screen still contains receding trails"
    );
    assert!(top > bottom * 3, "the rain should recede toward the bottom");
}
