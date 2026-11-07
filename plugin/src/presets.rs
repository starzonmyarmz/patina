use crate::macros::{MacroMatrix, Target, MACROS, TARGETS_PER_MACRO};
use crate::params::{Kind, Pid, PlainValues, Values};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const FORMAT: u32 = 1;
const FACTORY_JSON: &str = include_str!("../../presets/factory.json");

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroTarget {
    pub param: String,
    pub amount: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MacroDef {
    pub name: String,
    #[serde(default)]
    pub targets: Vec<MacroTarget>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub format: u32,
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub author: String,
    /// Plain values by parameter ID: numbers, enum names as strings, and
    /// booleans. Missing IDs take the default.
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    #[serde(default)]
    pub macros: Vec<MacroDef>,
}

#[derive(Deserialize, Serialize)]
pub struct Bank {
    pub format: u32,
    pub presets: Vec<Preset>,
}

impl Preset {
    pub fn capture(name: &str, category: &str, values: &impl Values, macros: &[MacroDef]) -> Self {
        let params = Pid::ALL.iter().map(|&pid| (pid.spec().id.to_string(), to_json(pid, values.get(pid)))).collect();
        Self {
            format: FORMAT,
            name: name.to_string(),
            category: category.to_string(),
            tags: Vec::new(),
            author: String::new(),
            params,
            macros: macros.to_vec(),
        }
    }

    pub fn values(&self) -> PlainValues {
        let mut v = PlainValues::default();
        for (id, value) in &self.params {
            if let Some(pid) = Pid::from_id(id) {
                if let Some(plain) = from_json(pid, value) {
                    v.set(pid, pid.spec().clamp(plain));
                }
            }
        }
        v
    }

    pub fn matrix(&self) -> MacroMatrix {
        matrix_from(&self.macros)
    }

    pub fn macro_names(&self) -> [String; MACROS] {
        std::array::from_fn(|i| self.macros.get(i).map(|m| m.name.clone()).unwrap_or_default())
    }

    /// Sets every parameter, including ones the preset leaves at default.
    pub fn apply(&self, mut set: impl FnMut(Pid, f32)) {
        let values = self.values();
        for &pid in Pid::ALL {
            set(pid, values.get(pid));
        }
    }

    /// True when `current` sounds different from this preset.
    pub fn differs_from(&self, current: &impl Values, macros: &[MacroDef]) -> bool {
        let saved = self.values();
        let moved = Pid::ALL.iter().any(|&pid| {
            let spec = pid.spec();
            (spec.normalize(saved.get(pid)) - spec.normalize(current.get(pid))).abs() > 1e-4
        });
        moved || self.macros != macros
    }
}

pub fn matrix_from(macros: &[MacroDef]) -> MacroMatrix {
    let mut m = MacroMatrix::default();
    for (slot, def) in m.0.iter_mut().zip(macros) {
        let targets = def.targets.iter().filter_map(|t| {
            Pid::from_id(&t.param).map(|pid| Target { pid, amount: t.amount.clamp(-1.0, 1.0) })
        });
        for (s, t) in slot.iter_mut().zip(targets.take(TARGETS_PER_MACRO)) {
            *s = Some(t);
        }
    }
    m
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    #[default]
    New,
    Factory,
    User,
}

/// What's loaded, saved with the DAW project next to the parameter values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub name: String,
    pub category: String,
    pub source: Source,
    pub macros: Vec<MacroDef>,
    /// The preset as last loaded or saved, for the unsaved-changes dot and
    /// for compare.
    pub saved: Option<Preset>,
}

impl Session {
    pub fn from_preset(preset: &Preset, source: Source) -> Self {
        Self {
            name: preset.name.clone(),
            category: preset.category.clone(),
            source,
            macros: preset.macros.clone(),
            saved: Some(preset.clone()),
        }
    }
}

fn to_json(pid: Pid, plain: f32) -> Value {
    match pid.spec().kind {
        Kind::Enum(names) => Value::from(names[(plain as usize).min(names.len() - 1)]),
        Kind::Bool => Value::from(plain >= 0.5),
        Kind::Int { .. } => Value::from(plain.round() as i64),
        Kind::Float { .. } => Value::from(((plain as f64) * 1000.0).round() / 1000.0),
    }
}

fn from_json(pid: Pid, value: &Value) -> Option<f32> {
    match (&pid.spec().kind, value) {
        (Kind::Enum(names), Value::String(s)) => names.iter().position(|n| n.eq_ignore_ascii_case(s)).map(|i| i as f32),
        (_, Value::Bool(b)) => Some(*b as u8 as f32),
        (_, Value::Number(n)) => n.as_f64().map(|f| f as f32),
        _ => None,
    }
}

pub fn factory() -> &'static [Preset] {
    static BANK: OnceLock<Vec<Preset>> = OnceLock::new();
    BANK.get_or_init(|| serde_json::from_str::<Bank>(FACTORY_JSON).map(|b| b.presets).unwrap_or_default())
}

pub fn user_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join("Library/Audio/Presets/Patina/User")
}

/// Every readable preset in `dir`, sorted by name. Unreadable files are skipped.
pub fn load_dir(dir: &Path) -> Vec<(PathBuf, Preset)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut presets: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| {
            let preset = serde_json::from_str::<Preset>(&std::fs::read_to_string(&p).ok()?).ok()?;
            Some((p, preset))
        })
        .collect();
    presets.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));
    presets
}

/// Writes `preset` into `dir` as `<name>.json`, replacing a preset with the
/// same name.
pub fn save_to(dir: &Path, preset: &Preset) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.json", file_stem(&preset.name)));
    let json = serde_json::to_string_pretty(preset).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

pub(crate) fn file_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_()'".contains(c) { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Untitled".into()
    } else {
        trimmed.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Preset {
        let mut v = PlainValues::default();
        v.set(Pid::Osc1Type, 6.0).set(Pid::Cutoff, 800.0).set(Pid::RingMod, 1.0).set(Pid::Unison, 5.0);
        let macros = vec![MacroDef {
            name: "Bloom".into(),
            targets: vec![
                MacroTarget { param: "flt_cutoff".into(), amount: 0.3 },
                MacroTarget { param: "no_such_param".into(), amount: 1.0 },
            ],
        }];
        Preset::capture("Wide/Saw: 1", "Pad", &v, &macros)
    }

    #[test]
    fn capture_round_trips_through_json() {
        let p = sample();
        let back: Preset = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
        assert_eq!(back.params["osc1_type"], Value::from("Supersaw"));
        assert_eq!(back.params["ring_mod"], Value::from(true));
        let v = back.values();
        assert_eq!(v.get(Pid::Osc1Type), 6.0);
        assert!((v.get(Pid::Cutoff) - 800.0).abs() < 0.01);
        assert!(!back.differs_from(&v, &back.macros));
    }

    #[test]
    fn missing_and_bad_values_fall_back() {
        let json = r#"{"format":1,"name":"x","category":"Bass","params":{"flt_res":250,"osc2_type":"Kazoo","unison":"lots"}}"#;
        let p: Preset = serde_json::from_str(json).unwrap();
        let v = p.values();
        assert_eq!(v.get(Pid::Resonance), 100.0);
        assert_eq!(v.get(Pid::Osc2Type), Pid::Osc2Type.spec().default);
        assert_eq!(v.get(Pid::Unison), Pid::Unison.spec().default);
        assert_eq!(v.get(Pid::Volume), Pid::Volume.spec().default);
    }

    #[test]
    fn matrix_skips_unknown_params() {
        let m = sample().matrix();
        assert_eq!(m.0[0][0], Some(Target { pid: Pid::Cutoff, amount: 0.3 }));
        assert_eq!(m.0[0][1], None);
    }

    #[test]
    fn edits_are_detected() {
        let p = sample();
        let mut v = p.values();
        v.set(Pid::Resonance, 70.0);
        assert!(p.differs_from(&v, &p.macros));
        assert!(p.differs_from(&p.values(), &[]));
    }

    #[test]
    fn save_and_load_user_presets() {
        let dir = std::env::temp_dir().join(format!("patina-test-{}", std::process::id()));
        let p = sample();
        let path = save_to(&dir, &p).unwrap();
        assert_eq!(path.file_name().unwrap(), "Wide_Saw_ 1.json");
        save_to(&dir, &p).unwrap();
        std::fs::write(dir.join("broken.json"), "{").unwrap();
        let loaded = load_dir(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].1, p);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn factory_bank_parses() {
        let bank: Bank = serde_json::from_str(FACTORY_JSON).unwrap();
        assert_eq!(bank.presets.len(), factory().len());
        for p in factory() {
            assert_eq!(p.format, FORMAT, "{}", p.name);
            for id in p.params.keys() {
                assert!(Pid::from_id(id).is_some(), "{}: unknown param {id}", p.name);
            }
        }
    }
}
