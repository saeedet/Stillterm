//! Per-user native settings; independent from the terminal's explicit config.
use serde::Deserialize;
use stillterm_engine::{EffectConfig, Theme};

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub theme: String,
    pub fps: u16,
    pub font_size: u16,
    pub seed: u64,
    pub speed: Option<f64>,
    pub density: Option<f64>,
    pub intensity: Option<f64>,
    pub characters: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "monochrome".into(),
            fps: 30,
            font_size: 18,
            seed: 42,
            speed: None,
            density: None,
            intensity: None,
            characters: String::new(),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(Theme, EffectConfig), String> {
        if !(10..=60).contains(&self.fps) {
            return Err("FPS must be between 10 and 60".into());
        }
        if !(10..=48).contains(&self.font_size) {
            return Err("Character size must be between 10 and 48".into());
        }
        let theme: Theme = self
            .theme
            .parse()
            .map_err(|e: stillterm_engine::ConfigError| e.to_string())?;
        let preset = theme.rain_defaults();
        let effect = EffectConfig::new(
            self.speed.unwrap_or(preset.speed),
            self.density.unwrap_or(preset.density),
            self.intensity.unwrap_or(preset.intensity),
            if self.characters.is_empty() {
                preset.characters
            } else {
                &self.characters
            },
        )
        .map_err(|e| e.to_string())?;
        Ok((theme, effect))
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 65_536 {
            return Err("Settings exceed 64 KiB".into());
        }
        let settings: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        settings.validate()?;
        Ok(settings)
    }
    pub fn to_toml(&self) -> Result<String, String> {
        self.validate()?;
        // TOML's string formatter escapes quotes and backslashes in custom glyphs.
        let chars = toml::Value::String(self.characters.clone());
        let mut text = format!(
            "theme = \"{}\"\nfps = {}\nfont_size = {}\nseed = {}\ncharacters = {}\n",
            self.theme, self.fps, self.font_size, self.seed, chars
        );
        for (key, value) in [
            ("speed", self.speed),
            ("density", self.density),
            ("intensity", self.intensity),
        ] {
            if let Some(value) = value {
                text.push_str(&format!("{key} = {value}\n"));
            }
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_and_overrides_validate() {
        for theme in ["monochrome", "matrix"] {
            let settings =
                Settings::parse(&format!("theme = '{theme}'\nseed = 9223372036854775807")).unwrap();
            let roundtrip = Settings::parse(&settings.to_toml().unwrap()).unwrap();
            assert_eq!(roundtrip.theme, theme);
            assert_eq!(roundtrip.seed, settings.seed);
        }
        let settings = Settings {
            characters: "01\"\\".into(),
            speed: Some(0.5),
            ..Settings::default()
        };
        assert_eq!(
            Settings::parse(&settings.to_toml().unwrap())
                .unwrap()
                .characters,
            settings.characters
        );
    }
    #[test]
    fn invalid_settings_fail_before_a_window_opens() {
        for input in [
            "fps = 0",
            "font_size = 100",
            "speed = nan",
            "density = 2.0",
            "theme = 'missing'",
            "characters = '界'",
            "seed = -1",
            "speeed = 1",
        ] {
            assert!(Settings::parse(input).is_err(), "{input}");
        }
    }
}
