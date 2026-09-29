use std::{error::Error, fs::File, io::Read};

use serde::Deserialize;
use stillterm_engine::EffectConfig;

use crate::args::Args;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileConfig {
    effect: String,
    fps: u16,
    speed: f64,
    density: f64,
    intensity: f64,
    characters: String,
    seed: u64,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            effect: "rain".into(),
            fps: 30,
            speed: 1.0,
            density: 0.18,
            intensity: 0.7,
            characters: "0123456789.:+*".into(),
            seed: 42,
        }
    }
}

pub struct Settings {
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
        Ok(Self {
            effect: EffectConfig::new(
                args.speed.unwrap_or(config.speed),
                args.density.unwrap_or(config.density),
                args.intensity.unwrap_or(config.intensity),
                args.characters.as_deref().unwrap_or(&config.characters),
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
            "characters = ''",
        ] {
            let config = toml::from_str(input).unwrap();
            assert!(Settings::merge(config, &Args::parse_from(["stillterm"])).is_err());
        }
    }
}
