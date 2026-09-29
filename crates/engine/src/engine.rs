use rand_chacha::{ChaCha8Rng, rand_core::SeedableRng};

use crate::{
    EffectConfig, Frame, GridSize,
    effects::{Effect, Rain},
};

pub const TICKS_PER_SECOND: u32 = 30;

/// Owns simulation and frame storage. The caller supplies every time step.
pub struct Engine {
    effect: Box<dyn Effect>,
    frame: Frame,
}

impl Engine {
    pub fn rain(size: GridSize, config: EffectConfig, seed: u64) -> Self {
        Self::new(
            size,
            Box::new(Rain::new(config, ChaCha8Rng::seed_from_u64(seed))),
        )
    }

    pub fn new(size: GridSize, mut effect: Box<dyn Effect>) -> Self {
        effect.resize(size);
        Self {
            effect,
            frame: Frame::new(size),
        }
    }

    pub fn size(&self) -> GridSize {
        self.frame.size()
    }

    pub fn resize(&mut self, size: GridSize) {
        if size != self.frame.size() {
            self.effect.resize(size);
            self.frame.resize(size);
        }
    }

    pub fn step(&mut self) {
        self.effect.step();
    }

    pub fn frame(&mut self) -> &Frame {
        self.frame.clear();
        self.effect.render(&mut self.frame);
        &self.frame
    }
}
