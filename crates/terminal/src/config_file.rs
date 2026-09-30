use std::{error::Error, fs::File, io::Read};

use serde::Deserialize;
use stillterm_engine::{EffectConfig, Theme};

use crate::args::Args;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileConfig {
    effect: String,
    theme: String,
    fps: u16,
    speed: Option<f64>,
    density: Option<f64>,
    intensity: Option<f64>,
    characters: Option<String>,
    seed: u64,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            effect: "rain".into(),
            theme: "monochrome".into(),
            fps: 30,
            speed: None,
            density: None,
            intensity: None,
            characters: None,
            seed: 42,
        }
    }
}

pub struct Settings {
    pub theme: Theme,
    pub effect: EffectConfig,
    pub fps: u16,
    pub seed: u64,
}

impl Settings {
    pub fn load(args: &Args) -> Result<Self, Box<dyn Error>> {
        let config = if let Some(path) = &args.config {
            let mut text = String::new();
            File::open(path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?
                .take(65_537)
                .read_to_string(&mut text)?;
            if text.len() > 65_536 {
                return Err("configuration file must be at most 64 KiB".into());
            }
            toml::from_str(&text)
                .map_err(|e| format!("invalid configuration in {}: {e}", path.display()))?
        } else {
            FileConfig::default()
        };
        Self::merge(config, args)
    }

    fn merge(config: FileConfig, args: &Args) -> Result<Self, Box<dyn Error>> {
        let effect = args.effect.as_deref().unwrap_or(&config.effect);
        if effect != "rain" {
            return Err(format!("unknown effect {effect:?}; available: rain").into());
        }
        let fps = args.fps.unwrap_or(config.fps);
        if !(10..=60).contains(&fps) {
            return Err("fps must be between 10 and 60".into());
        }
        let theme: Theme = args.theme.as_deref().unwrap_or(&config.theme).parse()?;
        let defaults = theme.rain_defaults();
        Ok(Self {
            theme,
            effect: EffectConfig::new(
                args.speed.or(config.speed).unwrap_or(defaults.speed),
                args.density.or(config.density).unwrap_or(defaults.density),
                args.intensity
                    .or(config.intensity)
                    .unwrap_or(defaults.intensity),
                args.characters
                    .as_deref()
                    .or(config.characters.as_deref())
                    .unwrap_or(defaults.characters),
            )?,
            fps,
            seed: args.seed.unwrap_or(config.seed),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn theme_defaults_apply_before_explicit_file_and_cli_settings() {
        use stillterm_engine::{Engine, GridSize};
        let args = Args::parse_from(["stillterm", "--theme", "matrix", "--speed", "0.5"]);
        let settings = Settings::merge(
            toml::from_str("characters = '01'\ndensity = 0.2").unwrap(),
            &args,
        )
        .unwrap();
        assert_eq!(settings.theme, Theme::Matrix);
        let size = GridSize::new(40, 20).unwrap();
        let mut actual = Engine::rain(size, settings.effect, 42);
        let mut expected = Engine::rain(size, EffectConfig::new(0.5, 0.2, 1.0, "01").unwrap(), 42);
        for _ in 0..60 {
            actual.step();
            expected.step();
        }
        assert_eq!(actual.frame(), expected.frame());
        let file = toml::from_str("theme = 'matrix'").unwrap();
        let settings = Settings::merge(file, &Args::parse_from(["stillterm"])).unwrap();
        let mut actual = Engine::rain(size, settings.effect, 42);
        let mut expected = Engine::rain(size, Theme::Matrix.rain_config(), 42);
        assert_eq!(actual.frame(), expected.frame());
        let file = toml::from_str("theme = 'matrix'").unwrap();
        let settings = Settings::merge(
            file,
            &Args::parse_from(["stillterm", "--theme", "monochrome"]),
        )
        .unwrap();
        assert_eq!(settings.theme, Theme::Monochrome);
    }

    #[test]
    fn defaults_file_and_cli_have_explicit_precedence() {
        let file: FileConfig = toml::from_str("fps = 20\nseed = 7\nspeed = 0.5").unwrap();
        let args = Args::parse_from(["stillterm", "run", "--fps", "60"]);
        let settings = Settings::merge(file, &args).unwrap();
        assert_eq!(settings.fps, 60);
        assert_eq!(settings.seed, 7);
        assert_eq!(
            Settings::load(&Args::parse_from(["stillterm"]))
                .unwrap()
                .fps,
            30
        );
    }

    #[test]
    fn file_typos_and_invalid_values_are_errors() {
        assert!(toml::from_str::<FileConfig>("speeed = 1.0").is_err());
        for input in [
            "fps = 0",
            "fps = 61",
            "speed = nan",
            "density = 2.0",
            "effect = 'other'",
            "theme = 'other'",
            "characters = ''",
        ] {
            let config = toml::from_str(input).unwrap();
            assert!(Settings::merge(config, &Args::parse_from(["stillterm"])).is_err());
        }
    }
}
