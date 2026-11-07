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

## Factory presets

```sh
cargo run --release -p presetgen
```

Rewrites `presets/factory.json`, which is compiled into the plugin. The
generator starts from hand-designed archetypes in
`presetgen/src/archetypes.rs`, makes seeded variations, assigns each
category's macros, then renders every preset through the engine: it
rejects silent, broken or DC-offset presets, sets each one's volume to the
same loudness, and checks that all macros at full stay clean.

## Layout

| Path | What |
|---|---|
| `plugin/src/params.rs` | The parameter table: IDs, ranges, defaults, JSFX slider mapping. Single source of truth. |
| `plugin/src/engine/` | DSP: band-limited oscillators, filters, envelopes, LFOs, voices. |
| `plugin/src/lib.rs` | nih-plug wrapper: MIDI handling, sample-accurate event splitting. |
| `plugin/src/layout.rs` | Which parameter sits under each of the 16 knobs on each page. |
| `plugin/src/macros.rs` | Per-preset macro matrix. |
| `plugin/src/presets.rs` | Preset format, factory bank, user preset files, the session saved with projects. |
| `plugin/tests/engine.rs` | Offline render tests and the CPU benchmark. |
| `presetgen/` | Factory bank generator and quality gate. |
