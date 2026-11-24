# Patina

A polysynth plugin (VST3 + CLAP) and Mac app in Rust

Built for one person on one Mac: Apple Silicon, REAPER and an Arturia MiniLab MK2. Releases also build for Intel Macs and Windows.

Read before working:
- `DESIGN.md` before touching anything people see.

## Commands

```sh
cargo test --workspace                      # engine, layout and preset tests, offline
cargo test --release -p patina --test engine -- --ignored --nocapture   # CPU budget
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo xtask bundle patina --release     # target/bundled/Patina.{vst3,clap}
scripts/install.sh                          # bundle, sign ad hoc and install on macOS as X.Y.Z-dev+<sha>, then /Applications/Patina.app
scripts/clap-validator.sh "target/bundled/Patina.clap"
cargo run --release -p presetgen            # regenerate presets/factory.json and merge ratings
scripts/release --dry-run                   # what the next release would be
```

Work on the editor in a browser against the mock: `cargo run -p patina --example ui_mock` (writes `ui/dev/mock.json` from the real parameter table, page layout and factory bank), then `cd ui && python3 -m http.server 8765` and open http://127.0.0.1:8765. To load the page live in the plugin, set `PATINA_UI_URL` to that URL before starting the host.

## Layout

- `plugin/src/params.rs`: the parameter table (IDs, display names, ranges, defaults, kinds, units). The single source of truth for the DSP, the UI and the preset generator.
- `plugin/src/engine/`: the DSP, with no host types, so `cargo test` runs it offline: band-limited oscillators, filters, envelopes, LFOs, voices and effects.
- `plugin/src/lib.rs`: host plumbing, plugin IDs, MIDI handling and sample-accurate event splitting.
- `plugin/src/arp.rs`: the arpeggiator, between MIDI input and the voice engine.
- `plugin/src/layout.rs`: which parameter sits under each of the 16 knobs on each page. Authoritative; a test checks every parameter is on exactly one page.
- `plugin/src/presets.rs`: the preset format, the compiled-in factory bank, user preset files and the session saved with projects.
- `plugin/src/macros.rs`: the per-preset macro matrix.
- `plugin/src/library.rs`: preset keys, favorites, saving and deleting user presets.
- `plugin/src/audition.rs`, `ratings.rs`: audition mode's test phrases (a short loop per category) and its verdicts, which the plugin writes and the generator reads.
- `plugin/src/minilab.rs`: MiniLab CC numbers and relative encoder decoding. `controls.rs`: hardware state shared by the audio thread, the GUI-thread task and the editor.
- `plugin/src/editor.rs`, `bridge.rs`: the webview window (its size is `SIZE` in `editor.rs`), keyboard forwarding, and the message protocol (values, presets, session, compare).
- `plugin/src/app.rs`, `chrome.rs`: hooks only the standalone app sets (device lists, relaunch, saving the sound) and its title-bar-less window. Neither runs inside a DAW.
- `plugin/tests/engine.rs`: offline render tests and the CPU benchmark (`#[ignore]`d).
- `plugin/examples/ui_mock.rs`: writes `ui/dev/mock.json` for the browser mock.
- `ui/index.html`: the whole editor, compiled into the plugin. In a browser it talks to a mock bridge that reads `ui/dev/mock.json`.
- `presetgen/`: the factory bank generator and quality gate. `presets/factory.json` is its output, compiled into the plugin; `presets/ratings.json` holds merged audition verdicts.
- `app/`: the Mac app: device choice, then nice-plug's standalone wrapper. `scripts/install-extra.sh` bundles and installs it.
- `docs/minilab-setup.md`, `docs/Patina.minilabmk2`: the MiniLab setup and its MIDI Control Center template.
- `xtask/`, `bundler.toml`: nice-plug's bundler.
- `scripts/`: install, clap-validator and release.
- `.claude/skills/`: `design-kickoff` (settle the look before building the editor). Use it rather than working from memory.

## Invariants and footguns

What was learned the hard way. Add to this list whenever something breaks in a way the code doesn't make obvious.

- `CLAP_ID` and `VST3_CLASS_ID` in `lib.rs` never change after the first release, and parameter IDs never change without a major release. Hosts use them to find the plugin and restore saved projects.
- After a change to `plugin/src` or `ui/`, run `scripts/install.sh` so the user can try it. It installs as `X.Y.Z-dev+<sha>`. REAPER keeps a plugin's dylib loaded: tell the user to restart it.
- Don't bump the version by hand. It only changes at release.

### Parameters and the engine

- Every parameter has a stable string ID (`osc1_type`, `flt_cutoff`, …) that is never renamed or reused, because host automation and saved presets refer to it.
- Each parameter has an ID, display name, range, default, kind (float, int, enum or bool), unit, page and knob slot. The UI and the preset generator read an exported copy of the table, so nothing is listed twice.
- Performance budget: 16 voices × up to 8 unison, under about 5% of one Apple Silicon core for a typical pad. The ignored CPU benchmark in `plugin/tests/engine.rs` checks it.
- LFOs and the delay each have a free/sync toggle and a host-synced division.
- Tempo sync: read the host transport in `process` and pass the engine plain values. Fall back to 120 BPM when the host gives no tempo. The app always runs at 120 BPM.

### Arpeggiator

- The arp sits between MIDI input and the voice engine. Off, notes go straight through. On, it collects held notes and sends the engine one step at a time. Everything after it (polyphony, glide, envelopes, FX) works the same, and so does every note source: keys, pad bank 2, audition phrases, or a guitar-to-MIDI plugin before Patina in REAPER.
- Parameters, in table order and on the Arp page's row 1 (row 2 is empty):

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

- Modes: up and down sort held notes by pitch. Up-down and down-up don't repeat the top and bottom notes. As played keeps the order the keys went down. Random picks a different note than the last one when it can. Chord plays all held notes on every step.
- Octaves: the pattern plays across this many octaves going up from the played notes, then starts over. Gate: at 100 % each note holds until the next starts, so glide connects them. Swing delays every second step by up to half a step. Repeats plays each note that many times before moving on. Each step uses the velocity its key was played with.
- Holding notes: without latch, the pattern stops when the last key is released, and a released key leaves the pattern at the next step. With latch, notes keep playing after release; keys pressed while others are held join the pattern, and a key pressed after all are up starts a new one. Turning latch off releases notes whose keys are already up. The sustain pedal is a temporary latch that holds the arp's notes, not the voices. Turning the arp off or changing preset ends the current note.
- Timing with the host playing: steps line up with the host's beat grid, so a pattern started early or late still lands on the beat. A pattern started up to 30 ms after a step plays at once instead of waiting for the next one.
- Timing with the host stopped, or in the app: the arp runs its own clock at the host or app tempo. It starts when the first key goes down, and the first step plays 10 ms later so every key of a hand-played chord is in it.
- Steps are scheduled at their exact sample inside a block, never rounded to block boundaries.
- Arp parameters are ordinary preset parameters. Old presets don't have them, so they load with the arp off. A preset can turn the arp on (a sequenced bass), and macros can target its continuous parameters, such as gate and swing.

### Presets

- One JSON object per preset:

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

- `params` holds plain values, not normalized. Missing IDs take the default, so adding a parameter never breaks old presets.
- 8 macros per preset; each preset names its macros and says what they move. Amounts are normalized (−1…1). A macro at 0 leaves the preset exactly as saved; turning it up adds `amount × macro` to each target.
- Factory presets are compiled into the binary and read-only. Saving an edit creates a user preset in `~/Library/Audio/Presets/Patina/User/<name>.json`. A dot marks unsaved changes; "compare" toggles between the edit and the saved preset.
- Favorites, ratings and settings live in `~/Library/Application Support/Patina/state.json`.
- The DAW project stores the full current state (params, macros, preset name, dirty flag), so a project reopens exactly as left even if the preset file changed.
- The bank is a hand-tuned core plus generated variations from archetypes in `presetgen/src/archetypes.rs`. The generator rejects silent, broken or DC-offset presets, levels loudness and checks that all macros at full stay clean. Those checks catch broken presets, not boring ones: audition mode (Shift A, see the README) is how the bank gets good.
- Verdicts are keyed by preset name, so changing the generator can orphan them; it warns about any that no longer match. Commit `presets/ratings.json` after merging.

### MiniLab MK2

- The setup lives in one of the device's memories, stored with MIDI Control Center. Patina only reads the MiniLab and never writes to it. The exact settings are in `docs/minilab-setup.md`; tested on firmware 1.1.2.25.
- Knobs 1–16: relative mode, CC 102–117. All three of Arturia's relative encodings are supported; the one in use is a setting.
- Clickable knobs 1 and 9: CC 118 and 119 as gates; every knob turns 8× finer while either is held.
- Pad bank 1: CC 20–27, momentary. Pads 1–7 open Osc, Shape, Filter, Mod, FX, Macro and Arp (the app's 1–7 keys do the same). Pad 8 switches the knobs between the page and browsing, like the editor's **browse** key, and the two stay in step.
- Browsing: knob 1 steps through the presets the browser shows, loading each; knob 2 picks the browser's filter and loads its first preset. Four encoder ticks make one step. A page pad ends browsing.
- Pad bank 2: notes, as the factory setup sends them. In audition mode it rates presets instead.
- Touch strips: pitch bend and mod wheel (CC1). The mod wheel adds to macro 1.
- Parameters must work before the editor opens, so MIDI sets them through `ActivateContext::gui_context()` from the nice-plug fork. The plugin drops that context on deactivate, so knob turns while inactive are discarded.
- Don't bring back pad lights or writing the setup over SysEx. They were built and removed: every plugin instance took over the device, and dropped or overlapping writes left it half set up. Routing the plugin's MIDI output back to the device instead would lose VST3, which can't send SysEx.
- SysEx reference, all `F0 00 20 6B 7F 42 … F7`: `01 00 <param> <control>` reads a setting and the device answers `02 00 <param> <control> <value>`; `02 00 <param> <control> <value>` writes one. Params: 01 mode (01 control, 08 switched, 09 note), 02 channel (41 global), 03 CC or note, 04 min, 05 max, 06 behaviour (knobs: 00 absolute, 01–03 relative #1–#3; switched: 01 gate), 10 pad color (write-only). Controls: knobs 1–16 are `30 01 02 09 0B 0C 0D 0E 33 03 04 0A 05 06 07 08`, knob 1 and 9 clicks `31 34`, Shift-turned `32 35`, pads 1–16 `70`–`7F`.

### Things clap-validator catches

Run `scripts/clap-validator.sh` after any DSP change, not just in CI.

- Subnormal output. Recursive filters, envelopes and smoothers decay into subnormals in silence. Flush state and output below about 1e-30 to zero.
- nice-plug's `v2s_f32_hz_then_khz` already appends Hz/kHz, so adding `with_unit(" Hz")` doubles it. `v2s_f32_gain_to_db` and `v2s_f32_percentage` don't, so `with_unit` is right with those.
- NaN from filters whose corner frequency is above Nyquist at low sample rates. Clamp corners below Nyquist.
- nih-plug failed four clap-validator 0.4 state tests: it never asked the host to rescan parameter values after a state load, and trusted the state's length prefix for one allocation. nice-plug fixes both.

### Editor

- The window size is `SIZE` in `editor.rs` and the page's outer element in `ui/index.html`. Change them together.
- Knobs on screen move when a MiniLab knob or host automation moves them.
- The browser preview gets cached: add `?v=N` after each edit. Rerun the `ui_mock` example after changing parameters, pages or the factory bank.
- SVG reused through `<use>` doesn't get page CSS rules. Stamp it from a `<template>` instead, or put attributes on the elements themselves.
- Fonts can't load from the network: embed them as base64 `@font-face` data URLs, and keep the file and its license in `ui/fonts`.
- The macOS webview is the system WKWebView, so check a newer CSS feature is in Safari for the macOS versions you support. Windows uses WebView2.
- After a visible UI change, retake `docs/screenshot-*.jpg` by hand and keep the README `alt` text current. The kit's screenshot script doesn't fit this editor, which needs the generated mock.

### Testing in REAPER

- REAPER only monitors input on armed tracks.

### nice-plug fork

Plugins pin the `patina` branch of [starzonmyarmz/nice-plug](https://github.com/starzonmyarmz/nice-plug/tree/patina): upstream plus one commit that adds `ActivateContext::gui_context()`. Keep the fork to that one commit.

- Upgrading: rebase `patina` onto upstream, push, and bump the `rev` in `plugin/` and `xtask/` `Cargo.toml` in every plugin. Then run the tests and clap-validator in each.
- Don't send the patch upstream. It was written with AI, and nice-plug's [AI policy](https://rust.audio/community/ai/) doesn't accept AI-written code, PR descriptions or comments.

## Conventions

- Conventional Commits with Title-Case scopes: `feat(Engine): …`. The body explains why.
- Tests ship in the same commit as the change. Pure logic is tested; the UI or host shell stays thin.
- A `feat`, `fix` or `perf` commit that someone using Patina would notice adds a line under `## [Unreleased]` in `CHANGELOG.md`, in the same commit, written for them, not as a code summary.
- This repo is public. Keep real names, logins, paths and work data out of code, fixtures, docs, the changelog and commits.
- Breaking means a renamed parameter ID, a changed parameter range, or a preset that no longer loads.

## Releasing

Releases happen only when asked. Never bump, tag or publish on your own.

`scripts/release` works out the bump from commits since the last tag, finalizes the changelog, commits `chore(Release): X.Y.Z`, tags `vX.Y.Z` and pushes. The tag triggers `.github/workflows/release.yml`.
