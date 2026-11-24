# Design

How Patina looks and feels. When this file and your instincts disagree, this file wins. Change it on purpose, in the same commit as the code.

## The look in one line

An OP-1-style synth in its light finish: warm off-white panel, dark knob caps, a black screen and small monospace labels.

## Principles

Every plugin gets its own look, but they share an approach.

- **Skeuomorphic.** The editor is a physical object (Patina an OP-1-style synth, Halfwit a stompbox, Ghosted a vintage radio), with real materials like powder coat, bakelite, walnut, brass and grille cloth, built in CSS gradients and shadows rather than images.
- **One theme.** Pick the object from the plugin's vibe and name, and let it run through everything: the wordmark, the art and the control labels. Names should be playful and capture the vibe.
- **Restrained.** One knob style and colour family per plugin, and few controls. Fold many settings into a handful of knobs with evocative names rather than exposing every one.
- **Colour and art mean something.** If two waves or knobs differ in colour, it's because they differ in function; otherwise make them the same. Art can follow parameters (Ghosted's ghosts fade with Haunt, the dial needle follows Distance), but don't fake audio metering.
- **Plain labels.** Words a guitarist understands, not DSP terms or fieldset jargon. Numbers go in a tooltip readout.
- **Type with character.** Embed a display face for the wordmark (Futura-like Jost, pillowy Modak), never a system or "commercial" font, and centre it optically on its badge.
- **Drawn, not vector-perfect.** Vary repeated elements (size, tilt, expression), and give outlines weight that swells on the shadow side, like inked comic art.
- **A footswitch on effects.** Pedals and other effects get an on/off toggle (`stomp`, on by default). Off fades the effect out and passes the dry signal; a lamp or LED shows the state, and the art reacts (Ghosted's dial dims and its ghosts leave).
- **The version on the panel.** Print it small and quiet as part of the object, like a model number on a pedal's base plate or a serial plate on a radio's back (`v1.2.0`), so anyone can tell which build they're running. Local installs show `X.Y.Z-dev+<sha>`.
- **Physical details behave physically.** Shadows stay put while a knob turns, pressed keys sink, lit things glow.

## This plugin

- **Object:** a Teenage Engineering OP-1 in its light finish, laid out like the MiniLab MK2 it's played from.
- **Palette:** warm off-white panel (`--panel`), near-black ink, a black screen with white text, and orange (`--orange`) for whatever is active: the current page, a pressed key, the unsaved-changes dot, a favorite star.
- **Wordmark font:** none embedded. Everything is small monospace (`ui-monospace`, SF Mono, Menlo), with uppercase muted labels.
- **Controls:** 16 knobs in a 2×8 grid that mirrors the MiniLab's knobs, each a dark cap with a white pointer inside an ink arc on a pale track. Keys along the top switch pages, step presets and open the browser. On-screen knobs move when a MiniLab knob or host automation moves them.
- **Screen:** shows the page, the preset name and a live graphic of whatever the last touched knob controls: waveform, filter curve, envelope, LFO, echoes or EQ.
- **Version placement:** not shown yet (see Known drift).

### Pages

Each page fills the 2×8 grid in titled sections. Slots marked `—` are empty. `plugin/src/layout.rs` is authoritative.

| Page | Row 1 (knobs 1–8) | Row 2 (knobs 9–16) |
|---|---|---|
| Osc | Osc 1 type, semi, detune, level · Osc 2 type, semi, detune, level | FM, ring · unison voices, detune, spread · sub, noise, pink |
| Shape | wavetable position, PWM width, supersaw spread · pitch env on, amount, attack, decay · — | velocity→amp, aftertouch→filter, aftertouch→vibrato · polyphony, glide, volume · —, — |
| Filter | type, cutoff, resonance, drive, env amount, vowel · keytrack, velocity→filter | filter env A, D, S, R · amp env A, D, S, R |
| Mod | LFO 1 rate, shape, →pitch, →filter, →amp · LFO 3 rate, shape, →pan | LFO 2 rate, shape, →pitch, →filter · FM env A, D, S, R |
| FX | distortion, bitcrush · chorus mix, rate · delay time, feedback, mix · LFO 3 →amp (tremolo) | reverb size, mix · EQ low, mid, high · compressor, width, limiter |
| Macro | macros 1–8 | sync and division for LFO 1, LFO 2, LFO 3, delay |
| Arp | arp on, mode, rate, octaves, gate, swing, repeats, latch | — |

The page names fit pad bank 1, with pad 8 left for browsing.

### Presets in the UI

- The browser has categories, tags, search and favorites, plus an Unrated filter in audition mode.
- Factory presets are read-only. **save** makes a user preset, an orange dot marks unsaved changes, and **compare** flips between the edit and the saved preset.
- Audition mode (Shift A) turns the page keys into **keep**, **tweak**, **cut**, **replay**, **skip** and **exit**.

## Tokens

Every color, size and space comes from a token. If nothing fits, add a token here before using a new value.

| Token | Value | Used for |
|---|---|---|
| `--panel` | `#e8e6e1` | the panel |
| `--panel-edge` | `#d6d3cc` | the selected row in the browser |
| `--panel-deep` | `#c9c5bc` | recessed edges and key shadows |
| `--key-top` | `#f5f4f0` | top of a key's gradient |
| `--key-bottom` | `#dcd9d2` | bottom of a key's gradient |
| `--ink` | `#1d1d1b` | text and the knob arc |
| `--muted` | `#8a877f` | knob labels |
| `--screen` | `#0d0d0d` | the screen |
| `--screen-ink` | `#f2f2f2` | screen text and graphics |
| `--screen-dim` | `#5a5955` | dimmed screen text |
| `--orange` | `#ff5a1f` | active and unsaved states, focus rings |
| `--white` | `#f7f6f2` | the browser's search and list fields |
| `--track` | `#cfccc4` | the knob arc's unfilled track |
| `--gutter` | `24px` | panel padding and the gap between sections |
| `--cap` | `58px` | knob cap diameter |
| `--ring` | `4px` | knob arc width |
| `--radius` | `6px` | corners |

## Components

## Patterns

## Adding something new

1. Look for an existing component or pattern that already does it.
2. Build it from tokens only.
3. Check Reduce Motion if it moves.
4. Retake the screenshot if it shows in one.
5. Record it above.

## Known drift

Existing debt to clean up, not to copy.

- The panel doesn't print the version, so a `-dev+<sha>` install can't be told apart from a release in the editor.
- The knob cap's gradient (`#4a4945`, `#2b2a28`, `#161615`) and the shadows' `rgb()` values aren't tokens.
- The original plan gave the caps four colors (blue, green, white, orange) repeated across each row of 8, like the OP-1. They're all dark today.
