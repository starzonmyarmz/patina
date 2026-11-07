use std::time::Instant;
use patina::engine::{Engine, Transport, CHUNK};
use patina::macros::{MacroMatrix, Target};
use patina::params::{Pid, PlainValues, FILTER_TYPES, OSC_TYPES};

const SR: f32 = 48_000.0;

fn render(engine: &mut Engine, values: &PlainValues, seconds: f32) -> (Vec<f32>, Vec<f32>) {
    let n = (SR * seconds) as usize;
    let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
    let mut pos = 0;
    while pos < n {
        let end = (pos + CHUNK).min(n);
        engine.update(values, Transport::default());
        engine.render(&mut l[pos..end], &mut r[pos..end]);
        pos = end;
    }
    (l, r)
}

fn peak(buf: &[f32]) -> f32 {
    buf.iter().fold(0.0f32, |m, x| m.max(x.abs()))
}

fn play(values: &PlainValues, notes: &[u8], seconds: f32) -> (Engine, Vec<f32>, Vec<f32>) {
    let mut e = Engine::new(SR);
    e.update(values, Transport::default());
    for &n in notes {
        e.note_on(n, 0.8);
    }
    let (l, r) = render(&mut e, values, seconds);
    (e, l, r)
}

#[test]
fn every_oscillator_and_filter_makes_bounded_sound() {
    for osc in 0..OSC_TYPES.len() {
        for flt in 0..FILTER_TYPES.len() {
            let mut v = PlainValues::default();
            v.set(Pid::Osc1Type, osc as f32).set(Pid::Osc2Type, osc as f32).set(Pid::FilterType, flt as f32);
            let (_, l, r) = play(&v, &[48, 55, 64], 0.5);
            let label = format!("{} / {}", OSC_TYPES[osc], FILTER_TYPES[flt]);
            assert!(l.iter().chain(&r).all(|x| x.is_finite()), "{label}: non-finite output");
            let p = peak(&l).max(peak(&r));
            assert!(p > 1e-3, "{label}: silent ({p})");
            assert!(p <= 1.0, "{label}: peak {p}");
        }
    }
}

#[test]
fn heavy_patch_stays_finite() {
    let mut v = PlainValues::default();
    v.set(Pid::Osc1Type, 6.0)
        .set(Pid::Unison, 8.0)
        .set(Pid::FmAmount, 100.0)
        .set(Pid::RingMod, 1.0)
        .set(Pid::FilterType, 1.0)
        .set(Pid::Resonance, 100.0)
        .set(Pid::FilterDrive, 100.0)
        .set(Pid::FilterEnvAmount, 48.0)
        .set(Pid::PitchEnv, 1.0)
        .set(Pid::Lfo1Pitch, 100.0)
        .set(Pid::Lfo1Filter, 100.0)
        .set(Pid::Lfo1Rate, 20.0)
        .set(Pid::Lfo1Shape, 3.0)
        .set(Pid::Glide, 200.0);
    let (_, l, r) = play(&v, &[24, 36, 60, 96, 120], 1.0);
    assert!(l.iter().chain(&r).all(|x| x.is_finite()));
    assert!(peak(&l).max(peak(&r)) <= 1.0);
}

#[test]
fn released_notes_fall_silent() {
    let mut v = PlainValues::default();
    v.set(Pid::AmpRelease, 50.0).set(Pid::ReverbMix, 0.0);
    let (mut e, _, _) = play(&v, &[60, 64], 0.2);
    e.note_off(60);
    e.note_off(64);
    let (l, _) = render(&mut e, &v, 1.0);
    assert_eq!(e.active_voices(), 0);
    assert!(peak(&l[l.len() - 4800..]) < 1e-4);
}

#[test]
fn polyphony_limit_steals_voices() {
    let mut v = PlainValues::default();
    v.set(Pid::Polyphony, 4.0);
    let notes: Vec<u8> = (40..60).collect();
    let (e, _, _) = play(&v, &notes, 0.1);
    assert_eq!(e.active_voices(), 4);
}

#[test]
fn sustain_pedal_holds_until_lifted() {
    let mut v = PlainValues::default();
    v.set(Pid::AmpRelease, 20.0);
    let (mut e, _, _) = play(&v, &[60], 0.1);
    e.set_sustain(true);
    e.note_off(60);
    render(&mut e, &v, 0.5);
    assert_eq!(e.active_voices(), 1);
    e.set_sustain(false);
    render(&mut e, &v, 0.5);
    assert_eq!(e.active_voices(), 0);
}

#[test]
fn synced_lfo_follows_tempo() {
    let mut v = PlainValues::default();
    v.set(Pid::Lfo1Sync, 1.0).set(Pid::Lfo1Division, 9.0).set(Pid::Lfo1Amp, 100.0).set(Pid::Lfo1Shape, 1.0);
    let mut e = Engine::new(SR);
    let t = Transport { playing: true, tempo: Some(120.0), pos_beats: Some(0.0) };
    e.update(&v, t);
    e.follow_transport(t);
    e.note_on(60, 1.0);
    let n = SR as usize;
    let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
    let mut pos = 0;
    while pos < n {
        let end = (pos + CHUNK).min(n);
        e.update(&v, t);
        e.render(&mut l[pos..end], &mut r[pos..end]);
        pos = end;
    }
    // A square LFO on amp at 1/4 and 120 BPM gates the sound every 250 ms.
    let window = |at: f32| peak(&l[(at * SR) as usize..((at + 0.05) * SR) as usize]);
    assert!(window(0.30) > 10.0 * window(0.05), "open {} closed {}", window(0.30), window(0.05));
}

fn transport(tempo: f64) -> Transport {
    Transport { playing: false, tempo: Some(tempo), pos_beats: None }
}

/// Plays a 20 ms blip and returns the left channel's peak in 10 ms windows.
fn blip_envelope(v: &PlainValues, tempo: f64, seconds: f32) -> Vec<f32> {
    let mut e = Engine::new(SR);
    e.update(v, transport(tempo));
    e.note_on(60, 1.0);
    let n = (SR * seconds) as usize;
    let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
    let off = (SR * 0.02) as usize;
    let mut pos = 0;
    while pos < n {
        if pos == off {
            e.note_off(60);
        }
        let end = (pos + CHUNK).min(n).min(if pos < off { off } else { n });
        e.update(v, transport(tempo));
        e.render(&mut l[pos..end], &mut r[pos..end]);
        pos = end;
    }
    l.chunks(480).map(peak).collect()
}

fn dry_blip() -> PlainValues {
    let mut v = PlainValues::default();
    v.set(Pid::AmpAttack, 0.0)
        .set(Pid::AmpRelease, 5.0)
        .set(Pid::ReverbMix, 0.0)
        .set(Pid::DelayMix, 100.0)
        .set(Pid::DelayFeedback, 0.0);
    v
}

fn loudest_window_after(env: &[f32], skip: usize) -> usize {
    env.iter().enumerate().skip(skip).max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0
}

#[test]
fn delay_echoes_at_its_time() {
    let mut v = dry_blip();
    v.set(Pid::DelayTime, 300.0);
    let env = blip_envelope(&v, 120.0, 0.6);
    assert_eq!(loudest_window_after(&env, 5), 30, "{env:?}");
}

#[test]
fn synced_delay_follows_tempo() {
    let mut v = dry_blip();
    v.set(Pid::DelaySync, 1.0).set(Pid::DelayDivision, 9.0);
    // A quarter note at 150 BPM is 400 ms.
    let env = blip_envelope(&v, 150.0, 0.6);
    assert_eq!(loudest_window_after(&env, 5), 40, "{env:?}");
}

#[test]
fn reverb_tail_outlasts_the_note() {
    let mut v = PlainValues::default();
    v.set(Pid::AmpAttack, 0.0).set(Pid::AmpRelease, 5.0).set(Pid::ReverbMix, 60.0).set(Pid::ReverbSize, 90.0);
    let env = blip_envelope(&v, 120.0, 1.0);
    assert!(env[50] > 1e-3, "tail at 500 ms: {}", env[50]);
}

#[test]
fn every_effect_at_full_stays_bounded() {
    let mut v = PlainValues::default();
    v.set(Pid::Distortion, 100.0)
        .set(Pid::Bitcrush, 100.0)
        .set(Pid::ChorusMix, 100.0)
        .set(Pid::ChorusRate, 100.0)
        .set(Pid::DelayMix, 100.0)
        .set(Pid::DelayFeedback, 90.0)
        .set(Pid::DelayTime, 10.0)
        .set(Pid::ReverbMix, 100.0)
        .set(Pid::ReverbSize, 100.0)
        .set(Pid::EqLow, 12.0)
        .set(Pid::EqMid, 12.0)
        .set(Pid::EqHigh, 12.0)
        .set(Pid::Compressor, 100.0)
        .set(Pid::Width, 200.0);
    let (_, l, r) = play(&v, &[36, 48, 60, 72], 3.0);
    assert!(l.iter().chain(&r).all(|x| x.is_finite()));
    assert!(peak(&l).max(peak(&r)) <= 1.0);
    assert!(peak(&l[l.len() - 4800..]) > 1e-3);
}

#[test]
fn mod_wheel_drives_macro_one() {
    let mut v = PlainValues::default();
    v.set(Pid::ReverbMix, 0.0);
    let mut m = MacroMatrix::default();
    m.0[0][0] = Some(Target { pid: Pid::Volume, amount: -1.0 });
    let mut e = Engine::new(SR);
    e.set_macros(m);
    e.update(&v, Transport::default());
    e.note_on(60, 1.0);
    let (open, _) = render(&mut e, &v, 0.3);
    e.set_mod_wheel(1.0);
    let (closed, _) = render(&mut e, &v, 0.3);
    assert!(peak(&open[7200..]) > 0.05);
    assert!(peak(&closed[7200..]) < 1e-3, "{}", peak(&closed[7200..]));
}

/// cargo test --release -p patina --test engine -- --ignored --nocapture
#[test]
#[ignore]
fn cpu_budget_for_a_dense_pad() {
    let mut v = PlainValues::default();
    v.set(Pid::Osc1Type, 1.0)
        .set(Pid::Osc2Type, 4.0)
        .set(Pid::Unison, 8.0)
        .set(Pid::FilterType, 1.0)
        .set(Pid::SubLevel, 30.0)
        .set(Pid::PinkLevel, 10.0)
        .set(Pid::Lfo1Filter, 30.0)
        .set(Pid::AmpAttack, 500.0)
        .set(Pid::ChorusMix, 30.0)
        .set(Pid::DelayMix, 25.0)
        .set(Pid::ReverbMix, 30.0)
        .set(Pid::EqLow, 3.0);
    let notes: Vec<u8> = (0..16).map(|i| 40 + i * 3).collect();
    let mut e = Engine::new(SR);
    e.update(&v, Transport::default());
    notes.iter().for_each(|&n| e.note_on(n, 0.7));
    let seconds = 10.0;
    let started = Instant::now();
    render(&mut e, &v, seconds);
    let cpu = started.elapsed().as_secs_f32() / seconds * 100.0;
    println!("16 voices x 8 unison x 2 osc, ladder, chorus/delay/reverb/EQ: {cpu:.2}% of one core");
    assert_eq!(e.active_voices(), 16);
}
