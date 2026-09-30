//! Small visual presets shared by terminal and future native adapters.
use std::str::FromStr;

use crate::{ConfigError, EffectConfig};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Monochrome,
    Matrix,
}

/// Theme defaults; adapters apply explicit user overrides before validation.
pub struct RainDefaults {
    pub speed: f64,
    pub density: f64,
    pub intensity: f64,
    pub characters: &'static str,
}

impl Theme {
    pub fn rain_defaults(self) -> RainDefaults {
        match self {
            Self::Monochrome => RainDefaults {
                speed: 1.0,
                density: 0.18,
                intensity: 0.7,
                characters: "0123456789.:+*",
            },
            Self::Matrix => RainDefaults {
                speed: 1.6,
                density: 0.35,
                intensity: 1.0,
                // Half-width Katakana; exclude combining voiced sound marks.
                characters: "ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜﾝ0123456789:<>*+-",
            },
        }
    }

    pub fn rain_config(self) -> EffectConfig {
        let defaults = self.rain_defaults();
        EffectConfig::new(
            defaults.speed,
            defaults.density,
            defaults.intensity,
            defaults.characters,
        )
        .expect("valid built-in theme")
    }

    /// Ideal RGB color, before an adapter reduces it to its display palette.
    pub fn rgb(self, intensity: u8, emphasis: bool) -> [u8; 3] {
        let value = u16::from(intensity);
        match self {
            Self::Monochrome => [intensity; 3],
            Self::Matrix if emphasis => [
                (value * 210 / 255) as u8,
                intensity,
                (value * 225 / 255) as u8,
            ],
            Self::Matrix => [0, (value * 220 / 255) as u8, (value * 45 / 255) as u8],
        }
    }
}

impl FromStr for Theme {
    type Err = ConfigError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "monochrome" => Ok(Self::Monochrome),
            "matrix" => Ok(Self::Matrix),
            _ => Err(ConfigError(format!(
                "unknown theme {value:?}; available: monochrome, matrix"
            ))),
        }
    }
}
