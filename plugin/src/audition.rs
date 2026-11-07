//! Test phrases for audition mode: a short loop per category, played by the
//! plugin itself so presets can be judged hands-free.

use crate::library::CATEGORY_ORDER;
use std::sync::OnceLock;

const DEFAULT_TEMPO: f64 = 120.0;
const MAX_EVENTS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Note {
    at: f64,
    len: f64,
    key: u8,
    velocity: f32,
}

pub struct Phrase {
    beats: f64,
    notes: Vec<Note>,
}

fn notes(at: f64, len: f64, keys: &[u8], velocity: f32) -> impl Iterator<Item = Note> + '_ {
    keys.iter().map(move |&key| Note { at, len, key, velocity })
}

fn line(start: f64, step: f64, len: f64, keys: &[u8], velocities: &[f32]) -> Vec<Note> {
    keys.iter()
        .enumerate()
        .map(|(i, &key)| Note { at: start + i as f64 * step, len, key, velocity: velocities[i % velocities.len()] })
        .collect()
}

/// One phrase per category, in `CATEGORY_ORDER`. Each leaves room at the end
/// for release tails.
fn build() -> Vec<Phrase> {
    let accents = [1.0, 0.6, 0.8, 0.6];
    let bass = {
        let riff = [36, 36, 48, 36, 43, 46, 36, 39, 36, 36, 48, 36, 41, 43, 46, 48];
        let mut n = line(0.0, 0.5, 0.4, &riff, &accents);
        n.push(Note { at: 8.0, len: 2.0, key: 36, velocity: 0.9 });
        Phrase { beats: 12.0, notes: n }
    };
    let lead = {
        let mut n = line(0.0, 0.5, 0.45, &[60, 63, 67, 70], &[0.9, 0.7]);
        n.extend([(2.0, 1.5, 72), (3.5, 0.5, 70), (4.0, 1.0, 67), (5.0, 1.0, 63), (6.0, 0.5, 65), (6.5, 0.5, 67)]
            .map(|(at, len, key)| Note { at, len, key, velocity: 0.8 }));
        n.push(Note { at: 7.0, len: 3.5, key: 72, velocity: 1.0 });
        Phrase { beats: 12.0, notes: n }
    };
    let pad = {
        let mut n: Vec<Note> = notes(0.0, 7.0, &[48, 51, 55, 58, 62], 0.7).collect();
        n.extend(notes(8.0, 6.0, &[44, 48, 51, 55], 0.8));
        Phrase { beats: 16.0, notes: n }
    };
    let keys = {
        let cm = [48, 51, 55, 58];
        let fm = [53, 56, 60, 63];
        let mut n = Vec::new();
        for (bar, chord) in [(0.0, &cm), (4.0, &fm)] {
            n.extend(notes(bar, 1.5, chord, 0.9));
            n.extend(notes(bar + 2.0, 0.5, chord, 0.6));
            n.extend(notes(bar + 2.5, 1.5, chord, 0.75));
        }
        n.extend(notes(8.0, 3.0, &cm, 0.8));
        Phrase { beats: 12.0, notes: n }
    };
    let pluck = {
        let arp = [48, 55, 60, 63, 67, 63, 60, 55];
        let keys: Vec<u8> = arp.iter().cycle().take(24).copied().collect();
        Phrase { beats: 8.0, notes: line(0.0, 0.25, 0.2, &keys, &accents) }
    };
    let bell = {
        let mut n: Vec<Note> = [(0.0, 72, 0.9), (1.5, 79, 0.7), (3.0, 75, 0.8), (4.5, 84, 0.6)]
            .map(|(at, key, velocity)| Note { at, len: 0.5, key, velocity })
            .into();
        n.extend(notes(6.0, 1.0, &[72, 79], 1.0));
        Phrase { beats: 12.0, notes: n }
    };
    let brass = {
        let mut n: Vec<Note> = notes(0.0, 3.5, &[48, 55, 60, 63], 0.8).collect();
        n.extend(notes(4.0, 3.5, &[46, 53, 58, 62], 0.9));
        n.extend(line(8.0, 0.5, 0.4, &[60, 60, 63], &[1.0, 0.7, 0.9]));
        Phrase { beats: 12.0, notes: n }
    };
    let texture = {
        let n = vec![Note { at: 0.0, len: 12.0, key: 48, velocity: 0.8 }, Note { at: 4.0, len: 8.0, key: 55, velocity: 0.7 }];
        Phrase { beats: 16.0, notes: n }
    };
    vec![bass, lead, pad, keys, pluck, bell, brass, texture]
}

/// Builds the phrases. Call once off the audio thread, since it allocates.
pub fn phrases() -> &'static [Phrase] {
    static PHRASES: OnceLock<Vec<Phrase>> = OnceLock::new();
    PHRASES.get_or_init(build)
}

/// The phrase for a category; unknown categories get the lead phrase.
pub fn phrase_for(category: &str) -> usize {
    CATEGORY_ORDER.iter().position(|c| *c == category).unwrap_or(1)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    On(u8, f32),
    Off(u8),
}

/// Events for one block, by sample offset. Fixed size so the audio thread
/// never allocates.
pub struct Events {
    items: [(u32, Event); MAX_EVENTS],
    len: usize,
}

impl Default for Events {
    fn default() -> Self {
        Self { items: [(0, Event::Off(0)); MAX_EVENTS], len: 0 }
    }
}

impl Events {
    pub fn as_slice(&self) -> &[(u32, Event)] {
        &self.items[..self.len]
    }

    fn push(&mut self, offset: u32, event: Event) {
        if self.len < MAX_EVENTS {
            self.items[self.len] = (offset, event);
            self.len += 1;
        }
    }

    /// By offset, with note-offs before note-ons at the same offset so a
    /// repeated key retriggers.
    fn sort(&mut self) {
        let rank = |e: &(u32, Event)| (e.0, matches!(e.1, Event::On(..)));
        self.items[..self.len].sort_unstable_by_key(rank);
    }
}

#[derive(Default)]
pub struct Player {
    phrase: Option<usize>,
    beat: f64,
    held: u128,
}

impl Player {
    pub fn playing(&self) -> bool {
        self.phrase.is_some()
    }

    /// Starts `phrase` from the top. Call `stop` first to release what's held.
    pub fn start(&mut self, phrase: usize) {
        self.phrase = Some(phrase.min(phrases().len() - 1));
        self.beat = 0.0;
    }

    /// Stops, calling `release` for every note still held.
    pub fn stop(&mut self, mut release: impl FnMut(u8)) {
        for key in 0..128u8 {
            if self.held & (1 << key) != 0 {
                release(key);
            }
        }
        self.held = 0;
        self.phrase = None;
    }

    /// The phrase's events in the next `samples` samples.
    pub fn block(&mut self, samples: usize, sample_rate: f32, tempo: Option<f64>, out: &mut Events) {
        out.len = 0;
        let Some(index) = self.phrase else { return };
        let phrase = &phrases()[index];
        let samples_per_beat = sample_rate as f64 * 60.0 / tempo.filter(|t| *t > 0.0).unwrap_or(DEFAULT_TEMPO);
        let start = self.beat;
        let end = start + samples as f64 / samples_per_beat;
        let offset = |beat: f64| (((beat - start) * samples_per_beat) as u32).min(samples as u32 - 1);
        let first_loop = (start / phrase.beats).floor() as i64;
        let last_loop = (end / phrase.beats).floor() as i64;
        for k in first_loop..=last_loop {
            let base = k as f64 * phrase.beats;
            for n in &phrase.notes {
                let (on, off) = (base + n.at, base + n.at + n.len);
                if (start..end).contains(&off) {
                    out.push(offset(off), Event::Off(n.key));
                }
                if (start..end).contains(&on) {
                    out.push(offset(on), Event::On(n.key, n.velocity));
                }
            }
        }
        out.sort();
        for (_, e) in out.as_slice() {
            match *e {
                Event::On(key, _) => self.held |= 1 << key,
                Event::Off(key) => self.held &= !(1 << key),
            }
        }
        self.beat = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(player: &mut Player, blocks: usize, size: usize) -> Vec<(usize, Event)> {
        let mut out = Events::default();
        let mut all = Vec::new();
        for b in 0..blocks {
            player.block(size, 48_000.0, None, &mut out);
            all.extend(out.as_slice().iter().map(|&(o, e)| (b * size + o as usize, e)));
        }
        all
    }

    #[test]
    fn every_phrase_fits_its_loop_and_range() {
        assert_eq!(phrases().len(), CATEGORY_ORDER.len());
        for p in phrases() {
            assert!(!p.notes.is_empty());
            for n in &p.notes {
                assert!(n.at + n.len < p.beats, "{n:?} runs past {}", p.beats);
                assert!(n.key < 128 && n.velocity > 0.0 && n.velocity <= 1.0);
            }
        }
    }

    #[test]
    fn plays_the_same_notes_whatever_the_block_size() {
        let beats = phrases()[0].beats;
        let samples = (beats * 24_000.0) as usize * 2;
        let mut a = Player::default();
        a.start(0);
        let mut b = Player::default();
        b.start(0);
        let big = run(&mut a, samples / 4096 + 1, 4096);
        let small = run(&mut b, samples / 37 + 1, 37);
        let window = |v: Vec<(usize, Event)>| v.into_iter().filter(|e| e.0 + 2 < samples).collect::<Vec<_>>();
        let (big, small) = (window(big), window(small));
        assert_eq!(big.len(), small.len());
        for (x, y) in big.iter().zip(&small) {
            assert_eq!(x.1, y.1);
            assert!(x.0.abs_diff(y.0) <= 1);
        }
        let ons = big.iter().filter(|e| matches!(e.1, Event::On(..))).count();
        assert_eq!(ons, phrases()[0].notes.len() * 2);
    }

    #[test]
    fn stop_releases_held_notes() {
        let mut p = Player::default();
        p.start(phrase_for("Pad"));
        run(&mut p, 10, 512);
        let mut released = Vec::new();
        p.stop(|k| released.push(k));
        assert_eq!(released, vec![48, 51, 55, 58, 62]);
        assert!(!p.playing());
    }

    #[test]
    fn unknown_categories_get_the_lead_phrase() {
        assert_eq!(phrase_for("Lead"), 1);
        assert_eq!(phrase_for("Kazoo"), 1);
        assert_eq!(phrase_for("Bass"), 0);
    }
}
