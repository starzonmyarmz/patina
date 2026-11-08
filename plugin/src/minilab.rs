//! The Arturia MiniLab MK2 as set up in docs/minilab-setup.md.

use crate::layout::KNOBS;
use crate::ratings::Verdict;

pub const KNOB_FIRST_CC: u8 = 102;
pub const PAD_FIRST_CC: u8 = 20;
pub const PADS: u8 = 8;
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

pub type SysEx = [u8; 12];

/// Sets one of a control's settings, as MIDI Control Center does.
fn set(param: u8, control: u8, value: u8) -> SysEx {
    [0xF0, 0x00, 0x20, 0x6B, 0x7F, 0x42, 0x02, 0x00, param, control, value, 0xF7]
}

const MODE: u8 = 0x01;
const CHANNEL: u8 = 0x02;
const NUMBER: u8 = 0x03;
const MIN: u8 = 0x04;
const MAX: u8 = 0x05;
const BEHAVIOUR: u8 = 0x06;
const COLOR: u8 = 0x10;

const MODE_CONTROL: u8 = 0x01;
const MODE_SWITCHED: u8 = 0x08;
const MODE_NOTE: u8 = 0x09;
const GLOBAL_CHANNEL: u8 = 0x41;
const GATE: u8 = 0x01;

/// SysEx IDs of knobs 1–16.
const KNOB_IDS: [u8; KNOBS] = [0x30, 0x01, 0x02, 0x09, 0x0B, 0x0C, 0x0D, 0x0E, 0x33, 0x03, 0x04, 0x0A, 0x05, 0x06, 0x07, 0x08];
/// Knobs 1 and 9 turned with Shift held.
const SHIFT_KNOB_IDS: [u8; 2] = [0x32, 0x35];
const CLICK_IDS: [u8; CLICKS as usize] = [0x31, 0x34];
/// Pads 1–16: bank 1, then bank 2.
const PAD_FIRST_ID: u8 = 0x70;

/// Writes the setup in docs/minilab-setup.md to the MiniLab's current
/// settings.
pub fn setup(mode: KnobMode) -> Vec<SysEx> {
    let mut out = Vec::new();
    let mut control = |id: u8, mode: u8, number: u8, behaviour: u8| {
        out.extend([set(MODE, id, mode), set(CHANNEL, id, GLOBAL_CHANNEL), set(NUMBER, id, number), set(BEHAVIOUR, id, behaviour)]);
    };
    for (i, id) in KNOB_IDS.into_iter().enumerate() {
        control(id, MODE_CONTROL, KNOB_FIRST_CC + i as u8, mode as u8);
    }
    for (i, id) in SHIFT_KNOB_IDS.into_iter().enumerate() {
        control(id, MODE_CONTROL, KNOB_FIRST_CC + 8 * i as u8, mode as u8);
    }
    for (i, id) in CLICK_IDS.into_iter().enumerate() {
        control(id, MODE_SWITCHED, CLICK_FIRST_CC + i as u8, GATE);
    }
    for i in 0..PADS {
        control(PAD_FIRST_ID + i, MODE_SWITCHED, PAD_FIRST_CC + i, GATE);
    }
    for i in 0..PADS {
        control(PAD_FIRST_ID + PADS + i, MODE_NOTE, RATING_FIRST_NOTE + i, GATE);
    }
    for id in CLICK_IDS.into_iter().chain((0..PADS).map(|i| PAD_FIRST_ID + i)) {
        out.extend([set(MIN, id, 0), set(MAX, id, 127)]);
    }
    out
}

/// The pad colors the MK2 can show. It has no brightness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Off = 0x00,
    Red = 0x01,
    Green = 0x04,
    Yellow = 0x05,
    Blue = 0x10,
    Magenta = 0x11,
    Cyan = 0x14,
    White = 0x7F,
}

pub const LIGHTS: usize = 2 * PADS as usize;

pub fn pad_color(pad: usize, color: Color) -> SysEx {
    set(COLOR, PAD_FIRST_ID + pad as u8, color as u8)
}

/// Bank 1 shows the page; bank 2 shows the rating pads in audition mode.
/// A rated preset lights only its verdict's pad.
pub fn lights(page: usize, audition: Option<Option<Verdict>>) -> [Color; LIGHTS] {
    let mut out = [Color::Off; LIGHTS];
    for (i, light) in out[..NEXT_PAD].iter_mut().enumerate() {
        *light = if i == page { Color::White } else { Color::Blue };
    }
    out[NEXT_PAD] = Color::Green;
    let Some(verdict) = audition else { return out };
    let bank = &mut out[PADS as usize..];
    for (light, pad) in bank.iter_mut().zip(RatingPad::ALL) {
        let shows = |v: Verdict, color| if verdict.is_none_or(|current| current == v) { color } else { Color::Off };
        *light = match pad {
            RatingPad::Keep => shows(Verdict::Keep, Color::Green),
            RatingPad::Tweak => shows(Verdict::Tweak, Color::Yellow),
            RatingPad::Cut => shows(Verdict::Cut, Color::Red),
            RatingPad::Clear if verdict.is_some() => Color::White,
            RatingPad::Clear => Color::Off,
            RatingPad::Replay => Color::Magenta,
            RatingPad::NextUnrated => Color::Cyan,
            RatingPad::Previous | RatingPad::Next => Color::Green,
        };
    }
    out
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
    fn setup_gives_each_control_its_own_message() {
        let messages = setup(KnobMode::Relative2);
        assert!(messages.iter().all(|m| m[..8] == [0xF0, 0x00, 0x20, 0x6B, 0x7F, 0x42, 0x02, 0x00] && m[11] == 0xF7));
        let number = |id| messages.iter().find(|m| m[8] == NUMBER && m[9] == id).map(|m| m[10]);
        assert_eq!(number(0x30), Some(102));
        assert_eq!(number(0x08), Some(117));
        assert_eq!(number(0x34), Some(119));
        assert_eq!(number(0x77), Some(27));
        assert_eq!(number(0x7F), Some(51));
        assert!(messages.iter().filter(|m| m[8] == BEHAVIOUR && KNOB_IDS.contains(&m[9])).all(|m| m[10] == 2));
    }

    #[test]
    fn lights_show_the_page_and_the_verdict() {
        let browsing = lights(2, None);
        assert_eq!(browsing[2], Color::White);
        assert_eq!(browsing[0], Color::Blue);
        assert_eq!(browsing[6..8], [Color::Blue, Color::Green]);
        assert!(browsing[8..].iter().all(|c| *c == Color::Off));

        let unrated = lights(0, Some(None));
        assert_eq!(unrated[8..11], [Color::Green, Color::Yellow, Color::Red]);
        assert_eq!(unrated[11], Color::Off);

        let cut = lights(0, Some(Some(Verdict::Cut)));
        assert_eq!(cut[8..12], [Color::Off, Color::Off, Color::Red, Color::White]);
    }

    #[test]
    fn modes_round_trip_through_their_number() {
        for m in KnobMode::ALL {
            assert_eq!(KnobMode::from_u8(m as u8), m);
        }
        assert_eq!(KnobMode::from_u8(0), KnobMode::Relative1);
    }
}
