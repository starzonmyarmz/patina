# MiniLab MK2 setup

Patina writes the settings below to the MiniLab over SysEx whenever it
connects to it: when the plugin or app starts, and when the MiniLab is
plugged back in. There’s nothing to set up. The device forgets them when it
powers off, so other apps get its own settings back.

Every control sends on the MiniLab’s global channel.

## Knobs

| Knobs | Mode | CC |
|---|---|---|
| 1–16 | Control, **Relative #1** | 102–117, in order (knob 1 = 102, knob 16 = 117) |
| 1 and 9, clicked | Switched control, momentary (gate) | 118 (knob 1), 119 (knob 9) |
| 1 and 9, with Shift | Same as the knob | 102, 110 |

- The knob mode in the **midi** dialog picks Relative #1, #2 or #3, and
  changing it updates the device.
- Relative mode means the knobs never jump: turning a knob nudges whatever
  parameter is under it on the current page, from its current value.
- Knobs 1–8 are the top row on screen and 9–16 the bottom row. Empty slots on
  a page ignore their knob.
- Hold knob 1 or 9 down to turn every knob 8× finer. Menus step as usual.
- Out of the box the clicks send CC 113 and 115, which Patina would read as
  knobs 12 and 14; the setup moves them.

## Pads, bank 1

| Pad | Mode | CC | Does | Light |
|---|---|---|---|---|
| 1 | Switched control, momentary (gate) | 20 | Osc page | White on the page shown, else blue |
| 2 | 〃 | 21 | Shape page | 〃 |
| 3 | 〃 | 22 | Filter page | 〃 |
| 4 | 〃 | 23 | Mod page | 〃 |
| 5 | 〃 | 24 | FX page | 〃 |
| 6 | 〃 | 25 | Macro page | 〃 |
| 7 | 〃 | 26 | Previous preset | Green |
| 8 | 〃 | 27 | Next preset | Green |

The pads act on press (any value above 0). Previous and next step through
the presets the browser is showing, so filter to "Pad" or your favorites first
to step through just those.

## Pads, bank 2

They send notes 44–51 and play normally, unlit. In audition mode (Shift A in
the plugin window) they rate presets instead:

| Pad | Note | Does | Light |
|---|---|---|---|
| 9 | 44 | Keep | Green |
| 10 | 45 | Tweak | Yellow |
| 11 | 46 | Cut | Red |
| 12 | 47 | Clear the verdict | White when rated |
| 13 | 48 | Replay the phrase | Magenta |
| 14 | 49 | Next unrated preset | Cyan |
| 15 | 50 | Previous preset | Green |
| 16 | 51 | Next preset | Green |

Once a preset has a verdict, only that verdict’s pad stays lit. In audition
mode the keyboard’s G♯2–B2 are taken by the pads, whichever sends them.

## Touch strips

Leave them as they are: pitch bend, and mod wheel (CC 1). The mod wheel adds
to macro 1.

## Lights

Patina sends the setup and the pad lights straight to the MiniLab over
CoreMIDI, not through the DAW, so they need no routing and work in both VST3 and CLAP. It
finds the MiniLab by name, picks it up within a second or two of being
plugged in, and turns the lights off when the plugin closes. With several
Patina instances open, the one you touched last wins.

## In REAPER

- Give the track the MiniLab as MIDI input, and arm it with monitoring on.
- Knobs and pads work with the plugin window closed. The screen follows the
  page next time it opens.
- Knob turns are real parameter changes, so REAPER records them as automation
  when the track is in a write mode.
