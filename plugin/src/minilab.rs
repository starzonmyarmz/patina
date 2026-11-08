//! The Arturia MiniLab MK2 as set up in docs/minilab-setup.md.

use crate::layout::KNOBS;

pub const KNOB_FIRST_CC: u8 = 102;
pub const PAD_FIRST_CC: u8 = 20;
pub const PADS: u8 = 8;
pub const PREV_PAD: usize = 6;
pub const NEXT_PAD: usize = 7;
/// Knobs 1 and 9 send these while clicked. The factory CCs, 113 and 115,
/// fall among the knobs'.
pub const CLICK_FIRST_CC: u8 = 118;
pub const CLICKS: u8 = 2;
/// The second pad bank's notes, as the MiniLab sends them out of the box.
pub const RATING_FIRST_NOTE: u8 = 44;

/// What the second pad bank does in audition mode, left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RatingPad {
    Keep,
    Tweak,
    Cut,
    Clear,
    Replay,
    NextUnrated,
    Previous,
    Next,
}

impl RatingPad {
    pub const ALL: [RatingPad; 8] = [
        RatingPad::Keep,
        RatingPad::Tweak,
        RatingPad::Cut,
        RatingPad::Clear,
        RatingPad::Replay,
        RatingPad::NextUnrated,
        RatingPad::Previous,
        RatingPad::Next,
    ];
}

pub fn rating_pad(note: u8) -> Option<usize> {
    (RATING_FIRST_NOTE..RATING_FIRST_NOTE + PADS).contains(&note).then(|| (note - RATING_FIRST_NOTE) as usize)
}

/// Arturia's three relative encoder encodings, as named in MIDI Control
/// Center.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum KnobMode {
    /// 64 is still; 65 is +1, 63 is −1.
    #[default]
    Relative1 = 1,
    /// Two's complement: 1 is +1, 127 is −1.
    Relative2 = 2,
    /// 16 is still; 17 is +1, 15 is −1.
    Relative3 = 3,
}

impl KnobMode {
    pub const ALL: [KnobMode; 3] = [KnobMode::Relative1, KnobMode::Relative2, KnobMode::Relative3];

    pub fn from_u8(n: u8) -> Self {
        Self::ALL.into_iter().find(|m| *m as u8 == n).unwrap_or_default()
    }

    pub fn decode(self, value: u8) -> i32 {
        let v = value.min(127) as i32;
        match self {
            KnobMode::Relative1 => v - 64,
            KnobMode::Relative2 if v >= 64 => v - 128,
            KnobMode::Relative2 => v,
            KnobMode::Relative3 => v - 16,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Control {
    Knob { index: usize, ticks: i32 },
    /// A pad in bank 1 went down.
    Pad(usize),
    /// Knob 1 (index 0) or knob 9 (index 1) was pressed or released.
    Click { index: usize, down: bool },
}

pub fn classify(cc: u8, value: u8, mode: KnobMode) -> Option<Control> {
    if (KNOB_FIRST_CC..KNOB_FIRST_CC + KNOBS as u8).contains(&cc) {
        let ticks = mode.decode(value);
        return (ticks != 0).then_some(Control::Knob { index: (cc - KNOB_FIRST_CC) as usize, ticks });
    }
    if (PAD_FIRST_CC..PAD_FIRST_CC + PADS).contains(&cc) {
        return (value > 0).then_some(Control::Pad((cc - PAD_FIRST_CC) as usize));
    }
    if (CLICK_FIRST_CC..CLICK_FIRST_CC + CLICKS).contains(&cc) {
        return Some(Control::Click { index: (cc - CLICK_FIRST_CC) as usize, down: value > 0 });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_each_encoding() {
        assert_eq!([65, 63, 70].map(|v| KnobMode::Relative1.decode(v)), [1, -1, 6]);
        assert_eq!([1, 127, 120].map(|v| KnobMode::Relative2.decode(v)), [1, -1, -8]);
        assert_eq!([17, 15, 20].map(|v| KnobMode::Relative3.decode(v)), [1, -1, 4]);
    }

    #[test]
    fn maps_knobs_and_pads() {
        let m = KnobMode::Relative1;
        assert_eq!(classify(102, 65, m), Some(Control::Knob { index: 0, ticks: 1 }));
        assert_eq!(classify(117, 62, m), Some(Control::Knob { index: 15, ticks: -2 }));
        assert_eq!(classify(120, 65, m), None);
        assert_eq!(classify(102, 64, m), None);
        assert_eq!(classify(20, 127, m), Some(Control::Pad(0)));
        assert_eq!(classify(27, 127, m), Some(Control::Pad(NEXT_PAD)));
        assert_eq!(classify(27, 0, m), None);
        assert_eq!(classify(1, 127, m), None);
        assert_eq!(classify(118, 127, m), Some(Control::Click { index: 0, down: true }));
        assert_eq!(classify(119, 0, m), Some(Control::Click { index: 1, down: false }));
    }

    #[test]
    fn modes_round_trip_through_their_number() {
        for m in KnobMode::ALL {
            assert_eq!(KnobMode::from_u8(m as u8), m);
        }
        assert_eq!(KnobMode::from_u8(0), KnobMode::Relative1);
    }
}
