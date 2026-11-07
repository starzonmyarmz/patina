mod editor;
pub mod engine;
pub mod layout;
pub mod library;
pub mod macros;
pub mod params;
pub mod presets;

use engine::{Engine, Transport, CHUNK};
use nih_plug::prelude::*;
use params::ThousandParams;
use std::num::NonZeroU32;
use std::sync::Arc;

pub struct ThousandSynths {
    params: Arc<ThousandParams>,
    engine: Engine,
    matrix_seen: u32,
}

impl Default for ThousandSynths {
    fn default() -> Self {
        Self { params: Arc::new(ThousandParams::default()), engine: Engine::new(48_000.0), matrix_seen: u32::MAX }
    }
}

impl ThousandSynths {
    fn handle(&mut self, event: NoteEvent<()>) {
        match event {
            NoteEvent::NoteOn { note, velocity, .. } if velocity > 0.0 => self.engine.note_on(note, velocity),
            NoteEvent::NoteOn { note, .. } | NoteEvent::NoteOff { note, .. } => self.engine.note_off(note),
            NoteEvent::MidiPitchBend { value, .. } => self.engine.set_pitch_bend(value),
            NoteEvent::MidiChannelPressure { pressure, .. } => self.engine.set_aftertouch(pressure),
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
}

impl Plugin for ThousandSynths {
    const NAME: &'static str = "1000 Synths";
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
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        Some(Box::new(editor::WebEditor::new(self.params.clone())))
    }

    fn initialize(
        &mut self,
        _layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
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
        let transport = Transport { playing: t.playing, tempo: t.tempo, pos_beats: t.pos_beats() };
        self.engine.update(&*self.params, transport);
        self.engine.follow_transport(transport);

        let len = buffer.samples();
        let [left, right] = buffer.as_slice() else {
            return ProcessStatus::Normal;
        };
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
            let mut end = (pos + CHUNK).min(len);
            if let Some(e) = &event {
                end = end.min(e.timing() as usize);
            }
            self.engine.update(&*self.params, transport);
            self.engine.render(&mut left[pos..end], &mut right[pos..end]);
            pos = end;
        }
        ProcessStatus::Normal
    }
}

impl ClapPlugin for ThousandSynths {
    const CLAP_ID: &'static str = "com.danmarino.1000-synths";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A polysynth with a curated bank of presets");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Stereo];
}

impl Vst3Plugin for ThousandSynths {
    const VST3_CLASS_ID: [u8; 16] = *b"1000SynthsDanMrn";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(ThousandSynths);
nih_export_vst3!(ThousandSynths);
