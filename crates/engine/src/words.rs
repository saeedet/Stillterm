//! Optional phrases bind to real rain streams; ordinary rain keeps its sequence.
use std::str::FromStr;

use rand_chacha::{ChaCha8Rng, rand_core::RngCore};

use crate::{Cell, ConfigError, Frame, Glyph, GridSize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WordFrequency {
    Rare,
    #[default]
    Occasional,
    Frequent,
}

impl WordFrequency {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rare => "rare",
            Self::Occasional => "occasional",
            Self::Frequent => "frequent",
        }
    }

    fn delay(self, rng: &mut ChaCha8Rng) -> u32 {
        let (minimum, range) = match self {
            Self::Rare => (900, 601),
            Self::Occasional => (360, 241),
            Self::Frequent => (120, 91),
        };
        minimum + rng.next_u32() % range
    }
}

impl FromStr for WordFrequency {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "rare" => Ok(Self::Rare),
            "occasional" => Ok(Self::Occasional),
            "frequent" => Ok(Self::Frequent),
            _ => Err(ConfigError(
                "word frequency must be rare, occasional, or frequent".into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WordConfig {
    phrases: Vec<Vec<Glyph>>,
    frequency: WordFrequency,
}

impl WordConfig {
    pub(crate) fn new(text: &str, frequency: WordFrequency) -> Result<Self, ConfigError> {
        if text.len() > 8192 {
            return Err(ConfigError(
                "words must contain at most 8 KiB of UTF-8 text".into(),
            ));
        }
        let mut phrases = Vec::new();
        for line in text.lines() {
            let line = line.trim_matches(' ');
            if line.is_empty() {
                continue;
            }
            if phrases.len() == 16 {
                return Err(ConfigError(
                    "use at most 16 words or phrases, one per line".into(),
                ));
            }
            let glyphs: Vec<_> = line.chars().map(Glyph::new).collect::<Result<_, _>>()?;
            if glyphs.len() > 64 {
                return Err(ConfigError(
                    "each word or phrase must contain at most 64 characters".into(),
                ));
            }
            phrases.push(glyphs);
        }
        Ok(Self { phrases, frequency })
    }
}

const FADE_TICKS: u32 = 30;
const FADE_STAGGER: u32 = 9;

struct Appearance {
    phrase: usize,
    column: usize,
    row: u16,
    salt: u64,
    revealed: usize,
    fade_age: Option<u32>,
}

pub(crate) struct Words {
    rng: ChaCha8Rng,
    wait: u32,
    active: Option<Appearance>,
}

impl Words {
    pub(crate) fn new(mut rng: ChaCha8Rng) -> Self {
        let wait = 90 + rng.next_u32() % 61;
        Self {
            rng,
            wait,
            active: None,
        }
    }

    pub(crate) fn resize(&mut self) {
        self.active = None;
    }

    /// Anchor a phrase just ahead of a real stream. Each head crossing reveals
    /// another letter in place; the previous letters stay bright. Only after
    /// the last crossing do the letters fade, from top to bottom.
    /// Stream tuples contain (column, lifetime salt, head row).
    pub(crate) fn step(
        &mut self,
        config: &WordConfig,
        size: GridSize,
        streams: impl Iterator<Item = (usize, u64, u32)>,
    ) {
        if config.phrases.is_empty() || size.is_empty() {
            return;
        }
        if let Some(active) = &mut self.active {
            let length = config.phrases[active.phrase].len();
            let finished = if let Some(age) = &mut active.fade_age {
                *age += 1;
                *age >= FADE_TICKS + (length as u32 - 1) * FADE_STAGGER
            } else if let Some((_, _, head)) = streams
                .into_iter()
                .find(|(column, salt, _)| *column == active.column && *salt == active.salt)
            {
                active.revealed = (head + 1)
                    .saturating_sub(u32::from(active.row))
                    .min(length as u32) as usize;
                if active.revealed == length {
                    active.fade_age = Some(0);
                }
                false
            } else {
                true
            };
            if finished {
                self.active = None;
                self.wait = config.frequency.delay(&mut self.rng);
            }
            return;
        }
        if self.wait > 0 {
            self.wait -= 1;
            return;
        }
        let fitting: Vec<_> = config
            .phrases
            .iter()
            .enumerate()
            .filter(|(_, phrase)| phrase.len() <= usize::from(size.rows()))
            .map(|(index, _)| index)
            .collect();
        if fitting.is_empty() {
            return;
        }
        let phrase = fitting[self.rng.next_u32() as usize % fitting.len()];
        let length = config.phrases[phrase].len() as u32;
        let last_start = u32::from(size.rows()) - length;
        let candidates: Vec<_> = streams.filter(|(_, _, head)| *head <= last_start).collect();
        if candidates.is_empty() {
            return;
        }
        let (column, salt, head) = candidates[self.rng.next_u32() as usize % candidates.len()];
        let row = head + (last_start - head).min(2);
        let revealed = (head + 1).saturating_sub(row) as usize;
        self.active = Some(Appearance {
            phrase,
            column,
            row: row as u16,
            salt,
            revealed,
            fade_age: (revealed == length as usize).then_some(0),
        });
    }

    pub(crate) fn render(&self, config: &WordConfig, intensity: u32, frame: &mut Frame) {
        let Some(active) = &self.active else {
            return;
        };
        for (offset, glyph) in config.phrases[active.phrase]
            .iter()
            .take(active.revealed)
            .enumerate()
        {
            let age = active
                .fade_age
                .unwrap_or(0)
                .saturating_sub(offset as u32 * FADE_STAGGER);
            let remaining = FADE_TICKS.saturating_sub(age);
            if remaining == 0 {
                continue;
            }
            frame.set(
                active.column as u16,
                active.row + offset as u16,
                Cell {
                    glyph: *glyph,
                    intensity: (255 * intensity / 1000 * remaining / FADE_TICKS) as u8,
                    emphasis: true,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::rand_core::SeedableRng;

    #[test]
    fn free_is_revealed_in_place_holds_then_fades_from_f_to_last_e() {
        let config = WordConfig::new("FREE", WordFrequency::Frequent).unwrap();
        let size = GridSize::new(1, 10).unwrap();
        let mut words = Words::new(ChaCha8Rng::seed_from_u64(42));
        words.wait = 0;
        words.step(&config, size, std::iter::once((0, 123, 0)));
        let mut frame = Frame::new(size);
        let render = |words: &Words, frame: &mut Frame| {
            frame.clear();
            words.render(&config, 1000, frame);
            frame.cells()[2..6].to_vec()
        };
        assert!(
            render(&words, &mut frame)
                .iter()
                .all(|cell| cell.intensity == 0)
        );
        for (head, expected) in [(2, "F"), (3, "FR"), (4, "FRE"), (5, "FREE")] {
            words.step(&config, size, std::iter::once((0, 123, head)));
            let cells = render(&words, &mut frame);
            let visible: String = cells
                .iter()
                .filter(|cell| cell.intensity > 0)
                .map(|cell| cell.glyph.as_char())
                .collect();
            assert_eq!(visible, expected);
            assert!(
                cells[..expected.len()]
                    .iter()
                    .all(|cell| cell.intensity == 255 && cell.emphasis)
            );
        }
        for age in 1..=57 {
            // Once all letters are revealed, fading survives even the original
            // drop leaving the screen. It never moves the retained letters.
            words.step(&config, size, std::iter::empty());
            let cells = render(&words, &mut frame);
            for (offset, letter) in "FREE".chars().enumerate() {
                if cells[offset].intensity > 0 {
                    assert_eq!(cells[offset].glyph.as_char(), letter);
                }
            }
            match age {
                1 => {
                    assert!(cells[0].intensity < 255);
                    assert!(cells[1..].iter().all(|cell| cell.intensity == 255));
                }
                10 => {
                    assert!(cells[1].intensity < 255);
                    assert!(cells[2..].iter().all(|cell| cell.intensity == 255));
                }
                19 => {
                    assert!(cells[2].intensity < 255);
                    assert_eq!(cells[3].intensity, 255);
                }
                28 => assert!(cells[3].intensity < 255),
                30 => {
                    assert_eq!(cells[0].intensity, 0);
                    assert!(cells[1..].iter().all(|cell| cell.intensity > 0));
                }
                39 => {
                    assert_eq!(cells[1].intensity, 0);
                    assert!(cells[2..].iter().all(|cell| cell.intensity > 0));
                }
                48 => {
                    assert_eq!(cells[2].intensity, 0);
                    assert!(cells[3].intensity > 0);
                }
                57 => assert!(cells.iter().all(|cell| cell.intensity == 0)),
                _ => {}
            }
        }
    }
}
