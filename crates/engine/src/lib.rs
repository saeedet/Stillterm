//! Deterministic character animation without terminal, window, or clock access.
//!
//! Adapters advance an [`Engine`] at [`TICKS_PER_SECOND`] and present its [`Frame`].
//! Rendering is read-only with respect to simulation state.

mod config;
pub mod effects;
mod engine;
mod frame;
mod theme;

pub use config::{ConfigError, EffectConfig};
pub use engine::{Engine, TICKS_PER_SECOND};
pub use frame::{Cell, Frame, Glyph, GridSize, MAX_CELLS};
pub use theme::{RainDefaults, Theme};

mod timing;
pub use timing::StepClock;
