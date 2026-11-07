//! The editor's message protocol, kept apart from the webview so it can be
//! tested and so a browser mock can be generated from it.

use crate::layout::PAGES;
use crate::library::Library;
use crate::macros::MACROS;
use crate::params::{Kind, Pid, PlainValues, ThousandParams, Values, COUNT};
use crate::presets::{Preset, Session, Source};
use nih_plug::prelude::FloatRange;
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

pub struct Bridge {
    params: Arc<ThousandParams>,
    library: Library,
    sent: [f32; COUNT],
    sent_session: Value,
    seen_generation: u32,
    /// The edit, put aside while compare shows the saved preset.
    stashed: Option<PlainValues>,
}

impl Bridge {
    pub fn new(params: Arc<ThousandParams>, library: Library) -> Self {
        Self { params, library, sent: [f32::NAN; COUNT], sent_session: Value::Null, seen_generation: u32::MAX, stashed: None }
    }

    pub fn handle(&mut self, msg: &Value, host: &mut impl Host, out: &mut Vec<Value>) {
        let pid = msg["id"].as_str().and_then(Pid::from_id);
        let key = msg["key"].as_str().unwrap_or_default();
        match (msg["type"].as_str().unwrap_or_default(), pid) {
            ("ready", _) => {
                self.sent = [f32::NAN; COUNT];
                self.sent_session = Value::Null;
                out.push(json!({ "type": "init", "params": describe_params(), "pages": describe_pages() }));
                out.push(self.presets_message());
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
                if let Some((preset, source)) = self.library.get(key) {
                    let preset = preset.clone();
                    apply(host, &preset.values());
                    self.stashed = None;
                    self.params.set_session(Session::from_preset(&preset, source));
                }
            }
            ("save", _) => self.save(msg, host, out),
            ("delete", _) => {
                let session = self.params.session();
                if self.library.delete(key).is_ok() {
                    if self.library.key_of(&session).is_none() && session.source == Source::User {
                        self.params.set_session(Session { source: Source::New, saved: None, ..session });
                    }
                    out.push(self.presets_message());
                }
            }
            ("favorite", _) => {
                if self.library.set_favorite(key, msg["on"].as_bool().unwrap_or(false)).is_ok() {
                    out.push(self.presets_message());
                }
            }
            ("compare", _) => self.compare(msg["on"].as_bool().unwrap_or(false), host),
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
        let generation = self.params.session_generation();
        if values_changed || generation != self.seen_generation {
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
        match self.library.save(&preset) {
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

    fn presets_message(&self) -> Value {
        json!({ "type": "presets", "presets": self.library.listing() })
    }

    fn session_message(&self, host: &impl Host) -> Value {
        let session = self.params.session();
        let comparing = self.stashed.is_some();
        let dirty = comparing
            || match &session.saved {
                Some(saved) => saved.differs_from(&current(host), &session.macros),
                None => true,
            };
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
            "key": self.library.key_of(&session),
            "dirty": dirty,
            "comparing": comparing,
            "macros": macros,
        })
    }
}

fn current(host: &impl Host) -> PlainValues {
    PlainValues(std::array::from_fn(|i| {
        let spec = Pid::ALL[i].spec();
        spec.denormalize(host.normalized(Pid::ALL[i]))
    }))
}

/// Sets every parameter that differs, each as its own gesture.
fn apply(host: &mut impl Host, values: &impl Values) {
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
        let root = std::env::temp_dir().join(format!("1000-synths-bridge-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let library = Library::at(root.join("User"), root.join("state.json"));
        (root, Bridge::new(Arc::new(ThousandParams::default()), library), FakeHost::new())
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
        assert_eq!(types, ["init", "presets", "values", "session"]);
        assert_eq!(out[0]["params"].as_array().unwrap().len(), COUNT);
        assert_eq!(out[2]["values"].as_array().unwrap().len(), COUNT);
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
