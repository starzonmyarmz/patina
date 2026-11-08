//! Talks to the MiniLab's MIDI input directly rather than through the host,
//! so the lights work in VST3 and need no routing in the DAW.

use crate::controls::Controls;
use crate::minilab::{lights, pad_color, Color, SysEx, LIGHTS};
use crate::params::ThousandParams;
use crate::ratings::Verdict;
use midir::{MidiOutput, MidiOutputConnection};
use std::sync::Weak;
use std::time::{Duration, Instant};

const DEVICE: &str = "MiniLab";
const POLL: Duration = Duration::from_millis(30);
const RECONNECT: Duration = Duration::from_secs(2);

fn connect() -> Option<MidiOutputConnection> {
    let out = MidiOutput::new("Patina").ok()?;
    let port = out.ports().into_iter().find(|p| out.port_name(p).is_ok_and(|n| n.contains(DEVICE)))?;
    out.connect(&port, "Patina lights").ok()
}

fn turn_off(conn: &mut MidiOutputConnection) {
    for i in 0..LIGHTS {
        let _ = conn.send(&pad_color(i, Color::Off));
    }
}

/// For the app, which exits without dropping the plugin.
pub fn turn_off_now() {
    if let Some(mut conn) = connect() {
        turn_off(&mut conn);
    }
}

/// Runs until the plugin goes away, then turns the lights off.
pub fn spawn(params: Weak<ThousandParams>, controls: Weak<Controls>) {
    let _ = std::thread::Builder::new().name("Patina lights".into()).spawn(move || {
        let mut connection: Option<MidiOutputConnection> = None;
        let mut tried = None::<Instant>;
        let mut sent: [Option<Color>; LIGHTS] = [None; LIGHTS];
        let mut verdict = VerdictLookup::default();
        loop {
            let (Some(params), Some(controls)) = (params.upgrade(), controls.upgrade()) else { break };
            if connection.is_none() && tried.is_none_or(|t| t.elapsed() >= RECONNECT) {
                tried = Some(Instant::now());
                connection = connect();
                sent = [None; LIGHTS];
            }
            if let Some(conn) = &mut connection {
                let mut messages: Vec<SysEx> = Vec::new();
                let audition = controls.audition().then(|| verdict.get(&params, &controls));
                for (i, color) in lights(controls.page(), audition).into_iter().enumerate() {
                    if sent[i] != Some(color) {
                        messages.push(pad_color(i, color));
                        sent[i] = Some(color);
                    }
                }
                if messages.iter().any(|m| conn.send(m).is_err()) {
                    connection = None;
                }
            }
            drop((params, controls));
            std::thread::sleep(POLL);
        }
        if let Some(mut conn) = connection {
            turn_off(&mut conn);
        }
    });
}

/// The loaded preset's verdict, looked up again only when the preset or a
/// rating changes.
#[derive(Default)]
struct VerdictLookup {
    seen: Option<(u32, u32)>,
    verdict: Option<Verdict>,
}

impl VerdictLookup {
    fn get(&mut self, params: &ThousandParams, controls: &Controls) -> Option<Verdict> {
        let now = (params.session_generation(), controls.ratings_generation());
        if self.seen != Some(now) {
            self.seen = Some(now);
            let library = controls.library();
            self.verdict = library.key_of(&params.session()).and_then(|k| library.rating(&k).map(|r| r.verdict));
        }
        self.verdict
    }
}
