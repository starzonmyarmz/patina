use super::filter::fast_tanh;
use crate::params::{Pid, Values, DIVISION_BEATS};
use std::f32::consts::TAU;

const MAX_DELAY_SECONDS: f32 = 4.0;
const CHORUS_SECONDS: f32 = 0.05;
const COMB_TUNING: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNING: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;
const EQ_LOW_HZ: f32 = 250.0;
const EQ_MID_HZ: f32 = 1000.0;
const EQ_HIGH_HZ: f32 = 4000.0;

/// Effect settings, refreshed every chunk.
#[derive(Default)]
pub struct FxPatch {
    grit: f32,
    drive: f32,
    drive_norm: f32,
    crush: f32,
    crush_rate: f32,
    crush_levels: f32,
    chorus_mix: f32,
    chorus_inc: f32,
    delay_samples: f32,
    delay_feedback: f32,
    delay_mix: f32,
    reverb_feedback: f32,
    reverb_damp: f32,
    reverb_mix: f32,
    eq: Option<[BiquadCoeffs; 3]>,
    comp_threshold: f32,
    width: f32,
}

impl FxPatch {
    pub fn read(p: &impl Values, sr: f32, tempo: f32) -> Self {
        let pct = |id| p.get(id) / 100.0;
        let grit = pct(Pid::Distortion);
        let drive = 1.0 + grit * 6.0;
        let crush = pct(Pid::Bitcrush);
        let delay_seconds = if p.on(Pid::DelaySync) {
            DIVISION_BEATS[p.index(Pid::DelayDivision)] * 60.0 / tempo
        } else {
            p.get(Pid::DelayTime) / 1000.0
        };
        let size = pct(Pid::ReverbSize);
        let gains = [p.get(Pid::EqLow), p.get(Pid::EqMid), p.get(Pid::EqHigh)];
        let comp = pct(Pid::Compressor);
        Self {
            grit,
            drive,
            drive_norm: 1.0 / drive.tanh(),
            crush,
            crush_rate: 1.0 - crush * 0.98,
            crush_levels: (8.0 - crush * 7.0).exp2(),
            chorus_mix: pct(Pid::ChorusMix) * 0.5,
            chorus_inc: (0.1 + pct(Pid::ChorusRate) * 4.9) / sr,
            delay_samples: (delay_seconds * sr).clamp(1.0, MAX_DELAY_SECONDS * sr - 4.0),
            delay_feedback: pct(Pid::DelayFeedback),
            delay_mix: pct(Pid::DelayMix),
            reverb_feedback: 0.7 + size * 0.28,
            reverb_damp: 0.2 + (1.0 - size) * 0.3,
            reverb_mix: pct(Pid::ReverbMix),
            eq: gains.iter().any(|g| g.abs() > 0.01).then(|| {
                [
                    BiquadCoeffs::low_shelf(EQ_LOW_HZ, gains[0], sr),
                    BiquadCoeffs::peak(EQ_MID_HZ, 0.7, gains[1], sr),
                    BiquadCoeffs::high_shelf(EQ_HIGH_HZ, gains[2], sr),
                ]
            }),
            comp_threshold: if comp > 0.0 { 10f32.powf(-40.0 * comp / 20.0) } else { 0.0 },
            width: pct(Pid::Width),
        }
    }
}

/// Past Nyquist the design turns unstable, so the band stops just short of it.
fn omega(freq: f32, sr: f32) -> f32 {
    TAU * freq.min(sr * 0.45) / sr
}

/// RBJ cookbook biquad, normalized by a0.
#[derive(Clone, Copy, Default)]
pub struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl BiquadCoeffs {
    fn shelf(freq: f32, gain_db: f32, sr: f32, high: bool) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w = omega(freq, sr);
        let (sin, cos) = w.sin_cos();
        let alpha = sin / 2.0 * 2f32.sqrt();
        let sq = 2.0 * a.sqrt() * alpha;
        let s = if high { 1.0 } else { -1.0 };
        let a0 = (a + 1.0) - s * (a - 1.0) * cos + sq;
        Self {
            b0: a * ((a + 1.0) + s * (a - 1.0) * cos + sq) / a0,
            b1: -2.0 * s * a * ((a - 1.0) + s * (a + 1.0) * cos) / a0,
            b2: a * ((a + 1.0) + s * (a - 1.0) * cos - sq) / a0,
            a1: 2.0 * s * ((a - 1.0) - s * (a + 1.0) * cos) / a0,
            a2: ((a + 1.0) - s * (a - 1.0) * cos - sq) / a0,
        }
    }

    pub fn low_shelf(freq: f32, gain_db: f32, sr: f32) -> Self {
        Self::shelf(freq, gain_db, sr, false)
    }

    pub fn high_shelf(freq: f32, gain_db: f32, sr: f32) -> Self {
        Self::shelf(freq, gain_db, sr, true)
    }

    pub fn peak(freq: f32, q: f32, gain_db: f32, sr: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w = omega(freq, sr);
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha / a;
        Self {
            b0: (1.0 + alpha * a) / a0,
            b1: -2.0 * cos / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha / a) / a0,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Biquad {
    z1: f32,
    z2: f32,
}

impl Biquad {
    #[inline]
    fn tick(&mut self, x: f32, c: &BiquadCoeffs) -> f32 {
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y
    }
}

struct Line {
    buf: Box<[f32]>,
    pos: usize,
}

impl Line {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(2)].into_boxed_slice(), pos: 0 }
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
    }

    #[inline]
    fn push(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
    }

    /// Linear-interpolated read `delay` samples behind the next write.
    #[inline]
    fn read(&self, delay: f32) -> f32 {
        let len = self.buf.len();
        let mut p = self.pos as f32 - delay;
        if p < 0.0 {
            p += len as f32;
        }
        let i0 = p as usize % len;
        let i1 = (i0 + 1) % len;
        let fr = p - p.floor();
        self.buf[i0] + (self.buf[i1] - self.buf[i0]) * fr
    }

    /// The oldest sample, for fixed-length comb and allpass lines.
    #[inline]
    fn front(&self) -> f32 {
        self.buf[self.pos]
    }
}

struct ReverbComb {
    line: Line,
    store: f32,
}

impl ReverbComb {
    #[inline]
    fn tick(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let y = self.line.front();
        self.store = y * (1.0 - damp) + self.store * damp;
        self.line.push(x + self.store * feedback);
        y
    }
}

#[inline]
fn allpass(line: &mut Line, x: f32) -> f32 {
    let b = line.front();
    line.push(x + b * 0.5);
    b - x
}

/// Freeverb: 8 damped combs and 4 allpasses per side, right side detuned.
struct Reverb {
    combs: [Vec<ReverbComb>; 2],
    allpasses: [Vec<Line>; 2],
}

impl Reverb {
    fn new(sr: f32) -> Self {
        let scale = sr / 44_100.0;
        let len = |n: usize| (n as f32 * scale) as usize;
        let side = |spread: usize| {
            (
                COMB_TUNING.iter().map(|&n| ReverbComb { line: Line::new(len(n + spread)), store: 0.0 }).collect(),
                ALLPASS_TUNING.iter().map(|&n| Line::new(len(n + spread))).collect(),
            )
        };
        let (cl, al) = side(0);
        let (cr, ar) = side(STEREO_SPREAD);
        Self { combs: [cl, cr], allpasses: [al, ar] }
    }

    fn clear(&mut self) {
        for c in self.combs.iter_mut().flatten() {
            c.line.clear();
            c.store = 0.0;
        }
        self.allpasses.iter_mut().flatten().for_each(Line::clear);
    }

    #[inline]
    fn tick(&mut self, input: f32, feedback: f32, damp: f32) -> (f32, f32) {
        let mut out = [0.0; 2];
        for (side, o) in out.iter_mut().enumerate() {
            let mut acc = 0.0;
            for c in &mut self.combs[side] {
                acc += c.tick(input, feedback, damp);
            }
            for a in &mut self.allpasses[side] {
                acc = allpass(a, acc);
            }
            *o = acc;
        }
        (out[0], out[1])
    }
}

pub struct Fx {
    crush_phase: f32,
    crush_hold: (f32, f32),
    chorus: [Line; 2],
    chorus_phase: f32,
    chorus_base: f32,
    chorus_depth: f32,
    delay: [Line; 2],
    delay_samples: f32,
    delay_glide: f32,
    delay_damp: f32,
    delay_lp: (f32, f32),
    reverb: Reverb,
    eq: [[Biquad; 3]; 2],
    comp_env: f32,
    comp_attack: f32,
    comp_release: f32,
}

impl Fx {
    pub fn new(sr: f32) -> Self {
        let delay_len = (MAX_DELAY_SECONDS * sr) as usize;
        let chorus_len = (CHORUS_SECONDS * sr) as usize;
        Self {
            crush_phase: 0.0,
            crush_hold: (0.0, 0.0),
            chorus: [Line::new(chorus_len), Line::new(chorus_len)],
            chorus_phase: 0.0,
            chorus_base: 0.007 * sr,
            chorus_depth: 0.004 * sr,
            delay: [Line::new(delay_len), Line::new(delay_len)],
            delay_samples: 0.0,
            delay_glide: 1.0 - (-1.0 / (0.05 * sr)).exp(),
            delay_damp: 1.0 - (-TAU * 6000.0 / sr).exp(),
            delay_lp: (0.0, 0.0),
            reverb: Reverb::new(sr),
            eq: Default::default(),
            comp_env: 0.0,
            comp_attack: 1.0 - (-1.0 / (0.005 * sr)).exp(),
            comp_release: 1.0 - (-1.0 / (0.1 * sr)).exp(),
        }
    }

    pub fn reset(&mut self) {
        self.chorus.iter_mut().for_each(Line::clear);
        self.delay.iter_mut().for_each(Line::clear);
        self.delay_lp = (0.0, 0.0);
        self.reverb.clear();
        self.eq = Default::default();
        self.comp_env = 0.0;
    }

    #[inline]
    pub fn tick(&mut self, mut l: f32, mut r: f32, p: &FxPatch) -> (f32, f32) {
        if p.grit > 0.001 {
            l = (l * p.drive).tanh() * p.drive_norm;
            r = (r * p.drive).tanh() * p.drive_norm;
        }

        if p.crush > 0.0 {
            self.crush_phase += p.crush_rate;
            if self.crush_phase >= 1.0 {
                self.crush_phase -= 1.0;
                let q = |x: f32| (x * p.crush_levels).round() / p.crush_levels;
                self.crush_hold = (q(l), q(r));
            }
            (l, r) = self.crush_hold;
        }

        if p.chorus_mix > 0.0 {
            self.chorus_phase = (self.chorus_phase + p.chorus_inc).fract();
            let (s, c) = (TAU * self.chorus_phase).sin_cos();
            self.chorus[0].push(l);
            self.chorus[1].push(r);
            let wl = self.chorus[0].read(self.chorus_base + (s * 0.5 + 0.5) * self.chorus_depth);
            let wr = self.chorus[1].read(self.chorus_base + (c * 0.5 + 0.5) * self.chorus_depth);
            l += (wl - l) * p.chorus_mix;
            r += (wr - r) * p.chorus_mix;
        }

        if p.delay_mix > 0.0 {
            if self.delay_samples == 0.0 {
                self.delay_samples = p.delay_samples;
            }
            self.delay_samples += (p.delay_samples - self.delay_samples) * self.delay_glide;
            let tl = self.delay[0].read(self.delay_samples);
            let tr = self.delay[1].read(self.delay_samples * 0.75);
            self.delay_lp.0 += (tr - self.delay_lp.0) * self.delay_damp;
            self.delay_lp.1 += (tl - self.delay_lp.1) * self.delay_damp;
            self.delay[0].push(l + fast_tanh(self.delay_lp.0 * p.delay_feedback));
            self.delay[1].push(r + fast_tanh(self.delay_lp.1 * p.delay_feedback));
            l += tl * p.delay_mix;
            r += tr * p.delay_mix;
        }

        if p.reverb_mix > 0.0 {
            let (wl, wr) = self.reverb.tick((l + r) * 0.015, p.reverb_feedback, p.reverb_damp);
            l = l * (1.0 - p.reverb_mix * 0.5) + wl * p.reverb_mix * 3.0;
            r = r * (1.0 - p.reverb_mix * 0.5) + wr * p.reverb_mix * 3.0;
        }

        if let Some(eq) = &p.eq {
            for (band, c) in eq.iter().enumerate() {
                l = self.eq[0][band].tick(l, c);
                r = self.eq[1][band].tick(r, c);
            }
        }

        if p.comp_threshold > 0.0 {
            let peak = l.abs().max(r.abs());
            let k = if peak > self.comp_env { self.comp_attack } else { self.comp_release };
            self.comp_env += (peak - self.comp_env) * k;
            if self.comp_env > p.comp_threshold {
                let gain = p.comp_threshold * (self.comp_env / p.comp_threshold).powf(0.25) / self.comp_env;
                l *= gain;
                r *= gain;
            }
        }

        if p.width != 1.0 {
            let mid = (l + r) * 0.5;
            let side = (l - r) * 0.5 * p.width;
            l = mid + side;
            r = mid - side;
        }
        (l, r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(c: &BiquadCoeffs, freq: f32, sr: f32) -> f32 {
        let mut b = Biquad::default();
        let n = (sr * 0.2) as usize;
        let mut sum = 0.0;
        for i in 0..n {
            let y = b.tick((TAU * freq * i as f32 / sr).sin(), c);
            if i >= n / 2 {
                sum += y * y;
            }
        }
        20.0 * ((sum / (n - n / 2) as f32).sqrt() * 2f32.sqrt()).log10()
    }

    #[test]
    fn bands_above_nyquist_stay_stable() {
        let sr = 1234.57;
        for c in [
            BiquadCoeffs::low_shelf(EQ_LOW_HZ, 6.0, sr),
            BiquadCoeffs::peak(EQ_MID_HZ, 0.7, 6.0, sr),
            BiquadCoeffs::high_shelf(EQ_HIGH_HZ, -6.0, sr),
        ] {
            let gain = response(&c, 100.0, sr);
            assert!(gain.is_finite() && gain < 12.0, "{gain} dB");
        }
    }

    #[test]
    fn shelves_and_peak_hit_their_gain() {
        let sr = 48_000.0;
        let low = BiquadCoeffs::low_shelf(EQ_LOW_HZ, 12.0, sr);
        assert!((response(&low, 40.0, sr) - 12.0).abs() < 0.5);
        assert!(response(&low, 8000.0, sr).abs() < 0.5);
        let high = BiquadCoeffs::high_shelf(EQ_HIGH_HZ, -12.0, sr);
        assert!((response(&high, 15_000.0, sr) + 12.0).abs() < 0.8);
        assert!(response(&high, 100.0, sr).abs() < 0.5);
        let mid = BiquadCoeffs::peak(EQ_MID_HZ, 0.7, 6.0, sr);
        assert!((response(&mid, EQ_MID_HZ, sr) - 6.0).abs() < 0.3);
    }

    #[test]
    fn reverb_rings_after_an_impulse() {
        let mut r = Reverb::new(48_000.0);
        r.tick(1.0, 0.98, 0.2);
        let mut energy = 0.0;
        for i in 0..48_000 {
            let (l, rr) = r.tick(0.0, 0.98, 0.2);
            if i > 24_000 {
                energy += l * l + rr * rr;
            }
        }
        assert!(energy > 1e-4 && energy.is_finite(), "{energy}");
    }

    #[test]
    fn unity_line_read_matches_delay() {
        let mut line = Line::new(16);
        for i in 0..10 {
            line.push(i as f32);
        }
        assert_eq!(line.read(1.0), 9.0);
        assert_eq!(line.read(3.0), 7.0);
        assert!((line.read(2.5) - 7.5).abs() < 1e-6);
    }
}
