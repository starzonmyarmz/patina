//! Hooks for the standalone app. The plugin never sets them, so none of this
//! runs inside a DAW.

use crate::bridge::{apply, current, Host};
use crate::params::ThousandParams;
use crate::presets::{Preset, Session};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Devices {
    pub outputs: Vec<String>,
    pub midi_inputs: Vec<String>,
    pub output: Option<String>,
    pub midi_input: Option<String>,
}

pub struct App {
    /// Lists devices now, so ones plugged in after launch show up.
    pub devices: Box<dyn Fn() -> Devices + Send + Sync>,
    /// Saves the choice and starts a new instance; the caller then quits.
    pub relaunch: Box<dyn Fn(Option<String>, Option<String>) + Send + Sync>,
}

static APP: OnceLock<App> = OnceLock::new();

pub fn install(app: App) {
    let _ = APP.set(app);
}

pub fn get() -> Option<&'static App> {
    APP.get()
}

/// The sound on screen when the app last saved: the session and the
/// current values, which may differ from the session's saved preset.
#[derive(Serialize, Deserialize)]
struct Snapshot {
    session: Session,
    current: Preset,
}

fn snapshot_path() -> PathBuf {
    crate::library::support_dir().join("app-state.json")
}

pub fn save(params: &ThousandParams, host: &impl Host) {
    let session = params.session();
    let current = Preset::capture(&session.name, &session.category, &current(host), &session.macros);
    let Ok(json) = serde_json::to_string_pretty(&Snapshot { session, current }) else { return };
    let path = snapshot_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// Puts back the last saved sound. Returns false when there was none.
pub fn restore(params: &ThousandParams, host: &mut impl Host) -> bool {
    let Some(snapshot) = std::fs::read_to_string(snapshot_path())
        .ok()
        .and_then(|json| serde_json::from_str::<Snapshot>(&json).ok())
    else {
        return false;
    };
    apply(host, &snapshot.current.values());
    params.set_session(snapshot.session);
    true
}
