use std::f32::consts::PI;

#[inline]
pub fn fast_tanh(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

#[inline]
fn prewarp(fc: f32, inv_sr: f32) -> f32 {
    (PI * fc * inv_sr).tan()
}

/// Simper/Cytomic trapezoidal state-variable filter.
#[derive(Default, Clone, Copy)]
pub struct Svf {
    s1: f32,
    s2: f32,
}

pub struct SvfOut {
    pub low: f32,
    pub band: f32,
}

impl Svf {
    #[inline]
    pub fn tick(&mut self, x: f32, g: f32, k: f32) -> SvfOut {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.s2;
        let v1 = a1 * self.s1 + a2 * v3;
        let v2 = self.s2 + a2 * self.s1 + a3 * v3;
        self.s1 = 2.0 * v1 - self.s1;
        self.s2 = 2.0 * v2 - self.s2;
        SvfOut { low: v2, band: v1 }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Zero-delay-feedback 4-pole ladder with a saturating input stage.
#[derive(Default, Clone, Copy)]
pub struct Ladder {
    s: [f32; 4],
}

impl Ladder {
    #[inline]
    pub fn tick(&mut self, x: f32, g: f32, res: f32) -> f32 {
        let k = res * 4.0;
        let big_g = g / (1.0 + g);
        let inv = 1.0 / (1.0 + g);
        let g2 = big_g * big_g;
        let sigma = (g2 * big_g * self.s[0] + g2 * self.s[1] + big_g * self.s[2] + self.s[3]) * inv;
        let u = fast_tanh((x - k * sigma) / (1.0 + k * g2 * g2));
        let mut y = u;
        for s in &mut self.s {
            let v = (y - *s) * big_g;
            y = v + *s;
            *s = y + v;
        }
        y * (1.0 + k * 0.5)
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub const COMB_LEN: usize = 2048;

pub struct Comb {
    buf: Box<[f32]>,
    pos: usize,
}

impl Comb {
    pub fn new() -> Self {
        Self { buf: vec![0.0; COMB_LEN].into_boxed_slice(), pos: 0 }
    }

    #[inline]
    pub fn tick(&mut self, x: f32, fc: f32, res: f32, sr: f32) -> f32 {
        let d = (sr / fc).clamp(2.0, (COMB_LEN - 4) as f32);
        let mut rp = self.pos as f32 - d;
        if rp < 0.0 {
            rp += COMB_LEN as f32;
        }
        let i0 = rp as usize;
        let fr = rp - i0 as f32;
        let i1 = (i0 + 1) % COMB_LEN;
        let fb = 0.5 + res * 0.45;
        let y = x + fb * (self.buf[i0] + (self.buf[i1] - self.buf[i0]) * fr);
        self.buf[self.pos] = y;
        self.pos = (self.pos + 1) % COMB_LEN;
        y * (1.0 - fb) * 1.5
    }

    pub fn reset(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
    }
}

const VOWEL_F1: [f32; 5] = [730.0, 530.0, 270.0, 570.0, 300.0];
const VOWEL_F2: [f32; 5] = [1090.0, 1840.0, 2290.0, 840.0, 870.0];

/// Two band-passes tuned to the first two formants of A-E-I-O-U.
#[derive(Default, Clone, Copy)]
pub struct Formant {
    a: Svf,
    b: Svf,
}

impl Formant {
    /// `shift` is the cutoff knob as 0..1, which moves both formants ±1 octave.
    #[inline]
    pub fn tick(&mut self, x: f32, vowel: f32, shift: f32, res: f32, sr: f32, inv_sr: f32) -> f32 {
        let p = vowel.clamp(0.0, 1.0) * 4.0;
        let i = (p as usize).min(3);
        let fr = p - i as f32;
        let sh = (2.0f32).powf((shift - 0.5) * 2.0);
        let fa = ((VOWEL_F1[i] + (VOWEL_F1[i + 1] - VOWEL_F1[i]) * fr) * sh).min(sr * 0.45);
        let fb = ((VOWEL_F2[i] + (VOWEL_F2[i + 1] - VOWEL_F2[i]) * fr) * sh).min(sr * 0.45);
        let k = 0.3 - 0.25 * res;
        let b1 = self.a.tick(x, prewarp(fa, inv_sr), k).band * k;
        let b2 = self.b.tick(x, prewarp(fb, inv_sr), k).band * k;
        (b1 + b2 * 0.7) * 2.0
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub const LOWPASS: usize = 0;
pub const LADDER: usize = 1;
pub const COMB: usize = 2;
pub const FORMANT: usize = 3;

pub struct VoiceFilter {
    svf: Svf,
    ladder: Ladder,
    comb: Comb,
    formant: Formant,
}

impl VoiceFilter {
    pub fn new() -> Self {
        Self { svf: Svf::default(), ladder: Ladder::default(), comb: Comb::new(), formant: Formant::default() }
    }

    pub fn reset(&mut self) {
        self.svf.reset();
        self.ladder.reset();
        self.comb.reset();
        self.formant.reset();
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn tick(&mut self, kind: usize, x: f32, fc: f32, cn: f32, res: f32, vowel: f32, sr: f32, inv_sr: f32) -> f32 {
        match kind {
            LOWPASS => self.svf.tick(x, prewarp(fc, inv_sr), 2.0 - 1.96 * res).low,
            LADDER => self.ladder.tick(x, prewarp(fc, inv_sr), res),
            COMB => self.comb.tick(x, fc, res, sr),
            FORMANT => self.formant.tick(x, vowel, cn, res, sr, inv_sr),
            _ => x,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms_through(mut f: impl FnMut(f32) -> f32, freq: f32, sr: f32) -> f32 {
        let n = (sr * 0.5) as usize;
        let mut sum = 0.0;
        for i in 0..n {
            let y = f((std::f32::consts::TAU * freq * i as f32 / sr).sin());
            if i > n / 2 {
                sum += y * y;
            }
        }
        (sum / (n / 2) as f32).sqrt()
    }

    #[test]
    fn svf_lowpass_attenuates_highs() {
        let sr = 48_000.0;
        let g = prewarp(500.0, 1.0 / sr);
        let mut a = Svf::default();
        let low = rms_through(|x| a.tick(x, g, 1.4).low, 100.0, sr);
        let mut b = Svf::default();
        let high = rms_through(|x| b.tick(x, g, 1.4).low, 8000.0, sr);
        assert!(low > 0.6 && high < 0.01, "low {low} high {high}");
    }

    #[test]
    fn ladder_is_stable_at_full_resonance() {
        let sr = 48_000.0;
        let g = prewarp(2000.0, 1.0 / sr);
        let mut l = Ladder::default();
        let out = rms_through(|x| l.tick(x, g, 1.0), 300.0, sr);
        assert!(out.is_finite() && out < 4.0, "{out}");
    }
}
