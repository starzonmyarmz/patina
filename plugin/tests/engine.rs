use std::time::Instant;
use thousand_synths::engine::{Engine, Transport, CHUNK};
use thousand_synths::params::{Pid, PlainValues, FILTER_TYPES, OSC_TYPES};

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
    v.set(Pid::AmpRelease, 50.0);
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

/// cargo test --release -p thousand_synths --test engine -- --ignored --nocapture
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
        .set(Pid::AmpAttack, 500.0);
    let notes: Vec<u8> = (0..16).map(|i| 40 + i * 3).collect();
    let mut e = Engine::new(SR);
    e.update(&v, Transport::default());
    notes.iter().for_each(|&n| e.note_on(n, 0.7));
    let seconds = 10.0;
    let started = Instant::now();
    render(&mut e, &v, seconds);
    let cpu = started.elapsed().as_secs_f32() / seconds * 100.0;
    println!("16 voices x 8 unison x 2 osc, ladder: {cpu:.2}% of one core");
    assert_eq!(e.active_voices(), 16);
}
