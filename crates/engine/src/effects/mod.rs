//! Built-in effects and their small extension interface.

mod rain;
pub use rain::Rain;

use crate::{Frame, GridSize};

/// Effects operate on fixed simulation steps, never wall time.
pub trait Effect {
    /// Resize simulation state. Existing columns should survive where practical.
    fn resize(&mut self, size: GridSize);
    /// Advance one tick at [`crate::TICKS_PER_SECOND`].
    fn step(&mut self);
    /// Draw into an already cleared frame of the current size. Must be repeatable.
    fn render(&self, frame: &mut Frame);
}
