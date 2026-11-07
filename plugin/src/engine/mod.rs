pub mod env;
pub mod filter;
pub mod lfo;
pub mod osc;
mod voice;

use crate::params::{Pid, Values, DIVISION_BEATS};
use env::EnvCoeffs;
use filter::fast_tanh;
use lfo::Lfo;
use osc::Tables;
use std::f32::consts::TAU;
use voice::Voice;

pub const MAX_VOICES: usize = 16;
pub const MAX_UNISON: usize = 8;
pub const CHUNK: usize = 32;
const SUPERSAW_COPIES: usize = 7;
const MOD_WHEEL_BRIGHTNESS: f32 = 0.24;
const BEND_RANGE: f32 = 2.0;

/// Parameter values turned into DSP-ready numbers, refreshed every chunk.
#[derive(Default)]
pub struct Patch {
    pub poly: usize,
    pub osc1: usize,
    pub osc2: usize,
    pub o1_ratio: f32,
    pub o2_ratio: f32,
    pub o1_lvl: f32,
    pub o2_lvl: f32,
    pub fm: f32,
    pub ring: bool,
    pub n1: usize,
    pub n2: usize,
    pub r1: [f32; MAX_UNISON],
    pub r2: [f32; MAX_UNISON],
    pub g1: f32,
    pub g2: f32,
    pub sub: f32,
    pub noise: f32,
    pub pink: f32,
    pub wt_pos: f32,
    pub pwm: f32,
    pub spread: f32,
    pub glide: f32,
    pub ftype: usize,
    pub cut: f32,
    pub res: f32,
    pub drive: f32,
    pub fenv: f32,
    pub vowel: f32,
    pub fc_smooth: f32,
    pub amp: EnvCoeffs,
    pub flt: EnvCoeffs,
    pub modenv: EnvCoeffs,
    pub pitch: EnvCoeffs,
    pub pitch_on: bool,
    pub pitch_amt: f32,
    pub lfo_shape: [usize; 3],
    pub lfo_hz: [f32; 3],
    pub lfo_sync: [Option<f32>; 3],
    pub lfo1_pitch: f32,
    pub lfo1_filter: f32,
    pub lfo1_amp: f32,
    pub lfo2_pitch: f32,
    pub lfo2_filter: f32,
    pub lfo3_pan: f32,
    pub lfo3_amp: f32,
    pub vel_filter: f32,
    pub vel_amp: f32,
    pub at_filter: f32,
    pub at_vibrato: f32,
    pub keytrack: f32,
    pub volume: f32,
    pub limiter: bool,
}

fn unison_ratios(n: usize, cents: f32) -> [f32; MAX_UNISON] {
    let mut r = [1.0; MAX_UNISON];
    if n > 1 {
        for (u, r) in r.iter_mut().enumerate().take(n) {
            *r = ((u as f32 / (n - 1) as f32 - 0.5) * cents / 1200.0).exp2();
        }
    }
    r
}

impl Patch {
    pub fn read(p: &impl Values, sr: f32, mod_wheel: f32) -> Self {
        let pct = |id| p.get(id) / 100.0;
        let semis = |semi: Pid, cents: Pid| ((p.get(semi) + p.get(cents) / 100.0) / 12.0).exp2();
        let env = |a, d, s, r| EnvCoeffs::new(p.get(a), p.get(d), pct(s), p.get(r), sr);

        let osc1 = p.index(Pid::Osc1Type);
        let unison = p.index(Pid::Unison).clamp(1, MAX_UNISON);
        let supersaw = osc1 == osc::SUPERSAW;
        let n1 = if supersaw && unison < 2 { SUPERSAW_COPIES } else { unison };
        let spread1 = if supersaw { p.get(Pid::SupersawDetune) } else { p.get(Pid::UnisonDetune) };
        let glide_ms = p.get(Pid::Glide);
        let sync = |on: Pid, div: Pid| p.on(on).then(|| DIVISION_BEATS[p.index(div)]);

        Self {
            poly: p.index(Pid::Polyphony).clamp(1, MAX_VOICES),
            osc1,
            osc2: p.index(Pid::Osc2Type),
            o1_ratio: semis(Pid::Osc1Semi, Pid::Osc1Detune),
            o2_ratio: semis(Pid::Osc2Semi, Pid::Osc2Detune),
            o1_lvl: pct(Pid::Osc1Level),
            o2_lvl: pct(Pid::Osc2Level),
            fm: pct(Pid::FmAmount) * 12.0,
            ring: p.on(Pid::RingMod),
            n1,
            n2: unison,
            r1: unison_ratios(n1, spread1),
            r2: unison_ratios(unison, p.get(Pid::UnisonDetune)),
            g1: 1.0 / (n1 as f32).sqrt(),
            g2: 1.0 / (unison as f32).sqrt(),
            sub: pct(Pid::SubLevel),
            noise: pct(Pid::NoiseLevel),
            pink: pct(Pid::PinkLevel),
            wt_pos: pct(Pid::WavetablePos),
            pwm: 0.05 + pct(Pid::PwmWidth) * 0.9,
            spread: pct(Pid::Spread),
            glide: if glide_ms > 0.0 { 1.0 - (-1.0 / (glide_ms * 0.001 * sr).max(1.0)).exp() } else { 0.0 },
            ftype: p.index(Pid::FilterType),
            cut: ((p.get(Pid::Cutoff) / 20.0).ln() / 1000f32.ln() + mod_wheel * MOD_WHEEL_BRIGHTNESS).clamp(0.0, 1.0),
            res: pct(Pid::Resonance),
            drive: pct(Pid::FilterDrive) * 2.0,
            fenv: p.get(Pid::FilterEnvAmount),
            vowel: pct(Pid::Vowel),
            fc_smooth: 1.0 - (-1.0 / (0.002 * sr)).exp(),
            amp: env(Pid::AmpAttack, Pid::AmpDecay, Pid::AmpSustain, Pid::AmpRelease),
            flt: env(Pid::FilterAttack, Pid::FilterDecay, Pid::FilterSustain, Pid::FilterRelease),
            modenv: env(Pid::ModAttack, Pid::ModDecay, Pid::ModSustain, Pid::ModRelease),
            pitch: EnvCoeffs::new(p.get(Pid::PitchEnvAttack), p.get(Pid::PitchEnvDecay), 0.0, p.get(Pid::AmpRelease), sr),
            pitch_on: p.on(Pid::PitchEnv),
            pitch_amt: p.get(Pid::PitchEnvAmount),
            lfo_shape: [p.index(Pid::Lfo1Shape), p.index(Pid::Lfo2Shape), p.index(Pid::Lfo3Shape)],
            lfo_hz: [p.get(Pid::Lfo1Rate), p.get(Pid::Lfo2Rate), p.get(Pid::Lfo3Rate)],
            lfo_sync: [
                sync(Pid::Lfo1Sync, Pid::Lfo1Division),
                sync(Pid::Lfo2Sync, Pid::Lfo2Division),
                sync(Pid::Lfo3Sync, Pid::Lfo3Division),
            ],
            lfo1_pitch: pct(Pid::Lfo1Pitch),
            lfo1_filter: pct(Pid::Lfo1Filter) * 0.5,
            lfo1_amp: pct(Pid::Lfo1Amp),
            lfo2_pitch: pct(Pid::Lfo2Pitch),
            lfo2_filter: pct(Pid::Lfo2Filter) * 0.5,
            lfo3_pan: pct(Pid::Lfo3Pan),
            lfo3_amp: pct(Pid::Lfo3Amp),
            vel_filter: pct(Pid::VelFilter) * 0.25,
            vel_amp: pct(Pid::VelAmp),
            at_filter: pct(Pid::AtFilter) * 0.2,
            at_vibrato: pct(Pid::AtVibrato),
            keytrack: pct(Pid::KeyTrack) * 0.3,
            volume: pct(Pid::Volume),
            limiter: p.on(Pid::Limiter),
        }
    }
}

/// Per-sample modulation shared by all voices.
pub struct Mod {
    pub lfo: [f32; 3],
    pub pitch_mul: f32,
    pub aftertouch: f32,
}

#[derive(Clone, Copy, Default)]
pub struct Transport {
    pub playing: bool,
    pub tempo: Option<f64>,
    pub pos_beats: Option<f64>,
}

pub struct Engine {
    sr: f32,
    inv_sr: f32,
    tables: Tables,
    voices: Vec<Voice>,
    patch: Patch,
    lfos: [Lfo; 3],
    lfo_inc: [f32; 3],
    vibrato_phase: f32,
    bend: f32,
    aftertouch: f32,
    mod_wheel: f32,
    sustain: bool,
    last_freq: Option<f32>,
    stamp: u64,
    volume: f32,
    dc: [(f32, f32); 2],
}

impl Engine {
    pub fn new(sr: f32) -> Self {
        Self {
            sr,
            inv_sr: 1.0 / sr,
            tables: Tables::new(sr),
            voices: (0..MAX_VOICES).map(|i| Voice::new(0x9E37_79B9 ^ (i as u32 + 1).wrapping_mul(0x85EB_CA6B))).collect(),
            patch: Patch::default(),
            lfos: [Lfo::new(11), Lfo::new(23), Lfo::new(37)],
            lfo_inc: [0.0; 3],
            vibrato_phase: 0.0,
            bend: 0.0,
            aftertouch: 0.0,
            mod_wheel: 0.0,
            sustain: false,
            last_freq: None,
            stamp: 0,
            volume: 0.0,
            dc: [(0.0, 0.0); 2],
        }
    }

    pub fn reset(&mut self) {
        self.voices.iter_mut().for_each(Voice::kill);
        self.lfos.iter_mut().for_each(Lfo::reset);
        self.dc = [(0.0, 0.0); 2];
        self.sustain = false;
        self.bend = 0.0;
        self.aftertouch = 0.0;
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    pub fn update(&mut self, params: &impl Values, transport: Transport) {
        self.patch = Patch::read(params, self.sr, self.mod_wheel);
        let poly = self.patch.poly;
        self.voices[poly..].iter_mut().filter(|v| v.active).for_each(Voice::kill);
        let tempo = transport.tempo.unwrap_or(120.0) as f32;
        for i in 0..3 {
            let hz = match self.patch.lfo_sync[i] {
                Some(beats) => tempo / 60.0 / beats,
                None => self.patch.lfo_hz[i],
            };
            self.lfo_inc[i] = hz * self.inv_sr;
        }
    }

    /// Call once per host block, after `update`, to phase-lock synced LFOs.
    pub fn follow_transport(&mut self, transport: Transport) {
        if let (true, Some(pos)) = (transport.playing, transport.pos_beats) {
            for i in 0..3 {
                if let Some(beats) = self.patch.lfo_sync[i] {
                    self.lfos[i].set_phase((pos / beats as f64).fract() as f32);
                }
            }
        }
    }

    pub fn note_on(&mut self, note: u8, velocity: f32) {
        let poly = self.patch.poly;
        let pool = &self.voices[..poly];
        let pick = pool
            .iter()
            .position(|v| v.active && v.note == note)
            .or_else(|| pool.iter().position(|v| !v.active))
            .or_else(|| oldest(pool, Voice::releasing))
            .or_else(|| oldest(pool, |_| true))
            .unwrap_or(0);
        self.stamp += 1;
        let voice = &mut self.voices[pick];
        voice.start(note, velocity, self.last_freq, self.patch.spread, self.stamp);
        self.last_freq = Some(440.0 * 2f32.powf((note as f32 - 69.0) / 12.0));
    }

    pub fn note_off(&mut self, note: u8) {
        let sustain = self.sustain;
        for v in self.voices.iter_mut().filter(|v| v.active && v.gate && v.note == note) {
            if sustain {
                v.held_by_pedal = true;
                v.gate = false;
            } else {
                v.release();
            }
        }
    }

    pub fn set_sustain(&mut self, down: bool) {
        self.sustain = down;
        if !down {
            self.voices.iter_mut().filter(|v| v.held_by_pedal).for_each(Voice::release);
        }
    }

    pub fn all_notes_off(&mut self) {
        self.voices.iter_mut().filter(|v| v.active).for_each(Voice::release);
    }

    pub fn all_sound_off(&mut self) {
        self.voices.iter_mut().for_each(Voice::kill);
    }

    /// `value` is 0..1 with 0.5 at rest.
    pub fn set_pitch_bend(&mut self, value: f32) {
        self.bend = (value * 2.0 - 1.0) * BEND_RANGE;
    }

    pub fn set_aftertouch(&mut self, value: f32) {
        self.aftertouch = value;
    }

    pub fn set_mod_wheel(&mut self, value: f32) {
        self.mod_wheel = value;
    }

    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let p = &self.patch;
        let at_vib = p.at_vibrato;
        let bend_mul = (self.bend / 12.0).exp2();
        for v in self.voices[..p.poly].iter_mut().filter(|v| v.active) {
            v.prepare(p, bend_mul);
        }
        let vib_inc = 5.5 * self.inv_sr;
        let vol_step = 1.0 - (-1.0 / (0.01 * self.sr)).exp();

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let lfo = [
                self.lfos[0].tick(p.lfo_shape[0], self.lfo_inc[0]),
                self.lfos[1].tick(p.lfo_shape[1], self.lfo_inc[1]),
                self.lfos[2].tick(p.lfo_shape[2], self.lfo_inc[2]),
            ];
            self.vibrato_phase = (self.vibrato_phase + vib_inc).fract();
            let vibrato = (TAU * self.vibrato_phase).sin() * self.aftertouch * at_vib;
            let m = Mod {
                pitch_mul: ((self.bend + lfo[0] * p.lfo1_pitch + lfo[1] * p.lfo2_pitch + vibrato) / 12.0).exp2(),
                lfo,
                aftertouch: self.aftertouch,
            };

            let (mut sl, mut sr) = (0.0, 0.0);
            for v in self.voices[..p.poly].iter_mut().filter(|v| v.active) {
                let (vl, vr) = v.render(&self.tables, p, &m, self.sr, self.inv_sr);
                sl += vl;
                sr += vr;
            }
            let (mut xl, mut xr) = (sl * 0.35, sr * 0.35);

            if p.limiter {
                xl = fast_tanh(xl);
                xr = fast_tanh(xr);
            }
            self.volume += (p.volume - self.volume) * vol_step;
            xl *= self.volume;
            xr *= self.volume;
            *l = dc_block(&mut self.dc[0], xl);
            *r = dc_block(&mut self.dc[1], xr);
        }
    }
}

fn oldest(pool: &[Voice], filter: impl Fn(&Voice) -> bool) -> Option<usize> {
    pool.iter().enumerate().filter(|(_, v)| filter(v)).min_by_key(|(_, v)| v.started).map(|(i, _)| i)
}

#[inline]
fn dc_block(state: &mut (f32, f32), x: f32) -> f32 {
    let y = x - state.0 + 0.995 * state.1;
    *state = (x, y);
    y
}
