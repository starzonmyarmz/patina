# 1000 Synths

A polysynth plugin (VST3 + CLAP) in Rust, built with
[nih-plug](https://github.com/robbert-vdh/nih-plug). See [SPEC.md](SPEC.md) for
the design and build order.

## Install

```sh
./scripts/install.sh
```

Builds release bundles and copies them, ad-hoc signed, to
`~/Library/Audio/Plug-Ins/VST3` and `~/Library/Audio/Plug-Ins/CLAP`. In
REAPER, rescan plugins (**Preferences → Plug-ins → VST/CLAP → Re-scan**) and
add **1000 Synths** to a track. Prefer the CLAP version.

There's no custom UI yet; REAPER shows its generic parameter sliders, grouped
by section.

## Test

```sh
cargo test -p thousand_synths
cargo test --release -p thousand_synths --test engine -- --ignored --nocapture   # CPU budget
```

## Layout

| Path | What |
|---|---|
| `plugin/src/params.rs` | The parameter table: IDs, ranges, defaults, JSFX slider mapping. Single source of truth. |
| `plugin/src/engine/` | DSP: band-limited oscillators, filters, envelopes, LFOs, voices. |
| `plugin/src/lib.rs` | nih-plug wrapper: MIDI handling, sample-accurate event splitting. |
| `plugin/tests/engine.rs` | Offline render tests and the CPU benchmark. |
