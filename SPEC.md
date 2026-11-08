# Patina — plugin spec

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
| Arpeggiator | Built in, between MIDI input and the voice engine. Always tempo-synced. Saved in presets and off by default. |
| Performance | 16 voices × up to 8 unison, under ~5% of one Apple Silicon core for a typical pad. |
| Presets | Hand-tuned core plus generated variations. Shipping fewer than 1000 at first is fine. |
| Macros | 8 per preset. Each preset defines what its macros are called and what they move. |
| Editing | Factory presets are read-only. Saving an edit creates a user preset. A dot marks unsaved changes; "compare" toggles between the edit and the saved preset. |
| Curation | Hidden audition mode: plays a test phrase per preset; pads rate keep / cut / tweak into a file the generator reads. |
| Layout | Pages that mirror the MiniLab's 2×8 knobs: Osc, Shape, Filter, Mod, FX, Macro, Arp. |
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
6. **Audition mode** and curation sessions. Done: see the README's "Curating the
   factory bank".
7. **Pad lights** and writing the MiniLab setup over SysEx. Done: see
   `docs/minilab-setup.md`.
8. **Arpeggiator.** Done: see Arpeggiator below.

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

Each page fills the 2×8 grid, in titled sections. Slots marked `—` are empty.

| Page | Row 1 (knobs 1–8) | Row 2 (knobs 9–16) |
|---|---|---|
| Osc | Osc 1 type, semi, detune, level · Osc 2 type, semi, detune, level | FM, ring · unison voices, detune, spread · sub, noise, pink |
| Shape | wavetable position, PWM width, supersaw spread · pitch env on, amount, attack, decay · — | velocity→amp, aftertouch→filter, aftertouch→vibrato · polyphony, glide, volume · —, — |
| Filter | type, cutoff, resonance, drive, env amount, vowel · keytrack, velocity→filter | filter env A, D, S, R · amp env A, D, S, R |
| Mod | LFO 1 rate, shape, →pitch, →filter, →amp · LFO 3 rate, shape, →pan | LFO 2 rate, shape, →pitch, →filter · FM env A, D, S, R |
| FX | distortion, bitcrush · chorus mix, rate · delay time, feedback, mix · LFO 3 →amp (tremolo) | reverb size, mix · EQ low, mid, high · compressor, width, limiter |
| Macro | macros 1–8 | sync and division for LFO 1, LFO 2, LFO 3, delay |

The seven page names fit pad bank 1 with one pad left for browsing.
`plugin/src/layout.rs` is authoritative; a test checks that every parameter is
on exactly one page.

### Browsing

Pad 8 switches the knobs between the page and browsing, like the editor's
**browse** key, and the two stay in step. While browsing, knob 1 steps
through the presets the browser shows, loading each, and knob 2 picks the
browser's filter and loads its first preset. Four encoder ticks make one
step, as for menus. A page pad ends browsing.

## Arpeggiator

The arp sits between MIDI input and the voice engine. When it's off, notes
go straight through. When it's on, it collects the held notes and sends the
engine one step at a time. Everything after the arp works the same:
polyphony, glide, envelopes and FX. Because the arp only sees notes, it
works the same with any note source: keys, pad bank 2, audition phrases, or
a guitar-to-MIDI plugin placed before Patina in REAPER.

### Parameters

| ID | Name | Range | Default |
|---|---|---|---|
| `arp_on` | Arp | off / on | off |
| `arp_mode` | Mode | up, down, up-down, down-up, as played, random, chord | up |
| `arp_division` | Rate | 1/4, 1/4T, 1/8, 1/8T, 1/16, 1/16T, 1/32 | 1/16 |
| `arp_octaves` | Octaves | 1–4 | 1 |
| `arp_gate` | Gate | 5–100 % of a step | 50 % |
| `arp_swing` | Swing | 0–50 % | 0 % |
| `arp_repeats` | Repeats | 1–4 | 1 |
| `arp_latch` | Latch | off / on | off |

- **Modes.** Up and down sort the held notes by pitch. Up-down and down-up
  don't repeat the top and bottom notes. As played keeps the order the keys
  went down. Random picks a different note than the last one when it can.
  Chord plays all held notes together on every step.
- **Octaves.** The pattern plays across this many octaves, going up from
  the played notes, then starts over.
- **Gate.** How long each note lasts, as a share of a step. At 100 % each
  note holds until the next one starts, so glide connects them.
- **Swing.** Delays every second step by up to half a step.
- **Repeats.** Plays each note this many times before moving to the next.
- **Velocity.** Each step uses the velocity its key was played with.

### Holding notes

- Without latch, the pattern stops when the last key is released. Releasing
  a key in the middle of a pattern removes that note at the next step.
- With latch, notes keep playing after release. Keys pressed while others
  are held join the pattern. Pressing a key after all keys are up starts a
  new pattern. Turning latch off releases notes whose keys are already up.
- The sustain pedal works like a temporary latch while held. It holds the
  arp's notes, not the voices.
- Turning the arp off, or changing preset, ends the current note.

### Timing

- **Host playing.** Steps line up with the host's beat grid, so a pattern
  started early or late still lands on the beat. A pattern started up to
  30 ms after a step plays at once instead of waiting for the next one.
- **Host stopped, or the app.** The arp keeps its own clock at the host or
  app tempo (120 BPM in the app). It starts when the first key goes down,
  and the first step plays 10 ms later, so every key of a chord played by
  hand is in it.
- Steps can fall anywhere inside a block. They're scheduled at their exact
  sample, not rounded to block boundaries.

### Presets

Arp parameters are ordinary preset parameters. Old presets don't have them,
so they load with the arp off. A preset can turn the arp on (for example, a
sequenced bass), and macros can target the arp's continuous parameters,
such as gate and swing.

### Page and MiniLab

The arp gets a seventh page, **Arp**. Row 1 has the eight parameters in the
table order. Row 2 is empty for now.

Pad 7 opens Arp. The app's 1–7 keys switch pages.

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
- User presets: `~/Library/Audio/Presets/Patina/User/<name>.json`.
- Favorites, ratings and settings:
  `~/Library/Application Support/Patina/state.json`.
- The DAW project stores the full current state (params, macros, preset name,
  dirty flag), so a project reopens exactly as left even if the preset file
  changed.

## MiniLab MK2

Patina writes the setup to the device over SysEx on every connect, since the
device forgets SysEx-written settings when it powers off. The exact settings are in `docs/minilab-setup.md`. Tested
on firmware 1.1.2.25.

- **Knobs 1–16**: relative mode, CC 102–117. The plugin supports all three of
  Arturia's relative encodings; the one in use is a setting.
- **Clickable knobs 1 and 9**: CC 118 and 119 as gates; every knob turns 8×
  finer while either is held.
- **Pad bank 1**: CC 20–27, momentary. Pages Osc, Shape, Filter, Mod, FX,
  Macro, previous preset, next preset.
- **Pad bank 2**: notes, as the factory setup sends them.
- **Touch strips**: pitch bend and mod wheel (CC1). Mod wheel adds to macro 1.
- In audition mode, pad bank 2 rates presets instead of playing notes.
- **Pad lights**: bank 1 shows the page, bank 2 the rating pads in audition
  mode.

SysEx, all `F0 00 20 6B 7F 42 … F7`: `01 00 <param> <control>` reads a
setting and the device answers `02 00 <param> <control> <value>`; `02 00
<param> <control> <value>` writes one. Params: 01 mode (01 control, 08
switched, 09 note), 02 channel (41 global), 03 CC or note, 04 min, 05 max,
06 behaviour (knobs: 00 absolute, 01–03 relative #1–#3; switched: 01 gate),
10 pad color (write-only). Controls: knobs 1–16 are `30 01 02 09 0B 0C 0D 0E
33 03 04 0A 05 06 07 08`, knob 1 and 9 clicks `31 34`, Shift-turned `32 35`,
pads 1–16 `70`–`7F`.

## UI

- OP-1 light: warm off-white panel (`#e8e6e1`), four cap colors (blue, green,
  white, orange) repeated across each row of 8, a black screen showing the
  page, preset name and a live graphic (envelope, waveform or LFO), small
  monospace labels.
- Knobs on screen move when a MiniLab knob or host automation moves them.
- Developed in a browser against a mock parameter bridge, then embedded.

## Risks

1. **Resolved: pad lights.** The community-known SysEx works on this unit.
   Rather than route the plugin's MIDI output back to the MiniLab in each
   REAPER project (and lose VST3, which can't send SysEx), a background
   thread opens the MiniLab's port directly with midir.
2. **Webview crate maturity.** nih-plug's webview support is a small community
   crate. The spike in step 4 decides whether it holds up or needs a fork.
3. **Resolved: nih-plug failed 4 clap-validator 0.4 state tests.** It never
   asked the host to rescan parameter values after a host state load, and
   trusted the state's length prefix for one allocation. Fixed in the
   vendored copy (`vendor/nih_plug`); clap-validator passes.
4. **Resolved: parameters could only be set through the editor's context.**
   The vendored nih-plug adds `InitContext::gui_context()`, so MiniLab knobs
   and pads work without the editor ever opening. The plugin drops the
   context on deactivate, because it holds the plugin wrapper alive.
5. **Curation needs ears.** Rules and automated checks (silence, clipping, DC)
   catch broken presets, not boring ones. Audition mode is how the bank gets
   good.
