//! Optional phrases share the rain grid without changing its random sequence.
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

struct Appearance {
    phrase: usize,
    column: u16,
    row: u16,
    age: u32,
}

pub(crate) struct Words {
    rng: ChaCha8Rng,
    wait: u32,
    active: Option<Appearance>,
}

impl Words {
    pub(crate) fn new(mut rng: ChaCha8Rng) -> Self {
        // A quick first appearance makes settings previews useful. Subsequent
        // pauses follow the selected frequency, in fixed 30 Hz simulation ticks.
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

    pub(crate) fn step(&mut self, config: &WordConfig, size: GridSize) {
        if config.phrases.is_empty() || size.is_empty() {
            return;
        }
        if let Some(active) = &mut self.active {
            active.age += 1;
            if active.age >= 99 {
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
            .filter(|(_, phrase)| phrase.len() <= usize::from(size.columns()))
            .map(|(index, _)| index)
            .collect();
        if !fitting.is_empty() {
            let phrase = fitting[self.rng.next_u32() as usize % fitting.len()];
            let width = config.phrases[phrase].len() as u16;
            let margin = ((size.columns() - width) / 2).min(2);
            let column = margin
                + (self.rng.next_u32() % u32::from(size.columns() - width - 2 * margin + 1)) as u16;
            let row_margin = u16::from(size.rows() > 2);
            let row =
                row_margin + (self.rng.next_u32() % u32::from(size.rows() - 2 * row_margin)) as u16;
            self.active = Some(Appearance {
                phrase,
                column,
                row,
                age: 0,
            });
        } else {
            self.wait = config.frequency.delay(&mut self.rng);
        }
    }

    pub(crate) fn render(&self, config: &WordConfig, intensity: u32, frame: &mut Frame) {
        let Some(active) = &self.active else {
            return;
        };
        if intensity == 0 {
            return;
        }
        for (offset, glyph) in config.phrases[active.phrase].iter().enumerate() {
            let stagger = (offset as u32 * 17 + active.phrase as u32 * 13) % 15;
            if active.age < stagger || active.age >= 81 + stagger {
                continue;
            }
            let reveal = (active.age - stagger + 1).min(6);
            let fade = (99 - active.age).min(24);
            let brightness = 255 * intensity / 1000 * reveal / 6 * fade / 24;
            frame.set(
                active.column + offset as u16,
                active.row,
                Cell {
                    glyph: *glyph,
                    intensity: brightness as u8,
                    emphasis: active.age < 24,
                },
            );
        }
    }
}
