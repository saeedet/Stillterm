//! Optional phrases bind to real rain streams; ordinary rain keeps its sequence.
use std::str::FromStr;

use rand_chacha::{ChaCha8Rng, rand_core::RngCore};

use crate::{ConfigError, Glyph, GridSize};

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
    column: usize,
    salt: u64,
}

pub(crate) struct Words {
    rng: ChaCha8Rng,
    wait: u32,
    active: Option<Appearance>,
}

impl Words {
    pub(crate) fn new(mut rng: ChaCha8Rng) -> Self {
        // Try the first word after 3–5 seconds; wait for a suitable falling
        // stream if necessary. Only this independent RNG schedules words.
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

    /// Bind a phrase to an existing active stream. Return its column and the
    /// minimum trail length needed to keep the letters inside the fading rain.
    /// Stream tuples contain (column, lifetime salt, head row).
    pub(crate) fn step(
        &mut self,
        config: &WordConfig,
        size: GridSize,
        streams: impl Iterator<Item = (usize, u64, u32)> + Clone,
    ) -> Option<(usize, u32)> {
        if config.phrases.is_empty() || size.is_empty() {
            return None;
        }
        if let Some(active) = &self.active {
            let length = config.phrases[active.phrase].len() as u32;
            let alive = streams.clone().any(|(column, salt, head)| {
                column == active.column
                    && salt == active.salt
                    && head < u32::from(size.rows()) + length - 1
            });
            if !alive {
                self.active = None;
                self.wait = config.frequency.delay(&mut self.rng);
            }
            return None;
        }
        if self.wait > 0 {
            self.wait -= 1;
            return None;
        }
        let fitting: Vec<_> = config
            .phrases
            .iter()
            .enumerate()
            .filter(|(_, phrase)| phrase.len() <= usize::from(size.rows()))
            .map(|(index, _)| index)
            .collect();
        if fitting.is_empty() {
            return None;
        }
        let phrase = fitting[self.rng.next_u32() as usize % fitting.len()];
        let length = config.phrases[phrase].len() as u32;
        // Start near the top so the full word has time to travel through the
        // screen, instead of appearing just before its stream leaves the bottom.
        let last_head = length - 1 + (u32::from(size.rows()) - length).min(3);
        let candidates: Vec<_> = streams
            .filter(|(_, _, head)| *head + 1 >= length && *head <= last_head)
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let (column, salt, _) = candidates[self.rng.next_u32() as usize % candidates.len()];
        self.active = Some(Appearance {
            phrase,
            column,
            salt,
        });
        Some((column, length + 8))
    }

    /// Only replace glyphs. Rain owns motion, brightness, fading and emphasis.
    pub(crate) fn glyph(
        &self,
        config: &WordConfig,
        column: usize,
        salt: u64,
        distance: u32,
    ) -> Option<Glyph> {
        let active = self.active.as_ref()?;
        if active.column != column || active.salt != salt {
            return None;
        }
        let phrase = &config.phrases[active.phrase];
        let offset = phrase.len().checked_sub(distance as usize + 1)?;
        Some(phrase[offset])
    }
}
