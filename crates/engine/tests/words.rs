use stillterm_engine::{EffectConfig, Engine, GridSize, Theme, WordFrequency};

fn phrase_position(engine: &mut Engine, phrase: &str) -> Option<(usize, usize)> {
    let width = usize::from(engine.size().columns());
    let height = usize::from(engine.size().rows());
    let letters: Vec<_> = phrase.chars().collect();
    if width == 0 || letters.len() > height {
        return None;
    }
    let cells = engine.frame().cells();
    for row in 0..=height - letters.len() {
        for column in 0..width {
            if letters.iter().enumerate().all(|(offset, glyph)| {
                let cell = cells[(row + offset) * width + column];
                cell.intensity > 0 && cell.glyph.as_char() == *glyph
            }) {
                return Some((column, row));
            }
        }
    }
    None
}

fn has_phrase(engine: &mut Engine, phrase: &str) -> bool {
    phrase_position(engine, phrase).is_some()
}

#[test]
fn phrases_stay_fixed_as_the_head_reveals_them_then_fade_back_into_the_same_rain() {
    for theme in [Theme::Matrix, Theme::Monochrome] {
        let size = GridSize::new(48, 18).unwrap();
        let mut words = Engine::rain(
            size,
            theme
                .rain_config()
                .with_words("FREE", WordFrequency::Occasional)
                .unwrap(),
            42,
        );
        let mut plain = Engine::rain(size, theme.rain_config(), 42);
        let mut position = None;
        let mut disappeared = false;
        for _ in 0..600 {
            words.step();
            plain.step();
            if let Some(current) = phrase_position(&mut words, "FREE") {
                if let Some(previous) = position {
                    assert_eq!(current, previous, "word moved after it was revealed");
                } else {
                    position = Some(current);
                }
            }
            if position.is_some() && words.frame() == plain.frame() {
                disappeared = true;
                break;
            }
        }
        assert!(position.is_some(), "phrase never appeared");
        assert!(
            disappeared,
            "phrase did not fade back into the unchanged rain"
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
    for dimensions in [(48, 18), (0, 0), (1, 1), (1, 8), (2, 2), (48, 18)] {
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

#[test]
fn vertical_phrases_fit_a_single_column_and_need_sufficient_height() {
    let config = EffectConfig::new(1.0, 1.0, 1.0, "01")
        .unwrap()
        .with_words("anything", WordFrequency::Frequent)
        .unwrap();
    let mut narrow = Engine::rain(GridSize::new(1, 12).unwrap(), config.clone(), 42);
    let mut short = Engine::rain(GridSize::new(40, 7).unwrap(), config, 42);
    let mut appeared = false;
    for _ in 0..2400 {
        narrow.step();
        short.step();
        appeared |= has_phrase(&mut narrow, "anything");
        assert!(!has_phrase(&mut short, "anything"));
    }
    assert!(appeared, "vertical word did not fit a one-column stream");
}

#[test]
fn zero_density_has_no_word_streams() {
    let config = EffectConfig::new(1.0, 0.0, 1.0, "01")
        .unwrap()
        .with_words("anything", WordFrequency::Frequent)
        .unwrap();
    let mut engine = Engine::rain(GridSize::new(40, 20).unwrap(), config, 42);
    for _ in 0..1800 {
        engine.step();
        assert!(
            engine
                .frame()
                .cells()
                .iter()
                .all(|cell| cell.intensity == 0)
        );
    }
}
