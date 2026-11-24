---
name: design-kickoff
description: Settle a plugin's look with the user before building its editor - the physical object, palette, what the art reacts to, and the wordmark font. Use before replacing the starter knob page with a hand-laid design, or when the user asks to design or redesign the editor.
---

# Design kickoff

Read the principles in `DESIGN.md` first. Every choice here has to fit
them.
Don't write the page until the user has picked the object, palette, art
and font.

1. **Read the plugin.** Its name, what it does to sound, and its
   parameters in `plugin/src/params.rs`. Note which ones could drive art
   (a depth, a mix, a time) and which are plain settings.
2. **Propose the object.** Use AskUserQuestion with two to four
   physical objects that fit the name and vibe (Halfwit is a stompbox,
   Ghosted a vintage radio, Patina an OP-1-style synth). Give each a
   short ASCII preview of the panel layout and its materials.
3. **Name the controls.** Fold the parameters into as few knobs as the
   sound needs, and give them evocative, plain-word names that suit the
   object (Ghosted's Haunt, Drift, Spook). Effects get a `stomp`
   footswitch, on by default, unless the user says otherwise. Ask the
   user to confirm the list, and where the version goes on the object
   (a model number on a base plate, a serial plate, small print under
   the wordmark).
4. **Palette and art.** Ask about the palette and what the art reacts
   to, with previews. Art follows parameters only. Don't fake metering.
   Colours differ only where functions do.
5. **Font.** Pick three or four display faces with character (not system
   or "commercial" fonts), and render the real wordmark in each, side by
   side on the actual badge, in the page served from `ui/`. Screenshot
   that and let the user choose.
6. **Build it.** Lay out the page by hand in `ui/index.html`, with the
   font embedded as a base64 `@font-face` subset and its file and
   license in `ui/fonts`. Set `PAGE.size` and the page's outer element
   to the same size. Print `v` plus `init`'s `version` where the user
   chose. Then use the `screenshot` skill.

Record the decisions (object, palette, font, control names, where the
version sits) under "This plugin" in `DESIGN.md`, so later changes keep
to them.
