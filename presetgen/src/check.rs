use thousand_synths::engine::{Engine, Transport, CHUNK};
use thousand_synths::macros::{MacroMatrix, MACRO_PIDS};
use thousand_synths::params::{Pid, PlainValues, Values};

const SR: f32 = 48_000.0;
/// Loudest 50 ms RMS window every preset is normalized to.
pub const TARGET_DB: f32 = -18.0;
const SILENT_DB: f32 = -60.0;
const MAX_SHORTFALL_DB: f32 = 6.0;
const MAX_DC: f32 = 0.02;

pub struct Rendered {
    left: Vec<f32>,
    right: Vec<f32>,
    held: usize,
}

pub fn render(values: &PlainValues, matrix: MacroMatrix, notes: &[u8], hold: f32, release: f32) -> Rendered {
    let mut e = Engine::new(SR);
    e.set_macros(matrix);
    let t = Transport { playing: true, tempo: Some(120.0), pos_beats: Some(0.0) };
    e.update(values, t);
    e.follow_transport(t);
    notes.iter().for_each(|&n| e.note_on(n, 0.8));
    let held = (hold * SR) as usize;
    let n = held + (release * SR) as usize;
    let (mut left, mut right) = (vec![0.0; n], vec![0.0; n]);
    let mut pos = 0;
    while pos < n {
        if pos == held {
            notes.iter().for_each(|&n| e.note_off(n));
        }
        let end = (pos + CHUNK).min(n).min(if pos < held { held } else { n });
        e.update(values, t);
        e.render(&mut left[pos..end], &mut right[pos..end]);
        pos = end;
    }
    Rendered { left, right, held }
}

impl Rendered {
    pub fn finite(&self) -> bool {
        self.left.iter().chain(&self.right).all(|x| x.is_finite())
    }

    pub fn peak(&self) -> f32 {
        self.left.iter().chain(&self.right).fold(0.0f32, |m, x| m.max(x.abs()))
    }

    pub fn loudness_db(&self) -> f32 {
        let w = (SR * 0.05) as usize;
        let loudest = self
            .left
            .chunks(w)
            .zip(self.right.chunks(w))
            .map(|(l, r)| l.iter().zip(r).map(|(a, b)| (a * a + b * b) * 0.5).sum::<f32>() / l.len() as f32)
            .fold(0.0f32, f32::max);
        10.0 * loudest.max(1e-12).log10()
    }

    pub fn dc(&self) -> f32 {
        let held = &self.left[..self.held];
        (held.iter().sum::<f32>() / held.len() as f32).abs()
    }
}

/// Sets the preset's volume so it hits `TARGET_DB`, then checks it with
/// every macro at full. Returns the final loudness.
pub fn normalize(values: &mut PlainValues, matrix: MacroMatrix, notes: &[u8], hold: f32) -> Result<f32, String> {
    let first = render(values, matrix, notes, hold, 1.5);
    if !first.finite() {
        return Err("non-finite output".into());
    }
    let loud = first.loudness_db();
    if loud < SILENT_DB {
        return Err(format!("silent ({loud:.1} dB)"));
    }
    if first.dc() > MAX_DC {
        return Err(format!("DC offset {:.3}", first.dc()));
    }

    // Volume is applied after the limiter, so loudness scales linearly with it.
    let volume = values.get(Pid::Volume);
    let wanted = volume * 10f32.powf((TARGET_DB - loud) / 20.0);
    values.set(Pid::Volume, wanted.clamp(1.0, 100.0));
    let checked = render(values, matrix, notes, hold, 1.5);
    let final_db = checked.loudness_db();
    if final_db < TARGET_DB - MAX_SHORTFALL_DB {
        return Err(format!("too quiet even at full volume ({final_db:.1} dB)"));
    }

    let mut extreme = values.clone();
    for pid in MACRO_PIDS {
        extreme.set(pid, 100.0);
    }
    let pushed = render(&extreme, matrix, notes, hold, 1.5);
    if !pushed.finite() || pushed.loudness_db() < SILENT_DB || pushed.peak() > 1.0 {
        return Err(format!("breaks with macros at full ({:.1} dB, peak {:.2})", pushed.loudness_db(), pushed.peak()));
    }
    Ok(final_db)
}

