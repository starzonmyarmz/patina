use crate::params::{Pid, PlainValues, Values, COUNT, SPECS};

pub const MACROS: usize = 8;
pub const TARGETS_PER_MACRO: usize = 4;
pub const MACRO_PIDS: [Pid; MACROS] =
    [Pid::Macro1, Pid::Macro2, Pid::Macro3, Pid::Macro4, Pid::Macro5, Pid::Macro6, Pid::Macro7, Pid::Macro8];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub pid: Pid,
    /// Normalized offset at full macro, −1..1.
    pub amount: f32,
}

/// What each macro moves. Copy-only so the audio thread can hold its own.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MacroMatrix(pub [[Option<Target>; TARGETS_PER_MACRO]; MACROS]);

impl MacroMatrix {
    /// Returns `base` with every macro's targets offset by `amounts` (0..1).
    pub fn apply(&self, base: &impl Values, amounts: [f32; MACROS]) -> PlainValues {
        let mut values = PlainValues(std::array::from_fn(|i| base.get(Pid::ALL[i])));
        let mut offsets = [0.0f32; COUNT];
        let mut touched = false;
        for (targets, &amount) in self.0.iter().zip(&amounts) {
            if amount == 0.0 {
                continue;
            }
            for t in targets.iter().flatten() {
                offsets[t.pid as usize] += t.amount * amount;
                touched = true;
            }
        }
        if touched {
            for (i, &offset) in offsets.iter().enumerate() {
                if offset != 0.0 {
                    let spec = &SPECS[i];
                    values.0[i] = spec.denormalize(spec.normalize(values.0[i]) + offset);
                }
            }
        }
        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_offsets_in_normalized_space_and_clamps() {
        let mut m = MacroMatrix::default();
        m.0[0][0] = Some(Target { pid: Pid::Resonance, amount: 0.5 });
        m.0[1][0] = Some(Target { pid: Pid::Resonance, amount: 0.5 });
        m.0[2][0] = Some(Target { pid: Pid::FilterType, amount: 1.0 });
        let base = PlainValues::default();

        let none = m.apply(&base, [0.0; MACROS]);
        assert_eq!(none.get(Pid::Resonance), base.get(Pid::Resonance));

        let half = m.apply(&base, [0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert!((half.get(Pid::Resonance) - 45.0).abs() < 1e-3);

        let full = m.apply(&base, [1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(full.get(Pid::Resonance), 100.0);
        assert_eq!(full.get(Pid::FilterType), 4.0);
    }
}
