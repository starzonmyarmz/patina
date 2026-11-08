//! The arpeggiator, between MIDI input and the voice engine. It only sees
//! notes, so keys, pads, audition phrases and other plugins all drive it.

use crate::audition::Event;
use crate::engine::osc::Rng;
use crate::engine::Transport;
use crate::params::{Pid, PlainValues, Values, ARP_DIVISION_BEATS};

const KEYS: usize = 128;
const MAX_OCTAVES: usize = 4;
const MAX_SEQUENCE: usize = 2 * KEYS * MAX_OCTAVES;
const DEFAULT_TEMPO: f64 = 120.0;
/// A pattern started this soon after a grid step plays at once instead of
/// waiting for the next one.
const LATE_START_SECONDS: f64 = 0.03;
/// A pattern's first step waits this long so all of a chord's keys, which
/// never arrive together by hand, are in it.
const GATHER_SECONDS: f64 = 0.01;
const SILENT: i64 = i64::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Up,
    Down,
    UpDown,
    DownUp,
    AsPlayed,
    Random,
    Chord,
}

impl Mode {
    const ALL: [Mode; 7] = [Mode::Up, Mode::Down, Mode::UpDown, Mode::DownUp, Mode::AsPlayed, Mode::Random, Mode::Chord];
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub on: bool,
    pub mode: Mode,
    pub beats: f64,
    pub octaves: usize,
    /// Share of a step, 0.05..1.
    pub gate: f64,
    /// Delay of every second step, as a share of a step, 0..0.5.
    pub swing: f64,
    pub repeats: usize,
    pub latch: bool,
}

impl Settings {
    pub fn read(p: &impl Values) -> Self {
        Self {
            on: p.on(Pid::ArpOn),
            mode: Mode::ALL[p.index(Pid::ArpMode).min(Mode::ALL.len() - 1)],
            beats: ARP_DIVISION_BEATS[p.index(Pid::ArpDivision).min(ARP_DIVISION_BEATS.len() - 1)] as f64,
            octaves: p.index(Pid::ArpOctaves).clamp(1, MAX_OCTAVES),
            gate: (p.get(Pid::ArpGate) as f64 / 100.0).clamp(0.05, 1.0),
            swing: (p.get(Pid::ArpSwing) as f64 / 100.0).clamp(0.0, 0.5),
            repeats: p.index(Pid::ArpRepeats).max(1),
            latch: p.on(Pid::ArpLatch),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::read(&PlainValues::default())
    }
}

/// A set of keys with their velocities, in the order they went down.
#[derive(Clone, Copy)]
struct Keys {
    order: [u8; KEYS],
    len: usize,
    velocity: [f32; KEYS],
}

impl Default for Keys {
    fn default() -> Self {
        Self { order: [0; KEYS], len: 0, velocity: [0.0; KEYS] }
    }
}

impl Keys {
    fn has(&self, note: u8) -> bool {
        self.velocity[note as usize] > 0.0
    }

    fn add(&mut self, note: u8, velocity: f32) {
        if !self.has(note) {
            self.order[self.len] = note;
            self.len += 1;
        }
        self.velocity[note as usize] = velocity.max(f32::MIN_POSITIVE);
    }

    fn remove(&mut self, note: u8) {
        if !self.has(note) {
            return;
        }
        self.velocity[note as usize] = 0.0;
        if let Some(i) = self.notes().iter().position(|&n| n == note) {
            self.order.copy_within(i + 1..self.len, i);
            self.len -= 1;
        }
    }

    fn retain(&mut self, keep: impl Fn(u8) -> bool) {
        for i in (0..self.len).rev() {
            let note = self.order[i];
            if !keep(note) {
                self.remove(note);
            }
        }
    }

    fn clear(&mut self) {
        *self = Self::default();
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn notes(&self) -> &[u8] {
        &self.order[..self.len]
    }
}

pub struct Arp {
    settings: Settings,
    /// Keys physically down.
    pressed: Keys,
    /// Notes in the pattern: the pressed keys, plus released ones while
    /// latch or the pedal holds them.
    pattern: Keys,
    pedal: bool,
    running: bool,
    /// The next step, in samples on `clock`.
    next: f64,
    /// The next step's index on the beat grid; odd steps swing.
    step: i64,
    /// The grid index played last while the host was playing.
    fired: Option<i64>,
    /// The first step is gathering a chord, off the host's grid.
    gathering: bool,
    index: usize,
    repeat: usize,
    last: Option<u8>,
    /// When each sounding note ends, in samples on `clock`.
    off: [i64; KEYS],
    /// Samples since the arp was created, at the start of the block. Times
    /// are kept on it so they come out the same whatever the block size.
    clock: i64,
    block: usize,
    samples_per_beat: f64,
    /// The host's beat at the start of the block, while it plays.
    host_beat: Option<f64>,
    late: f64,
    gather: f64,
    rng: Rng,
}

impl Default for Arp {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            pressed: Keys::default(),
            pattern: Keys::default(),
            pedal: false,
            running: false,
            next: 0.0,
            step: 0,
            fired: None,
            gathering: false,
            index: 0,
            repeat: 0,
            last: None,
            off: [SILENT; KEYS],
            clock: 0,
            block: 0,
            samples_per_beat: 24_000.0,
            host_beat: None,
            late: 0.0,
            gather: 0.0,
            rng: Rng::new(0x2545_F491),
        }
    }
}

impl Arp {
    pub fn on(&self) -> bool {
        self.settings.on
    }

    fn hold(&self) -> bool {
        self.settings.latch || self.pedal
    }

    /// Call at the start of every block, before any other call for it.
    pub fn begin(&mut self, len: usize, sample_rate: f32, transport: Transport, settings: Settings, out: &mut impl FnMut(Event)) {
        self.clock += std::mem::replace(&mut self.block, len) as i64;
        let tempo = transport.tempo.filter(|t| *t > 0.0).unwrap_or(DEFAULT_TEMPO);
        self.samples_per_beat = sample_rate as f64 * 60.0 / tempo;
        self.late = sample_rate as f64 * LATE_START_SECONDS;
        self.gather = (sample_rate as f64 * GATHER_SECONDS).round();
        self.host_beat = transport.pos_beats.filter(|_| transport.playing);

        let was = self.settings;
        let held = self.hold();
        self.settings = settings;
        if was.on && !settings.on {
            self.silence(out);
            self.stop();
            self.pattern.clear();
        } else if !was.on && settings.on {
            for &note in self.pressed.notes() {
                out(Event::Off(note));
            }
            self.pattern = self.pressed;
            if !self.pattern.is_empty() {
                self.start(0);
            }
        } else if held && !self.hold() {
            self.release_unpressed();
        }
        if let (true, false, Some(beat)) = (self.running, self.gathering, self.host_beat) {
            let mut step = self.grid_at_or_after(beat);
            if self.fired == Some(step) {
                step += 1;
            }
            self.step = step;
            self.next = self.clock as f64 + (self.grid_beat(step) - beat) * self.samples_per_beat;
        }
    }

    pub fn key_on(&mut self, at: usize, note: u8, velocity: f32, out: &mut impl FnMut(Event)) {
        let note = note.min(KEYS as u8 - 1);
        if !self.settings.on {
            self.pressed.add(note, velocity);
            return out(Event::On(note, velocity));
        }
        if self.hold() && self.pressed.is_empty() {
            self.pattern.clear();
            self.stop();
        }
        self.pressed.add(note, velocity);
        self.pattern.add(note, velocity);
        if !self.running {
            self.start(at);
        }
    }

    pub fn key_off(&mut self, note: u8, out: &mut impl FnMut(Event)) {
        let note = note.min(KEYS as u8 - 1);
        self.pressed.remove(note);
        if !self.settings.on {
            return out(Event::Off(note));
        }
        if !self.hold() {
            self.pattern.remove(note);
            if self.pattern.is_empty() {
                self.stop();
            }
        }
    }

    /// The sustain pedal holds the pattern's notes like a temporary latch.
    pub fn pedal(&mut self, down: bool) {
        let held = self.hold();
        self.pedal = down;
        if self.settings.on && held && !self.hold() {
            self.release_unpressed();
        }
    }

    /// Ends the sounding notes; the pattern carries on at the next step.
    pub fn end_note(&mut self, out: &mut impl FnMut(Event)) {
        self.silence(out);
    }

    /// Forgets every key and ends every note, for all-notes-off.
    pub fn panic(&mut self, out: &mut impl FnMut(Event)) {
        self.silence(out);
        self.stop();
        self.pattern.clear();
        self.pressed.clear();
    }

    /// The sample in this block at which `fire` next has work, if any.
    pub fn next_due(&self) -> Option<usize> {
        let step = self.running.then(|| self.next.floor() as i64);
        let off = self.off.iter().copied().filter(|o| *o != SILENT).min();
        let due = step.into_iter().chain(off).min()? - self.clock;
        (due < self.block as i64).then(|| due.max(0) as usize)
    }

    /// Ends and starts the notes due at sample `at` of this block.
    pub fn fire(&mut self, at: usize, out: &mut impl FnMut(Event)) {
        let now = self.clock + at as i64;
        for note in 0..KEYS {
            if self.off[note] <= now {
                self.off[note] = SILENT;
                out(Event::Off(note as u8));
            }
        }
        if self.running && self.next.floor() as i64 <= now {
            self.play_step(now as f64, out);
        }
    }

    fn silence(&mut self, out: &mut impl FnMut(Event)) {
        for note in 0..KEYS {
            if std::mem::replace(&mut self.off[note], SILENT) != SILENT {
                out(Event::Off(note as u8));
            }
        }
    }

    fn stop(&mut self) {
        self.running = false;
        self.fired = None;
    }

    fn release_unpressed(&mut self) {
        let pressed = self.pressed;
        self.pattern.retain(|n| pressed.has(n));
        if self.pattern.is_empty() {
            self.stop();
        }
    }

    fn swing(&self, step: i64) -> f64 {
        if step.rem_euclid(2) == 1 { self.settings.swing } else { 0.0 }
    }

    fn grid_beat(&self, step: i64) -> f64 {
        (step as f64 + self.swing(step)) * self.settings.beats
    }

    /// The first grid step at or after `beat`.
    fn grid_at_or_after(&self, beat: f64) -> i64 {
        let guess = (beat / self.settings.beats).floor() as i64;
        (guess - 1..=guess + 2).find(|&k| self.grid_beat(k) >= beat - 1e-9).unwrap_or(guess + 2)
    }

    /// Starts a new pattern from sample `at`: on the arp's own clock once
    /// the chord is gathered, or on the host's grid while it plays.
    fn start(&mut self, at: usize) {
        self.running = true;
        self.index = 0;
        self.repeat = 0;
        self.last = None;
        self.fired = None;
        let now = (self.clock + at as i64) as f64;
        self.step = 0;
        self.next = now + self.gather;
        self.gathering = true;
        if let Some(start) = self.host_beat {
            let beat = start + at as f64 / self.samples_per_beat;
            let next = self.grid_at_or_after(beat);
            let wait = (self.grid_beat(next) - beat) * self.samples_per_beat;
            let since = (beat - self.grid_beat(next - 1)) * self.samples_per_beat;
            if since <= self.late && self.gather < wait {
                self.step = next - 1;
            } else {
                self.step = next;
                self.next = now + wait;
                self.gathering = false;
            }
        }
    }

    fn play_step(&mut self, at: f64, out: &mut impl FnMut(Event)) {
        if self.pattern.is_empty() {
            return self.stop();
        }
        let step = self.step;
        self.step += 1;
        self.gathering = false;
        self.next = match self.host_beat {
            Some(start) => {
                self.fired = Some(step);
                self.clock as f64 + (self.grid_beat(step + 1) - start) * self.samples_per_beat
            }
            None => at + (1.0 + self.swing(step + 1) - self.swing(step)) * self.settings.beats * self.samples_per_beat,
        };
        let end = at as i64 + ((self.next - at) * self.settings.gate).round().max(1.0) as i64;

        let mut sequence = [(0u8, 0f32); MAX_SEQUENCE];
        let len = self.sequence(&mut sequence);
        if len == 0 {
            return;
        }
        if self.settings.mode == Mode::Chord {
            let octave = (self.index % self.settings.octaves) as u8 * 12;
            for &note in self.sorted().iter().take(self.pattern.len) {
                if let Some(n) = note.checked_add(octave).filter(|n| (*n as usize) < KEYS) {
                    self.sound(n, self.pattern.velocity[note as usize], end, out);
                }
            }
        } else {
            let pick = match self.settings.mode {
                Mode::Random if self.repeat > 0 => sequence[..len].iter().position(|s| Some(s.0) == self.last),
                Mode::Random => Some(self.random(&sequence[..len])),
                _ => Some(self.index % len),
            };
            let (note, velocity) = sequence[pick.unwrap_or(0)];
            self.last = Some(note);
            self.sound(note, velocity, end, out);
        }
        self.repeat += 1;
        if self.repeat >= self.settings.repeats {
            self.repeat = 0;
            self.index += 1;
        }
    }

    fn sound(&mut self, note: u8, velocity: f32, end: i64, out: &mut impl FnMut(Event)) {
        if self.off[note as usize] != SILENT {
            out(Event::Off(note));
        }
        out(Event::On(note, velocity));
        self.off[note as usize] = end;
    }

    fn random(&mut self, sequence: &[(u8, f32)]) -> usize {
        let len = sequence.len();
        let roll = |rng: &mut Rng, n: usize| ((rng.unit() * n as f32) as usize).min(n - 1);
        let pick = roll(&mut self.rng, len);
        if len > 1 && Some(sequence[pick].0) == self.last {
            (pick + 1 + roll(&mut self.rng, len - 1)) % len
        } else {
            pick
        }
    }

    fn sorted(&self) -> [u8; KEYS] {
        let mut notes = self.pattern.order;
        notes[..self.pattern.len].sort_unstable();
        notes
    }

    /// The notes one pass of the pattern plays, in order.
    fn sequence(&self, out: &mut [(u8, f32); MAX_SEQUENCE]) -> usize {
        let base = if self.settings.mode == Mode::AsPlayed { self.pattern.order } else { self.sorted() };
        let mut len = 0;
        for octave in 0..self.settings.octaves as u8 {
            for &note in &base[..self.pattern.len] {
                if let Some(n) = note.checked_add(octave * 12).filter(|n| (*n as usize) < KEYS) {
                    out[len] = (n, self.pattern.velocity[note as usize]);
                    len += 1;
                }
            }
        }
        let turn = |out: &mut [(u8, f32); MAX_SEQUENCE], len: usize| {
            for i in (1..len.saturating_sub(1)).rev() {
                out[len + (len - 2 - i)] = out[i];
            }
            len + len.saturating_sub(2)
        };
        match self.settings.mode {
            Mode::Down => out[..len].reverse(),
            Mode::UpDown => len = turn(out, len),
            Mode::DownUp => {
                out[..len].reverse();
                len = turn(out, len);
            }
            _ => {}
        }
        len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;
    /// A 1/16 at 120 BPM.
    const STEP: usize = 6_000;
    const GATHER: usize = 480;

    enum Input {
        On(u8, f32),
        Off(u8),
        Pedal(bool),
    }

    struct Rig {
        arp: Arp,
        settings: Settings,
        transport: Transport,
        block: usize,
        now: usize,
        events: Vec<(usize, Event)>,
    }

    impl Rig {
        fn new(change: impl FnOnce(&mut Settings)) -> Self {
            let mut settings = Settings { on: true, ..Settings::default() };
            change(&mut settings);
            Self { arp: Arp::default(), settings, transport: Transport::default(), block: 512, now: 0, events: Vec::new() }
        }

        /// Runs until `until`, applying `inputs` (absolute sample, input) on
        /// the way.
        fn run(&mut self, until: usize, inputs: &[(usize, Input)]) -> &mut Self {
            while self.now < until {
                let len = self.block.min(until - self.now);
                let start = self.now;
                let mut transport = self.transport;
                if transport.playing {
                    let spb = SR as f64 * 60.0 / transport.tempo.unwrap();
                    transport.pos_beats = Some(transport.pos_beats.unwrap() + start as f64 / spb);
                }
                let events = &mut self.events;
                let mut at = start;
                let mut sink = |e| events.push((at, e));
                self.arp.begin(len, SR, transport, self.settings, &mut sink);
                let mut pos = 0;
                let mut pending = inputs.iter().filter(|(t, _)| (start..start + len).contains(t)).peekable();
                while pos < len {
                    at = start + pos;
                    while let Some((_, input)) = pending.next_if(|(t, _)| *t - start <= pos) {
                        let mut sink = |e| events.push((at, e));
                        match *input {
                            Input::On(n, v) => self.arp.key_on(pos, n, v, &mut sink),
                            Input::Off(n) => self.arp.key_off(n, &mut sink),
                            Input::Pedal(down) => self.arp.pedal(down),
                        }
                    }
                    let mut sink = |e| events.push((at, e));
                    self.arp.fire(pos, &mut sink);
                    let mut end = len;
                    if let Some(due) = self.arp.next_due().filter(|d| *d > pos) {
                        end = end.min(due);
                    }
                    if let Some((t, _)) = pending.peek() {
                        end = end.min(t - start);
                    }
                    pos = end.max(pos + 1);
                }
                self.now += len;
            }
            self
        }

        fn ons(&self) -> Vec<(usize, u8)> {
            self.events.iter().filter_map(|&(t, e)| if let Event::On(n, _) = e { Some((t, n)) } else { None }).collect()
        }

        fn notes(&self) -> Vec<u8> {
            self.ons().into_iter().map(|(_, n)| n).collect()
        }
    }

    fn chord(at: usize, notes: &[u8]) -> Vec<(usize, Input)> {
        notes.iter().map(|&n| (at, Input::On(n, 0.8))).collect()
    }

    /// When step `k` of a pattern started at sample 0 plays.
    fn step(k: usize) -> usize {
        GATHER + k * STEP
    }

    fn mode(m: Mode) -> Vec<u8> {
        let mut rig = Rig::new(|s| s.mode = m);
        rig.run(step(8), &chord(0, &[64, 60, 67]));
        rig.notes()
    }

    #[test]
    fn off_passes_notes_straight_through() {
        let mut rig = Rig::new(|s| s.on = false);
        rig.run(STEP * 4, &[(10, Input::On(60, 0.5)), (100, Input::Off(60))]);
        assert_eq!(rig.events, [(10, Event::On(60, 0.5)), (100, Event::Off(60))]);
    }

    #[test]
    fn steps_on_the_division_with_each_keys_velocity() {
        let mut rig = Rig::new(|_| {});
        rig.run(step(4), &[(0, Input::On(60, 0.9)), (0, Input::On(64, 0.4))]);
        let half = STEP / 2;
        assert_eq!(
            rig.events,
            [
                (step(0), Event::On(60, 0.9)),
                (step(0) + half, Event::Off(60)),
                (step(1), Event::On(64, 0.4)),
                (step(1) + half, Event::Off(64)),
                (step(2), Event::On(60, 0.9)),
                (step(2) + half, Event::Off(60)),
                (step(3), Event::On(64, 0.4)),
                (step(3) + half, Event::Off(64)),
            ]
        );
    }

    #[test]
    fn a_chord_played_by_hand_starts_from_its_first_note() {
        let mut rig = Rig::new(|_| {});
        rig.run(step(4), &[(0, Input::On(64, 0.8)), (150, Input::On(60, 0.8)), (300, Input::On(67, 0.8))]);
        assert_eq!(rig.notes(), [60, 64, 67, 60]);
    }

    #[test]
    fn modes_order_the_notes() {
        assert_eq!(mode(Mode::Up), [60, 64, 67, 60, 64, 67, 60, 64]);
        assert_eq!(mode(Mode::Down), [67, 64, 60, 67, 64, 60, 67, 64]);
        assert_eq!(mode(Mode::UpDown), [60, 64, 67, 64, 60, 64, 67, 64]);
        assert_eq!(mode(Mode::DownUp), [67, 64, 60, 64, 67, 64, 60, 64]);
        assert_eq!(mode(Mode::AsPlayed), [64, 60, 67, 64, 60, 67, 64, 60]);
        assert_eq!(mode(Mode::Chord), [60, 64, 67].repeat(8));
    }

    #[test]
    fn up_down_with_one_or_two_notes() {
        let mut one = Rig::new(|s| s.mode = Mode::UpDown);
        one.run(step(3), &chord(0, &[60]));
        assert_eq!(one.notes(), [60, 60, 60]);
        let mut two = Rig::new(|s| s.mode = Mode::UpDown);
        two.run(step(4), &chord(0, &[60, 64]));
        assert_eq!(two.notes(), [60, 64, 60, 64]);
    }

    #[test]
    fn random_never_repeats_a_note() {
        let mut rig = Rig::new(|s| s.mode = Mode::Random);
        rig.run(step(200), &chord(0, &[60, 64, 67]));
        let notes = rig.notes();
        assert_eq!(notes.len(), 200);
        assert!(notes.windows(2).all(|w| w[0] != w[1]));
        for n in [60, 64, 67] {
            assert!(notes.contains(&n));
        }
    }

    #[test]
    fn octaves_climb_then_start_over() {
        let mut rig = Rig::new(|s| s.octaves = 3);
        rig.run(step(7), &chord(0, &[60, 64]));
        assert_eq!(rig.notes(), [60, 64, 72, 76, 84, 88, 60]);

        let mut chords = Rig::new(|s| {
            s.octaves = 2;
            s.mode = Mode::Chord;
        });
        chords.run(step(3), &chord(0, &[60, 64]));
        assert_eq!(chords.notes(), [60, 64, 72, 76, 60, 64]);
    }

    #[test]
    fn octaves_skip_notes_past_the_top() {
        let mut rig = Rig::new(|s| s.octaves = 4);
        rig.run(step(3), &chord(0, &[110]));
        assert_eq!(rig.notes(), [110, 122, 110]);
    }

    #[test]
    fn repeats_play_each_note_again() {
        let mut rig = Rig::new(|s| s.repeats = 2);
        rig.run(step(5), &chord(0, &[60, 64]));
        assert_eq!(rig.notes(), [60, 60, 64, 64, 60]);
    }

    #[test]
    fn swing_delays_every_second_step() {
        let mut rig = Rig::new(|s| s.swing = 0.5);
        rig.run(step(4), &chord(0, &[60]));
        let times: Vec<_> = rig.ons().into_iter().map(|(t, _)| t - GATHER).collect();
        assert_eq!(times, [0, STEP * 3 / 2, STEP * 2, STEP * 7 / 2]);
    }

    #[test]
    fn full_gate_holds_until_the_next_note() {
        let mut rig = Rig::new(|s| s.gate = 1.0);
        rig.run(step(2) + 1, &chord(0, &[60, 64]));
        assert_eq!(
            rig.events,
            [
                (step(0), Event::On(60, 0.8)),
                (step(1), Event::Off(60)),
                (step(1), Event::On(64, 0.8)),
                (step(2), Event::Off(64)),
                (step(2), Event::On(60, 0.8)),
            ]
        );
    }

    #[test]
    fn a_released_key_leaves_at_the_next_step_and_the_last_stops_it() {
        let mut rig = Rig::new(|_| {});
        let mut inputs = chord(0, &[60, 64, 67]);
        inputs.push((step(1) + 100, Input::Off(67)));
        inputs.push((step(3) + 100, Input::Off(60)));
        inputs.push((step(3) + 100, Input::Off(64)));
        rig.run(step(6), &inputs);
        assert_eq!(rig.notes(), [60, 64, 60, 64]);
        assert!(matches!(rig.events.last(), Some((_, Event::Off(64)))));
    }

    #[test]
    fn latch_keeps_playing_and_a_fresh_press_starts_over() {
        let mut rig = Rig::new(|s| s.latch = true);
        let mut inputs = chord(0, &[60, 64]);
        inputs.extend([(100, Input::Off(60)), (100, Input::Off(64))]);
        rig.run(step(4), &inputs);
        assert_eq!(rig.notes(), [60, 64, 60, 64]);

        // Joins while one is held, replaces once all are up.
        let fresh = step(4);
        let at = |k| fresh + step(k);
        rig.run(at(4), &[(fresh, Input::On(67, 0.8)), (fresh + 10, Input::On(72, 0.8))]);
        assert_eq!(rig.notes()[4..], [67, 72, 67, 72]);

        rig.run(at(5), &[(at(4) + 10, Input::Off(67))]);
        rig.settings.latch = false;
        rig.run(at(8), &[]);
        assert_eq!(rig.notes()[8..], [67, 72, 72, 72]);
    }

    #[test]
    fn the_pedal_holds_like_latch() {
        let mut rig = Rig::new(|_| {});
        let mut inputs = vec![(0, Input::Pedal(true))];
        inputs.extend(chord(0, &[60, 64]));
        inputs.extend([(100, Input::Off(60)), (100, Input::Off(64)), (step(3) + 100, Input::Pedal(false))]);
        rig.run(step(6), &inputs);
        assert_eq!(rig.notes(), [60, 64, 60, 64]);
    }

    #[test]
    fn turning_off_ends_the_note_and_turning_on_takes_the_held_keys() {
        let mut rig = Rig::new(|_| {});
        let on = step(0) + 100;
        rig.run(on, &chord(0, &[60]));
        rig.settings.on = false;
        rig.run(on + 100, &[]);
        assert_eq!(rig.events, [(step(0), Event::On(60, 0.8)), (on, Event::Off(60))]);

        rig.run(on + 300, &[(on + 150, Input::On(64, 0.8))]);
        rig.settings.on = true;
        rig.run(on + 300 + GATHER + 1, &[]);
        assert_eq!(
            rig.events[2..],
            [(on + 150, Event::On(64, 0.8)), (on + 300, Event::Off(60)), (on + 300, Event::Off(64)), (on + 300 + GATHER, Event::On(60, 0.8))]
        );
    }

    #[test]
    fn end_note_keeps_the_pattern() {
        let mut rig = Rig::new(|_| {});
        let at = step(0) + 100;
        rig.run(at, &chord(0, &[60, 64]));
        rig.arp.end_note(&mut |e| rig.events.push((at, e)));
        rig.run(step(1) + 1, &[]);
        assert_eq!(rig.events, [(step(0), Event::On(60, 0.8)), (at, Event::Off(60)), (step(1), Event::On(64, 0.8))]);
    }

    #[test]
    fn the_same_notes_whatever_the_block_size() {
        let inputs = || {
            let mut i = chord(1234, &[60, 63, 67]);
            i.push((STEP * 5 + 77, Input::Off(63)));
            i
        };
        let run = |block| {
            let mut rig = Rig::new(|s| {
                s.swing = 0.3;
                s.gate = 0.7;
            });
            rig.block = block;
            rig.run(STEP * 12, &inputs());
            rig.events
        };
        let big = run(4096);
        assert_eq!(run(37), big);
        assert_eq!(run(1), big);
    }

    #[test]
    fn with_the_host_playing_steps_land_on_its_grid() {
        let host = |key_at| {
            let mut rig = Rig::new(|_| {});
            rig.transport = Transport { playing: true, tempo: Some(120.0), pos_beats: Some(0.0) };
            rig.run(STEP * 4, &chord(key_at, &[60]));
            rig.ons().into_iter().map(|(t, _)| t).collect::<Vec<_>>()
        };
        assert_eq!(host(STEP / 2), [STEP, STEP * 2, STEP * 3]);
        assert_eq!(host(STEP + 500), [STEP + 500 + GATHER, STEP * 2, STEP * 3]);
        assert_eq!(host(STEP * 2), [STEP * 2, STEP * 3]);
    }

    #[test]
    fn a_host_loop_jump_follows_the_grid() {
        let mut rig = Rig::new(|_| {});
        rig.block = STEP / 2;
        rig.transport = Transport { playing: true, tempo: Some(120.0), pos_beats: Some(0.0) };
        rig.run(STEP * 2, &chord(0, &[60, 64]));
        // The host jumps back to an eighth of a beat before step 0.
        rig.transport.pos_beats = Some(-0.125 - (STEP * 2) as f64 / STEP as f64 * 0.25);
        rig.run(STEP * 3, &[]);
        let times: Vec<_> = rig.ons().into_iter().map(|(t, _)| t).collect();
        assert_eq!(times, [0, STEP, STEP * 2 + STEP / 2]);
    }
}
