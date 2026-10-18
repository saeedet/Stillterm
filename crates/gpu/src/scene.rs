//! Two independently animated rain layers on each of five room surfaces.
use std::{collections::HashMap, time::Duration};
use stillterm_engine::{EffectConfig, Engine, GridSize, StepClock, WordFrequency};

pub const COLUMNS: u32 = 192;
pub const ROWS: u32 = 64;
pub const FACES: u32 = 5;
pub const SHEETS: u32 = FACES * 2;
pub const MAX_GLYPHS: usize = 1280;

/// Stable atlas order, including word letters missing from the rain alphabet.
pub fn glyphs(characters: &str, words: &str) -> Vec<char> {
    let mut result = Vec::new();
    for glyph in characters
        .chars()
        .chain(words.chars())
        .filter(|c| !c.is_control())
    {
        if !result.contains(&glyph) {
            result.push(glyph);
        }
    }
    result
}

pub struct Scene {
    sheets: Vec<Engine>,
    glyphs: HashMap<char, u16>,
    clock: StepClock,
    pixels: Vec<u8>,
}
impl Scene {
    pub fn new(config: EffectConfig, atlas: &[char], seed: u64) -> Self {
        let size = GridSize::new(COLUMNS as u16, ROWS as u16).expect("bounded sheet");
        let sheets = (0..SHEETS)
            .map(|sheet| {
                let face = sheet % FACES;
                let salt = u64::from(sheet).wrapping_mul(0x9e3779b97f4a7c15);
                let mut config = config.clone().with_vertical_fade(false);
                if sheet >= FACES || face == 2 || face == 3 {
                    config = config
                        .with_words("", WordFrequency::Occasional)
                        .expect("empty words");
                }
                Engine::rain(size, config, seed ^ salt)
            })
            .collect();
        Self {
            sheets,
            glyphs: atlas
                .iter()
                .enumerate()
                .map(|(i, c)| (*c, i as u16))
                .collect(),
            clock: StepClock::default(),
            pixels: vec![0; (COLUMNS * ROWS * SHEETS * 4) as usize],
        }
    }
    pub fn advance(&mut self, elapsed: Duration) {
        let elapsed = elapsed.min(Duration::from_millis(250));
        for _ in 0..self.clock.advance(elapsed) {
            for sheet in &mut self.sheets {
                sheet.step();
            }
        }
    }
    /// RGBA8Uint: low/high atlas index, intensity, head/word emphasis.
    pub fn pixels(&mut self) -> &[u8] {
        let mut destination = self.pixels.chunks_exact_mut((COLUMNS * ROWS * 4) as usize);
        for sheet in &mut self.sheets {
            for (pixel, cell) in destination
                .next()
                .expect("one buffer per rain sheet")
                .chunks_exact_mut(4)
                .zip(sheet.frame().cells())
            {
                let glyph = self.glyphs.get(&cell.glyph.as_char()).copied().unwrap_or(0);
                pixel.copy_from_slice(&[
                    glyph as u8,
                    (glyph >> 8) as u8,
                    cell.intensity,
                    u8::from(cell.emphasis),
                ]);
            }
        }
        &self.pixels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stillterm_engine::Theme;
    #[test]
    fn sheets_reproduce_frames_and_include_word_letters_in_the_atlas() {
        let defaults = Theme::DenseMatrix.rain_defaults();
        let atlas = glyphs(defaults.characters, "FREE");
        assert!(atlas.len() <= MAX_GLYPHS && atlas.contains(&'F'));
        let config = Theme::DenseMatrix
            .rain_config()
            .with_words("FREE", WordFrequency::Frequent)
            .unwrap();
        let mut a = Scene::new(config.clone(), &atlas, 42);
        let mut b = Scene::new(config, &atlas, 42);
        let initial = a.pixels().to_vec();
        for _ in 0..160 {
            a.advance(Duration::from_millis(100));
            b.advance(Duration::from_millis(100));
            assert_eq!(a.pixels(), b.pixels());
            for pixel in a.pixels().chunks_exact(4) {
                assert!(usize::from(u16::from_le_bytes([pixel[0], pixel[1]])) < atlas.len());
            }
        }
        assert_ne!(initial, a.pixels());
        let pixels = a.pixels();
        let face = (COLUMNS * ROWS * 4) as usize;
        assert_eq!(pixels.len(), face * SHEETS as usize);
        let sheets: Vec<_> = pixels.chunks_exact(face).collect();
        for (i, sheet) in sheets.iter().enumerate() {
            assert!(sheet.chunks_exact(4).any(|pixel| pixel[2] > 0));
            for other in &sheets[..i] {
                assert_ne!(sheet, other);
            }
        }
    }
}
