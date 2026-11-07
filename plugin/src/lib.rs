pub mod app;
pub mod audition;
pub mod bridge;
pub mod controls;
#[cfg(target_os = "macos")]
mod chrome;
mod editor;
pub mod engine;
pub mod layout;
pub mod library;
pub mod macros;
pub mod minilab;
pub mod params;
pub mod presets;
pub mod ratings;

use audition::{Event, Events, Player};
use engine::{Engine, Transport, CHUNK};
use nih_plug::prelude::*;
use controls::Controls;
use library::Library;
use params::ThousandParams;
use std::num::NonZeroU32;
use std::sync::Arc;

pub struct Patina {
    params: Arc<ThousandParams>,
    controls: Arc<Controls>,
    engine: Engine,
    matrix_seen: u32,
    player: Player,
    phrase_events: Events,
    phrase_seen: u32,
    restored: bool,
}

impl Default for Patina {
    fn default() -> Self {
        audition::phrases();
        Self {
            params: Arc::new(ThousandParams::default()),
            controls: Arc::new(Controls::new(Library::open())),
            engine: Engine::new(48_000.0),
            matrix_seen: u32::MAX,
            player: Player::default(),
            phrase_events: Events::default(),
            phrase_seen: u32::MAX,
            restored: false,
        }
    }
}

pub enum Task {
    /// Apply MiniLab knob turns and preset pads on the GUI thread.
    Controls,
}

impl Patina {
    fn handle(&mut self, event: NoteEvent<()>) {
        match event {
            NoteEvent::NoteOn { note, velocity, .. } if self.controls.midi_note(note, velocity > 0.0) => {}
            NoteEvent::NoteOff { note, .. } if self.controls.midi_note(note, false) => {}
            NoteEvent::NoteOn { note, velocity, .. } if velocity > 0.0 => self.engine.note_on(note, velocity),
            NoteEvent::NoteOn { note, .. } | NoteEvent::NoteOff { note, .. } => self.engine.note_off(note),
            NoteEvent::MidiPitchBend { value, .. } => self.engine.set_pitch_bend(value),
            NoteEvent::MidiChannelPressure { pressure, .. } => self.engine.set_aftertouch(pressure),
            NoteEvent::MidiCC { cc, value, .. } if self.controls.midi_cc(cc, (value * 127.0).round() as u8) => {}
            NoteEvent::MidiCC { cc, value, .. } => match cc {
                1 => self.engine.set_mod_wheel(value),
                64 => self.engine.set_sustain(value >= 0.5),
                120 => self.engine.all_sound_off(),
                123 => self.engine.all_notes_off(),
                _ => {}
            },
            _ => {}
        }
    }

    /// Starts, restarts or stops the audition phrase, and queues its notes
    /// for this block.
    fn audition(&mut self, len: usize, tempo: Option<f64>) {
        let on = self.controls.audition();
        let generation = self.params.session_generation();
        let new_preset = generation != self.phrase_seen;
        self.phrase_seen = generation;
        let restart = self.controls.take_restart() || (on && new_preset);
        if restart || (!on && self.player.playing()) {
            let engine = &mut self.engine;
            self.player.stop(|key| engine.note_off(key));
            if on {
                self.player.start(self.params.phrase());
            }
        }
        self.player.block(len, self.engine.sample_rate(), tempo, &mut self.phrase_events);
    }

    fn play(&mut self, event: Event) {
        match event {
            Event::On(key, velocity) => self.engine.note_on(key, velocity),
            Event::Off(key) => self.engine.note_off(key),
        }
    }
}

impl Plugin for Patina {
    const NAME: &'static str = "Patina";
    const VENDOR: &'static str = "Dan Marino";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = Task;

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        Some(Box::new(editor::WebEditor::new(self.params.clone(), self.controls.clone())))
    }

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        let params = self.params.clone();
        let controls = self.controls.clone();
        Box::new(move |task| match task {
            Task::Controls => controls.run(&params),
        })
    }

    fn deactivate(&mut self) {
        self.controls.drop_context();
    }

    fn initialize(
        &mut self,
        _layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.controls.keep_context(|| context.gui_context());
        if app::get().is_some() && !std::mem::replace(&mut self.restored, true) {
            let gui = context.gui_context();
            let tempo = app::restore(&self.params, &mut bridge::ContextHost::new(&*gui, &self.params));
            self.controls.set_app_tempo(tempo.unwrap_or(controls::DEFAULT_TEMPO));
        }
        if self.engine.sample_rate() != buffer_config.sample_rate {
            self.engine = Engine::new(buffer_config.sample_rate);
        }
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if let Some(matrix) = self.params.matrix_if_changed(&mut self.matrix_seen) {
            self.engine.set_macros(matrix);
        }
        let t = context.transport();
        let transport = match self.controls.app_tempo() {
            // The app's transport is stopped at its own tempo: synced rates
            // follow it, phases run free.
            Some(bpm) => Transport { playing: false, tempo: Some(bpm as f64), pos_beats: None },
            None => Transport { playing: t.playing, tempo: t.tempo, pos_beats: t.pos_beats() },
        };
        self.engine.update(&*self.params, transport);
        self.engine.follow_transport(transport);

        let len = buffer.samples();
        let [left, right] = buffer.as_slice() else {
            return ProcessStatus::Normal;
        };
        self.audition(len, transport.tempo);
        let mut phrase = 0;
        let mut event = context.next_event();
        let mut pos = 0;
        while pos < len {
            while let Some(e) = event {
                if e.timing() as usize > pos {
                    break;
                }
                self.handle(e);
                event = context.next_event();
            }
            while let Some(&(at, e)) = self.phrase_events.as_slice().get(phrase) {
                if at as usize > pos {
                    break;
                }
                self.play(e);
                phrase += 1;
            }
            let mut end = (pos + CHUNK).min(len);
            if let Some(e) = &event {
                end = end.min(e.timing() as usize);
            }
            if let Some(&(at, _)) = self.phrase_events.as_slice().get(phrase) {
                end = end.min(at as usize);
            }
            self.engine.update(&*self.params, transport);
            self.engine.render(&mut left[pos..end], &mut right[pos..end]);
            pos = end;
        }
        if self.controls.take_pending() {
            context.execute_gui(Task::Controls);
        }
        ProcessStatus::Normal
    }
}

impl ClapPlugin for Patina {
    const CLAP_ID: &'static str = "com.danmarino.patina";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A polysynth with a curated bank of presets");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Stereo];
}

impl Vst3Plugin for Patina {
    const VST3_CLASS_ID: [u8; 16] = *b"PatinaSynthDnMrn";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(Patina);
nih_export_vst3!(Patina);
