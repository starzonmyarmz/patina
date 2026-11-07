use crate::audition::phrase_for;
use crate::macros::MacroMatrix;
use crate::presets::{matrix_from, Session};
use nih_plug::prelude::*;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub enum Kind {
    /// `skew` is the exponent passed to `FloatRange::skew_factor`; 0 is linear.
    Float { min: f32, max: f32, skew: f32 },
    Int { min: i32, max: i32 },
    Enum(&'static [&'static str]),
    Bool,
}

pub struct Spec {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub kind: Kind,
    pub default: f32,
    pub unit: &'static str,
    pub jsfx: Option<u16>,
}

const fn lin(min: f32, max: f32) -> Kind {
    Kind::Float { min, max, skew: 0.0 }
}
const fn skew(min: f32, max: f32, skew: f32) -> Kind {
    Kind::Float { min, max, skew }
}
const fn int(min: i32, max: i32) -> Kind {
    Kind::Int { min, max }
}
const PCT: Kind = lin(0.0, 100.0);
const MS: Kind = skew(0.0, 10_000.0, -2.5);
const LFO_HZ: Kind = skew(0.03, 20.0, -2.0);

pub const OSC_TYPES: &[&str] = &["Sine", "Saw", "Square", "Triangle", "Wavetable", "PWM", "Supersaw", "Noise"];
pub const FILTER_TYPES: &[&str] = &["Lowpass", "Ladder", "Comb", "Formant", "Off"];
pub const LFO_SHAPES: &[&str] = &["Sine", "Square", "Triangle", "S&H"];
pub const DIVISIONS: &[&str] = &[
    "1/32T", "1/32", "1/16T", "1/16", "1/16D", "1/8T", "1/8", "1/8D", "1/4T", "1/4", "1/4D", "1/2",
    "1/2D", "1 bar", "2 bars", "4 bars",
];
pub const DIVISION_BEATS: &[f32] = &[
    1.0 / 12.0, 0.125, 1.0 / 6.0, 0.25, 0.375, 1.0 / 3.0, 0.5, 0.75, 2.0 / 3.0, 1.0, 1.5, 2.0, 3.0,
    4.0, 8.0, 16.0,
];

macro_rules! params {
    ($( $group:literal { $( $pid:ident = $id:literal, $name:literal, $kind:expr, $default:expr, $unit:literal, $jsfx:expr; )* } )*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(usize)]
        pub enum Pid { $( $( $pid, )* )* }

        impl Pid {
            pub const ALL: &'static [Pid] = &[$( $( Pid::$pid, )* )*];
        }

        pub const SPECS: &[Spec] = &[$( $( Spec {
            id: $id, name: $name, group: $group, kind: $kind,
            default: $default as f32, unit: $unit, jsfx: $jsfx,
        }, )* )*];
    };
}

params! {
    "Global" {
        Volume = "volume", "Volume", PCT, 80, " %", Some(11);
        Polyphony = "polyphony", "Polyphony", int(1, 16), 16, "", Some(12);
        Glide = "glide", "Glide", skew(0.0, 2000.0, -2.0), 0, " ms", Some(117);
    }
    "Macros" {
        Macro1 = "macro1", "Macro 1", PCT, 0, " %", None;
        Macro2 = "macro2", "Macro 2", PCT, 0, " %", None;
        Macro3 = "macro3", "Macro 3", PCT, 0, " %", None;
        Macro4 = "macro4", "Macro 4", PCT, 0, " %", None;
        Macro5 = "macro5", "Macro 5", PCT, 0, " %", None;
        Macro6 = "macro6", "Macro 6", PCT, 0, " %", None;
        Macro7 = "macro7", "Macro 7", PCT, 0, " %", None;
        Macro8 = "macro8", "Macro 8", PCT, 0, " %", None;
    }
    "Oscillators" {
        Osc1Type = "osc1_type", "Osc 1 type", Kind::Enum(OSC_TYPES), 1, "", Some(13);
        Osc1Semi = "osc1_semi", "Osc 1 semitone", int(-24, 24), 0, " st", Some(14);
        Osc1Detune = "osc1_detune", "Osc 1 detune", lin(-50.0, 50.0), 0, " ct", Some(15);
        Osc1Level = "osc1_level", "Osc 1 level", PCT, 80, " %", Some(16);
        Osc2Type = "osc2_type", "Osc 2 type", Kind::Enum(OSC_TYPES), 1, "", Some(17);
        Osc2Semi = "osc2_semi", "Osc 2 semitone", int(-24, 24), 7, " st", Some(18);
        Osc2Detune = "osc2_detune", "Osc 2 detune", lin(-50.0, 50.0), 0, " ct", Some(19);
        Osc2Level = "osc2_level", "Osc 2 level", PCT, 40, " %", Some(20);
        FmAmount = "fm_amount", "FM amount", PCT, 0, " %", Some(21);
        RingMod = "ring_mod", "Ring mod", Kind::Bool, 0, "", Some(22);
        Unison = "unison", "Unison voices", int(1, 8), 1, "", Some(23);
        UnisonDetune = "unison_detune", "Unison detune", lin(0.0, 120.0), 15, " ct", Some(24);
        Spread = "spread", "Stereo spread", PCT, 15, " %", Some(25);
        SubLevel = "sub_level", "Sub level", PCT, 0, " %", Some(26);
        NoiseLevel = "noise_level", "Noise level", PCT, 0, " %", Some(27);
        PinkLevel = "pink_level", "Pink noise level", PCT, 0, " %", Some(28);
        WavetablePos = "wt_pos", "Wavetable position", PCT, 50, " %", Some(59);
        PwmWidth = "pwm_width", "PWM width", PCT, 50, " %", Some(60);
        SupersawDetune = "supersaw_detune", "Supersaw detune", lin(0.0, 200.0), 40, " ct", Some(61);
    }
    "Filter" {
        FilterType = "flt_type", "Filter type", Kind::Enum(FILTER_TYPES), 0, "", Some(62);
        Cutoff = "flt_cutoff", "Cutoff", skew(20.0, 20_000.0, -2.5), 1262, " Hz", Some(63);
        Resonance = "flt_res", "Resonance", PCT, 20, " %", Some(64);
        FilterDrive = "flt_drive", "Filter drive", PCT, 0, " %", Some(65);
        FilterEnvAmount = "flt_env_amt", "Filter env amount", lin(-48.0, 48.0), 0, " st", Some(66);
        Vowel = "flt_vowel", "Formant vowel", PCT, 0, " %", Some(71);
        FilterAttack = "flt_attack", "Filter env attack", MS, 10, " ms", Some(67);
        FilterDecay = "flt_decay", "Filter env decay", MS, 300, " ms", Some(68);
        FilterSustain = "flt_sustain", "Filter env sustain", PCT, 50, " %", Some(69);
        FilterRelease = "flt_release", "Filter env release", MS, 300, " ms", Some(70);
    }
    "Amp" {
        AmpAttack = "amp_attack", "Amp attack", MS, 5, " ms", Some(72);
        AmpDecay = "amp_decay", "Amp decay", MS, 300, " ms", Some(73);
        AmpSustain = "amp_sustain", "Amp sustain", PCT, 80, " %", Some(74);
        AmpRelease = "amp_release", "Amp release", MS, 300, " ms", Some(75);
    }
    "Mod env" {
        ModAttack = "mod_attack", "Mod env attack", MS, 10, " ms", Some(76);
        ModDecay = "mod_decay", "Mod env decay", MS, 300, " ms", Some(77);
        ModSustain = "mod_sustain", "Mod env sustain", PCT, 50, " %", Some(78);
        ModRelease = "mod_release", "Mod env release", MS, 300, " ms", Some(79);
    }
    "Pitch env" {
        PitchEnv = "pitch_env", "Pitch env", Kind::Bool, 0, "", Some(80);
        PitchEnvAmount = "pitch_env_amt", "Pitch env amount", lin(-24.0, 24.0), 12, " st", Some(81);
        PitchEnvAttack = "pitch_env_attack", "Pitch env attack", skew(0.0, 1000.0, -2.0), 1, " ms", Some(82);
        PitchEnvDecay = "pitch_env_decay", "Pitch env decay", skew(0.0, 1000.0, -2.0), 60, " ms", Some(83);
    }
    "LFO 1" {
        Lfo1Rate = "lfo1_rate", "LFO 1 rate", LFO_HZ, 0.33, " Hz", Some(84);
        Lfo1Shape = "lfo1_shape", "LFO 1 shape", Kind::Enum(LFO_SHAPES), 0, "", Some(85);
        Lfo1Sync = "lfo1_sync", "LFO 1 sync", Kind::Bool, 0, "", None;
        Lfo1Division = "lfo1_division", "LFO 1 division", Kind::Enum(DIVISIONS), 9, "", None;
        Lfo1Pitch = "lfo1_pitch", "LFO 1 to pitch", PCT, 0, " %", Some(86);
        Lfo1Filter = "lfo1_filter", "LFO 1 to filter", PCT, 0, " %", Some(87);
        Lfo1Amp = "lfo1_amp", "LFO 1 to amp", PCT, 0, " %", Some(88);
    }
    "LFO 2" {
        Lfo2Rate = "lfo2_rate", "LFO 2 rate", LFO_HZ, 0.1, " Hz", Some(89);
        Lfo2Shape = "lfo2_shape", "LFO 2 shape", Kind::Enum(LFO_SHAPES), 2, "", Some(90);
        Lfo2Sync = "lfo2_sync", "LFO 2 sync", Kind::Bool, 0, "", None;
        Lfo2Division = "lfo2_division", "LFO 2 division", Kind::Enum(DIVISIONS), 11, "", None;
        Lfo2Pitch = "lfo2_pitch", "LFO 2 to pitch", PCT, 0, " %", Some(91);
        Lfo2Filter = "lfo2_filter", "LFO 2 to filter", PCT, 0, " %", Some(92);
    }
    "LFO 3" {
        Lfo3Rate = "lfo3_rate", "LFO 3 rate", LFO_HZ, 0.18, " Hz", Some(93);
        Lfo3Shape = "lfo3_shape", "LFO 3 shape", Kind::Enum(LFO_SHAPES), 0, "", Some(94);
        Lfo3Sync = "lfo3_sync", "LFO 3 sync", Kind::Bool, 0, "", None;
        Lfo3Division = "lfo3_division", "LFO 3 division", Kind::Enum(DIVISIONS), 13, "", None;
        Lfo3Pan = "lfo3_pan", "LFO 3 to pan", PCT, 0, " %", Some(95);
        Lfo3Amp = "lfo3_amp", "LFO 3 to amp", PCT, 0, " %", Some(96);
    }
    "Expression" {
        VelFilter = "vel_filter", "Velocity to filter", PCT, 30, " %", Some(97);
        VelAmp = "vel_amp", "Velocity to amp", PCT, 70, " %", Some(98);
        AtFilter = "at_filter", "Aftertouch to filter", PCT, 25, " %", Some(99);
        AtVibrato = "at_vibrato", "Aftertouch to vibrato", PCT, 15, " %", Some(100);
        KeyTrack = "keytrack", "Key tracking", PCT, 40, " %", Some(101);
    }
    "FX" {
        Distortion = "dist", "Distortion", PCT, 0, " %", Some(102);
        Bitcrush = "bitcrush", "Bitcrush", PCT, 0, " %", Some(103);
        ChorusMix = "chorus_mix", "Chorus mix", PCT, 0, " %", Some(104);
        ChorusRate = "chorus_rate", "Chorus rate", PCT, 25, " %", Some(105);
        DelayTime = "delay_time", "Delay time", skew(10.0, 2000.0, -1.0), 375, " ms", Some(106);
        DelaySync = "delay_sync", "Delay sync", Kind::Bool, 0, "", None;
        DelayDivision = "delay_division", "Delay division", Kind::Enum(DIVISIONS), 7, "", None;
        DelayFeedback = "delay_feedback", "Delay feedback", lin(0.0, 90.0), 30, " %", Some(107);
        DelayMix = "delay_mix", "Delay mix", PCT, 0, " %", Some(108);
        ReverbSize = "reverb_size", "Reverb size", PCT, 50, " %", Some(109);
        ReverbMix = "reverb_mix", "Reverb mix", PCT, 10, " %", Some(110);
        EqLow = "eq_low", "EQ low", lin(-12.0, 12.0), 0, " dB", Some(111);
        EqMid = "eq_mid", "EQ mid", lin(-12.0, 12.0), 0, " dB", Some(112);
        EqHigh = "eq_high", "EQ high", lin(-12.0, 12.0), 0, " dB", Some(113);
        Compressor = "compressor", "Compressor", PCT, 0, " %", Some(114);
        Width = "width", "Stereo width", lin(0.0, 200.0), 100, " %", Some(115);
        Limiter = "limiter", "Output limiter", Kind::Bool, 1, "", Some(116);
    }
}

pub const COUNT: usize = SPECS.len();

impl Pid {
    pub fn spec(self) -> &'static Spec {
        &SPECS[self as usize]
    }

    pub fn from_id(id: &str) -> Option<Pid> {
        SPECS.iter().position(|s| s.id == id).map(|i| Pid::ALL[i])
    }
}

impl Spec {
    fn bounds(&self) -> (f32, f32) {
        match self.kind {
            Kind::Float { min, max, .. } => (min, max),
            Kind::Int { min, max } => (min as f32, max as f32),
            Kind::Enum(names) => (0.0, names.len() as f32 - 1.0),
            Kind::Bool => (0.0, 1.0),
        }
    }

    /// Plain value to 0..1, matching nih-plug's own mapping.
    pub fn normalize(&self, plain: f32) -> f32 {
        let (min, max) = self.bounds();
        let linear = ((plain.clamp(min, max) - min) / (max - min)).clamp(0.0, 1.0);
        match self.kind {
            Kind::Float { skew, .. } if skew != 0.0 => linear.powf(FloatRange::skew_factor(skew)),
            _ => linear,
        }
    }

    /// 0..1 to a plain value; stepped kinds round to the nearest step.
    pub fn denormalize(&self, normalized: f32) -> f32 {
        let (min, max) = self.bounds();
        let n = normalized.clamp(0.0, 1.0);
        match self.kind {
            Kind::Float { skew, .. } if skew != 0.0 => n.powf(FloatRange::skew_factor(skew).recip()) * (max - min) + min,
            Kind::Float { .. } => n * (max - min) + min,
            _ => (n * (max - min) + min).round(),
        }
    }

    /// Number of steps for stepped kinds; None for continuous ones.
    pub fn steps(&self) -> Option<f32> {
        let (min, max) = self.bounds();
        (!matches!(self.kind, Kind::Float { .. })).then_some(max - min)
    }

    pub fn clamp(&self, plain: f32) -> f32 {
        self.denormalize(self.normalize(plain))
    }
}

pub enum Slot {
    Float(FloatParam),
    Int(IntParam),
    Bool(BoolParam),
}

pub struct ThousandParams {
    slots: Vec<Slot>,
    session: Mutex<Session>,
    matrix: Mutex<MacroMatrix>,
    generation: AtomicU32,
    phrase: AtomicUsize,
}

impl Default for ThousandParams {
    fn default() -> Self {
        Self {
            slots: SPECS.iter().map(build).collect(),
            session: Mutex::default(),
            matrix: Mutex::default(),
            generation: AtomicU32::new(0),
            phrase: AtomicUsize::new(0),
        }
    }
}

impl ThousandParams {
    pub fn session(&self) -> Session {
        self.session.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn set_session(&self, session: Session) {
        let matrix = matrix_from(&session.macros);
        self.phrase.store(phrase_for(&session.category), Ordering::Relaxed);
        if let Ok(mut s) = self.session.lock() {
            *s = session;
        }
        if let Ok(mut m) = self.matrix.lock() {
            *m = matrix;
        }
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Changes whenever the session is replaced.
    pub fn session_generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }

    /// The audition phrase for the session's category.
    pub fn phrase(&self) -> usize {
        self.phrase.load(Ordering::Relaxed)
    }

    pub fn ptr(&self, pid: Pid) -> ParamPtr {
        match &self.slots[pid as usize] {
            Slot::Float(p) => p.as_ptr(),
            Slot::Int(p) => p.as_ptr(),
            Slot::Bool(p) => p.as_ptr(),
        }
    }

    /// The value the user set, ignoring host modulation.
    pub fn normalized(&self, pid: Pid) -> f32 {
        match &self.slots[pid as usize] {
            Slot::Float(p) => p.unmodulated_normalized_value(),
            Slot::Int(p) => p.unmodulated_normalized_value(),
            Slot::Bool(p) => p.unmodulated_normalized_value(),
        }
    }

    pub fn display(&self, pid: Pid, normalized: f32) -> String {
        match &self.slots[pid as usize] {
            Slot::Float(p) => p.normalized_value_to_string(normalized, true),
            Slot::Int(p) => p.normalized_value_to_string(normalized, true),
            Slot::Bool(p) => p.normalized_value_to_string(normalized, true),
        }
    }

    /// For the audio thread: the macro matrix if it changed since `seen`.
    /// Never blocks; a busy lock just means trying again next block.
    pub fn matrix_if_changed(&self, seen: &mut u32) -> Option<MacroMatrix> {
        let generation = self.generation.load(Ordering::Acquire);
        if generation == *seen {
            return None;
        }
        let matrix = *self.matrix.try_lock().ok()?;
        *seen = generation;
        Some(matrix)
    }
}

fn build(spec: &Spec) -> Slot {
    match spec.kind {
        Kind::Float { min, max, skew } => {
            let range = if skew == 0.0 {
                FloatRange::Linear { min, max }
            } else {
                FloatRange::Skewed { min, max, factor: FloatRange::skew_factor(skew) }
            };
            let param = FloatParam::new(spec.name, spec.default, range).with_unit(spec.unit);
            let param = match spec.unit {
                " Hz" => param
                    .with_unit("")
                    .with_value_to_string(formatters::v2s_f32_hz_then_khz(2))
                    .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
                " ms" => param.with_value_to_string(formatters::v2s_f32_rounded(0)),
                _ => param.with_value_to_string(formatters::v2s_f32_rounded(1)),
            };
            Slot::Float(param)
        }
        Kind::Int { min, max } => {
            Slot::Int(IntParam::new(spec.name, spec.default as i32, IntRange::Linear { min, max }).with_unit(spec.unit))
        }
        Kind::Enum(names) => Slot::Int(
            IntParam::new(spec.name, spec.default as i32, IntRange::Linear { min: 0, max: names.len() as i32 - 1 })
                .with_value_to_string(Arc::new(move |v| names[v as usize].to_string()))
                .with_string_to_value(Arc::new(move |s| {
                    names.iter().position(|n| n.eq_ignore_ascii_case(s.trim())).map(|i| i as i32)
                })),
        ),
        Kind::Bool => Slot::Bool(BoolParam::new(spec.name, spec.default != 0.0)),
    }
}

/// Plain (unnormalized) parameter values, from the host or from a preset.
pub trait Values {
    fn get(&self, pid: Pid) -> f32;

    #[inline]
    fn index(&self, pid: Pid) -> usize {
        self.get(pid) as usize
    }

    #[inline]
    fn on(&self, pid: Pid) -> bool {
        self.get(pid) >= 0.5
    }
}

impl Values for ThousandParams {
    #[inline]
    fn get(&self, pid: Pid) -> f32 {
        match &self.slots[pid as usize] {
            Slot::Float(p) => p.value(),
            Slot::Int(p) => p.value() as f32,
            Slot::Bool(p) => p.value() as u8 as f32,
        }
    }
}

#[derive(Clone)]
pub struct PlainValues(pub [f32; COUNT]);

impl Default for PlainValues {
    fn default() -> Self {
        Self(std::array::from_fn(|i| SPECS[i].default))
    }
}

impl PlainValues {
    pub fn set(&mut self, pid: Pid, value: f32) -> &mut Self {
        self.0[pid as usize] = value;
        self
    }
}

impl Values for PlainValues {
    #[inline]
    fn get(&self, pid: Pid) -> f32 {
        self.0[pid as usize]
    }
}

unsafe impl Params for ThousandParams {
    fn param_map(&self) -> Vec<(String, ParamPtr, String)> {
        SPECS
            .iter()
            .zip(&self.slots)
            .map(|(spec, slot)| {
                let ptr = match slot {
                    Slot::Float(p) => p.as_ptr(),
                    Slot::Int(p) => p.as_ptr(),
                    Slot::Bool(p) => p.as_ptr(),
                };
                (spec.id.to_string(), ptr, spec.group.to_string())
            })
            .collect()
    }

    fn serialize_fields(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        if let Ok(json) = serde_json::to_string(&self.session()) {
            fields.insert("session".to_string(), json);
        }
        fields
    }

    fn deserialize_fields(&self, serialized: &BTreeMap<String, String>) {
        let session = serialized.get("session").and_then(|json| serde_json::from_str(json).ok()).unwrap_or_default();
        self.set_session(session);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = SPECS.iter().map(|s| s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), COUNT);
    }

    #[test]
    fn defaults_are_in_range() {
        for s in SPECS {
            let (lo, hi) = s.bounds();
            assert!((lo..=hi).contains(&s.default), "{} default {} outside {lo}..{hi}", s.id, s.default);
        }
    }

    #[test]
    fn normalization_matches_nih_plug() {
        let params = ThousandParams::default();
        for (spec, slot) in SPECS.iter().zip(&params.slots) {
            for step in 0..=20 {
                let n = step as f32 / 20.0;
                let plain = spec.denormalize(n);
                let theirs = match slot {
                    Slot::Float(p) => p.preview_normalized(plain),
                    Slot::Int(p) => p.preview_normalized(plain as i32),
                    Slot::Bool(p) => p.preview_normalized(plain >= 0.5),
                };
                assert!((spec.normalize(plain) - theirs).abs() < 1e-4, "{} at {n}", spec.id);
            }
        }
    }

    #[test]
    fn ids_round_trip() {
        for &pid in Pid::ALL {
            assert_eq!(Pid::from_id(pid.spec().id), Some(pid));
        }
    }

    #[test]
    fn session_survives_state_and_reaches_the_audio_thread() {
        use crate::presets::{MacroDef, MacroTarget, Source};
        let a = ThousandParams::default();
        let mut seen = u32::MAX;
        assert_eq!(a.matrix_if_changed(&mut seen), Some(MacroMatrix::default()));
        assert_eq!(a.matrix_if_changed(&mut seen), None);

        let session = Session {
            name: "Bloom Pad".into(),
            category: "Pad".into(),
            source: Source::User,
            macros: vec![MacroDef { name: "Open".into(), targets: vec![MacroTarget { param: "flt_cutoff".into(), amount: 0.4 }] }],
            saved: None,
        };
        let b = ThousandParams::default();
        a.set_session(session.clone());
        b.deserialize_fields(&a.serialize_fields());
        assert_eq!(b.session(), session);
        let mut seen_b = 0;
        let m = b.matrix_if_changed(&mut seen_b).unwrap();
        assert_eq!(m.0[0][0].unwrap().pid, Pid::Cutoff);
    }

    #[test]
    fn divisions_match_beats() {
        assert_eq!(DIVISIONS.len(), DIVISION_BEATS.len());
    }
}
