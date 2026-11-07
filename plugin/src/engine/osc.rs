use std::f32::consts::TAU;

pub const SINE: usize = 0;
pub const SAW: usize = 1;
pub const SQUARE: usize = 2;
pub const TRIANGLE: usize = 3;
pub const ORGAN: usize = 4;
const SHAPES: usize = 5;

pub const WAVETABLE: usize = 4;
pub const PWM: usize = 5;
pub const SUPERSAW: usize = 6;
pub const NOISE: usize = 7;

const N: usize = 2048;
const STRIDE: usize = N + 1;
const LEVELS: usize = 11;
const LOWEST_TOP_HZ: f32 = 20.0;

/// Band-limited single-cycle tables, one mip level per octave so the highest
/// harmonic of every level stays under Nyquist.
pub struct Tables {
    data: Box<[f32]>,
}

impl Tables {
    pub fn new(sample_rate: f32) -> Self {
        let sin: Vec<f32> = (0..N).map(|i| (TAU * i as f32 / N as f32).sin()).collect();
        let mut data = vec![0.0; SHAPES * LEVELS * STRIDE];
        let mut acc = vec![0.0f64; N];
        for shape in 0..SHAPES {
            for level in 0..LEVELS {
                let top = LOWEST_TOP_HZ * (1 << level) as f32;
                let max_h = ((sample_rate * 0.5 / top) as usize).clamp(1, N / 2 - 1);
                acc.iter_mut().for_each(|a| *a = 0.0);
                for h in 1..=max_h {
                    let amp = harmonic(shape, h);
                    if amp == 0.0 {
                        continue;
                    }
                    for (i, a) in acc.iter_mut().enumerate() {
                        *a += amp * sin[(h * i) % N] as f64;
                    }
                }
                let off = (shape * LEVELS + level) * STRIDE;
                for i in 0..N {
                    data[off + i] = acc[i] as f32;
                }
                data[off + N] = data[off];
            }
        }
        Self { data: data.into_boxed_slice() }
    }

    #[inline]
    pub fn level_for(freq: f32) -> usize {
        if freq <= LOWEST_TOP_HZ {
            return 0;
        }
        ((freq / LOWEST_TOP_HZ).log2().ceil() as usize).min(LEVELS - 1)
    }

    #[inline]
    pub fn read(&self, shape: usize, level: usize, phase: f32) -> f32 {
        let p = phase * N as f32;
        let i = p as usize;
        let fr = p - i as f32;
        let t = &self.data[(shape * LEVELS + level) * STRIDE..];
        t[i] + (t[i + 1] - t[i]) * fr
    }

    /// Morphs sine → saw → square → organ across `pos` 0..1.
    #[inline]
    pub fn wavetable(&self, level: usize, phase: f32, pos: f32) -> f32 {
        const FRAMES: [usize; 4] = [SINE, SAW, SQUARE, ORGAN];
        let p = pos.clamp(0.0, 1.0) * 3.0;
        let a = (p as usize).min(2);
        let fr = p - a as f32;
        let va = self.read(FRAMES[a], level, phase);
        let vb = self.read(FRAMES[a + 1], level, phase);
        va + (vb - va) * fr
    }

    /// Pulse from the difference of two band-limited saws; `width` 0..1.
    #[inline]
    pub fn pulse(&self, level: usize, phase: f32, width: f32) -> f32 {
        let mut shifted = phase + width;
        if shifted >= 1.0 {
            shifted -= 1.0;
        }
        self.read(SAW, level, phase) - self.read(SAW, level, shifted)
    }
}

fn harmonic(shape: usize, h: usize) -> f64 {
    let h = h as f64;
    let odd = h as usize % 2 == 1;
    match shape {
        SINE => (h == 1.0) as u8 as f64,
        SAW => -2.0 / (std::f64::consts::PI * h),
        SQUARE if odd => 4.0 / (std::f64::consts::PI * h),
        TRIANGLE if odd => {
            let sign = if (h as usize / 2) % 2 == 0 { 1.0 } else { -1.0 };
            8.0 / (std::f64::consts::PI.powi(2) * h * h) * sign
        }
        ORGAN => match h as usize {
            1 => 1.0 / 1.4,
            2 => 0.5 / 1.4,
            3 => 0.25 / 1.4,
            _ => 0.0,
        },
        _ => 0.0,
    }
}

#[derive(Clone, Copy)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    /// Uniform in -1..1.
    #[inline]
    pub fn bipolar(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    #[inline]
    pub fn unit(&mut self) -> f32 {
        self.bipolar() * 0.5 + 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saw_is_rising_and_bounded() {
        let t = Tables::new(48_000.0);
        let lvl = Tables::level_for(110.0);
        assert!(t.read(SAW, lvl, 0.25) < t.read(SAW, lvl, 0.75));
        for i in 0..N {
            assert!(t.read(SAW, lvl, i as f32 / N as f32).abs() < 1.2);
        }
    }

    #[test]
    fn high_levels_have_few_harmonics() {
        let t = Tables::new(48_000.0);
        let top = LEVELS - 1;
        // At ~20 kHz only the fundamental fits, so the saw is a pure sine.
        let s = t.read(SAW, top, 0.25);
        let q = t.read(SINE, top, 0.25);
        assert!((s.abs() - q.abs() * 2.0 / std::f32::consts::PI).abs() < 1e-3);
    }

    #[test]
    fn mip_level_rises_with_frequency() {
        assert_eq!(Tables::level_for(10.0), 0);
        assert!(Tables::level_for(1000.0) < Tables::level_for(8000.0));
        assert_eq!(Tables::level_for(100_000.0), LEVELS - 1);
    }
}
