//! The editor's message protocol, kept apart from the webview so it can be
//! tested and so a browser mock can be generated from it.

use crate::layout::PAGES;
use crate::controls::Controls;
use crate::library::Library;
use crate::minilab::KnobMode;
use crate::macros::MACROS;
use crate::params::{Kind, Pid, PlainValues, ThousandParams, Values, COUNT};
use crate::presets::{Preset, Session, Source};
use crate::ratings::Verdict;
use nih_plug::prelude::{FloatRange, GuiContext};
use serde_json::{json, Value};
use std::sync::Arc;

/// Where the bridge reads and writes parameter values: the GUI context in the
/// plugin, a plain array in tests.
pub trait Host {
    /// Unmodulated normalized value.
    fn normalized(&self, pid: Pid) -> f32;
    fn begin(&mut self, pid: Pid);
    fn set(&mut self, pid: Pid, normalized: f32);
    fn end(&mut self, pid: Pid);
}

/// The host's GUI context: sets go to the host, which records them as
/// automation and passes them back to the plugin.
pub struct ContextHost<'a> {
    context: &'a dyn GuiContext,
    params: &'a ThousandParams,
}

impl<'a> ContextHost<'a> {
    pub fn new(context: &'a dyn GuiContext, params: &'a ThousandParams) -> Self {
        Self { context, params }
    }
}

impl Host for ContextHost<'_> {
    fn normalized(&self, pid: Pid) -> f32 {
        self.params.normalized(pid)
    }
    fn begin(&mut self, pid: Pid) {
        unsafe { self.context.raw_begin_set_parameter(self.params.ptr(pid)) }
    }
    fn set(&mut self, pid: Pid, normalized: f32) {
        unsafe { self.context.raw_set_parameter_normalized(self.params.ptr(pid), normalized) }
    }
    fn end(&mut self, pid: Pid) {
        unsafe { self.context.raw_end_set_parameter(self.params.ptr(pid)) }
    }
}

/// Applies the preset under `key` and makes it the session.
pub fn load_preset(params: &ThousandParams, library: &Library, key: &str, host: &mut impl Host) -> bool {
    let Some((preset, source)) = library.get(key) else {
        return false;
    };
    apply(host, &preset.values());
    params.set_session(Session::from_preset(preset, source));
    true
}

pub struct Bridge {
    params: Arc<ThousandParams>,
    controls: Arc<Controls>,
    sent: [f32; COUNT],
    sent_session: Value,
    sent_page: usize,
    seen_generation: u32,
    sent_audition: Option<bool>,
    sent_tempo: Option<f32>,
    seen_ratings: u32,
    /// The edit, put aside while compare shows the saved preset.
    stashed: Option<PlainValues>,
}

impl Bridge {
    pub fn new(params: Arc<ThousandParams>, controls: Arc<Controls>) -> Self {
        Self {
            params,
            controls,
            sent: [f32::NAN; COUNT],
            sent_session: Value::Null,
            sent_page: usize::MAX,
            seen_generation: u32::MAX,
            sent_audition: None,
            sent_tempo: None,
            seen_ratings: 0,
            stashed: None,
        }
    }

    pub fn handle(&mut self, msg: &Value, host: &mut impl Host, out: &mut Vec<Value>) {
        let pid = msg["id"].as_str().and_then(Pid::from_id);
        let key = msg["key"].as_str().unwrap_or_default();
        match (msg["type"].as_str().unwrap_or_default(), pid) {
            ("ready", _) => {
                self.sent = [f32::NAN; COUNT];
                self.sent_session = Value::Null;
                self.sent_page = usize::MAX;
                self.sent_audition = None;
                self.sent_tempo = None;
                self.seen_ratings = self.controls.ratings_generation();
                out.push(json!({ "type": "init", "params": describe_params(), "pages": describe_pages() }));
                out.push(self.presets_message());
                out.push(self.settings_message());
                out.extend(devices_message());
            }
            ("devices", _) => out.extend(devices_message()),
            ("switchDevices", _) => {
                if let Some(app) = crate::app::get() {
                    crate::app::save(&self.params, self.controls.app_tempo(), host);
                    let name = |key: &str| msg[key].as_str().map(String::from);
                    (app.relaunch)(name("output"), name("midiInput"));
                }
            }
            ("quit", _) => {
                if crate::app::get().is_some() {
                    crate::app::save(&self.params, self.controls.app_tempo(), host);
                    std::process::exit(0);
                }
            }
            ("tempo", _) => {
                if self.controls.app_tempo().is_some() {
                    self.controls.set_app_tempo(msg["bpm"].as_f64().unwrap_or(120.0) as f32);
                }
            }
            ("page", _) => self.controls.set_page(msg["index"].as_u64().unwrap_or(0) as usize),
            ("order", _) => {
                let keys = msg["keys"].as_array().map(|k| k.iter().filter_map(|k| k.as_str().map(String::from)).collect());
                self.controls.set_order(keys.unwrap_or_default());
            }
            ("knobMode", _) => {
                let mode = KnobMode::from_u8(msg["mode"].as_u64().unwrap_or(1) as u8);
                self.controls.set_knob_mode(mode);
                out.push(self.settings_message());
            }
            ("begin", Some(pid)) => host.begin(pid),
            ("set", Some(pid)) => {
                if let Some(v) = msg["value"].as_f64() {
                    // Editing while comparing keeps the saved version as the
                    // starting point and forgets the stashed edit.
                    self.stashed = None;
                    host.set(pid, v.clamp(0.0, 1.0) as f32);
                }
            }
            ("end", Some(pid)) => host.end(pid),
            ("load", _) => {
                if load_preset(&self.params, &self.controls.library(), key, host) {
                    self.stashed = None;
                }
            }
            ("save", _) => self.save(msg, host, out),
            ("delete", _) => {
                let session = self.params.session();
                let deleted = self.controls.library().delete(key).is_ok();
                if deleted {
                    if self.controls.library().key_of(&session).is_none() && session.source == Source::User {
                        self.params.set_session(Session { source: Source::New, saved: None, ..session });
                    }
                    out.push(self.presets_message());
                }
            }
            ("favorite", _) => {
                let on = msg["on"].as_bool().unwrap_or(false);
                let saved = self.controls.library().set_favorite(key, on).is_ok();
                if saved {
                    out.push(self.presets_message());
                }
            }
            ("compare", _) => self.compare(msg["on"].as_bool().unwrap_or(false), host),
            ("audition", _) => self.controls.set_audition(msg["on"].as_bool().unwrap_or(false)),
            ("rate", _) => {
                if let Some(edit) = self.stashed.take() {
                    apply(host, &edit);
                }
                let verdict = msg["verdict"].as_str().and_then(Verdict::from_name);
                if let Err(message) = self.controls.rate(&self.params, verdict, host) {
                    out.push(json!({ "type": "error", "message": message }));
                }
            }
            ("replay", _) => self.controls.replay(),
            ("nextUnrated", _) => self.controls.next_unrated(&self.params, host),
            _ => {}
        }
        self.poll(host, true, out);
    }

    /// Sends parameter values that moved since last time (when
    /// `values_changed`) and the session when it changed.
    pub fn poll(&mut self, host: &impl Host, values_changed: bool, out: &mut Vec<Value>) {
        if values_changed {
            let mut changes = Vec::new();
            for &pid in Pid::ALL {
                let value = host.normalized(pid);
                let last = &mut self.sent[pid as usize];
                if *last != value {
                    *last = value;
                    changes.push(json!({ "id": pid.spec().id, "value": value, "text": self.params.display(pid, value) }));
                }
            }
            if !changes.is_empty() {
                out.push(json!({ "type": "values", "values": changes }));
            }
        }
        let audition = self.controls.audition();
        if self.sent_audition != Some(audition) {
            self.sent_audition = Some(audition);
            out.push(json!({ "type": "audition", "on": audition }));
        }
        let tempo = self.controls.app_tempo();
        if tempo.is_some() && tempo != self.sent_tempo {
            self.sent_tempo = tempo;
            out.push(json!({ "type": "tempo", "bpm": tempo }));
        }
        let ratings = self.controls.ratings_generation();
        if ratings != self.seen_ratings {
            self.seen_ratings = ratings;
            out.push(self.presets_message());
        }
        let page = self.controls.page();
        if page != self.sent_page {
            self.sent_page = page;
            out.push(json!({ "type": "page", "index": page }));
        }
        let generation = self.params.session_generation();
        if values_changed || generation != self.seen_generation {
            if generation != self.seen_generation && self.seen_generation != u32::MAX {
                // Another preset arrived (a pad, a project load): the stashed
                // edit belonged to the old one.
                self.stashed = None;
            }
            self.seen_generation = generation;
            let session = self.session_message(host);
            if session != self.sent_session {
                self.sent_session = session.clone();
                out.push(session);
            }
        }
    }

    fn save(&mut self, msg: &Value, host: &mut impl Host, out: &mut Vec<Value>) {
        let name = msg["name"].as_str().unwrap_or_default().trim();
        if name.is_empty() {
            return;
        }
        let session = self.params.session();
        let category = msg["category"].as_str().filter(|c| !c.trim().is_empty()).unwrap_or(&session.category);
        let values = match self.stashed.take() {
            Some(edit) => {
                apply(host, &edit);
                edit
            }
            None => current(host),
        };
        let mut preset = Preset::capture(name, category.trim(), &values, &session.macros);
        preset.author = "user".into();
        preset.tags = session.saved.map(|p| p.tags).unwrap_or_default();
        let saved = self.controls.library().save(&preset);
        match saved {
            Ok(_) => {
                self.params.set_session(Session::from_preset(&preset, Source::User));
                out.push(self.presets_message());
            }
            Err(e) => out.push(json!({ "type": "error", "message": format!("Couldn’t save: {e}") })),
        }
    }

    fn compare(&mut self, on: bool, host: &mut impl Host) {
        let saved = self.params.session().saved;
        match (on, self.stashed.take(), saved) {
            (true, None, Some(saved)) => {
                let edit = current(host);
                if saved.differs_from(&edit, &self.params.session().macros) {
                    apply(host, &saved.values());
                    self.stashed = Some(edit);
                }
            }
            (false, Some(edit), _) => apply(host, &edit),
            (_, stashed, _) => self.stashed = stashed,
        }
    }

    pub fn tempo(&self) -> Option<f32> {
        self.controls.app_tempo()
    }

    fn presets_message(&self) -> Value {
        json!({ "type": "presets", "presets": self.controls.library().listing() })
    }

    fn settings_message(&self) -> Value {
        json!({ "type": "settings", "knobMode": self.controls.knob_mode() as u8 })
    }

    fn session_message(&self, host: &impl Host) -> Value {
        let session = self.params.session();
        let comparing = self.stashed.is_some();
        let dirty = comparing
            || match &session.saved {
                Some(saved) => saved.differs_from(&current(host), &session.macros),
                None => true,
            };
        let key = self.controls.library().key_of(&session);
        let rating = key.as_deref().and_then(|k| self.controls.library().rating(k).map(|r| r.verdict.name()));
        let macros: Vec<&str> = (0..MACROS).map(|i| session.macros.get(i).map_or("", |m| m.name.as_str())).collect();
        let source = match session.source {
            Source::New => "new",
            Source::Factory => "factory",
            Source::User => "user",
        };
        json!({
            "type": "session",
            "name": session.name,
            "category": session.category,
            "source": source,
            "key": key,
            "rating": rating,
            "dirty": dirty,
            "comparing": comparing,
            "macros": macros,
        })
    }
}

pub fn current(host: &impl Host) -> PlainValues {
    PlainValues(std::array::from_fn(|i| {
        let spec = Pid::ALL[i].spec();
        spec.denormalize(host.normalized(Pid::ALL[i]))
    }))
}

fn devices_message() -> Option<Value> {
    let d = (crate::app::get()?.devices)();
    Some(json!({
        "type": "devices",
        "outputs": d.outputs,
        "midiInputs": d.midi_inputs,
        "output": d.output,
        "midiInput": d.midi_input,
    }))
}

/// Sets every parameter that differs, each as its own gesture.
pub fn apply(host: &mut impl Host, values: &impl Values) {
    for &pid in Pid::ALL {
        let target = pid.spec().normalize(values.get(pid));
        if (host.normalized(pid) - target).abs() > 1e-6 {
            host.begin(pid);
            host.set(pid, target);
            host.end(pid);
        }
    }
}

pub fn describe_params() -> Vec<Value> {
    Pid::ALL
        .iter()
        .map(|&pid| {
            let spec = pid.spec();
            let (kind, options, min, max, factor) = match spec.kind {
                Kind::Float { min, max, skew } => {
                    ("float", Value::Null, min, max, if skew == 0.0 { 1.0 } else { FloatRange::skew_factor(skew) })
                }
                Kind::Int { min, max } => ("int", Value::Null, min as f32, max as f32, 1.0),
                Kind::Enum(names) => ("enum", json!(names), 0.0, names.len() as f32 - 1.0, 1.0),
                Kind::Bool => ("bool", Value::Null, 0.0, 1.0, 1.0),
            };
            let steps = match spec.kind {
                Kind::Float { .. } => 0.0,
                _ => max - min,
            };
            json!({
                "id": spec.id, "name": spec.name, "group": spec.group, "kind": kind,
                "options": options, "steps": steps, "min": min, "max": max, "factor": factor,
                "unit": spec.unit.trim(), "default": spec.normalize(spec.default),
            })
        })
        .collect()
}

pub fn describe_pages() -> Vec<Value> {
    PAGES
        .iter()
        .map(|page| {
            let rows: Vec<Value> = page
                .rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|s| {
                            let knobs: Vec<Value> =
                                s.knobs.iter().map(|(pid, label)| json!({ "id": pid.spec().id, "label": label })).collect();
                            json!({ "title": s.title, "knobs": knobs })
                        })
                        .collect()
                })
                .collect();
            json!({ "name": page.name, "rows": rows })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Applies sets immediately, like a host that isn't processing audio.
    struct FakeHost {
        values: [f32; COUNT],
        gestures: usize,
    }

    impl FakeHost {
        fn new() -> Self {
            Self { values: std::array::from_fn(|i| Pid::ALL[i].spec().normalize(Pid::ALL[i].spec().default)), gestures: 0 }
        }
    }

    impl Host for FakeHost {
        fn normalized(&self, pid: Pid) -> f32 {
            self.values[pid as usize]
        }
        fn begin(&mut self, _: Pid) {
            self.gestures += 1;
        }
        fn set(&mut self, pid: Pid, normalized: f32) {
            self.values[pid as usize] = normalized;
        }
        fn end(&mut self, _: Pid) {}
    }

    fn setup(tag: &str) -> (PathBuf, Bridge, FakeHost) {
        let root = std::env::temp_dir().join(format!("patina-bridge-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let library = Library::at(root.join("User"), root.join("state.json"));
        (root, Bridge::new(Arc::new(ThousandParams::default()), Arc::new(Controls::new(library))), FakeHost::new())
    }

    fn send(bridge: &mut Bridge, host: &mut FakeHost, msg: Value) -> Vec<Value> {
        let mut out = Vec::new();
        bridge.handle(&msg, host, &mut out);
        out
    }

    fn session(out: &[Value]) -> &Value {
        out.iter().rev().find(|m| m["type"] == "session").expect("a session message")
    }

    fn factory_key(i: usize) -> String {
        format!("factory/{}", crate::presets::factory()[i].name)
    }

    #[test]
    fn ready_sends_everything_the_page_needs() {
        let (root, mut bridge, mut host) = setup("ready");
        let out = send(&mut bridge, &mut host, json!({ "type": "ready" }));
        let types: Vec<_> = out.iter().map(|m| m["type"].as_str().unwrap()).collect();
        assert_eq!(types, ["init", "presets", "settings", "values", "audition", "page", "session"]);
        assert_eq!(out[0]["params"].as_array().unwrap().len(), COUNT);
        assert_eq!(out[3]["values"].as_array().unwrap().len(), COUNT);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rating_in_audition_moves_on_and_resends_presets() {
        let (root, mut bridge, mut host) = setup("rate");
        send(&mut bridge, &mut host, json!({ "type": "ready" }));
        let out = send(&mut bridge, &mut host, json!({ "type": "rate", "verdict": "keep" }));
        assert_eq!(out[0]["type"], "error");

        let out = send(&mut bridge, &mut host, json!({ "type": "audition", "on": true }));
        assert!(out.contains(&json!({ "type": "audition", "on": true })));
        let first = bridge.controls.library().keys()[0].clone();
        send(&mut bridge, &mut host, json!({ "type": "load", "key": first }));
        let out = send(&mut bridge, &mut host, json!({ "type": "rate", "verdict": "cut" }));
        let presets = out.iter().find(|m| m["type"] == "presets").expect("a presets message");
        let listed = presets["presets"].as_array().unwrap().iter().find(|p| p["key"] == first.as_str()).unwrap();
        assert_eq!(listed["rating"], "cut");
        assert_ne!(session(&out)["key"], first.as_str());

        send(&mut bridge, &mut host, json!({ "type": "load", "key": first }));
        let out = send(&mut bridge, &mut host, json!({ "type": "rate", "verdict": null }));
        assert_eq!(session(&out)["rating"], Value::Null);
        assert_eq!(session(&out)["key"], first.as_str());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn tempo_is_only_the_apps() {
        let (root, mut bridge, mut host) = setup("tempo");
        let out = send(&mut bridge, &mut host, json!({ "type": "ready" }));
        assert!(!out.iter().any(|m| m["type"] == "tempo"));
        send(&mut bridge, &mut host, json!({ "type": "tempo", "bpm": 90 }));
        assert_eq!(bridge.tempo(), None);

        bridge.controls.set_app_tempo(120.0);
        let out = send(&mut bridge, &mut host, json!({ "type": "tempo", "bpm": 999 }));
        assert!(out.contains(&json!({ "type": "tempo", "bpm": 240.0 })));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn loading_sets_parameters_and_session() {
        let (root, mut bridge, mut host) = setup("load");
        let key = factory_key(5);
        let out = send(&mut bridge, &mut host, json!({ "type": "load", "key": key }));
        let preset = &crate::presets::factory()[5];
        let s = session(&out);
        assert_eq!(s["name"], preset.name.as_str());
        assert_eq!(s["key"], key.as_str());
        assert_eq!(s["dirty"], false);
        assert_eq!(s["macros"][0], preset.macros[0].name.as_str());
        assert!(host.gestures > 0);
        assert!(!preset.differs_from(&current(&host), &preset.macros));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn edits_mark_dirty_and_compare_round_trips() {
        let (root, mut bridge, mut host) = setup("compare");
        send(&mut bridge, &mut host, json!({ "type": "load", "key": factory_key(0) }));
        let before = host.normalized(Pid::Cutoff);
        let edited = (before + 0.2) % 1.0;
        let out = send(&mut bridge, &mut host, json!({ "type": "set", "id": "flt_cutoff", "value": edited }));
        assert_eq!(session(&out)["dirty"], true);

        let out = send(&mut bridge, &mut host, json!({ "type": "compare", "on": true }));
        assert_eq!(session(&out)["comparing"], true);
        assert_eq!(host.normalized(Pid::Cutoff), before);

        let out = send(&mut bridge, &mut host, json!({ "type": "compare", "on": false }));
        assert_eq!(session(&out)["comparing"], false);
        assert_eq!(host.normalized(Pid::Cutoff), edited);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn compare_without_edits_does_nothing() {
        let (root, mut bridge, mut host) = setup("compare-clean");
        send(&mut bridge, &mut host, json!({ "type": "load", "key": factory_key(1) }));
        let mut out = Vec::new();
        bridge.poll(&host, true, &mut out);
        let out = send(&mut bridge, &mut host, json!({ "type": "compare", "on": true }));
        assert!(out.iter().all(|m| m["type"] != "session"), "nothing changed: {out:?}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn saving_from_compare_keeps_the_edit() {
        let (root, mut bridge, mut host) = setup("save");
        send(&mut bridge, &mut host, json!({ "type": "load", "key": factory_key(2) }));
        send(&mut bridge, &mut host, json!({ "type": "set", "id": "flt_res", "value": 0.9 }));
        send(&mut bridge, &mut host, json!({ "type": "compare", "on": true }));
        let out = send(&mut bridge, &mut host, json!({ "type": "save", "name": "My Pad", "category": "" }));
        assert!((host.normalized(Pid::Resonance) - 0.9).abs() < 1e-6);
        let s = session(&out);
        assert_eq!((s["name"].as_str(), s["source"].as_str(), s["dirty"].as_bool()), (Some("My Pad"), Some("user"), Some(false)));
        assert_eq!(s["category"], crate::presets::factory()[2].category.as_str());
        let listed = out.iter().find(|m| m["type"] == "presets").unwrap();
        assert!(listed["presets"].as_array().unwrap().iter().any(|p| p["key"] == "user/My Pad"));

        let out = send(&mut bridge, &mut host, json!({ "type": "delete", "key": "user/My Pad" }));
        assert_eq!(session(&out)["source"], "new");
        assert_eq!(session(&out)["dirty"], true);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pages_sync_both_ways() {
        let (root, mut bridge, mut host) = setup("pages");
        send(&mut bridge, &mut host, json!({ "type": "ready" }));
        let out = send(&mut bridge, &mut host, json!({ "type": "page", "index": 3 }));
        assert!(out.iter().any(|m| m == &json!({ "type": "page", "index": 3 })));
        bridge.controls.midi_cc(20 + 1, 127);
        let mut out = Vec::new();
        bridge.poll(&host, false, &mut out);
        assert_eq!(out, [json!({ "type": "page", "index": 1 })]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_new_preset_from_elsewhere_ends_compare() {
        let (root, mut bridge, mut host) = setup("pad-load");
        send(&mut bridge, &mut host, json!({ "type": "load", "key": factory_key(0) }));
        send(&mut bridge, &mut host, json!({ "type": "set", "id": "flt_res", "value": 0.95 }));
        send(&mut bridge, &mut host, json!({ "type": "compare", "on": true }));
        let library = bridge.controls.library().keys();
        load_preset(&bridge.params, &bridge.controls.library(), &library[4], &mut host);
        let mut out = Vec::new();
        bridge.poll(&host, true, &mut out);
        assert_eq!(session(&out)["comparing"], false);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn every_page_knob_is_a_described_param() {
        let ids: Vec<_> = describe_params().iter().map(|p| p["id"].as_str().unwrap().to_string()).collect();
        for page in describe_pages() {
            for row in page["rows"].as_array().unwrap() {
                for section in row.as_array().unwrap() {
                    for knob in section["knobs"].as_array().unwrap() {
                        assert!(ids.contains(&knob["id"].as_str().unwrap().to_string()));
                    }
                }
            }
        }
    }
}
