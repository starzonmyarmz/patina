//! Talks to the MiniLab's MIDI input directly rather than through the host,
//! so the lights work in VST3 and need no routing in the DAW. The device
//! forgets settings written over SysEx when it powers off, so the setup goes
//! out on every connect.

use crate::controls::Controls;
use crate::minilab::{lights, pad_color, setup, Color, SysEx, LIGHTS};
use crate::params::ThousandParams;
use crate::ratings::Verdict;
use midir::{MidiOutput, MidiOutputConnection};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Weak;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const DEVICE: &str = "MiniLab";
const POLL: Duration = Duration::from_millis(30);
const RECHECK: Duration = Duration::from_secs(1);
/// The MiniLab lights a pad itself when it's hit, so the lights go out
/// again after each touch, and once more in case it repaints late.
const REPAINT_AGAIN: Duration = Duration::from_millis(150);

fn minilab_connected() -> bool {
    MidiOutput::new("Patina").is_ok_and(|out| out.ports().iter().any(|p| out.port_name(p).is_ok_and(|n| n.contains(DEVICE))))
}

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

/// The lights thread. Dropping it stops the thread and waits for it, so it
/// can't outlive the plugin's library being unloaded.
pub struct Lights {
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Lights {
    fn drop(&mut self) {
        self.stop.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Runs until stopped or the plugin goes away, then turns the lights off.
pub fn spawn(params: Weak<ThousandParams>, controls: Weak<Controls>) -> Lights {
    let (stop, stopped) = channel::<()>();
    let thread = std::thread::Builder::new().name("Patina lights".into()).spawn(move || {
        let mut connection: Option<MidiOutputConnection> = None;
        let mut checked = None::<Instant>;
        let mut sent: [Option<Color>; LIGHTS] = [None; LIGHTS];
        let mut set_up = None;
        let mut touches = 0;
        let mut repaint_at = None::<Instant>;
        let mut verdict = VerdictLookup::default();
        loop {
            let (Some(params), Some(controls)) = (params.upgrade(), controls.upgrade()) else { break };
            if checked.is_none_or(|t| t.elapsed() >= RECHECK) {
                checked = Some(Instant::now());
                // Sends to an unplugged device don't fail, so look for it.
                if connection.is_some() && !minilab_connected() {
                    connection = None;
                }
                if connection.is_none() {
                    connection = connect();
                    sent = [None; LIGHTS];
                    set_up = None;
                }
            }
            let now_touches = controls.pad_touches();
            if now_touches != touches {
                touches = now_touches;
                sent = [None; LIGHTS];
                repaint_at = Some(Instant::now() + REPAINT_AGAIN);
            } else if repaint_at.is_some_and(|t| Instant::now() >= t) {
                sent = [None; LIGHTS];
                repaint_at = None;
            }
            if let Some(conn) = &mut connection {
                let mut messages: Vec<SysEx> = Vec::new();
                let mode = controls.knob_mode();
                if set_up != Some(mode) {
                    messages.extend(setup(mode));
                    set_up = Some(mode);
                }
                let audition = controls.audition().then(|| verdict.get(&params, &controls));
                for (i, color) in lights(controls.page(), controls.browsing(), audition).into_iter().enumerate() {
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
            if stopped.recv_timeout(POLL) != Err(RecvTimeoutError::Timeout) {
                break;
            }
        }
        if let Some(mut conn) = connection {
            turn_off(&mut conn);
        }
    });
    Lights { stop: Some(stop), thread: thread.ok() }
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
