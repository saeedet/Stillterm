use std::{error::Error, fmt};

use crate::{Glyph, WordFrequency, words::WordConfig};

/// A configuration error suitable for displaying before opening a surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(pub(crate) String);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ConfigError {}

/// Validated effect settings. Floats are quantized once, before simulation.
#[derive(Debug, Clone)]
pub struct EffectConfig {
    pub(crate) speed: u32,
    pub(crate) density: u32,
    pub(crate) intensity: u32,
    pub(crate) glyphs: Vec<Glyph>,
    pub(crate) words: WordConfig,
    pub(crate) dense_trails: bool,
}

impl EffectConfig {
    /// Speed: 0.1–4.0; density and intensity: 0.0–1.0.
    /// Characters: 1–256 printable, single-column Unicode scalars.
    pub fn new(
        speed: f64,
        density: f64,
        intensity: f64,
        characters: &str,
    ) -> Result<Self, ConfigError> {
        fn scaled(name: &str, value: f64, min: f64, max: f64) -> Result<u32, ConfigError> {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(ConfigError(format!(
                    "{name} must be between {min} and {max}"
                )));
            }
            Ok((value * 1000.0).round() as u32)
        }
        let speed = scaled("speed", speed, 0.1, 4.0)?;
        let density = scaled("density", density, 0.0, 1.0)?;
        let intensity = scaled("intensity", intensity, 0.0, 1.0)?;
        let mut glyphs = Vec::new();
        for ch in characters.chars() {
            if glyphs.len() == 256 {
                return Err(ConfigError(
                    "characters must contain at most 256 glyphs".into(),
                ));
            }
            glyphs.push(Glyph::new(ch)?);
        }
        if glyphs.is_empty() {
            return Err(ConfigError("characters must not be empty".into()));
        }
        Ok(Self {
            speed,
            density,
            intensity,
            glyphs,
            words: WordConfig::default(),
            dense_trails: false,
        })
    }
    /// Apply a theme's trail style while preserving numeric and glyph overrides.
    pub fn with_theme_style(mut self, theme: crate::Theme) -> Self {
        self.dense_trails = theme == crate::Theme::DenseMatrix;
        self
    }
    /// Add up to 16 ordered phrases separated by commas or line breaks, each at
    /// most 64 printable single-column characters. Empty entries are ignored;
    /// empty text disables it.
    pub fn with_words(mut self, text: &str, frequency: WordFrequency) -> Result<Self, ConfigError> {
        self.words = WordConfig::new(text, frequency)?;
        Ok(self)
    }
}

impl Default for EffectConfig {
    fn default() -> Self {
        crate::Theme::Monochrome.rain_config()
    }
}
