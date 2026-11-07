#[derive(Clone, Copy, Default)]
pub struct EnvCoeffs {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl EnvCoeffs {
    /// Linear attack; decay and release are exponential and reach about
    /// −43 dB of their distance in the given time.
    pub fn new(attack_ms: f32, decay_ms: f32, sustain: f32, release_ms: f32, sr: f32) -> Self {
        let samples = |ms: f32| (ms * 0.001 * sr).max(1.0);
        let curve = |ms: f32| 1.0 - (-5.0 / samples(ms)).exp();
        Self {
            attack: 1.0 / samples(attack_ms),
            decay: curve(decay_ms),
            sustain: sustain.clamp(0.0, 1.0),
            release: curve(release_ms),
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Stage {
    #[default]
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Copy, Default)]
pub struct Env {
    pub level: f32,
    pub stage: Stage,
}

impl Env {
    /// Restarts from the current level, so a retriggered voice doesn't click.
    pub fn trigger(&mut self) {
        self.stage = Stage::Attack;
    }

    pub fn release(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    pub fn kill(&mut self) {
        *self = Self::default();
    }

    #[inline]
    pub fn tick(&mut self, c: &EnvCoeffs) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += c.attack;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level += (c.sustain - self.level) * c.decay;
                if (self.level - c.sustain).abs() < 1e-4 {
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => self.level = c.sustain,
            Stage::Release => {
                self.level -= self.level * c.release;
                if self.level < 1e-5 {
                    self.kill();
                }
            }
        }
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_through_all_stages() {
        let c = EnvCoeffs::new(10.0, 50.0, 0.5, 50.0, 1000.0);
        let mut e = Env::default();
        e.trigger();
        for _ in 0..10 {
            e.tick(&c);
        }
        assert_eq!(e.stage, Stage::Decay);
        for _ in 0..200 {
            e.tick(&c);
        }
        assert_eq!(e.stage, Stage::Sustain);
        assert!((e.level - 0.5).abs() < 1e-3);
        e.release();
        for _ in 0..500 {
            e.tick(&c);
        }
        assert_eq!(e.stage, Stage::Idle);
    }
}
