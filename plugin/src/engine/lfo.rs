use super::osc::Rng;
use std::f32::consts::TAU;

pub const SINE: usize = 0;
pub const SQUARE: usize = 1;
pub const TRIANGLE: usize = 2;

pub struct Lfo {
    phase: f32,
    held: f32,
    rng: Rng,
}

impl Lfo {
    pub fn new(seed: u32) -> Self {
        Self { phase: 0.0, held: 0.0, rng: Rng::new(seed) }
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Locks the phase to the host's position, for tempo-synced LFOs.
    pub fn set_phase(&mut self, phase: f32) {
        self.phase = phase.rem_euclid(1.0);
    }

    #[inline]
    pub fn tick(&mut self, shape: usize, inc: f32) -> f32 {
        self.phase += inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.held = self.rng.bipolar();
        }
        match shape {
            SINE => (TAU * self.phase).sin(),
            SQUARE => {
                if self.phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            TRIANGLE => 1.0 - 4.0 * (self.phase - 0.5).abs(),
            _ => self.held,
        }
    }
}
