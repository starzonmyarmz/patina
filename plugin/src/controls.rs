//! Hardware control state shared by the audio thread (which decodes MIDI),
//! a GUI-thread task (which turns it into parameter changes) and the editor.

use crate::bridge::{load_preset, ContextHost, Host};
use crate::layout::{KNOBS, PAGES};
use crate::library::Library;
use crate::minilab::{classify, Control, KnobMode, NEXT_PAD, PREV_PAD};
use crate::params::{Pid, ThousandParams};
use nih_plug::prelude::GuiContext;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Encoder ticks to sweep a continuous parameter end to end.
const TICKS_PER_SWEEP: f32 = 128.0;

pub struct Controls {
    page: AtomicUsize,
    ticks: [AtomicI32; KNOBS],
    step: AtomicI32,
    pending: AtomicBool,
    knob_mode: AtomicU8,
    /// Taken on initialize. Cleared on deactivate: the context holds the
    /// plugin wrapper, which owns us.
    context: Mutex<Option<Arc<dyn GuiContext>>>,
    library: Mutex<Library>,
    /// The browser's filtered order, which preset pads step through.
    order: Mutex<Vec<String>>,
    /// Partial steps for stepped parameters, per knob.
    fractions: Mutex<[f32; KNOBS]>,
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
            context: Mutex::new(None),
            library: Mutex::new(library),
            order: Mutex::new(Vec::new()),
            fractions: Mutex::new([0.0; KNOBS]),
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

    pub fn library(&self) -> MutexGuard<'_, Library> {
        self.library.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_order(&self, keys: Vec<String>) {
        *self.order.lock().unwrap_or_else(|e| e.into_inner()) = keys;
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
        match classify(cc, value, self.knob_mode()) {
            Some(Control::Knob { index, ticks }) => {
                self.ticks[index].fetch_add(ticks, Ordering::AcqRel);
                self.pending.store(true, Ordering::Release);
            }
            Some(Control::Pad(PREV_PAD)) => {
                self.step.fetch_sub(1, Ordering::AcqRel);
                self.pending.store(true, Ordering::Release);
            }
            Some(Control::Pad(NEXT_PAD)) => {
                self.step.fetch_add(1, Ordering::AcqRel);
                self.pending.store(true, Ordering::Release);
            }
            Some(Control::Pad(page)) => self.set_page(page),
            None => return false,
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
            }
        }
    }

    pub fn run_with(&self, params: &ThousandParams, host: &mut impl Host) {
        let slots = PAGES[self.page()].slots();
        {
            let mut fractions = self.fractions.lock().unwrap_or_else(|e| e.into_inner());
            for (i, slot) in slots.iter().enumerate() {
                let ticks = self.ticks[i].swap(0, Ordering::AcqRel);
                let Some(pid) = *slot else { continue };
                if ticks == 0 {
                    continue;
                }
                if let Some(target) = turn(pid, host.normalized(pid), ticks, &mut fractions[i]) {
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
    }

    fn step_preset(&self, params: &ThousandParams, step: i32, host: &mut impl Host) {
        let library = self.library();
        let order = self.order.lock().map(|o| o.clone()).unwrap_or_default();
        let keys = if order.is_empty() { library.keys() } else { order };
        let len = keys.len() as i32;
        if len == 0 {
            return;
        }
        let current = library.key_of(&params.session()).and_then(|k| keys.iter().position(|x| *x == k));
        let next = match current {
            Some(i) => (i as i32 + step).rem_euclid(len),
            None if step > 0 => 0,
            None => len - 1,
        };
        load_preset(params, &library, &keys[next as usize], host);
    }
}

/// The normalized value after `ticks` encoder ticks, or None when it
/// doesn't move. Stepped parameters take several ticks per step when they
/// have few steps, so menus don't race past.
fn turn(pid: Pid, current: f32, ticks: i32, fraction: &mut f32) -> Option<f32> {
    let target = match pid.spec().steps() {
        None => (current + ticks as f32 / TICKS_PER_SWEEP).clamp(0.0, 1.0),
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
    use crate::params::COUNT;
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
        let root = std::env::temp_dir().join(format!("1000-synths-controls-{tag}-{}", std::process::id()));
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
    fn menus_need_several_ticks_per_step() {
        let mut fraction = 0.0;
        let pid = Pid::Osc1Type;
        let start = pid.spec().normalize(1.0);
        assert_eq!(turn(pid, start, 3, &mut fraction), None);
        let next = turn(pid, start, 1, &mut fraction).unwrap();
        assert_eq!(pid.spec().denormalize(next), 2.0);
        assert_eq!(turn(Pid::Osc1Type, 0.0, -4, &mut 0.0), None);
    }

    #[test]
    fn pads_step_through_presets() {
        let (root, controls, params, mut host) = setup("pads");
        let keys = controls.library().keys();
        controls.midi_cc(27, 127);
        controls.run_with(&params, &mut host);
        assert_eq!(controls.library().key_of(&params.session()).as_ref(), Some(&keys[0]));
        controls.midi_cc(26, 127);
        controls.midi_cc(26, 127);
        controls.run_with(&params, &mut host);
        assert_eq!(controls.library().key_of(&params.session()).as_ref(), Some(&keys[keys.len() - 2]));

        controls.set_order(vec![keys[10].clone(), keys[3].clone()]);
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
    fn other_ccs_pass_through() {
        let (root, controls, ..) = setup("other");
        assert!(!controls.midi_cc(1, 100));
        assert!(!controls.midi_cc(64, 127));
        let _ = std::fs::remove_dir_all(root);
    }
}
