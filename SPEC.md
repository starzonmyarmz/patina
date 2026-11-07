# 1000 Synths — plugin spec

A polysynth plugin in Rust, grown out of the `1000Synths.jsfx` prototype
(`~/Code/jsfx/1000-synths`, frozen as a reference). Built for one person on one
Mac: Apple Silicon, REAPER, an Arturia MiniLab MK2.

## Decisions

| Area | Decision |
|---|---|
| Formats | VST3 + CLAP via [nih-plug](https://github.com/robbert-vdh/nih-plug). No AU, AAX or standalone. |
| Platform | macOS, Apple Silicon only. Ad-hoc signed, installed by script. No notarization or installer. |
| Engine | Same parameter set as the JSFX, better DSP. Presets carry over approximately, not bit-exactly. |
| Tempo sync | LFOs and delay each have a free/sync toggle and a host-synced division. |
| Performance | 16 voices × up to 8 unison, under ~5% of one Apple Silicon core for a typical pad. |
| Presets | Hand-tuned core plus generated variations. Shipping fewer than 1000 at first is fine. |
| Macros | 8 per preset. Each preset defines what its macros are called and what they move. |
| Editing | Factory presets are read-only. Saving an edit creates a user preset. A dot marks unsaved changes; "compare" toggles between the edit and the saved preset. |
| Curation | Hidden audition mode: plays a test phrase per preset; pads rate keep / cut / tweak into a file the generator reads. |
| Layout | Pages that mirror the MiniLab's 2×8 knobs: Osc, Filter, Env, LFO, FX, Macro. |
| Hardware | Knobs in relative mode. Pad bank 1 = navigation, pad bank 2 = notes. |
| UI | Native webview (WKWebView) with HTML/CSS. OP-1 light look. |
| Repo | This repo. The JSFX repo stays as a frozen reference. |

## v1 scope

All of these are v1:

1. Full engine and effects (every JSFX module, plus tempo sync).
2. Finished skeuomorphic UI.
3. Preset browser with categories, tags, search and favorites.
4. MiniLab pad lights (see Risks).

## Build order

Each step ends with something that loads and plays in REAPER.

1. **Engine core.** Cargo workspace, nih-plug, VST3 + CLAP bundles, install
   script. The parameter table in Rust is the single source of truth for DSP,
   UI and the preset generator. Voice engine: oscillators, unison, sub, noise,
   filters, envelopes, LFOs, modulation. Host's generic parameter UI.
2. **Effects and tempo sync.** Distortion, bitcrush, chorus, delay, reverb, EQ,
   compressor, width, limiter. Host-synced LFO and delay divisions.
3. **Presets.** Preset format, factory bank compiled in, user presets on disk,
   per-preset macros, the generator ported to the new parameter table.
4. **UI.** One-day webview spike first (a knob, two-way parameter sync,
   open/close in REAPER). Then the paged UI and the preset browser.
5. **MiniLab.** Relative encoders, pad mapping, a MIDI Control Center setup
   guide.
6. **Audition mode** and curation sessions.
7. **Pad lights**, after testing the SysEx on the device.

## Parameters

Every parameter has a stable string ID (`osc1_type`, `flt_cutoff`, …) that is
never renamed or reused, because host automation and saved presets refer to it.
Each entry also records the JSFX slider number it came from, so the old bank can
be imported.

The parameter table lives in `plugin/src/params.rs`. Each entry has: ID, display
name, range, default, kind (float / int / enum / bool), unit, page and knob slot.
The UI and the preset generator read an exported JSON copy of it, so nothing is
listed twice.

### Pages

Each page fills the 2×8 grid. Slots marked `—` are empty.

| Page | Row 1 (knobs 1–8) | Row 2 (knobs 9–16) |
|---|---|---|
| Osc | osc1 type, semi, detune, level · osc2 type, semi, detune, level | FM, ring, unison voices, unison detune, spread, sub, noise, pink |
| Shape | wavetable pos, PWM width, supersaw detune, glide · pitch env on, amount, attack, decay | velocity→filter, velocity→amp, aftertouch→filter, aftertouch→vibrato, keytrack, polyphony, volume, — |
| Filter | type, cutoff, resonance, drive, env amount, vowel, —, — | filter env A, D, S, R · amp env A, D, S, R |
| Mod | mod env A, D, S, R · LFO 1 rate, shape, →pitch, →filter | LFO 1 →amp, LFO 2 rate, shape, →pitch, →filter, LFO 3 rate, shape, →pan |
| FX | distortion, bitcrush, chorus mix, chorus rate, delay time, feedback, mix, reverb size | reverb mix, EQ low, mid, high, compressor, width, limiter, LFO 3 →amp |
| Macro | macros 1–8 | sync toggles and divisions for LFO 1–3 and delay |

The six page names fit pad bank 1 with two pads left for previous/next preset.
The exact assignment is revisited once the UI exists; the parameter table, not
this doc, is authoritative.

## Presets

One JSON object per preset:

```json
{
  "format": 1,
  "name": "Warm Pad 014",
  "category": "Pad",
  "tags": ["warm", "slow"],
  "author": "factory",
  "params": { "osc1_type": "saw", "flt_cutoff": 0.62 },
  "macros": [
    { "name": "Bloom", "targets": [{ "param": "flt_cutoff", "amount": 0.3 }] }
  ]
}
```

- `params` holds plain values (not normalized). Missing IDs take the default,
  so adding a parameter never breaks old presets.
- Macro amounts are in normalized units (−1…1). A macro at 0 leaves the preset
  exactly as saved; turning it up adds `amount × macro` to each target.
- Factory presets are compiled into the binary.
- User presets: `~/Library/Audio/Presets/1000 Synths/User/<name>.json`.
- Favorites, ratings and settings:
  `~/Library/Application Support/1000 Synths/state.json`.
- The DAW project stores the full current state (params, macros, preset name,
  dirty flag), so a project reopens exactly as left even if the preset file
  changed.

## MiniLab MK2

Configured once in Arturia MIDI Control Center and stored to the device. The
exact settings go in `docs/minilab-setup.md` (step 5).

- **Knobs 1–16**: relative mode, CC 102–117. The plugin supports all three of
  Arturia's relative encodings; the one in use is a setting. Verify which
  modes the MK2 firmware offers before writing the guide.
- **Clickable knobs 1 and 9**: fine adjust while held, if the device sends a
  usable message for the click.
- **Pad bank 1**: CC 20–27, momentary. Pages Osc, Shape, Filter, Mod, FX,
  Macro, previous preset, next preset.
- **Pad bank 2**: notes, as the factory setup sends them.
- **Touch strips**: pitch bend and mod wheel (CC1). Mod wheel adds to macro 1.
- In audition mode, pad bank 2 rates presets instead of playing notes.

## UI

- OP-1 light: warm off-white panel (`#e8e6e1`), four cap colors (blue, green,
  white, orange) repeated across each row of 8, a black screen showing the
  page, preset name and a live graphic (envelope, waveform or LFO), small
  monospace labels.
- Knobs on screen move when a MiniLab knob or host automation moves them.
- Developed in a browser against a mock parameter bridge, then embedded.

## Risks

1. **Pad lights.** Arturia doesn't document SysEx for the MK2 pad colors; the
   community-known messages need testing on this unit first. Lights also need
   the plugin's MIDI output routed back to the MiniLab in REAPER, per project.
   VST3 handles SysEx output poorly, so use the CLAP build.
2. **Webview crate maturity.** nih-plug's webview support is a small community
   crate. The spike in step 4 decides whether it holds up or needs a fork.
3. **Curation needs ears.** Rules and automated checks (silence, clipping, DC)
   catch broken presets, not boring ones. Audition mode is how the bank gets
   good.
