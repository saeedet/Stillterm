use rand_chacha::{ChaCha8Rng, rand_core::RngCore};

use super::Effect;
use crate::{Cell, EffectConfig, Frame, GridSize, words::Words};

#[derive(Debug)]
struct Stream {
    head: u32,
    length: u32,
    velocity: u32,
    wait: u32,
    salt: u64,
    active: bool,
}

/// Sparse streams with fixed-point movement and staggered glyph changes.
pub struct Rain {
    config: EffectConfig,
    rng: ChaCha8Rng,
    size: GridSize,
    streams: Vec<Stream>,
    tick: u64,
    words: Words,
}

impl Rain {
    /// Randomness is supplied explicitly; rendering never touches it.
    pub fn new(config: EffectConfig, rng: ChaCha8Rng) -> Self {
        let words = Words::new(rng.clone());
        Self {
            words,
            config,
            rng,
            size: GridSize::new(0, 0).expect("empty grid"),
            streams: Vec::new(),
            tick: 0,
        }
    }

    fn stream(&mut self, distributed: bool) -> Stream {
        let length = 5 + self.rng.next_u32() % 14;
        let active = self.rng.next_u32() % 1000 < self.config.density;
        Stream {
            head: if distributed {
                self.rng.next_u32() % (u32::from(self.size.rows()) + length) * 256
            } else {
                0
            },
            length,
            velocity: (12 + self.rng.next_u32() % 29) * self.config.speed / 1000,
            wait: 15 + self.rng.next_u32() % 90,
            salt: self.rng.next_u64(),
            active,
        }
    }
}

impl Effect for Rain {
    fn resize(&mut self, size: GridSize) {
        self.words.resize();
        self.size = size;
        let columns = if size.is_empty() {
            0
        } else {
            usize::from(size.columns())
        };
        self.streams.truncate(columns);
        while self.streams.len() < columns {
            let stream = self.stream(true);
            self.streams.push(stream);
        }
    }

    fn step(&mut self) {
        if self.size.is_empty() {
            return;
        }
        self.tick = self.tick.wrapping_add(1);
        for column in 0..self.streams.len() {
            let stream = &mut self.streams[column];
            if stream.active {
                stream.head += stream.velocity;
                if stream.head / 256 >= u32::from(self.size.rows()) + stream.length {
                    self.streams[column] = self.stream(false);
                }
            } else if stream.wait > 0 {
                stream.wait -= 1;
            } else {
                self.streams[column] = self.stream(false);
            }
        }
        if let Some((column, length)) = self.words.step(
            &self.config.words,
            self.size,
            self.streams
                .iter()
                .enumerate()
                .filter(|(_, stream)| stream.active)
                .map(|(column, stream)| (column, stream.salt, stream.head / 256)),
        ) {
            self.streams[column].length = self.streams[column].length.max(length);
        }
    }

    fn render(&self, frame: &mut Frame) {
        for (column, stream) in self.streams.iter().enumerate().filter(|(_, s)| s.active) {
            let head_row = stream.head / 256;
            let start = head_row.saturating_sub(stream.length);
            let end = head_row.min(u32::from(self.size.rows()).saturating_sub(1));
            for row in start..=end {
                let distance = stream.head - row * 256;
                let remaining = (stream.length * 256).saturating_sub(distance);
                let intensity =
                    (remaining * 255 / (stream.length * 256)) * self.config.intensity / 1000;
                // Integer hashing makes glyph choices independent of platform math libraries.
                let phase = self.tick.wrapping_add(u64::from(row) * 13) / 90;
                let mut hash =
                    stream.salt ^ u64::from(row).wrapping_mul(0x9e3779b97f4a7c15) ^ phase;
                hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d049bb133111eb);
                hash ^= hash >> 31;
                let word =
                    self.words
                        .glyph(&self.config.words, column, stream.salt, head_row - row);
                let glyph = word.unwrap_or(
                    self.config.glyphs[(hash % self.config.glyphs.len() as u64) as usize],
                );
                // Every letter receives the same bright treatment as the head.
                // The surrounding random glyphs retain the stream's fading trail.
                let intensity = if word.is_some() {
                    let head_remaining = stream.length * 256 - stream.head % 256;
                    (head_remaining * 255 / (stream.length * 256)) * self.config.intensity / 1000
                } else {
                    intensity
                };
                frame.set(
                    column as u16,
                    row as u16,
                    Cell {
                        glyph,
                        intensity: intensity as u8,
                        emphasis: word.is_some() || row == head_row,
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::rand_core::SeedableRng;

    #[test]
    fn stream_recycles_after_its_trail_leaves_the_grid() {
        let config = EffectConfig::new(1.0, 1.0, 1.0, "01").unwrap();
        let mut rain = Rain::new(config, ChaCha8Rng::seed_from_u64(42));
        rain.resize(GridSize::new(1, 10).unwrap());
        let stream = &mut rain.streams[0];
        let previous_salt = stream.salt;
        stream.head = (10 + stream.length) * 256 - stream.velocity;
        rain.step();
        assert_eq!(rain.streams[0].head, 0);
        assert!(rain.streams[0].active);
        assert_ne!(rain.streams[0].salt, previous_salt);
    }

    #[test]
    fn resize_keeps_surviving_columns_in_motion() {
        let mut rain = Rain::new(EffectConfig::default(), ChaCha8Rng::seed_from_u64(42));
        rain.resize(GridSize::new(80, 24).unwrap());
        let before: Vec<_> = rain.streams[..40]
            .iter()
            .map(|s| (s.head, s.salt))
            .collect();
        rain.resize(GridSize::new(40, 12).unwrap());
        assert_eq!(
            before,
            rain.streams
                .iter()
                .map(|s| (s.head, s.salt))
                .collect::<Vec<_>>()
        );
        rain.resize(GridSize::new(100, 40).unwrap());
        assert_eq!(
            before,
            rain.streams[..40]
                .iter()
                .map(|s| (s.head, s.salt))
                .collect::<Vec<_>>()
        );
    }
}

#[cfg(test)]
mod word_style_tests {
    use super::*;
    use crate::WordFrequency;
    use rand_chacha::rand_core::SeedableRng;

    #[test]
    fn every_word_letter_uses_the_heads_brightness_and_highlight() {
        let size = GridSize::new(40, 20).unwrap();
        let config = EffectConfig::new(1.6, 0.35, 1.0, "01")
            .unwrap()
            .with_words("anything", WordFrequency::Frequent)
            .unwrap();
        let mut rain = Rain::new(config, ChaCha8Rng::seed_from_u64(42));
        rain.resize(size);
        let mut words = Frame::new(size);
        let mut plain = Frame::new(size);
        let mut ready = false;
        for _ in 0..360 {
            rain.step();
            words.clear();
            rain.render(&mut words);
            if words
                .cells()
                .iter()
                .filter(|cell| {
                    matches!(
                        cell.glyph.as_char(),
                        'a' | 'n' | 'y' | 't' | 'h' | 'i' | 'g'
                    )
                })
                .count()
                == 8
            {
                ready = true;
                break;
            }
        }
        assert!(ready, "the complete word never appeared");
        rain.words.resize();
        rain.render(&mut plain);
        let mut changed = 0;
        for (index, (word, ordinary)) in words.cells().iter().zip(plain.cells()).enumerate() {
            if word.glyph != ordinary.glyph {
                let column = index % usize::from(size.columns());
                let head = rain.streams[column].head as usize / 256;
                let head_cell = plain.cells()[head * usize::from(size.columns()) + column];
                assert_eq!(word.intensity, head_cell.intensity);
                assert!(word.emphasis, "a word letter lost its bright highlight");
                changed += 1;
            } else {
                assert_eq!(word, ordinary, "ordinary rain styling changed");
            }
        }
        assert_eq!(changed, 8);
    }
}
