use stillterm_engine::{EffectConfig, Engine, GridSize, Theme, WordFrequency};

fn has_phrase(engine: &mut Engine, phrase: &str) -> bool {
    let width = usize::from(engine.size().columns());
    width > 0
        && engine.frame().cells().chunks(width).any(|row| {
            row.windows(phrase.chars().count()).any(|cells| {
                cells.iter().all(|cell| cell.intensity > 0)
                    && cells
                        .iter()
                        .map(|cell| cell.glyph.as_char())
                        .eq(phrase.chars())
            })
        })
}

#[test]
fn phrases_resolve_then_disappear_without_changing_the_underlying_rain() {
    for theme in [Theme::Matrix, Theme::Monochrome] {
        let size = GridSize::new(48, 18).unwrap();
        let mut words = Engine::rain(
            size,
            theme
                .rain_config()
                .with_words("anything", WordFrequency::Occasional)
                .unwrap(),
            42,
        );
        let mut plain = Engine::rain(size, theme.rain_config(), 42);
        let mut appeared = false;
        let mut returned_to_rain = false;
        for _ in 0..360 {
            words.step();
            plain.step();
            appeared |= has_phrase(&mut words, "anything");
            if appeared && words.frame() == plain.frame() {
                returned_to_rain = true;
            }
        }
        assert!(appeared, "phrase never became readable");
        assert!(
            returned_to_rain,
            "phrase did not dissolve into the unchanged rain"
        );
    }
}

#[test]
fn rendering_cadence_does_not_change_words_and_multiple_phrases_appear() {
    let size = GridSize::new(48, 18).unwrap();
    let config = Theme::Matrix
        .rain_config()
        .with_words("anything\nwake up", WordFrequency::Frequent)
        .unwrap();
    let mut regular = Engine::rain(size, config.clone(), 7);
    let mut sparse = Engine::rain(size, config, 7);
    let mut seen = [false; 2];
    for tick in 0..2400 {
        regular.step();
        sparse.step();
        seen[0] |= has_phrase(&mut regular, "anything");
        seen[1] |= has_phrase(&mut regular, "wake up");
        if tick % 7 == 0 {
            assert_eq!(regular.frame(), sparse.frame());
        }
    }
    assert_eq!(seen, [true, true]);
}

#[test]
fn empty_text_preserves_existing_frames_and_zero_brightness_hides_words() {
    let size = GridSize::new(40, 20).unwrap();
    let mut ordinary = Engine::rain(size, EffectConfig::default(), 42);
    let mut empty = Engine::rain(
        size,
        EffectConfig::default()
            .with_words(" \n\r\n", WordFrequency::Frequent)
            .unwrap(),
        42,
    );
    let mut dark = Engine::rain(
        size,
        EffectConfig::new(1.0, 0.2, 0.0, "01")
            .unwrap()
            .with_words("anything", WordFrequency::Frequent)
            .unwrap(),
        42,
    );
    for _ in 0..600 {
        ordinary.step();
        empty.step();
        dark.step();
        assert_eq!(ordinary.frame(), empty.frame());
        assert!(dark.frame().cells().iter().all(|cell| cell.intensity == 0));
    }
}

#[test]
fn phrases_handle_small_empty_and_resized_surfaces() {
    let config = EffectConfig::default()
        .with_words("anything", WordFrequency::Frequent)
        .unwrap();
    let mut engine = Engine::rain(GridSize::new(48, 18).unwrap(), config, 42);
    for dimensions in [(48, 18), (0, 0), (1, 1), (8, 1), (2, 2), (48, 18)] {
        engine.resize(GridSize::new(dimensions.0, dimensions.1).unwrap());
        for _ in 0..300 {
            engine.step();
            engine.frame();
        }
    }
    let mut appeared = false;
    for _ in 0..600 {
        engine.step();
        appeared |= has_phrase(&mut engine, "anything");
    }
    assert!(appeared);
}

#[test]
fn phrases_validate_bounds_and_single_column_text() {
    for text in [
        "界",
        "a\tb",
        "hello\x1b",
        &"a".repeat(65),
        &"a\n".repeat(17),
        &"a".repeat(8193),
    ] {
        assert!(
            EffectConfig::default()
                .with_words(text, WordFrequency::Occasional)
                .is_err(),
            "{text:?}"
        );
    }
    assert!(
        EffectConfig::default()
            .with_words("  wake up  \r\nｱ01", WordFrequency::Rare)
            .is_ok()
    );
    assert!("sometimes".parse::<WordFrequency>().is_err());
}

#[test]
fn frequency_controls_the_number_of_appearances() {
    let count = |frequency| {
        let mut engine = Engine::rain(
            GridSize::new(40, 20).unwrap(),
            EffectConfig::default()
                .with_words("anything", frequency)
                .unwrap(),
            42,
        );
        let mut count = 0;
        let mut previous = false;
        for _ in 0..3600 {
            engine.step();
            let visible = has_phrase(&mut engine, "anything");
            if visible && !previous {
                count += 1;
            }
            previous = visible;
        }
        count
    };
    let rare = count(WordFrequency::Rare);
    let occasional = count(WordFrequency::Occasional);
    let frequent = count(WordFrequency::Frequent);
    assert!(
        rare > 0 && rare < occasional && occasional < frequent,
        "frequency did not affect appearances: {rare}, {occasional}, {frequent}"
    );
}
