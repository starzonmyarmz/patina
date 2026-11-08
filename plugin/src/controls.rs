//! Hardware control state shared by the audio thread (which decodes MIDI),
//! a GUI-thread task (which turns it into parameter changes) and the editor.

use crate::bridge::{current, load_preset, ContextHost, Host};
use crate::layout::{KNOBS, PAGES};
use crate::library::Library;
use crate::minilab::{classify, rating_pad, Control, KnobMode, RatingPad, CLICKS, NEXT_PAD, PADS, PAD_FIRST_CC};
use crate::params::{Pid, ThousandParams};
use crate::presets::Preset;
use crate::ratings::{Rating, Verdict};
use nih_plug::prelude::GuiContext;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

pub const TEMPO_RANGE: (f32, f32) = (40.0, 240.0);
pub const DEFAULT_TEMPO: f32 = 120.0;

/// Encoder ticks to sweep a continuous parameter end to end.
const TICKS_PER_SWEEP: f32 = 128.0;
/// How much finer continuous parameters turn while knob 1 or 9 is held.
const FINE: f32 = 8.0;

pub struct Controls {
    page: AtomicUsize,
    ticks: [AtomicI32; KNOBS],
    step: AtomicI32,
    pending: AtomicBool,
    knob_mode: AtomicU8,
    /// Knobs 1 and 9, while clicked.
    held: [AtomicBool; CLICKS as usize],
    /// Counts pad presses and releases, which repaint the pad on the device.
    pad_touches: AtomicU32,
    /// Taken on initialize. Cleared on deactivate: the context holds the
    /// plugin wrapper, which owns us.
    context: Mutex<Option<Arc<dyn GuiContext>>>,
    library: Mutex<Library>,
    /// The browser's filtered order, which preset pads step through.
    order: Mutex<Vec<String>>,
    /// Partial steps for stepped parameters, per knob.
    fractions: Mutex<[f32; KNOBS]>,
    audition: AtomicBool,
    /// The rating pad pressed last, plus one; zero for none.
    rating_pad: AtomicU8,
    restart_phrase: AtomicBool,
    ratings_generation: AtomicU32,
    /// The app's tempo as f32 bits; zero when the host sets the tempo.
    app_tempo: AtomicU32,
}

impl Controls {
    pub fn new(library: Library) -> Self {
        let mode = library.knob_mode().map(KnobMode::from_u8).unwrap_or_default();
        Self {
            page: AtomicUsize::new(0),
            ticks: Default::default(),
            step: AtomicI32::new(0),
            pending: AtomicBool::new(false),
            knob_mode: AtomicU8::new(mode as u8),
            held: Default::default(),
            pad_touches: AtomicU32::new(0),
            context: Mutex::new(None),
            library: Mutex::new(library),
            order: Mutex::new(Vec::new()),
            fractions: Mutex::new([0.0; KNOBS]),
            audition: AtomicBool::new(false),
            rating_pad: AtomicU8::new(0),
            restart_phrase: AtomicBool::new(false),
            ratings_generation: AtomicU32::new(0),
            app_tempo: AtomicU32::new(0),
        }
    }

    pub fn page(&self) -> usize {
        self.page.load(Ordering::Relaxed)
    }

    pub fn set_page(&self, page: usize) {
        if page < PAGES.len() {
            self.page.store(page, Ordering::Relaxed);
        }
    }

    pub fn knob_mode(&self) -> KnobMode {
        KnobMode::from_u8(self.knob_mode.load(Ordering::Relaxed))
    }

    pub fn set_knob_mode(&self, mode: KnobMode) {
        self.knob_mode.store(mode as u8, Ordering::Relaxed);
        let _ = self.library().set_knob_mode(mode as u8);
    }

    pub fn pad_touches(&self) -> u32 {
        self.pad_touches.load(Ordering::Acquire)
    }

    fn fine(&self) -> bool {
        self.held.iter().any(|h| h.load(Ordering::Relaxed))
    }

    pub fn library(&self) -> MutexGuard<'_, Library> {
        self.library.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_order(&self, keys: Vec<String>) {
        *self.order.lock().unwrap_or_else(|e| e.into_inner()) = keys;
    }

    pub fn audition(&self) -> bool {
        self.audition.load(Ordering::Relaxed)
    }

    pub fn set_audition(&self, on: bool) {
        self.audition.store(on, Ordering::Relaxed);
        self.restart_phrase.store(true, Ordering::Release);
    }

    pub fn replay(&self) {
        self.restart_phrase.store(true, Ordering::Release);
    }

    /// Audio thread.
    pub fn take_restart(&self) -> bool {
        self.restart_phrase.swap(false, Ordering::AcqRel)
    }

    /// Changes whenever a rating changes.
    pub fn ratings_generation(&self) -> u32 {
        self.ratings_generation.load(Ordering::Acquire)
    }

    pub fn app_tempo(&self) -> Option<f32> {
        Some(f32::from_bits(self.app_tempo.load(Ordering::Relaxed))).filter(|t| *t > 0.0)
    }

    pub fn set_app_tempo(&self, bpm: f32) {
        let bpm = bpm.clamp(TEMPO_RANGE.0, TEMPO_RANGE.1);
        self.app_tempo.store(bpm.to_bits(), Ordering::Relaxed);
    }

    pub fn keep_context(&self, context: impl FnOnce() -> Arc<dyn GuiContext>) {
        let mut slot = self.context.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            *slot = Some(context());
        }
    }

    pub fn drop_context(&self) {
        *self.context.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Audio thread. Returns false for CCs the MiniLab setup doesn't use.
    pub fn midi_cc(&self, cc: u8, value: u8) -> bool {
        if (PAD_FIRST_CC..PAD_FIRST_CC + PADS).contains(&cc) {
            self.pad_touches.fetch_add(1, Ordering::Release);
        }
        match classify(cc, value, self.knob_mode()) {
            Some(Control::Knob { index, ticks }) => {
                self.ticks[index].fetch_add(ticks, Ordering::AcqRel);
                self.pending.store(true, Ordering::Release);
            }
            Some(Control::Pad(NEXT_PAD)) => {
                self.step.fetch_add(1, Ordering::AcqRel);
                self.pending.store(true, Ordering::Release);
            }
            Some(Control::Pad(page)) => self.set_page(page),
            Some(Control::Click { index, down }) => self.held[index].store(down, Ordering::Relaxed),
            None => return false,
        }
        true
    }

    /// Audio thread. In audition mode the second pad bank rates instead of
    /// playing; returns true for notes it takes.
    pub fn midi_note(&self, note: u8, down: bool) -> bool {
        let Some(pad) = rating_pad(note) else { return false };
        self.pad_touches.fetch_add(1, Ordering::Release);
        if !self.audition() {
            return false;
        }
        if down {
            self.rating_pad.store(pad as u8 + 1, Ordering::Release);
            self.pending.store(true, Ordering::Release);
        }
        true
    }

    /// Audio thread: true when there's work for `run`.
    pub fn take_pending(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }

    /// GUI thread. Without a context (the plugin isn't active) the pending
    /// moves are dropped rather than saved up.
    pub fn run(&self, params: &ThousandParams) {
        let context = self.context.lock().ok().and_then(|c| c.clone());
        match context {
            Some(context) => self.run_with(params, &mut ContextHost::new(&*context, params)),
            None => {
                self.ticks.iter().for_each(|t| t.store(0, Ordering::Relaxed));
                self.step.store(0, Ordering::Relaxed);
                self.rating_pad.store(0, Ordering::Relaxed);
            }
        }
    }

    pub fn run_with(&self, params: &ThousandParams, host: &mut impl Host) {
        let slots = PAGES[self.page()].slots();
        let sweep = if self.fine() { TICKS_PER_SWEEP * FINE } else { TICKS_PER_SWEEP };
        {
            let mut fractions = self.fractions.lock().unwrap_or_else(|e| e.into_inner());
            for (i, slot) in slots.iter().enumerate() {
                let ticks = self.ticks[i].swap(0, Ordering::AcqRel);
                let Some(pid) = *slot else { continue };
                if ticks == 0 {
                    continue;
                }
                if let Some(target) = turn(pid, host.normalized(pid), ticks, sweep, &mut fractions[i]) {
                    host.begin(pid);
                    host.set(pid, target);
                    host.end(pid);
                }
            }
        }
        let step = self.step.swap(0, Ordering::AcqRel);
        if step != 0 {
            self.step_preset(params, step, host);
        }
        let pad = self.rating_pad.swap(0, Ordering::AcqRel);
        if pad > 0 && self.audition() {
            let verdict = match RatingPad::ALL[pad as usize - 1] {
                RatingPad::Keep => Some(Verdict::Keep),
                RatingPad::Tweak => Some(Verdict::Tweak),
                RatingPad::Cut => Some(Verdict::Cut),
                RatingPad::Clear => None,
                RatingPad::Replay => return self.replay(),
                RatingPad::NextUnrated => return self.next_unrated(params, host),
                RatingPad::Previous => return self.step_preset(params, -1, host),
                RatingPad::Next => return self.step_preset(params, 1, host),
            };
            let _ = self.rate(params, verdict, host);
        }
    }

    pub fn step_preset(&self, params: &ThousandParams, step: i32, host: &mut impl Host) {
        self.step_to(params, step, host, |_, _| true);
    }

    pub fn next_unrated(&self, params: &ThousandParams, host: &mut impl Host) {
        self.step_to(params, 1, host, |library, key| key.starts_with("factory/") && library.rating(key).is_none());
    }

    /// Loads the next preset in the browser's order that `wanted` accepts.
    fn step_to(&self, params: &ThousandParams, step: i32, host: &mut impl Host, wanted: impl Fn(&Library, &str) -> bool) {
        let library = self.library();
        let order = self.order.lock().map(|o| o.clone()).unwrap_or_default();
        let keys = if order.is_empty() { library.keys() } else { order };
        let len = keys.len() as i32;
        if len == 0 {
            return;
        }
        let current = library.key_of(&params.session()).and_then(|k| keys.iter().position(|x| *x == k));
        let mut next = match current {
            Some(i) => i as i32 + step,
            None if step > 0 => 0,
            None => len - 1,
        };
        for _ in 0..len {
            let key = &keys[next.rem_euclid(len) as usize];
            if wanted(&library, key) {
                load_preset(params, &library, key, host);
                return;
            }
            next += step.signum();
        }
    }

    /// Rates the loaded factory preset, then moves on to the next one. A
    /// tweak keeps the edited sound when the knobs moved.
    pub fn rate(&self, params: &ThousandParams, verdict: Option<Verdict>, host: &mut impl Host) -> Result<(), String> {
        let session = params.session();
        let key = self.library().key_of(&session).filter(|k| k.starts_with("factory/"));
        let Some(key) = key else {
            return Err("Only factory presets can be rated".into());
        };
        let values = current(host);
        let edited = session.saved.as_ref().is_some_and(|p| p.differs_from(&values, &session.macros));
        let rating = verdict.map(|verdict| Rating {
            verdict,
            edit: (verdict == Verdict::Tweak && edited)
                .then(|| Preset::capture(&session.name, &session.category, &values, &session.macros)),
        });
        self.library().rate(&key, rating).map_err(|e| format!("Couldn’t save the rating: {e}"))?;
        self.ratings_generation.fetch_add(1, Ordering::Release);
        if verdict.is_some() {
            self.step_preset(params, 1, host);
        }
        Ok(())
    }
}

/// The normalized value after `ticks` encoder ticks, where `sweep` ticks
/// cover a continuous parameter's range, or None when it doesn't move.
/// Stepped parameters take several ticks per step when they have few steps,
/// so menus don't race past.
fn turn(pid: Pid, current: f32, ticks: i32, sweep: f32, fraction: &mut f32) -> Option<f32> {
    let target = match pid.spec().steps() {
        None => (current + ticks as f32 / sweep).clamp(0.0, 1.0),
        Some(steps) => {
            let ticks_per_step = if steps <= 16.0 { 4.0 } else { 1.0 };
            *fraction += ticks as f32 / ticks_per_step;
            let whole = fraction.trunc();
            *fraction -= whole;
            ((current * steps).round() + whole).clamp(0.0, steps) / steps
        }
    };
    (target != current).then_some(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{Values, COUNT};
    use std::path::PathBuf;

    struct FakeHost([f32; COUNT]);

    impl Host for FakeHost {
        fn normalized(&self, pid: Pid) -> f32 {
            self.0[pid as usize]
        }
        fn begin(&mut self, _: Pid) {}
        fn set(&mut self, pid: Pid, normalized: f32) {
            self.0[pid as usize] = normalized;
        }
        fn end(&mut self, _: Pid) {}
    }

    fn setup(tag: &str) -> (PathBuf, Controls, ThousandParams, FakeHost) {
        let root = std::env::temp_dir().join(format!("patina-controls-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let controls = Controls::new(Library::at(root.join("User"), root.join("state.json")));
        let host = FakeHost(std::array::from_fn(|i| Pid::ALL[i].spec().normalize(Pid::ALL[i].spec().default)));
        (root, controls, ThousandParams::default(), host)
    }

    #[test]
    fn knobs_follow_the_page() {
        let (root, controls, params, mut host) = setup("knobs");
        let filter_page = PAGES.iter().position(|p| p.name == "Filter").unwrap();
        assert!(controls.midi_cc(20 + filter_page as u8, 127));
        assert_eq!(controls.page(), filter_page);
        let cutoff = PAGES[filter_page].slots().iter().position(|s| *s == Some(Pid::Cutoff)).unwrap();
        let before = host.normalized(Pid::Cutoff);
        controls.midi_cc(102 + cutoff as u8, 64 + 8);
        assert!(controls.take_pending());
        controls.run_with(&params, &mut host);
        assert!((host.normalized(Pid::Cutoff) - (before + 8.0 / TICKS_PER_SWEEP)).abs() < 1e-6);
        assert!(!controls.take_pending());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn holding_knob_1_or_9_turns_finer() {
        let (root, controls, params, mut host) = setup("fine");
        let before = host.normalized(Pid::Osc1Level);
        let slot = PAGES[0].slots().iter().position(|s| *s == Some(Pid::Osc1Level)).unwrap();
        assert!(controls.midi_cc(119, 127));
        controls.midi_cc(102 + slot as u8, 64 + 8);
        controls.run_with(&params, &mut host);
        assert!((host.normalized(Pid::Osc1Level) - (before + 1.0 / TICKS_PER_SWEEP)).abs() < 1e-6);
        controls.midi_cc(119, 0);
        controls.midi_cc(102 + slot as u8, 64 + 8);
        controls.run_with(&params, &mut host);
        assert!((host.normalized(Pid::Osc1Level) - (before + 9.0 / TICKS_PER_SWEEP)).abs() < 1e-6);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn menus_need_several_ticks_per_step() {
        let mut fraction = 0.0;
        let pid = Pid::Osc1Type;
        let start = pid.spec().normalize(1.0);
        assert_eq!(turn(pid, start, 3, TICKS_PER_SWEEP, &mut fraction), None);
        let next = turn(pid, start, 1, TICKS_PER_SWEEP, &mut fraction).unwrap();
        assert_eq!(pid.spec().denormalize(next), 2.0);
        assert_eq!(turn(Pid::Osc1Type, 0.0, -4, TICKS_PER_SWEEP, &mut 0.0), None);
    }

    #[test]
    fn pads_step_through_presets() {
        let (root, controls, params, mut host) = setup("pads");
        let keys = controls.library().keys();
        controls.midi_cc(27, 127);
        controls.run_with(&params, &mut host);
        assert_eq!(controls.library().key_of(&params.session()).as_ref(), Some(&keys[0]));
        controls.midi_cc(27, 127);
        controls.midi_cc(27, 127);
        controls.run_with(&params, &mut host);
        assert_eq!(controls.library().key_of(&params.session()).as_ref(), Some(&keys[2]));

        controls.set_order(vec![keys[10].clone(), keys[2].clone()]);
        controls.midi_cc(27, 127);
        controls.run_with(&params, &mut host);
        assert_eq!(controls.library().key_of(&params.session()).as_ref(), Some(&keys[10]));
        controls.midi_cc(27, 127);
        controls.run_with(&params, &mut host);
        assert_eq!(controls.library().key_of(&params.session()).as_ref(), Some(&keys[2]));
        controls.midi_cc(27, 127);
        controls.run_with(&params, &mut host);
        assert_eq!(controls.library().key_of(&params.session()).as_ref(), Some(&keys[10]));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn without_a_context_moves_are_dropped() {
        let (root, controls, params, _) = setup("nocontext");
        controls.midi_cc(102, 70);
        controls.run(&params);
        assert_eq!(controls.ticks[0].load(Ordering::Relaxed), 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rating_pads_only_work_in_audition() {
        let (root, controls, params, mut host) = setup("audition");
        let keys = controls.library().keys();
        load_preset(&params, &controls.library(), &keys[0], &mut host);
        assert!(!controls.midi_note(44, true));
        controls.set_audition(true);
        assert!(controls.midi_note(44, true));
        assert!(controls.midi_note(44, false));
        assert!(!controls.midi_note(60, true));
        controls.run_with(&params, &mut host);
        assert_eq!(controls.library().rating(&keys[0]).map(|r| r.verdict), Some(Verdict::Keep));
        assert_eq!(controls.library().key_of(&params.session()).as_ref(), Some(&keys[1]));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_tweak_keeps_the_edit() {
        let (root, controls, params, mut host) = setup("tweak");
        let keys = controls.library().keys();
        load_preset(&params, &controls.library(), &keys[0], &mut host);
        controls.rate(&params, Some(Verdict::Tweak), &mut host).unwrap();
        assert!(controls.library().rating(&keys[0]).unwrap().edit.is_none());

        load_preset(&params, &controls.library(), &keys[0], &mut host);
        let cutoff = host.normalized(Pid::Cutoff);
        host.set(Pid::Cutoff, (cutoff + 0.2) % 1.0);
        controls.rate(&params, Some(Verdict::Tweak), &mut host).unwrap();
        let edit = controls.library().rating(&keys[0]).unwrap().edit.clone().unwrap();
        let spec = Pid::Cutoff.spec();
        assert!((spec.normalize(edit.values().get(Pid::Cutoff)) - (cutoff + 0.2) % 1.0).abs() < 1e-3);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn next_unrated_skips_rated_presets() {
        let (root, controls, params, mut host) = setup("unrated");
        let keys = controls.library().keys();
        load_preset(&params, &controls.library(), &keys[0], &mut host);
        for key in &keys[1..3] {
            controls.library().rate(key, Some(Rating { verdict: Verdict::Cut, edit: None })).unwrap();
        }
        controls.next_unrated(&params, &mut host);
        assert_eq!(controls.library().key_of(&params.session()).as_ref(), Some(&keys[3]));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn only_factory_presets_are_rated() {
        let (root, controls, params, mut host) = setup("rate-new");
        assert!(controls.rate(&params, Some(Verdict::Keep), &mut host).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pad_presses_and_releases_are_counted() {
        let (root, controls, ..) = setup("touches");
        controls.midi_cc(20, 127);
        controls.midi_cc(20, 0);
        controls.midi_note(44, true);
        controls.midi_note(60, true);
        assert_eq!(controls.pad_touches(), 3);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn other_ccs_pass_through() {
        let (root, controls, ..) = setup("other");
        assert!(!controls.midi_cc(1, 100));
        assert!(!controls.midi_cc(64, 127));
        let _ = std::fs::remove_dir_all(root);
    }
}
