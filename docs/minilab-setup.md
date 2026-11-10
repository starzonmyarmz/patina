# MiniLab MK2 setup

Patina only listens to the MiniLab; it never changes the device’s settings.
Set the controls up as below in MIDI Control Center, store them to one of the
MiniLab’s memories and make that memory the active one. Settings stored in a
memory survive unplugging, and other apps can use another memory.

Every control sends on the MiniLab’s global channel.

## Knobs

| Knobs | Mode | CC |
|---|---|---|
| 1–16 | Control, **Relative #1** | 102–117, in order (knob 1 = 102, knob 16 = 117) |
| 1 and 9, clicked | Switched control, momentary (gate) | 118 (knob 1), 119 (knob 9) |
| 1 and 9, with Shift | Same as the knob | 102, 110 |

- The knob mode in the **midi** dialog tells Patina which of Relative #1, #2
  or #3 the knobs send. It must match the memory.
- Relative mode means the knobs never jump: turning a knob nudges whatever
  parameter is under it on the current page, from its current value.
- Knobs 1–8 are the top row on screen and 9–16 the bottom row. Empty slots on
  a page ignore their knob.
- Hold knob 1 or 9 down to turn every knob 8× finer. Menus step as usual.
- Out of the box the clicks send CC 113 and 115, which Patina would read as
  knobs 12 and 14, so move them to 118 and 119.

## Pads, bank 1

| Pad | Mode | CC | Does |
|---|---|---|---|
| 1 | Switched control, momentary (gate), min 0, max 127 | 20 | Osc page |
| 2 | 〃 | 21 | Shape page |
| 3 | 〃 | 22 | Filter page |
| 4 | 〃 | 23 | Mod page |
| 5 | 〃 | 24 | FX page |
| 6 | 〃 | 25 | Macro page |
| 7 | 〃 | 26 | Arp page |
| 8 | 〃 | 27 | Browse presets, or back to the page |

The pads act on press (any value above 0). A page pad also ends browsing.

While browsing, only two knobs work:

- **Knob 1** steps through the presets the browser shows, loading each
  one, and wraps at the ends.
- **Knob 2** picks the browser's filter (all, favorites, user, then each
  category) and loads its first preset.

Pad 8 and the **browse** key in the plugin window do the same thing, and
the window shows the list while browsing.

## Pads, bank 2

They send notes 44–51 and play normally. In audition mode (Shift A in
the plugin window) they rate presets instead:

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

In audition mode the keyboard’s G♯2–B2 are taken by the pads, whichever sends them.

## Touch strips

Leave them as they are: pitch bend, and mod wheel (CC 1). The mod wheel adds
to macro 1.

## In REAPER

- Give the track the MiniLab as MIDI input, and arm it with monitoring on.
- Knobs and pads work with the plugin window closed. The screen follows the
  page next time it opens.
- Knob turns are real parameter changes, so REAPER records them as automation
  when the track is in a write mode.
