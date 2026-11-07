# MiniLab MK2 setup

Patina expects the MiniLab to send the messages below. Set them once in
Arturia **MIDI Control Center** (MCC) and store them to a memory on the
device. The plugin's **midi** dialog shows the same summary.

This was written without the device at hand. If a setting's name in MCC
doesn't match, the CC numbers and modes in the tables are what matter.

## Knobs

| Knobs | Mode | CC |
|---|---|---|
| 1–16 | Control, **Relative #1** | 102–117, in order (knob 1 = 102, knob 16 = 117) |

- In MCC, select every knob and set its option from Absolute to
  **Relative #1**. Relative #2 and #3 also work if you set the same mode in
  the plugin's **midi** dialog.
- Relative mode means the knobs never jump: turning a knob nudges whatever
  parameter is under it on the current page, from its current value.
- Knobs 1–8 are the top row on screen and 9–16 the bottom row. Empty slots on
  a page ignore their knob.

## Pads, bank 1

| Pad | Mode | CC | Does |
|---|---|---|---|
| 1 | Switched control, momentary (gate) | 20 | Osc page |
| 2 | 〃 | 21 | Shape page |
| 3 | 〃 | 22 | Filter page |
| 4 | 〃 | 23 | Mod page |
| 5 | 〃 | 24 | FX page |
| 6 | 〃 | 25 | Macro page |
| 7 | 〃 | 26 | Previous preset |
| 8 | 〃 | 27 | Next preset |

The pads act on press (any value above 0). Previous and next step through
the presets the browser is showing, so filter to "Pad" or your favorites first
to step through just those.

## Pads, bank 2

Leave them sending notes, as they do out of the box (notes 44–51). They play
notes normally. In audition mode (Shift A in the plugin window) they rate
presets instead:

| Pad | Note | Does |
|---|---|---|
| 9 | 44 | Keep |
| 10 | 45 | Tweak |
| 11 | 46 | Cut |
| 12 | 47 | Clear the verdict |
| 13 | 48 | Replay the phrase |
| 14 | 49 | Next unrated preset |
| 15 | 50 | Previous preset |
| 16 | 51 | Next preset |

If your pads send other notes, set bank 2 back to notes 44–51 in MCC. In
audition mode the keyboard's G♯2–B2 are taken by the pads, whichever
sends them.

## Touch strips

Leave them as they are: pitch bend, and mod wheel (CC 1). The mod wheel adds
to macro 1.

## Store it

In MCC, **Store To** one of the device memories, then select that memory on the
MiniLab. The setup survives power cycles.

## In REAPER

- Give the track the MiniLab as MIDI input, and arm it with monitoring on.
- Knobs and pads work with the plugin window closed. The screen follows the
  page next time it opens.
- Knob turns are real parameter changes, so REAPER records them as automation
  when the track is in a write mode.
