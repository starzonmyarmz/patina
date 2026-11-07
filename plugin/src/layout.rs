use crate::params::Pid;

pub const ROWS: usize = 2;
pub const COLUMNS: usize = 8;
pub const KNOBS: usize = ROWS * COLUMNS;

pub struct Section {
    pub title: &'static str,
    pub knobs: &'static [(Pid, &'static str)],
}

pub struct Page {
    pub name: &'static str,
    pub rows: [&'static [Section]; ROWS],
}

impl Page {
    /// The parameter under each of the 16 knobs, row by row.
    pub fn slots(&self) -> [Option<Pid>; KNOBS] {
        let mut slots = [None; KNOBS];
        for (r, row) in self.rows.iter().enumerate() {
            let knobs = row.iter().flat_map(|s| s.knobs.iter().map(|&(pid, _)| pid));
            for (c, pid) in knobs.enumerate().take(COLUMNS) {
                slots[r * COLUMNS + c] = Some(pid);
            }
        }
        slots
    }
}

const fn s(title: &'static str, knobs: &'static [(Pid, &'static str)]) -> Section {
    Section { title, knobs }
}

use Pid::*;

pub const PAGES: &[Page] = &[
    Page {
        name: "Osc",
        rows: [
            &[
                s("Osc 1", &[(Osc1Type, "Type"), (Osc1Semi, "Semi"), (Osc1Detune, "Detune"), (Osc1Level, "Level")]),
                s("Osc 2", &[(Osc2Type, "Type"), (Osc2Semi, "Semi"), (Osc2Detune, "Detune"), (Osc2Level, "Level")]),
            ],
            &[
                s("Cross", &[(FmAmount, "FM"), (RingMod, "Ring")]),
                s("Unison", &[(Unison, "Voices"), (UnisonDetune, "Detune"), (Spread, "Spread")]),
                s("Mix", &[(SubLevel, "Sub"), (NoiseLevel, "Noise"), (PinkLevel, "Pink")]),
            ],
        ],
    },
    Page {
        name: "Shape",
        rows: [
            &[
                s("Shape", &[(WavetablePos, "Table"), (PwmWidth, "Width"), (SupersawDetune, "Spread")]),
                s("Pitch env", &[(PitchEnv, "On"), (PitchEnvAmount, "Amount"), (PitchEnvAttack, "Attack"), (PitchEnvDecay, "Decay")]),
            ],
            &[
                s("Expression", &[(VelAmp, "Vel›amp"), (AtFilter, "AT›filter"), (AtVibrato, "AT›vibrato")]),
                s("Voice", &[(Polyphony, "Voices"), (Glide, "Glide"), (Volume, "Volume")]),
            ],
        ],
    },
    Page {
        name: "Filter",
        rows: [
            &[
                s(
                    "Filter",
                    &[(FilterType, "Type"), (Cutoff, "Cutoff"), (Resonance, "Reso"), (FilterDrive, "Drive"), (FilterEnvAmount, "Env"), (Vowel, "Vowel")],
                ),
                s("Track", &[(KeyTrack, "Key"), (VelFilter, "Velocity")]),
            ],
            &[
                s("Filter env", &[(FilterAttack, "Attack"), (FilterDecay, "Decay"), (FilterSustain, "Sustain"), (FilterRelease, "Release")]),
                s("Amp env", &[(AmpAttack, "Attack"), (AmpDecay, "Decay"), (AmpSustain, "Sustain"), (AmpRelease, "Release")]),
            ],
        ],
    },
    Page {
        name: "Mod",
        rows: [
            &[
                s("LFO 1", &[(Lfo1Rate, "Rate"), (Lfo1Shape, "Shape"), (Lfo1Pitch, "›Pitch"), (Lfo1Filter, "›Filter"), (Lfo1Amp, "›Amp")]),
                s("LFO 3", &[(Lfo3Rate, "Rate"), (Lfo3Shape, "Shape"), (Lfo3Pan, "›Pan")]),
            ],
            &[
                s("LFO 2", &[(Lfo2Rate, "Rate"), (Lfo2Shape, "Shape"), (Lfo2Pitch, "›Pitch"), (Lfo2Filter, "›Filter")]),
                s("FM env", &[(ModAttack, "Attack"), (ModDecay, "Decay"), (ModSustain, "Sustain"), (ModRelease, "Release")]),
            ],
        ],
    },
    Page {
        name: "FX",
        rows: [
            &[
                s("Dirt", &[(Distortion, "Drive"), (Bitcrush, "Crush")]),
                s("Chorus", &[(ChorusMix, "Mix"), (ChorusRate, "Rate")]),
                s("Delay", &[(DelayTime, "Time"), (DelayFeedback, "Feedback"), (DelayMix, "Mix")]),
                s("Trem", &[(Lfo3Amp, "LFO 3")]),
            ],
            &[
                s("Reverb", &[(ReverbSize, "Size"), (ReverbMix, "Mix")]),
                s("EQ", &[(EqLow, "Low"), (EqMid, "Mid"), (EqHigh, "High")]),
                s("Output", &[(Compressor, "Comp"), (Width, "Width"), (Limiter, "Limit")]),
            ],
        ],
    },
    Page {
        name: "Macro",
        rows: [
            &[s(
                "Macros",
                &[(Macro1, "1"), (Macro2, "2"), (Macro3, "3"), (Macro4, "4"), (Macro5, "5"), (Macro6, "6"), (Macro7, "7"), (Macro8, "8")],
            )],
            &[
                s("LFO 1", &[(Lfo1Sync, "Sync"), (Lfo1Division, "Division")]),
                s("LFO 2", &[(Lfo2Sync, "Sync"), (Lfo2Division, "Division")]),
                s("LFO 3", &[(Lfo3Sync, "Sync"), (Lfo3Division, "Division")]),
                s("Delay", &[(DelaySync, "Sync"), (DelayDivision, "Division")]),
            ],
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::COUNT;

    #[test]
    fn rows_fit_the_knobs() {
        for page in PAGES {
            for row in page.rows {
                let n: usize = row.iter().map(|s| s.knobs.len()).sum();
                assert!(n <= COLUMNS, "{} has a row of {n}", page.name);
            }
        }
    }

    #[test]
    fn every_parameter_is_on_exactly_one_page() {
        let mut seen = [0; COUNT];
        for page in PAGES {
            for pid in page.slots().into_iter().flatten() {
                seen[pid as usize] += 1;
            }
        }
        for &pid in Pid::ALL {
            assert_eq!(seen[pid as usize], 1, "{}", pid.spec().id);
        }
    }

    #[test]
    fn pages_fit_pad_bank_one() {
        assert!(PAGES.len() <= 6);
    }
}
