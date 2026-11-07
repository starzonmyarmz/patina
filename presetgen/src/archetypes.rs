use patina::params::Pid::{self, *};

pub enum Vary {
    /// Uniform in normalized space between two plain values.
    Range(f32, f32),
    Pick(&'static [f32]),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Bass,
    Lead,
    Pad,
    Pluck,
    Keys,
    Bell,
    Brass,
    Texture,
}

pub struct Archetype {
    pub name: &'static str,
    pub category: Category,
    pub set: &'static [(Pid, f32)],
    pub vary: &'static [(Pid, Vary)],
}

macro_rules! set {
    ($($p:ident = $v:expr),* $(,)?) => { &[$(($p, $v as f32)),*] };
}

macro_rules! vary {
    ($($p:ident : $spec:tt),* $(,)?) => { &[$(($p, vary!(@one $spec))),*] };
    (@one ($lo:expr, $hi:expr)) => { Vary::Range($lo as f32, $hi as f32) };
    (@one [$($c:expr),*]) => { Vary::Pick(&[$($c as f32),*]) };
}

/// Applied before every archetype so each one starts from silence-safe,
/// effect-free settings rather than the plugin's playable default.
pub const NEUTRAL: &[(Pid, f32)] = set![Osc2Level = 0, ReverbMix = 0, VelFilter = 20];

// Indices: osc Sine 0, Saw 1, Square 2, Triangle 3, Wavetable 4, PWM 5,
// Supersaw 6, Noise 7. Filter Lowpass 0, Ladder 1, Comb 2, Formant 3, Off 4.
// LFO Sine 0, Square 1, Triangle 2, S&H 3. Divisions 1/16 3, 1/8T 5, 1/8 6,
// 1/8D 7, 1/4T 8, 1/4 9, 1/2 11.
pub const ARCHETYPES: &[Archetype] = &[
    Archetype {
        name: "Moog Bass",
        category: Category::Bass,
        set: set![
            Osc1Type = 1, Osc2Type = 2, Osc2Semi = -12, Osc2Level = 60, FilterType = 1, Cutoff = 220,
            Resonance = 35, FilterEnvAmount = 30, FilterAttack = 1, FilterDecay = 250, FilterSustain = 10,
            FilterRelease = 150, AmpAttack = 2, AmpDecay = 400, AmpSustain = 90, AmpRelease = 120, SubLevel = 30,
            KeyTrack = 50, Polyphony = 1,
        ],
        vary: vary![
            Cutoff: (120, 600), Resonance: (10, 70), FilterEnvAmount: (12, 40), FilterDecay: (120, 600),
            Distortion: (0, 30), Glide: (0, 80),
        ],
    },
    Archetype {
        name: "Reese",
        category: Category::Bass,
        set: set![
            Osc1Type = 1, Osc2Type = 1, Osc2Detune = 18, Osc2Level = 80, Unison = 2, UnisonDetune = 25,
            Cutoff = 500, Resonance = 20, Lfo1Rate = 0.2, Lfo1Filter = 15, Distortion = 20, SubLevel = 40,
            AmpAttack = 5, AmpSustain = 100, AmpRelease = 200, Polyphony = 1,
        ],
        vary: vary![
            Osc2Detune: (8, 40), UnisonDetune: (10, 50), Cutoff: (250, 1200), Distortion: (0, 50),
            Lfo1Filter: (0, 40), Lfo1Rate: (0.05, 1.0),
        ],
    },
    Archetype {
        name: "Acid",
        category: Category::Bass,
        set: set![
            Osc1Type = 1, FilterType = 1, Cutoff = 300, Resonance = 80, FilterEnvAmount = 36, FilterAttack = 1,
            FilterDecay = 180, FilterSustain = 0, AmpAttack = 1, AmpSustain = 100, AmpRelease = 60, Glide = 60,
            Distortion = 25, Polyphony = 1, DelaySync = 1, DelayDivision = 7, DelayMix = 15, DelayFeedback = 30,
        ],
        vary: vary![
            Osc1Type: [1, 2], Resonance: (60, 92), FilterEnvAmount: (24, 48), FilterDecay: (100, 400),
            Cutoff: (150, 600), Distortion: (0, 60),
        ],
    },
    Archetype {
        name: "808",
        category: Category::Bass,
        set: set![
            Osc1Type = 0, FilterType = 4, PitchEnv = 1, PitchEnvAmount = 12, PitchEnvAttack = 0,
            PitchEnvDecay = 40, AmpAttack = 0, AmpDecay = 1200, AmpSustain = 0, AmpRelease = 300, Distortion = 15,
            Polyphony = 1, VelAmp = 40,
        ],
        vary: vary![
            AmpDecay: (500, 2500), PitchEnvAmount: (5, 24), PitchEnvDecay: (15, 120), Distortion: (0, 50),
        ],
    },
    Archetype {
        name: "Rubber Bass",
        category: Category::Bass,
        set: set![
            Osc1Type = 2, Cutoff = 180, Resonance = 30, FilterEnvAmount = 36, FilterDecay = 150, FilterSustain = 0,
            AmpAttack = 1, AmpDecay = 500, AmpSustain = 30, AmpRelease = 100, SubLevel = 30, KeyTrack = 50,
        ],
        vary: vary![
            Osc1Type: [2, 5], PwmWidth: (20, 50), Cutoff: (100, 400), FilterDecay: (80, 300),
            Resonance: (10, 60), SubLevel: (0, 60),
        ],
    },
    Archetype {
        name: "FM Bass",
        category: Category::Bass,
        set: set![
            Osc1Type = 0, Osc2Type = 0, Osc2Semi = 0, FmAmount = 45, ModAttack = 0, ModDecay = 300, ModSustain = 10,
            Cutoff = 2000, AmpAttack = 1, AmpDecay = 600, AmpSustain = 60, AmpRelease = 120, Polyphony = 1,
            SubLevel = 20,
        ],
        vary: vary![
            FmAmount: (20, 80), ModDecay: (100, 800), Osc2Semi: [0, 12, 19], Cutoff: (800, 6000),
            Distortion: (0, 30),
        ],
    },
    Archetype {
        name: "Wobble",
        category: Category::Bass,
        set: set![
            Osc1Type = 1, Osc2Type = 2, Osc2Semi = -12, Osc2Level = 60, Cutoff = 200, Resonance = 50,
            Lfo1Sync = 1, Lfo1Division = 6, Lfo1Filter = 70, Lfo1Shape = 0, Distortion = 35, SubLevel = 30,
            AmpAttack = 2, AmpSustain = 100, AmpRelease = 100, Polyphony = 1,
        ],
        vary: vary![
            Lfo1Division: [3, 5, 6, 8], Lfo1Shape: [0, 2, 3], Distortion: (10, 60), Resonance: (30, 75),
            Cutoff: (120, 400),
        ],
    },
    Archetype {
        name: "Saw Lead",
        category: Category::Lead,
        set: set![
            Osc1Type = 1, Osc2Type = 1, Osc2Semi = 12, Osc2Detune = 8, Osc2Level = 50, Unison = 3,
            UnisonDetune = 12, Cutoff = 3500, Resonance = 25, FilterEnvAmount = 12, FilterDecay = 400,
            AmpAttack = 5, AmpSustain = 100, AmpRelease = 200, Glide = 40, Polyphony = 1, AtVibrato = 40,
            DelaySync = 1, DelayDivision = 7, DelayMix = 20, DelayFeedback = 35, ReverbMix = 15,
        ],
        vary: vary![
            Cutoff: (1500, 8000), UnisonDetune: (0, 30), Osc2Semi: [7, 12, 19], Glide: (0, 120),
            DelayMix: (0, 35), Distortion: (0, 30),
        ],
    },
    Archetype {
        name: "Square Lead",
        category: Category::Lead,
        set: set![
            Osc1Type = 5, PwmWidth = 30, Osc2Type = 2, Osc2Semi = -12, Osc2Level = 30, Cutoff = 2500,
            Resonance = 15, AmpAttack = 3, AmpSustain = 100, AmpRelease = 150, Glide = 30, Polyphony = 1,
            AtVibrato = 40, ReverbMix = 15,
        ],
        vary: vary![
            PwmWidth: (10, 50), Cutoff: (1200, 6000), Glide: (0, 100), Osc2Level: (0, 50), DelayMix: (0, 30),
        ],
    },
    Archetype {
        name: "Screamer",
        category: Category::Lead,
        set: set![
            Osc1Type = 6, SupersawDetune = 35, FilterType = 1, Cutoff = 1800, Resonance = 55, FilterDrive = 60,
            Distortion = 40, AmpAttack = 2, AmpSustain = 100, AmpRelease = 200, Polyphony = 1, Glide = 30,
            DelaySync = 1, DelayDivision = 7, DelayMix = 20, DelayFeedback = 30, ReverbMix = 15,
        ],
        vary: vary![
            Cutoff: (900, 4000), Resonance: (30, 80), Distortion: (20, 70), SupersawDetune: (15, 80),
        ],
    },
    Archetype {
        name: "Chip Lead",
        category: Category::Lead,
        set: set![
            Osc1Type = 2, FilterType = 4, Bitcrush = 30, AmpAttack = 0, AmpDecay = 100, AmpSustain = 80,
            AmpRelease = 50, Lfo1Rate = 7, Lfo1Pitch = 25, Polyphony = 1, VelAmp = 20,
        ],
        vary: vary![
            Osc1Type: [2, 3, 5], PwmWidth: (10, 40), Bitcrush: (0, 60), Lfo1Pitch: (0, 40), DelayMix: (0, 25),
        ],
    },
    Archetype {
        name: "Whistle",
        category: Category::Lead,
        set: set![
            Osc1Type = 0, Osc2Type = 3, Osc2Semi = 12, Osc2Level = 20, NoiseLevel = 5, Cutoff = 6000,
            AmpAttack = 40, AmpSustain = 100, AmpRelease = 250, Lfo1Rate = 5.5, Lfo1Pitch = 20, Glide = 60,
            Polyphony = 1, ReverbMix = 30,
        ],
        vary: vary![
            NoiseLevel: (0, 20), Lfo1Pitch: (5, 35), Glide: (20, 150), AmpAttack: (10, 150), ReverbMix: (15, 50),
        ],
    },
    Archetype {
        name: "Vowel Lead",
        category: Category::Lead,
        set: set![
            Osc1Type = 1, Osc2Type = 5, Osc2Level = 40, FilterType = 3, Vowel = 30, Resonance = 40,
            Cutoff = 1262, AmpAttack = 10, AmpSustain = 100, AmpRelease = 200, Polyphony = 1, Glide = 40,
            AtVibrato = 50, ReverbMix = 20,
        ],
        vary: vary![
            Vowel: (0, 100), Resonance: (10, 80), Cutoff: (500, 3000), Glide: (0, 100),
        ],
    },
    Archetype {
        name: "Warm Pad",
        category: Category::Pad,
        set: set![
            Osc1Type = 1, Osc2Type = 1, Osc2Detune = 10, Osc2Level = 70, Unison = 4, UnisonDetune = 20,
            Cutoff = 1200, Resonance = 10, FilterEnvAmount = 6, FilterAttack = 2000, FilterDecay = 3000,
            FilterSustain = 60, AmpAttack = 1200, AmpDecay = 1500, AmpSustain = 80, AmpRelease = 2500,
            Lfo1Rate = 0.2, Lfo1Filter = 10, ChorusMix = 35, ReverbMix = 45, ReverbSize = 75, VelAmp = 30,
        ],
        vary: vary![
            Cutoff: (500, 3000), AmpAttack: (300, 3000), UnisonDetune: (8, 40), ChorusMix: (0, 60),
            ReverbMix: (25, 70), Lfo1Filter: (0, 30),
        ],
    },
    Archetype {
        name: "Glass Pad",
        category: Category::Pad,
        set: set![
            Osc1Type = 4, WavetablePos = 20, Osc2Type = 0, Osc2Semi = 12, Osc2Level = 40, Cutoff = 5000,
            AmpAttack = 800, AmpSustain = 85, AmpRelease = 3000, ReverbMix = 55, ReverbSize = 80, DelaySync = 1,
            DelayDivision = 9, DelayMix = 20, DelayFeedback = 40, VelAmp = 30,
        ],
        vary: vary![
            WavetablePos: (0, 60), Cutoff: (2000, 10000), AmpAttack: (200, 2500), DelayMix: (0, 35),
            Osc2Semi: [12, 19, 24],
        ],
    },
    Archetype {
        name: "Trance Pad",
        category: Category::Pad,
        set: set![
            Osc1Type = 6, SupersawDetune = 50, Cutoff = 4000, Resonance = 10, AmpAttack = 300, AmpSustain = 90,
            AmpRelease = 1500, ChorusMix = 20, ReverbMix = 40, ReverbSize = 70, VelAmp = 30,
        ],
        vary: vary![
            SupersawDetune: (25, 100), Cutoff: (1500, 9000), AmpAttack: (50, 1200), ReverbMix: (20, 60),
            Lfo2Filter: (0, 25),
        ],
    },
    Archetype {
        name: "Choir",
        category: Category::Pad,
        set: set![
            Osc1Type = 1, Osc2Type = 5, Osc2Level = 50, FilterType = 3, Vowel = 50, Resonance = 40, Unison = 3,
            UnisonDetune = 15, AmpAttack = 900, AmpSustain = 90, AmpRelease = 2000, Lfo1Rate = 4.5, Lfo1Pitch = 8,
            ChorusMix = 50, ReverbMix = 60, ReverbSize = 80, VelAmp = 30,
        ],
        vary: vary![
            Vowel: (0, 100), Resonance: (20, 70), AmpAttack: (300, 2000), ChorusMix: (20, 80),
        ],
    },
    Archetype {
        name: "Drone",
        category: Category::Pad,
        set: set![
            Osc1Type = 1, Osc1Semi = -12, Osc2Type = 2, Osc2Semi = -24, Osc2Level = 40, FilterType = 1,
            Cutoff = 400, Resonance = 40, Lfo1Rate = 0.07, Lfo1Filter = 40, Lfo3Rate = 0.1, Lfo3Pan = 40,
            PinkLevel = 10, AmpAttack = 2500, AmpSustain = 100, AmpRelease = 4000, ReverbMix = 70,
            ReverbSize = 90, VelAmp = 10,
        ],
        vary: vary![
            Cutoff: (200, 900), Resonance: (20, 75), Lfo1Filter: (15, 60), PinkLevel: (0, 30),
            Lfo1Rate: (0.03, 0.3),
        ],
    },
    Archetype {
        name: "Strings",
        category: Category::Pad,
        set: set![
            Osc1Type = 1, Osc2Type = 1, Osc2Detune = 12, Osc2Level = 80, Unison = 3, UnisonDetune = 15,
            Cutoff = 3000, Resonance = 5, AmpAttack = 400, AmpDecay = 500, AmpSustain = 90, AmpRelease = 800,
            VelFilter = 40, ChorusMix = 45, ReverbMix = 40, ReverbSize = 70, Lfo1Rate = 5, Lfo1Pitch = 10,
        ],
        vary: vary![
            Cutoff: (1200, 6000), AmpAttack: (80, 1200), UnisonDetune: (5, 30), ChorusMix: (20, 70),
        ],
    },
    Archetype {
        name: "Air Pad",
        category: Category::Pad,
        set: set![
            Osc1Type = 3, Osc2Type = 0, Osc2Semi = 19, Osc2Level = 30, PinkLevel = 25, Cutoff = 7000,
            AmpAttack = 1500, AmpSustain = 90, AmpRelease = 3500, ReverbMix = 80, ReverbSize = 95, DelaySync = 1,
            DelayDivision = 11, DelayMix = 25, DelayFeedback = 50, VelAmp = 20,
        ],
        vary: vary![
            PinkLevel: (10, 50), Cutoff: (3000, 12000), AmpAttack: (500, 4000), Osc2Semi: [12, 19, 24],
        ],
    },
    Archetype {
        name: "Pluck",
        category: Category::Pluck,
        set: set![
            Osc1Type = 1, Osc2Type = 2, Osc2Semi = 12, Osc2Level = 30, Cutoff = 400, Resonance = 20,
            FilterEnvAmount = 36, FilterDecay = 200, FilterSustain = 0, AmpAttack = 0, AmpDecay = 450,
            AmpSustain = 0, AmpRelease = 300, DelaySync = 1, DelayDivision = 7, DelayMix = 25, DelayFeedback = 35,
            ReverbMix = 25,
        ],
        vary: vary![
            Cutoff: (200, 1200), FilterDecay: (80, 500), AmpDecay: (150, 900), Resonance: (0, 60),
            Unison: [1, 2, 3],
        ],
    },
    Archetype {
        name: "Harp",
        category: Category::Pluck,
        set: set![
            Osc1Type = 3, Osc2Type = 1, Osc2Semi = 12, Osc2Level = 20, Cutoff = 800, FilterEnvAmount = 30,
            FilterDecay = 300, FilterSustain = 0, AmpAttack = 0, AmpDecay = 1200, AmpSustain = 0,
            AmpRelease = 900, ReverbMix = 35, ReverbSize = 65,
        ],
        vary: vary![Cutoff: (400, 2000), AmpDecay: (600, 2500), Osc2Level: (0, 40), ReverbMix: (15, 55)],
    },
    Archetype {
        name: "Metal Pluck",
        category: Category::Pluck,
        set: set![
            Osc1Type = 7, Osc1Level = 100, Osc2Type = 1, Osc2Level = 50, FilterType = 2, Cutoff = 400,
            Resonance = 85, KeyTrack = 100, AmpAttack = 0, AmpDecay = 600, AmpSustain = 0, AmpRelease = 500,
            ReverbMix = 25,
        ],
        vary: vary![Cutoff: (150, 1500), Resonance: (60, 95), AmpDecay: (200, 1200), Osc2Level: (30, 80)],
    },
    Archetype {
        name: "Marimba",
        category: Category::Pluck,
        set: set![
            Osc1Type = 0, Osc2Type = 0, Osc2Semi = 24, Osc2Level = 30, PitchEnv = 1, PitchEnvAmount = 2,
            PitchEnvDecay = 10, Cutoff = 4000, AmpAttack = 0, AmpDecay = 600, AmpSustain = 0, AmpRelease = 400,
            ReverbMix = 20,
        ],
        vary: vary![AmpDecay: (250, 1200), Osc2Level: (10, 50), Osc2Semi: [19, 24, 28], ReverbMix: (5, 40)],
    },
    Archetype {
        name: "Pizz",
        category: Category::Pluck,
        set: set![
            Osc1Type = 1, Cutoff = 1500, FilterEnvAmount = 20, FilterDecay = 120, FilterSustain = 0,
            AmpAttack = 0, AmpDecay = 250, AmpSustain = 0, AmpRelease = 200, ReverbMix = 30, ReverbSize = 60,
        ],
        vary: vary![Cutoff: (700, 3000), AmpDecay: (120, 500), Unison: [1, 2, 3], ReverbMix: (15, 50)],
    },
    Archetype {
        name: "Tine Piano",
        category: Category::Keys,
        set: set![
            Osc1Type = 0, Osc2Type = 0, Osc2Semi = 0, FmAmount = 25, ModAttack = 0, ModDecay = 400,
            ModSustain = 0, FilterType = 4, AmpAttack = 2, AmpDecay = 1800, AmpSustain = 30, AmpRelease = 600,
            Lfo3Rate = 4, Lfo3Pan = 30, ChorusMix = 20, ReverbMix = 20,
        ],
        vary: vary![FmAmount: (10, 45), ModDecay: (150, 900), Lfo3Pan: (0, 60), ChorusMix: (0, 50)],
    },
    Archetype {
        name: "Organ",
        category: Category::Keys,
        set: set![
            Osc1Type = 4, WavetablePos = 100, Osc2Type = 0, Osc2Semi = 12, Osc2Level = 40, SubLevel = 30,
            FilterType = 4, AmpAttack = 5, AmpSustain = 100, AmpRelease = 80, Lfo3Rate = 6, Lfo3Pan = 30,
            ChorusMix = 40, ReverbMix = 20, VelAmp = 0,
        ],
        vary: vary![Osc2Level: (0, 70), SubLevel: (0, 60), Lfo3Rate: (0.8, 7), Distortion: (0, 30)],
    },
    Archetype {
        name: "Clav",
        category: Category::Keys,
        set: set![
            Osc1Type = 5, PwmWidth = 20, Cutoff = 1500, Resonance = 30, FilterEnvAmount = 24, FilterDecay = 150,
            FilterSustain = 20, AmpAttack = 0, AmpDecay = 400, AmpSustain = 40, AmpRelease = 60, Distortion = 15,
            VelFilter = 50,
        ],
        vary: vary![PwmWidth: (5, 40), Cutoff: (700, 3000), Resonance: (10, 60), Distortion: (0, 40)],
    },
    Archetype {
        name: "Poly Keys",
        category: Category::Keys,
        set: set![
            Osc1Type = 5, PwmWidth = 50, SubLevel = 30, Cutoff = 2000, FilterEnvAmount = 12, FilterDecay = 500,
            AmpAttack = 2, AmpDecay = 900, AmpSustain = 60, AmpRelease = 400, ChorusMix = 50, ReverbMix = 20,
        ],
        vary: vary![PwmWidth: (20, 50), Cutoff: (800, 5000), ChorusMix: (20, 80), AmpRelease: (150, 1200)],
    },
    Archetype {
        name: "FM Bell",
        category: Category::Bell,
        set: set![
            Osc1Type = 0, Osc2Type = 0, Osc2Semi = 22, Osc2Detune = -31, FmAmount = 60, ModAttack = 0,
            ModDecay = 1500, ModSustain = 0, FilterType = 4, AmpAttack = 0, AmpDecay = 3000, AmpSustain = 0,
            AmpRelease = 2000, ReverbMix = 40, ReverbSize = 70,
        ],
        vary: vary![FmAmount: (30, 90), ModDecay: (400, 3000), Osc2Semi: [19, 22, 26, 31], AmpDecay: (1500, 5000)],
    },
    Archetype {
        name: "Music Box",
        category: Category::Bell,
        set: set![
            Osc1Type = 3, Osc1Semi = 12, Osc2Type = 0, Osc2Semi = 24, Osc2Level = 20, Cutoff = 8000,
            AmpAttack = 0, AmpDecay = 1200, AmpSustain = 0, AmpRelease = 1200, DelaySync = 1, DelayDivision = 9,
            DelayMix = 15, DelayFeedback = 30, ReverbMix = 35,
        ],
        vary: vary![AmpDecay: (600, 2000), Osc2Level: (0, 40), DelayMix: (0, 30), ReverbMix: (20, 55)],
    },
    Archetype {
        name: "Ring Bell",
        category: Category::Bell,
        set: set![
            Osc1Type = 0, Osc2Type = 0, Osc2Semi = 17, Osc2Detune = 13, RingMod = 1, Osc1Level = 80,
            FilterType = 4, AmpAttack = 0, AmpDecay = 2000, AmpSustain = 0, AmpRelease = 1500, ReverbMix = 50,
            ReverbSize = 75,
        ],
        vary: vary![Osc2Semi: [7, 14, 17, 23], Osc2Detune: (-30, 30), AmpDecay: (800, 4000), ReverbMix: (30, 70)],
    },
    Archetype {
        name: "Synth Brass",
        category: Category::Brass,
        set: set![
            Osc1Type = 1, Osc2Type = 1, Osc2Detune = 8, Osc2Level = 80, Unison = 2, UnisonDetune = 10,
            Cutoff = 600, Resonance = 15, FilterEnvAmount = 30, FilterAttack = 80, FilterDecay = 600,
            FilterSustain = 40, AmpAttack = 40, AmpSustain = 90, AmpRelease = 250, VelFilter = 50, ChorusMix = 15,
            ReverbMix = 20,
        ],
        vary: vary![Cutoff: (300, 1500), FilterAttack: (20, 300), FilterEnvAmount: (15, 40), AmpAttack: (10, 150)],
    },
    Archetype {
        name: "Stab",
        category: Category::Brass,
        set: set![
            Osc1Type = 6, SupersawDetune = 40, Cutoff = 2500, FilterEnvAmount = 24, FilterDecay = 200,
            FilterSustain = 20, AmpAttack = 0, AmpDecay = 300, AmpSustain = 0, AmpRelease = 200, ReverbMix = 30,
            DelaySync = 1, DelayDivision = 6, DelayMix = 15, DelayFeedback = 25,
        ],
        vary: vary![Cutoff: (1000, 6000), AmpDecay: (150, 600), SupersawDetune: (20, 80), Distortion: (0, 40)],
    },
    Archetype {
        name: "Wind",
        category: Category::Texture,
        set: set![
            Osc1Level = 0, NoiseLevel = 80, Cutoff = 800, Resonance = 70, Lfo1Rate = 0.15, Lfo1Filter = 50,
            Lfo2Rate = 0.07, Lfo2Filter = 20, AmpAttack = 1500, AmpSustain = 100, AmpRelease = 3000,
            ReverbMix = 60, ReverbSize = 85, KeyTrack = 80, VelAmp = 0,
        ],
        vary: vary![Cutoff: (300, 2500), Resonance: (40, 90), Lfo1Rate: (0.05, 0.5), Lfo1Filter: (20, 70)],
    },
    Archetype {
        name: "Crackle",
        category: Category::Texture,
        set: set![
            Osc1Level = 0, PinkLevel = 70, Bitcrush = 40, Cutoff = 4000, Lfo1Rate = 12, Lfo1Shape = 3,
            Lfo1Amp = 60, AmpAttack = 200, AmpSustain = 100, AmpRelease = 1500, ReverbMix = 50, VelAmp = 0,
        ],
        vary: vary![Bitcrush: (20, 70), Lfo1Rate: (4, 20), Lfo1Amp: (30, 90), Cutoff: (1500, 9000)],
    },
    Archetype {
        name: "Sci-Fi Sweep",
        category: Category::Texture,
        set: set![
            Osc1Type = 1, Osc2Type = 2, Osc2Semi = -5, Osc2Level = 60, RingMod = 1, PitchEnv = 1,
            PitchEnvAmount = -24, PitchEnvAttack = 0, PitchEnvDecay = 900, FilterType = 1, Resonance = 70,
            Cutoff = 600, FilterEnvAmount = 36, FilterDecay = 900, AmpAttack = 5, AmpSustain = 80,
            AmpRelease = 800, DelaySync = 1, DelayDivision = 7, DelayFeedback = 60, DelayMix = 35, ReverbMix = 30,
        ],
        vary: vary![PitchEnvAmount: (-24, 24), PitchEnvDecay: (200, 1000), Resonance: (40, 90), DelayFeedback: (30, 80)],
    },
    Archetype {
        name: "Metal Drone",
        category: Category::Texture,
        set: set![
            Osc1Type = 1, Osc1Semi = -12, FilterType = 2, Cutoff = 300, Resonance = 80, Lfo1Rate = 0.1,
            Lfo1Filter = 30, PinkLevel = 15, AmpAttack = 2000, AmpSustain = 100, AmpRelease = 3000,
            ReverbMix = 60, ReverbSize = 85, VelAmp = 0,
        ],
        vary: vary![Cutoff: (100, 900), Resonance: (60, 95), Lfo1Filter: (10, 60), PinkLevel: (0, 40)],
    },
];

pub struct MacroTemplate {
    pub name: &'static str,
    pub targets: &'static [(Pid, f32)],
}

macro_rules! macros {
    ($($name:literal => [$($p:ident $a:expr),*]),* $(,)?) => {
        &[$(MacroTemplate { name: $name, targets: &[$(($p, $a)),*] }),*]
    };
}

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Category::Bass => "Bass",
            Category::Lead => "Lead",
            Category::Pad => "Pad",
            Category::Pluck => "Pluck",
            Category::Keys => "Keys",
            Category::Bell => "Bell",
            Category::Brass => "Brass",
            Category::Texture => "Texture",
        }
    }

    /// Notes the quality checks play: a single low note, a single mid
    /// note, or a chord.
    pub fn test_notes(self) -> &'static [u8] {
        match self {
            Category::Bass => &[36],
            Category::Lead | Category::Pluck | Category::Bell => &[60],
            Category::Texture => &[48],
            Category::Pad | Category::Keys | Category::Brass => &[48, 52, 55, 59],
        }
    }

    pub fn macros(self) -> &'static [MacroTemplate] {
        match self {
            Category::Bass => macros![
                "Tone" => [Cutoff 0.35], "Growl" => [Distortion 0.5, FilterDrive 0.4],
                "Punch" => [FilterEnvAmount 0.25, FilterDecay -0.15], "Squelch" => [Resonance 0.5],
                "Sub" => [SubLevel 0.6], "Wide" => [UnisonDetune 0.3, Spread 0.5], "Glide" => [Glide 0.5],
                "Space" => [ReverbMix 0.4, DelayMix 0.3],
            ],
            Category::Lead | Category::Brass => macros![
                "Tone" => [Cutoff 0.3], "Bite" => [Resonance 0.4, FilterDrive 0.4], "Grit" => [Distortion 0.5],
                "Vibrato" => [Lfo1Pitch 0.25], "Wide" => [UnisonDetune 0.4, Spread 0.5], "Glide" => [Glide 0.4],
                "Echo" => [DelayMix 0.5, DelayFeedback 0.2], "Space" => [ReverbMix 0.5],
            ],
            Category::Pad => macros![
                "Bright" => [Cutoff 0.3], "Motion" => [Lfo1Filter 0.4, Lfo3Pan 0.5], "Swell" => [AmpAttack 0.3],
                "Tail" => [AmpRelease 0.3, ReverbSize 0.3], "Shimmer" => [ChorusMix 0.5], "Air" => [PinkLevel 0.3],
                "Width" => [Width 0.25, UnisonDetune 0.3], "Space" => [ReverbMix 0.5, DelayMix 0.25],
            ],
            Category::Pluck | Category::Keys | Category::Bell => macros![
                "Tone" => [Cutoff 0.3], "Decay" => [AmpDecay 0.3, AmpRelease 0.2], "Snap" => [FilterEnvAmount 0.25],
                "Metal" => [FmAmount 0.4], "Grit" => [Distortion 0.4, Bitcrush 0.2],
                "Wide" => [ChorusMix 0.4, Width 0.25], "Echo" => [DelayMix 0.5], "Space" => [ReverbMix 0.5],
            ],
            Category::Texture => macros![
                "Tone" => [Cutoff 0.3], "Resonance" => [Resonance 0.4], "Motion" => [Lfo1Filter 0.4, Lfo2Filter 0.3],
                "Speed" => [Lfo1Rate 0.3, Lfo2Rate 0.3], "Crush" => [Bitcrush 0.5],
                "Noise" => [NoiseLevel 0.4, PinkLevel 0.4], "Echo" => [DelayMix 0.5], "Space" => [ReverbMix 0.5],
            ],
        }
    }
}

/// Words for naming a variation after the parameter it moved most:
/// (lower than the archetype, higher).
pub fn descriptor(pid: Pid) -> Option<(&'static str, &'static str)> {
    Some(match pid {
        Cutoff => ("Dark", "Bright"),
        Resonance => ("Smooth", "Squelchy"),
        FilterEnvAmount | PitchEnvAmount => ("Soft", "Snappy"),
        FilterDecay | ModDecay => ("Tight", "Long"),
        Distortion | FilterDrive => ("Clean", "Gritty"),
        UnisonDetune | SupersawDetune | Osc2Detune => ("Tight", "Wide"),
        AmpAttack => ("Quick", "Slow"),
        AmpDecay | AmpRelease => ("Short", "Ringing"),
        ReverbMix => ("Dry", "Wet"),
        DelayMix | DelayFeedback => ("Dry", "Echo"),
        FmAmount => ("Pure", "Metallic"),
        Glide => ("Stepped", "Sliding"),
        SubLevel => ("Thin", "Deep"),
        Lfo1Filter | Lfo2Filter | Lfo1Amp => ("Still", "Moving"),
        Lfo1Rate => ("Slow", "Fast"),
        Lfo1Pitch => ("Steady", "Wobbly"),
        ChorusMix => ("Plain", "Lush"),
        Bitcrush => ("Clean", "Crushed"),
        NoiseLevel | PinkLevel => ("Clean", "Airy"),
        Vowel => ("Open", "Hollow"),
        PwmWidth => ("Nasal", "Hollow"),
        WavetablePos => ("Round", "Buzzy"),
        Osc2Level => ("Single", "Layered"),
        Lfo3Pan => ("Centered", "Swirling"),
        Lfo3Rate => ("Slow", "Fast"),
        _ => return None,
    })
}
