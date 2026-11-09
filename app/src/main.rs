//! Patina as a Mac app: the plugin run by nice-plug's standalone wrapper on
//! CoreAudio, with the MiniLab (or the chosen port) as MIDI input.

use cpal::traits::{DeviceTrait, HostTrait};
use midir::MidiInput;
use nice_plug::wrapper::standalone::nice_export_standalone_with_args;
use patina::app::{App, Devices};
use patina::Patina;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

const PERIOD: &str = "256";

#[derive(Default, Serialize, Deserialize)]
struct Settings {
    output: Option<String>,
    midi_input: Option<String>,
}

fn settings_path() -> PathBuf {
    patina::library::support_dir().join("app.json")
}

impl Settings {
    fn load() -> Self {
        std::fs::read_to_string(settings_path()).ok().and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default()
    }

    fn save(&self) {
        let path = settings_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn host() -> cpal::Host {
    cpal::host_from_id(cpal::HostId::CoreAudio).unwrap_or_else(|_| cpal::default_host())
}

/// The name nice-plug's standalone wrapper matches `--output-device` against.
fn name(device: &cpal::Device) -> Option<String> {
    device.description().ok().map(|d| d.name().to_string())
}

fn outputs() -> Vec<String> {
    host().output_devices().map(|ds| ds.filter_map(|d| name(&d)).collect()).unwrap_or_default()
}

fn midi_inputs() -> Vec<String> {
    let Ok(midi) = MidiInput::new("Patina devices") else { return Vec::new() };
    midi.ports().iter().filter_map(|p| midi.port_name(p).ok()).collect()
}

/// The saved choices where those devices still exist, else the system
/// output and the MiniLab (or the first MIDI port).
fn choose(settings: &Settings) -> Devices {
    let outputs = outputs();
    let midi_inputs = midi_inputs();
    let output = settings
        .output
        .clone()
        .filter(|o| outputs.contains(o))
        .or_else(|| host().default_output_device().and_then(|d| name(&d)));
    let midi_input = settings
        .midi_input
        .clone()
        .filter(|m| midi_inputs.contains(m))
        .or_else(|| midi_inputs.iter().find(|m| m.contains("MiniLab")).cloned())
        .or_else(|| midi_inputs.first().cloned());
    Devices { outputs, midi_inputs, output, midi_input }
}

fn sample_rate(output: Option<&str>) -> u32 {
    let device = host().output_devices().ok().and_then(|mut ds| ds.find(|d| name(d).as_deref() == output));
    let device = device.or_else(|| host().default_output_device());
    device.and_then(|d| d.default_output_config().ok()).map_or(48_000, |c| c.sample_rate())
}

/// The `.app` bundle this binary runs from, if any.
fn bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(Path::to_path_buf)
}

fn relaunch(output: Option<String>, midi_input: Option<String>) {
    Settings { output, midi_input }.save();
    let started = match bundle() {
        Some(app) => Command::new("open").arg("-n").arg(app).spawn(),
        None => std::env::current_exe().and_then(|exe| Command::new(exe).spawn()),
    };
    if started.is_ok() {
        std::process::exit(0);
    }
}

fn args(output: Option<&str>, midi_input: Option<&str>) -> Vec<String> {
    let mut args = vec!["Patina".to_string(), "--backend".into(), "core-audio".into(), "--period-size".into(), PERIOD.into()];
    args.extend(["--sample-rate".into(), sample_rate(output).to_string()]);
    if let Some(o) = output {
        args.extend(["--output-device".into(), o.to_string()]);
    }
    if let Some(m) = midi_input {
        args.extend(["--midi-input".into(), m.to_string()]);
    }
    args
}

fn main() {
    let chosen = choose(&Settings::load());
    let (output, midi_input) = (chosen.output.clone(), chosen.midi_input.clone());
    patina::app::install(App {
        devices: Box::new(move || Devices {
            outputs: outputs(),
            midi_inputs: midi_inputs(),
            output: output.clone(),
            midi_input: midi_input.clone(),
        }),
        relaunch: Box::new(relaunch),
    });
    // A device can refuse the settings (sample rate, period); then fall back
    // to the system output without MIDI, then to no audio at all, so the
    // window still opens and the devices can be changed.
    let attempts = [
        args(chosen.output.as_deref(), chosen.midi_input.as_deref()),
        args(None, None),
        vec!["Patina".into(), "--backend".into(), "dummy".into()],
    ];
    for attempt in attempts {
        if nice_export_standalone_with_args::<Patina, _>(attempt) {
            return;
        }
    }
}
