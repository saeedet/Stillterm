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
