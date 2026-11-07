use super::env::{Env, Stage};
use super::filter::{fast_tanh, VoiceFilter};
use super::osc::{self, Rng, Tables};
use super::{Mod, Patch, MAX_UNISON};
use std::f32::consts::{FRAC_PI_4, SQRT_2, TAU};

const LN_1000: f32 = 6.907_755;

pub struct Voice {
    pub active: bool,
    pub note: u8,
    pub gate: bool,
    pub held_by_pedal: bool,
    pub started: u64,
    vel: f32,
    freq: f32,
    glide_freq: f32,
    drift: f32,
    pan: f32,
    amp: Env,
    flt: Env,
    modenv: Env,
    pitch: Env,
    ph1: [f32; MAX_UNISON],
    ph2: [f32; MAX_UNISON],
    lvl1: [usize; MAX_UNISON],
    lvl2: [usize; MAX_UNISON],
    sub_phase: f32,
    pink: [f32; 3],
    fc: f32,
    filter: VoiceFilter,
    rng: Rng,
}

impl Voice {
    pub fn new(seed: u32) -> Self {
        Self {
            active: false,
            note: 0,
            gate: false,
            held_by_pedal: false,
            started: 0,
            vel: 0.0,
            freq: 440.0,
            glide_freq: 440.0,
            drift: 1.0,
            pan: 0.0,
            amp: Env::default(),
            flt: Env::default(),
            modenv: Env::default(),
            pitch: Env::default(),
            ph1: [0.0; MAX_UNISON],
            ph2: [0.0; MAX_UNISON],
            lvl1: [0; MAX_UNISON],
            lvl2: [0; MAX_UNISON],
            sub_phase: 0.0,
            pink: [0.0; 3],
            fc: 0.0,
            filter: VoiceFilter::new(),
            rng: Rng::new(seed),
        }
    }

    pub fn releasing(&self) -> bool {
        self.active && !self.gate && !self.held_by_pedal
    }

    pub fn start(&mut self, note: u8, vel: f32, glide_from: Option<f32>, spread: f32, stamp: u64) {
        self.freq = 440.0 * 2f32.powf((note as f32 - 69.0) / 12.0);
        if !self.active {
            for u in 0..MAX_UNISON {
                self.ph1[u] = self.rng.unit();
                self.ph2[u] = self.rng.unit();
            }
            self.sub_phase = 0.0;
            self.fc = 0.0;
            self.pink = [0.0; 3];
            self.filter.reset();
            self.glide_freq = glide_from.unwrap_or(self.freq);
        }
        self.active = true;
        self.note = note;
        self.vel = vel;
        self.gate = true;
        self.held_by_pedal = false;
        self.started = stamp;
        self.drift = 2f32.powf(self.rng.bipolar() * 3.0 / 1200.0);
        self.pan = self.rng.bipolar() * spread;
        self.amp.trigger();
        self.flt.trigger();
        self.modenv.trigger();
        self.pitch.level = 0.0;
        self.pitch.trigger();
    }

    pub fn release(&mut self) {
        self.gate = false;
        self.held_by_pedal = false;
        self.amp.release();
        self.flt.release();
        self.modenv.release();
        self.pitch.release();
    }

    pub fn kill(&mut self) {
        self.active = false;
        self.gate = false;
        self.held_by_pedal = false;
        self.amp.kill();
        self.flt.kill();
        self.modenv.kill();
        self.pitch.kill();
    }

    /// Picks mip levels once per chunk; FM wobble within a chunk is ignored.
    pub fn prepare(&mut self, p: &Patch, pitch_mul: f32) {
        let base = self.glide_freq * pitch_mul * self.drift;
        for u in 0..p.n1 {
            self.lvl1[u] = Tables::level_for(base * p.o1_ratio * p.r1[u]);
        }
        for u in 0..p.n2 {
            self.lvl2[u] = Tables::level_for(base * p.o2_ratio * p.r2[u]);
        }
    }

    #[inline]
    fn wave(&mut self, t: &Tables, kind: usize, level: usize, phase: f32, p: &Patch) -> f32 {
        match kind {
            osc::SINE => t.read(osc::SINE, 0, phase),
            osc::SQUARE => t.read(osc::SQUARE, level, phase),
            osc::TRIANGLE => t.read(osc::TRIANGLE, level, phase),
            osc::WAVETABLE => t.wavetable(level, phase, p.wt_pos),
            osc::PWM => t.pulse(level, phase, p.pwm),
            osc::NOISE => self.rng.bipolar(),
            _ => t.read(osc::SAW, level, phase),
        }
    }

    #[inline]
    pub fn render(&mut self, t: &Tables, p: &Patch, m: &Mod, sr: f32, inv_sr: f32) -> (f32, f32) {
        let amp = self.amp.tick(&p.amp);
        if self.amp.stage == Stage::Idle {
            self.kill();
            return (0.0, 0.0);
        }
        let fe = self.flt.tick(&p.flt);
        let me = self.modenv.tick(&p.modenv);
        let pe = if p.pitch_on { self.pitch.tick(&p.pitch) } else { 0.0 };

        let nf = if p.glide > 0.0 {
            self.glide_freq += (self.freq - self.glide_freq) * p.glide;
            self.glide_freq
        } else {
            self.glide_freq = self.freq;
            self.freq
        } * m.pitch_mul
            * self.drift;

        let f2 = nf * p.o2_ratio;
        let mut o2 = 0.0;
        for u in 0..p.n2 {
            let dt = f2 * p.r2[u] * inv_sr;
            self.ph2[u] = (self.ph2[u] + dt).fract();
            o2 += self.wave(t, p.osc2, self.lvl2[u], self.ph2[u], p);
        }
        o2 *= p.g2;

        let bend = p.fm * me * o2 + pe * p.pitch_amt;
        let f1 = nf * p.o1_ratio * if bend != 0.0 { (bend / 12.0).exp2() } else { 1.0 };
        let mut o1 = 0.0;
        for u in 0..p.n1 {
            let dt = f1 * p.r1[u] * inv_sr;
            self.ph1[u] = (self.ph1[u] + dt).rem_euclid(1.0);
            o1 += self.wave(t, p.osc1, self.lvl1[u], self.ph1[u], p);
        }
        o1 *= p.g1;
        if p.ring {
            o1 *= o2;
        }

        self.sub_phase = (self.sub_phase + nf * 0.5 * inv_sr).fract();
        let sub = (TAU * self.sub_phase).sin();

        let w = self.rng.bipolar();
        self.pink[0] = 0.99765 * self.pink[0] + w * 0.099_046;
        self.pink[1] = 0.963 * self.pink[1] + w * 0.296_516_4;
        self.pink[2] = 0.57 * self.pink[2] + w * 1.052_691_3;
        let pink = (self.pink[0] + self.pink[1] + self.pink[2] + w * 0.1848) * 0.2;

        let mix = o1 * p.o1_lvl + o2 * p.o2_lvl + sub * p.sub + w * p.noise * 0.5 + pink * p.pink;

        let cn = (p.cut
            + m.lfo[0] * p.lfo1_filter
            + m.lfo[1] * p.lfo2_filter
            + self.vel * p.vel_filter
            + m.aftertouch * p.at_filter
            + (self.note as f32 - 60.0) / 60.0 * p.keytrack)
            .clamp(0.0, 1.0);
        let target = (20.0 * (cn * LN_1000).exp() * (fe * p.fenv / 12.0).exp2()).clamp(20.0, sr * 0.45);
        self.fc = if self.fc == 0.0 { target } else { self.fc + (target - self.fc) * p.fc_smooth };

        let x = if p.drive > 0.0 { fast_tanh(mix * (1.0 + p.drive * 3.0)) } else { mix };
        let out = self.filter.tick(p.ftype, x, self.fc, cn, p.res, p.vowel, sr, inv_sr);

        let mut sig = out * amp * (1.0 - p.vel_amp + p.vel_amp * self.vel);
        sig *= (1.0 - p.lfo1_amp * (m.lfo[0] * 0.5 + 0.5) - p.lfo3_amp * (m.lfo[2] * 0.5 + 0.5)).max(0.0);
        let pan = (self.pan + m.lfo[2] * p.lfo3_pan).clamp(-1.0, 1.0);
        let angle = (pan + 1.0) * FRAC_PI_4;
        (sig * angle.cos() * SQRT_2, sig * angle.sin() * SQRT_2)
    }
}
