use std::time::Duration;

use stillterm_engine::TICKS_PER_SECOND;

/// Integer tick accumulation. Hosts discard excessive suspension time.
#[derive(Default)]
pub struct StepClock {
    scaled_nanos: u128,
}

impl StepClock {
    pub fn advance(&mut self, elapsed: Duration) -> u32 {
        let elapsed = elapsed.min(Duration::from_millis(250));
        self.scaled_nanos += elapsed.as_nanos() * u128::from(TICKS_PER_SECOND);
        let ticks = self.scaled_nanos / 1_000_000_000;
        self.scaled_nanos %= 1_000_000_000;
        ticks as u32
    }

    pub fn reset(&mut self) {
        self.scaled_nanos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendering_cadence_does_not_change_tick_count() {
        let mut fast = StepClock::default();
        let mut slow = StepClock::default();
        let a: u32 = (0..100)
            .map(|_| fast.advance(Duration::from_millis(10)))
            .sum();
        let b: u32 = (0..10)
            .map(|_| slow.advance(Duration::from_millis(100)))
            .sum();
        assert_eq!(a, TICKS_PER_SECOND);
        assert_eq!(a, b);
    }

    #[test]
    fn suspension_is_bounded_and_reset_discards_partial_steps() {
        let mut clock = StepClock::default();
        assert!(clock.advance(Duration::from_secs(3600)) <= 8);
        clock.reset();
        assert_eq!(clock.advance(Duration::from_millis(16)), 0);
    }
}
