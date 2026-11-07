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

## Using it

- **Pages**: six keys (or 1–6 on the keyboard) switch the 16 knobs between
  Osc, Shape, Filter, Mod, FX and Macro. The screen draws whatever the last
  touched knob controls: waveform, filter curve, envelope, LFO, echoes, EQ.
- **Knobs**: drag up/down (Shift for fine), scroll, or arrow keys when
  focused. Double-click resets. Click a switch knob to toggle it.
- **Presets**: ‹ › (or [ ]) step through the browser's current filter.
  Click the preset name or **browse** to search, filter by category and
  star favorites. Arrow keys in the search box audition presets.
- **Saving**: **save** writes a user preset to
  `~/Library/Audio/Presets/1000 Synths/User/`. Factory presets are never
  changed. The orange dot means unsaved changes; **compare** flips between
  your edit and the saved preset.

## MiniLab MK2

See [docs/minilab-setup.md](docs/minilab-setup.md). Knobs follow the page on
screen; bank 1 pads switch pages and step presets.

## Working on the UI

```sh
cargo run -p thousand_synths --example ui_mock   # writes ui/dev/mock.json
cd ui && python3 -m http.server 8765               # open http://localhost:8765
```

Outside the plugin the page talks to a mock bridge built from the real
parameter table, page layout and factory bank.

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
| `plugin/src/library.rs` | Preset keys, favorites, saving and deleting user presets. |
| `plugin/src/bridge.rs` | The editor's message protocol: values, presets, session, compare. |
| `plugin/src/editor.rs` | The webview window and keyboard forwarding. |
| `ui/index.html` | The whole interface: plain HTML, CSS and JavaScript. |
| `plugin/src/minilab.rs` | MiniLab CC numbers and relative encoder decoding. |
| `plugin/src/controls.rs` | Hardware state shared by the audio thread, the GUI-thread task and the editor. |
| `plugin/src/macros.rs` | Per-preset macro matrix. |
| `plugin/src/presets.rs` | Preset format, factory bank, user preset files, the session saved with projects. |
| `plugin/tests/engine.rs` | Offline render tests and the CPU benchmark. |
| `presetgen/` | Factory bank generator and quality gate. |
