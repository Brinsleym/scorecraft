# Ink and markup

> **Last reviewed:** 2026-10-31 · **Last updated:** 2026-10-31 · **Change:** minor (M4 part 5: export with ink; earlier: M4 part 4: the pen in the app; earlier: M4 part 3: rectangle select, shapes switch, pen button; earlier: ANNO-8 text notes from the owner's observation; earlier: owner's answers to the open questions; earlier: owner's direction on the toolbar; earlier: first version) · **Target:** enScore 3.7 (2026)

How the player marks up a score: pens, highlighters, whiteout, erasers, text notes, stamps,
layers, selection, shapes, and exporting the result. Written for milestone M4.

## Sources

- Public listings S1, S2, S4 and S8: [../observations/public-listings-2026-10-28.md](../observations/public-listings-2026-10-28.md)
  ("Markup", "Export", the S8 notes on highlighters and tiers). They name the capabilities: pen,
  finger ("draw with touch") and mouse input; Apple Pencil and Windows Ink with pressure; pencil
  shortcuts; highlighters that blend with the page like real ones; custom pen colours; whiteout
  that covers a mark so a new one can be written over it; fingerings; typed text notes; stroke
  and pixel erasers; annotation layers that can be hidden for a performance; PDFs exported with
  ink and layers.
- Observation: [../observations/anno-markup-toolbar.md](../observations/anno-markup-toolbar.md),
  the markup toolbar on Windows and the owner's direction for ScoreCraft's markup (2026-10-31):
  pen-only drawing by default with a *Draw with Touch* switch, a touch lock, the tool set, eight
  changeable swatches with a fixed palette of extra colours (screenshot 25), a width slider per
  tool, eraser options, a rectangle tool for snips, shortcuts and moving ink, and a ruler whose
  button is hidden unless a setting shows it. A second pass answers this spec's open questions:
  the chisel highlighter (screenshot 26), the ruler (screenshot 27), the highlighter's own
  swatches and palette (screenshot 28), the eraser's size, palm rejection after the pen lifts,
  the stylus shortcut, and stamps (ScoreCraft's own, in the Edwin font). A third pass covers the
  text tool (screenshot 29).
- Font: [Edwin](https://github.com/MuseScoreFonts/Edwin) (MuseScore BVBA, SIL OFL 1.1) for stamp
  text, loaded from the fonts directory like every font ([fonts](../standards/fonts.md)).
- Platform documentation on palm rejection: Windows' *Ignore touch input when I'm using my pen*
  setting ([guide](https://geekrewind.com/how-to-enable-or-disable-ignore-touch-input-when-using-pen-in-windows-11/),
  [Microsoft hardware docs](https://learn.microsoft.com/vi-vn/windows-hardware/customize/desktop/pen-and-ink));
  Android's palm flags ([Stylus palm rejection](https://developer.android.com/develop/adaptive-apps/cookbook/stylus-palm-rejection)).
- Standard: ISO 32000-1 §12.5 (annotations: `/Ink`, `/FreeText`, `/Square`, `/Line`, appearance
  streams) and §11.3.5 (blend modes: `Multiply`) for export (EXP-2).
- Standard: SMuFL 1.4 (Standard Music Font Layout), for the code points stamps use (ANNO-9).
- Architecture rules: annotations are stored in the unrotated source page's PDF user space
  ([architecture §1.1](../architecture.md#11-rules-we-adopt-from-day-one), §5), so they survive
  page edits, and the imported file is never rewritten (AGENTS.md rule 3).
- The listings don't say how these tools behave in detail. Where they are silent, this spec picks
  behaviour that suits a player marking a part on a stand, and lists the choice under
  [Open questions](#open-questions) for an observation pass ([G17](../gaps.md#g17-unknowns-behaviour-not-yet-observed)).

## Terms

- **Markup mode**: the reader with the markup toolbar shown. Outside it, pointers turn and pan
  pages as in [page-turning](page-turning.md) and nothing draws.
- **Tool**: what a pen does on the page in markup mode. Exactly one tool is active: pen,
  highlighter, eraser, text or rectangle select. The ruler is a guide shown alongside them.
- **Stroke**: the samples one pen-down to pen-up gesture produced, with a tool, a colour and a
  nominal width.
- **Page space**: points on the source page, y up, before the score's own rotation or crop.
  Everything placed on a page is kept in page space.
- **Active layer**: the layer new markings go into.

## Requirements

### The markup toolbar

- Entering markup mode shows one toolbar, in this order: *Draw with Touch*, touch lock, pen,
  highlighter, eraser, text, stamp, rectangle select, ruler (only when the *Show ruler button* setting is
  on; it is off by default); then eight colour swatches, a palette button and a width slider. The
  active tool and the selected swatch are marked. The swatches and palette are the active tool's:
  the pen's and the highlighter's are different (ANNO-4).
- With the eraser selected, its *stroke* and *pixel* options appear in the bar next to it. With
  the stamp tool selected, the stamp choices appear instead of the swatches (ANNO-9). With the
  text tool selected, the *Draw with Touch* toggle reads *Type with Touch* (ANNO-8).
- The pen is active the first time; afterwards markup opens with the last tool, colours, widths
  and swatches.
- Leaving markup mode (or turning the page with a pedal) keeps every marking and the tool
  settings. Page turns by pedal and keys keep working in markup mode.

### ANNO-1 Pen

- With the pen active, a pen drawn across a page leaves ink along its path (a mouse or finger
  too when *Draw with Touch* is on, ANNO-14). Ink is drawn while the pointer moves; it doesn't wait for the pointer to lift.
- One pen-down to pen-up is one stroke and one undo step.
- A stroke that starts on a page belongs to that page. Samples that leave the page are kept (ink
  may run into the margin) but a stroke never crosses onto another page.
- A tap without movement leaves a dot as wide as the pen.
- Ink stays sharp at every zoom: it is kept as geometry, not as pixels.
- Smoothing removes the jitter of a hand-held stylus but keeps the corners of handwritten
  fingerings and accents: a direction change sharper than 60° is not rounded off.
- Samples closer than 0.1 pt to the previous one are dropped. A stroke holds at most 10,000
  samples; a longer gesture ends the stroke and continues as a new one.

### ANNO-2 Pressure

- Where the device reports pressure, the stroke is wider where the pen pressed harder. Width at a
  sample is `width × (floor + (1 − floor) × pressure^γ)` with floor 0.25 and γ 0.6 by default, so a
  light touch still leaves a visible line. Devices without pressure draw at full width.
- The width changes smoothly along the stroke; the first sample of a stroke doesn't show a blob or
  a spike when the device's first pressure reading is low.
- Highlighter and whiteout ignore pressure: their width is constant.

### ANNO-3 Highlighter

- The nib is a **chisel held upright** (screenshot 26): a rectangle one third as wide as it is
  tall, swept along the stroke. Strokes running across the page are as thick as the nib is tall;
  strokes running up and down are as thin as it is wide; diagonals fall between. The ends are cut
  straight up and down, and a tap leaves the nib's rectangle. The width slider sets the nib's
  height. The nib stays upright on the page (it turns with a rotated page, as a marker on paper
  would).
- A setting, *Round highlighter ends*, swaps the chisel for a round nib: a constant-width band
  with round ends, like a wide pen. It applies to new strokes; each stroke keeps the nib it was
  drawn with.
- Blends like a real highlighter: it multiplies the page by its colour at full strength, so black
  notation under it stays black and white paper takes the swatch's colour. It never covers
  notation.
- No build-up within a stroke: where one stroke crosses itself, the colour is the same as where it
  doesn't. Two separate strokes over the same place do darken it again, as two passes would.
- Default: the highlighter's first swatch (yellow), 12 pt nib.

### ANNO-4 Colours, widths and presets

- The pen and the highlighter each have their own **eight swatches** and their own **palette**.
  - Pen swatches start as black, red, blue, green, amber, purple, brown and grey; the pen palette
    is 30 colours (six columns, five rows: black, white and greys; reds, oranges and yellows;
    greens, cyan, blue and indigo; purples and browns; pastels).
  - Highlighter swatches start as yellow, pink, light green, light blue, orange, lavender, coral
    and aqua (screenshot 28); the highlighter palette is 30 highlighter colours (yellows and
    golds; peaches, oranges and corals; pinks, roses, magenta and orchid; yellow-greens, greens,
    mint and turquoise; blues, lavenders and violet).
- Tapping a swatch selects it and gives the tool its colour. The palette button opens the active
  tool's palette; it offers only those colours, not a free colour picker. Picking one replaces the
  **selected swatch** with it and gives the tool that colour. Swatches are kept between launches.
  ScoreCraft chooses its own values for every colour.
- The width slider is per tool: it shows and sets the active tool's own width: pen 0.25 to 12 pt,
  highlighter nib 4 to 40 pt, eraser 2 to 60 pt (the eraser's size). With the text tool it is the
  text size slider, 8 to 36 pt (ANNO-8).
- Pen, highlighter and whiteout each keep their own colour and width (picking red for the pen
  doesn't change the highlighter); changing them affects new strokes only.
- A player can save the current tool, colour and width as a named preset, recall it in one action,
  and delete it. Presets are kept between launches.

### ANNO-5 Whiteout

- Whiteout is the pen in white: picking the palette's white (or a swatch set to it) for the pen
  makes it paint whiteout. The toolbar has no separate whiteout button. Choosing another colour
  returns the pen to ink.
- Paints an opaque band in the page's paper colour (white on a normal page) that covers printed
  marks under it. Pen strokes drawn after it, on the same or a higher layer, show on top of it.
- Under a display filter (dark, sepia) whiteout shows in that filter's paper colour, so it still
  reads as blank paper.
- Default width 10 pt.

### ANNO-6 Stroke eraser

- The toolbar has one eraser with two modes, *stroke* (the default) and *pixel* (ANNO-7), chosen
  from the options that appear when the eraser is selected. The eraser keeps the mode it was last
  used in.
- With the eraser in stroke mode, every stroke the eraser touches is removed whole. The eraser is a
  disc in page space, 8 pt across by default; its size is the eraser's width on the one width
  slider (2 to 60 pt). A stroke is touched when any part of its drawn shape is within the disc.
- One eraser gesture is one undo step, however many strokes it removed. A gesture that touched
  nothing records nothing.
- Only strokes on visible, unlocked layers are erased. Text notes and stamps are not erased by
  either eraser (they are deleted by selecting them, ANNO-12).

### ANNO-7 Pixel eraser

- With the eraser in pixel mode, erases only the part of a stroke under the eraser disc (default 8
  pt across, the same slider). What remains of the stroke becomes one or more new strokes, each
  keeping the original's tool, nib, colour and width, and the pressure of its remaining samples
  exactly as they were (the pressure profile is not reshaped). A stroke erased entirely is removed.
- One gesture is one undo step; undo restores the original stroke.

### ANNO-8 Text notes

Observed in enScore (screenshot 29, [observation](../observations/anno-markup-toolbar.md)):

- With the text tool selected, the toolbar's first toggle reads **Type with Touch** instead of
  *Draw with Touch*: it is the same setting (ANNO-14), named for what touch does with this tool.
  Without it, only the pen (or a tap with the mouse, as for drawing: see ANNO-14) places notes.
- Tapping the page places a **new note** there: an empty box outlined in the accent colour, with
  a dark rounded **handle** on its left marked with grip lines, and the text cursor in the box.
  Typing fills it. The same box outline and handle show on a note that is selected for editing.
  Dragging the handle moves the note.
- A note looks like plain text on the page: black, a sans-serif face (ScoreCraft uses Inter,
  which is always built in), no border and no background, so the music shows around the letters.
- **Text size** is set with the toolbar's slider, which becomes a **size slider** while the text
  tool is active: from **8 pt to 36 pt**, **12 pt** by default (owner, 2026-10-31). It sets the
  size of new notes and of the selected note.
- A note is as wide as its longest line, up to a **maximum width of 86% of the page's width**.
  Longer text wraps at spaces (a single word longer than the maximum breaks where it must). Lines
  are 1.5 times the text size apart.
- **Notes stay inside the page.** A note is placed, grown and moved so that no part of it crosses
  a page edge: tapped near the right or bottom edge, it starts far enough left or up to fit its
  text, and as it grows it shifts back inside; it can't be dragged past an edge. A note can start
  exactly at the page's corner.
- Notes are stored in page space like ink, so they stay on the same music when the page is
  rotated or moved.

ScoreCraft's own additions, beyond what was observed:

- The style of a note can be changed from its selection: colour (the pen's swatches), bold and
  italic, and an optional filled background. The defaults are the observed look above.
- Notes can be edited, moved and deleted. Placing, each finished edit and each move is one undo
  step. Empty text is not a note: finishing an edit with no text deletes it.

### ANNO-9 Stamps (ScoreCraft's own)

- enScore has no stamps; ScoreCraft adds them because a dynamic is the commonest marking a player
  writes in.
- The stamp tool's bar shows the dynamics **pp, p, mp, mf, f, ff**, then a **>** button that opens
  the **crescendo** and **diminuendo hairpins**. **p** is selected the first time; afterwards the
  last stamp used.
- Dynamics are set in the **Edwin** font, bold italic as engraved dynamics are. Without the font
  installed, a built-in fallback shape is used, so stamps always show.
- Tapping the page places the selected dynamic centred where the player tapped, at a size the
  player picks (8–96 pt, 18 pt by default), in the pen's colour.
- A hairpin is placed by dragging from where it starts to where it ends (a tap places one 72 pt
  long). A placed hairpin shows three handles when selected: one at each end, which stretch it
  longer or shorter, and one in the middle, which moves it. Its opening is a fixed height (a
  third of the size setting) whatever its length.
- Every placed stamp can be moved, resized, recoloured and deleted like any selection (ANNO-12).
  Each placement and each handle drag is one undo step.

### ANNO-10 Layers

- Every score has at least one layer. A new score has one, "Layer 1", which is active.
- Layers can be added, renamed, reordered, shown or hidden, locked or unlocked, and deleted
  (deleting the last layer is refused; deleting a layer deletes its markings, as one undo step).
- Layers draw bottom first, in their list order; within a layer, markings draw in the order they
  were made.
- Hidden layers are not drawn in the reader and can be left out of an export. A locked layer is
  drawn but can't be drawn on, erased or edited.
- The active layer can't be hidden or locked while the player draws; drawing on a hidden or locked
  active layer is refused with an error the player sees, never silently dropped.

### ANNO-11 Pen shortcuts

- A stylus barrel button or double-tap (where the platform reports one) switches between the pen
  and the eraser by default: from any other tool it switches to the eraser, and pressing again
  returns to the tool before. The player can choose another pair in settings (the tool the button
  switches to; pressing it on that tool returns to the one before, or the pen the first time). The
  eraser end of a stylus always erases whole strokes. The shell sends each press as
  `annotate.pen.button`; the setting is `annotate.pen.buttonTool`.

### ANNO-12 Rectangle select: move, copy, delete; snips and shortcuts

- The rectangle tool drags out a rectangle on one page. When the drag ends, the player chooses
  what to do with it:
  - **Move**, **copy** or **delete** the ink inside it: every marking whose shape lies mostly
    inside the rectangle (more than half its samples, or its box's centre for notes and stamps).
    Moving drags the markings; copying leaves the originals and places copies (new ids) where the
    player drops them, on the same page or another. A selection can also be scaled about its
    centre (0.1× to 10×) and recoloured. Each action is one undo step.
  - **Snip** (PAGE-7): the rectangle's bars are copied as a Score Snip and pasted where the player
    taps, typically at the bottom of the page before a turn.
  - **Shortcut** (NAV-1): the rectangle becomes a Score Shortcut; the player then picks the page
    (and optionally the place) it jumps to.
- An empty selection offers only snip and shortcut. Snips and shortcuts are specified with
  setlists (M6); the rectangle tool is their way in.
- Only markings on visible, unlocked layers are selected. A selection is kept until another
  rectangle, *Deselect* or opening another score; after undo takes some of its markings away, the
  actions apply to the ones left, and with none left they are refused. A moved or copied
  selection's rectangle centre lands on the drop point, kept on the target page; notes and stamps
  stay inside it. Scaling keeps widths, text and stamp sizes inside their tools' ranges.
  Recolouring leaves whiteout paper-coloured.

### ANNO-13 Undo and redo

- Every markup step above is undoable and redoable, in the score's own history (shared with its
  page edits). Undo during a stroke cancels the stroke in progress.

### ANNO-14 Pen only and Draw with Touch (palm rejection)

- By default only a pen draws in markup mode. A mouse, and a finger or palm on a touch screen,
  never mark up: they turn, pan and zoom as outside markup mode, so a resting hand leaves no ink.
- The *Draw with Touch* switch lets the mouse and touch draw (and erase, write text and select)
  like a pen, at full width since they have no pressure. It is off by default and kept between
  launches.
- **Palm rejection while the pen draws.** While a pen is touching the screen, every touch is
  ignored: a hand leaning on the screen never turns, pans, zooms or (with *Draw with Touch*)
  draws. In detail:
  - a touch that starts while the pen is down is ignored until that finger lifts, even if the pen
    lifts first;
  - a touch gesture already in progress when the pen goes down is cancelled: a pending tap never
    turns the page, and a pan or pinch stops where it is;
  - when no pen is touching, touch works as normal: tapping and swiping turn pages, two fingers
    pinch to zoom and pan, so using the pen and then zooming with two fingers needs no switch;
  - the mouse is never rejected;
  - a touch that starts within **50 ms** after the pen lifts is ignored too, to catch a hand that
    lands as the pen comes off; no deliberate tap follows a pen that quickly, so taps and pinches
    are never lost;
  - a touch the platform marks as a palm or cancels (Android 13 and later flag palm touches as
    cancelled) is dropped as if it never happened, and any ink or gesture it started is undone.
- **Touch lock** (the padlock): while it is on, every touch is ignored on the page: no drawing,
  erasing, turning, panning or zooming by touch. The pen, the mouse, keys and pedals keep working.
  It can be switched only in markup mode and applies only there; leaving markup mode lets touch
  turn pages again, and the lock is as the player left it next time. It is off by default.
- **Telling a pen from a finger** (the desktop app, M4 part 4). winit 0.30 reports pens and
  fingers alike as touches, so the app takes a touch that carries a pressure as a pen: Windows
  Ink pens report one and touch screens normally don't. Where a device reports pressure for
  fingers, or none for its pen, *Draw with Touch* lets every pointer draw. winit reports neither
  the eraser end nor the barrel button yet, so ANNO-11 waits on it in the app.
- The platforms help but can't be relied on alone: Windows has an *Ignore touch input when I'm
  using my pen* setting (Pen & Windows Ink) that suppresses touch while the pen is near the screen,
  but the user can turn it off and some devices hide it; Android only marks palm touches, so the
  app has to drop them. ScoreCraft's own rule above works either way and doesn't fight the
  platform's.

### ANNO-17 Ruler

- The ruler is a straight edge laid over the page to rule lines against (bar lines, slurs, a
  cut). Its toolbar button is hidden by default and appears when the *Show ruler button* setting
  is on; the ruler works the same either way (an agent can show it by command).
- Showing it lays a light band, with tick marks along both long edges, horizontally across the
  **whole width of the screen** at its vertical centre (screenshot 27). It is a band on the
  screen, not on a page: it stays put while pages turn, zoom and pan, and in the two-page layout
  it crosses both pages. Dragging it (one finger, the mouse, or the pen on the ruler body) moves
  it up and down.
- It stays horizontal for now: turning it and showing its angle are ANNO-18, at the bottom of the
  low-priority list.
- While it is shown, a pen or highlighter stroke that starts within 18 screen pixels of either
  long edge is drawn straight along that edge, from where it started to where the pen's movement
  along the edge ended. Strokes that start further away are drawn freehand.
- The ruler is a guide, not a marking: it is never stored in the score or exported. Hiding it, or
  leaving markup mode, removes it from the screen.

### ANNO-15 The file is never changed

- No markup action writes to the imported PDF. Markings live in the library, keyed by source page,
  in page space. The app opens every score through its library (`library.openFile`: the same file,
  by content, reopens the same score), so markings are there next time.
- Markings stay on their place in the music when the page is rotated, cropped, moved, hidden and
  shown again, or when the page range changes. Deleting a page from the score keeps its markings
  in the library until the score itself is deleted.

### ANNO-16 Shapes

- With **Shapes** on (a switch in the Markup menu, off by default, `annotate.shapes`), a pen or
  highlighter stroke that is nearly straight becomes a straight line, and a closed stroke that is
  nearly round becomes an ellipse; anything else stays freehand. The result is a stroke like any
  other. A switch rather than a toolbar tool, so the owner's toolbar stays as specified.
- *Nearly straight*: at least 6 pt between its ends, a path no more than 1.2× that distance, and
  no sample further from the line between the ends than 6% of its length (or 1.5 pt). *Nearly
  round*: ends within a quarter of its size, at least 315° round its centre, and samples within
  12% of the fitted ellipse on average (35% at worst). A ruled stroke is left as ruled.

### EXP-2 Export with ink

- Exporting writes a new PDF; the imported file is untouched. The player chooses which layers go
  in (by default, the visible ones) and how:
  - **flattened**: markings drawn into the page content, so every reader shows them the same;
  - **annotations**: pen and highlighter strokes as `/Ink` annotations (highlighter with a
    `Multiply` blend in its appearance), text notes as `/FreeText`, stamps as `/Stamp`, each with an
    appearance stream so readers that ignore annotation dictionaries still draw them.
- Page order, rotation, crop, hidden pages and the page range of the score are applied to the
  exported pages.
- How (M4 part 5): `score.export.pdf {path, mode, layers}`. The source pages are copied into a new
  document in the score's order (`pdfcraft-organize`), turned and cropped as the score shows them.
  Ink is the CPU renderer's own geometry (`scorecraft-render`'s fills: stroke outlines, glyph
  outlines of notes and dynamics, hairpin lines) written as filled paths, so the export looks
  like the reader on white paper; the highlighter's paths use an `ExtGState` with
  `/BM /Multiply`. Flattened ink follows the page's content, which is wrapped in `q`/`Q`. Text
  is drawn as outlines, so no font is embedded; `/FreeText` keeps the words in `/Contents`.
  The file is written beside the target and renamed into place. Snips are drawn into the page
  as their source page's vector content (PAGE-7); shortcuts are not exported.

### Drawing and colours under filters

- Ink keeps its own colour under sepia. Under dark, ink darker than mid grey is drawn light (its
  lightness inverted, hue kept) so a black pen stays legible on a dark page. Multiplying a dark
  page would hide the highlighter, so under dark it is laid over the page at 40% instead, keeping
  its hue and leaving the light notation readable through it.

### Speed (P5)

- Pen-down to first visible ink within one display frame where the platform allows it, with
  predicted samples drawn ahead and replaced by real ones (perf scenario P5).
- When a stroke is committed, the page shows the committed ink in place of the live stroke with no
  frame in between showing neither.
- Committing, erasing and undoing redraw only the page they changed; turning pages never waits on
  ink.

## Acceptance tests

| Id | Given | When | Then |
|---|---|---|---|
| INK-1 | An open library score, pen active | a stroke is drawn across page 1 | page 1 has one stroke on the active layer, in page space, and one undo step |
| INK-2 | INK-1 | the player undoes | the stroke is gone; redo brings it back unchanged |
| INK-3 | Page 1 rotated 90° | a stroke is drawn from the screen's top left to bottom right | the stroke's samples, mapped through the page's rotation, land where they were drawn; rotating back to 0° keeps them on the same notes |
| INK-4 | A stroke with pressures 0.1 then 1.0 | its outline is built | it is narrower at the start than the end, and never narrower than 25% of the width |
| INK-5 | A highlighter stroke that crosses itself over white paper | the page is composed | the crossing pixel has the same colour as a pixel the stroke covers once |
| INK-6 | A highlighter over black notation | the page is composed | the notation pixel stays black |
| INK-7 | Whiteout over notation, then a pen stroke over it | the page is composed | the notation is covered by paper colour and the pen shows on top |
| INK-8 | Three strokes, the eraser crosses two | one stroke-eraser gesture | two strokes are removed in one undo step |
| INK-9 | A straight stroke | the pixel eraser crosses its middle | two strokes remain, one on each side, with the original's style; undo restores the one |
| INK-10 | The active layer is locked | a stroke is drawn | it is refused with an error, and nothing changes |
| INK-11 | A hidden layer with a stroke | the page is composed | the stroke is not drawn |
| INK-12 | A stroke on page 2 | page 2 is moved to the end of the score | the stroke is still on that page and on the same notes |
| INK-13 | A stroke of 12,000 samples | it is submitted | it is refused (UI splits long strokes before 10,000) |
| INK-14 | A dark filter and a black pen stroke | the page is composed | the stroke is drawn light |
| INK-15 | A sharp 90° corner drawn with jitter | it is smoothed | the corner survives within 1 pt |
| INK-16 | Any markup command | it gets adversarial params | it returns an error, never panics (`panic_hunt`) |
| INK-17 | Markup mode, *Draw with Touch* off | a stroke arrives from a mouse or touch | nothing is drawn; the same stroke from a pen draws |
| INK-18 | *Draw with Touch* on | a stroke arrives from a mouse | it draws at full width |
| INK-20 | The pen is drawing | a touch starts, moves and lifts | nothing turns, pans or zooms; the stroke is unaffected |
| INK-21 | A one-finger touch is down (no pen) | the pen touches the screen | the touch is cancelled: lifting it doesn't turn the page |
| INK-22 | No pen is touching | two fingers pinch | the page zooms, as outside markup mode |
| INK-23 | The platform cancels a touch as a palm | it had started a stroke (*Draw with Touch* on) | the stroke is removed |
| INK-24 | Markup mode, touch lock on | a touch taps the right third, pinches, or (with *Draw with Touch*) draws | nothing happens; a pen still draws |
| INK-25 | The second swatch selected | the player picks the palette's lavender | the second swatch becomes lavender and the active tool draws in it |
| INK-26 | The palette | a colour that isn't one of its 30 is asked for | it is refused |
| INK-27 | The ruler shown across the screen | a pen stroke starts 5 pixels from its edge and wobbles along it | the stored stroke is a straight line along the edge |
| INK-29 | The default highlighter | a horizontal and a vertical stroke and a tap are drawn | the horizontal band is three times as thick as the vertical one, ends are cut vertically, the tap is an upright rectangle |
| INK-30 | *Round highlighter ends* on | a highlighter stroke is drawn | it has round ends and the same thickness in every direction; strokes drawn before keep the chisel |
| INK-31 | The pen just lifted | a touch starts 20 ms later, then another 200 ms later | the first is ignored, the second turns or zooms as usual |
| INK-32 | Pen swatches changed | the highlighter is chosen | its own swatches and palette show; picking from its palette doesn't change the pen's |
| INK-28 | The pen with white picked | a stroke is drawn | it is a whiteout stroke |
| INK-33 | The text tool, a page 612 pt wide | a long note is typed, and a short one is placed 20 pt from the right edge | the long note wraps at 526 pt (86%); the short one is moved left so its right side meets the page edge; neither crosses an edge |
| INK-34 | A score with one layer | a layer is added and a stroke drawn | the new layer is active and holds the stroke; choosing the first layer again sends the next stroke there |
| INK-35 | A layer with two strokes | it is deleted | the layer and both strokes go in one undo step; undo restores both; the last layer can't be deleted |
| INK-36 | A highlighter preset saved, then the pen chosen | the preset is applied | the highlighter is active with the preset's colour and width |
| INK-37 | The stamp tool, first time | the page is tapped | a **p** is placed, centred on the tap, 18 pt, in the pen's colour; near an edge it is kept inside the page |
| INK-38 | A crescendo placed by a tap | its end handle is dragged, then its middle | it stretches to the new end, then moves whole; each is one undo step; its opening stays a third of the size |
| INK-39 | Three strokes and a note inside a rectangle, one stroke mostly outside | they are selected and dropped on page 2 | the three move to page 2 keeping their ids, the other stays; one undo puts them back |
| INK-40 | A pen and a highlighter stroke selected | they are copied, scaled 2×, recoloured, then deleted | copies get new ids and originals stay; points spread and widths double about the centre; both take the colour; delete is one undo step; 11× is refused |
| INK-41 | Shapes on | a wobbly straight stroke and a rough circle are drawn | they are stored as a straight line and an ellipse; with shapes off the same stroke stays freehand |
| INK-42 | The pen | the pen button is pressed twice, then from the highlighter twice | eraser then pen; eraser then highlighter |
| INK-43 | The app in markup mode | a pen drags across the page, then a finger, then a finger with *Draw with Touch* | the pen and the last finger draw; the first finger draws nothing |
| INK-44 | The app, the pen drawing | a hand lands, drags across the page and lifts before the pen does | no turn; one stroke |
| INK-45 | The app in markup mode | the mouse drags across the page, before and after *Draw with Touch* | nothing, then one stroke; no turn either time |
| INK-46 | Highlighter with swatch 3 at 20 pt, shapes on, *Draw with Touch* on | the app is closed and opened again | the same tool, colours, swatches, widths and switches; markup mode itself starts off |
| INK-47 | A red stroke on score page 2, page 1 hidden | the score is exported flattened | two pages; the first shows the stroke; elsewhere the pages look like the source; the imported file is unchanged |
| INK-48 | A stroke and a text note | the score is exported as annotations | two annotations with appearances; the stroke renders from its appearance |
| INK-49 | A stroke on a hidden layer | the score is exported by default, then naming that layer | no markings, then one; bad modes, layers and paths are refused |
| INK-19 | Pen red, highlighter yellow | the player picks the highlighter's light blue swatch | new highlighter strokes are light blue; the pen is still red |

INK-33 is a `scorecraft-text` and engine test (`crates/text/src/layout.rs`,
`crates/engine/src/text_cmds/tests.rs`). INK-34 to INK-36 are engine tests
(`crates/engine/src/layer_cmds/tests.rs`); INK-37 and INK-38 are engine tests
(`crates/engine/src/stamp_cmds/tests.rs`). INK-29 and INK-30 are `scorecraft-ink` and
`scorecraft-render` tests; INK-31 is a
`scorecraft-input` test; INK-32 is an engine test. INK-20 to INK-23 are `scorecraft-input` tests of the palm filter the shell routes pointers through. INK-1 to INK-3, INK-8 to INK-13, INK-16 to INK-19, INK-24 to INK-28 are engine tests
(`crates/engine/src/annotate_cmds.rs`, `crates/engine/tests/panic_hunt.rs`); INK-4 and INK-15 are
`scorecraft-ink` tests; INK-5 to INK-7, INK-11 and INK-14 are `scorecraft-render` tests. Text,
stamps, layers, selection, shapes and export get their acceptance tests when their part of M4
lands, and are added to this table. INK-39 to INK-42 are engine tests
(`crates/engine/src/select_cmds/tests.rs`), with `scorecraft-ink` tests of the shape fit
(`crates/ink/src/shape.rs`). INK-43 to INK-46 are shell tests driving egui frames
(`crates/ui-egui/src/app.rs`). INK-47 to INK-49 are engine tests
(`crates/engine/src/export_cmds/tests.rs`), with `scorecraft-pdf` tests of the writer
(`crates/pdf/src/export.rs`).

## Not in scope

- Score Snips (PAGE-7) and Score Shortcuts (NAV-1): they are annotations in the model but belong to
  M6 with setlists.
- Importing ink from `.4sc` (IMP-5) and from PDF annotations (IMP-7): M9.
- Handwriting recognition and tilt-shaded pencil strokes: not in the public listings.
- Syncing markings between devices: M8 (the model already stamps every marking with a hybrid
  logical clock).

## Open questions

- Whether two separate enScore highlighter strokes darken where they overlap (screenshot 26 shows
  only strokes crossing themselves). ScoreCraft multiplies again, as two real passes would.
- Whether enScore's chisel stays upright on screen or on the page when a page is rotated.
- How enScore sets a note's maximum width (a share of the page, as measured, or a fixed size),
  and whether it has colour or font options (none show in screenshot 29).
- How strokes follow enScore's ruler edges; ANNO-17's snapping is ScoreCraft's own until observed.

Answered by the owner on 2026-10-31: highlighter build-up within a stroke and its ends (chisel,
screenshot 26; round ends as a setting); the eraser's size (the one width slider) and the pressure
profile (kept); stamps (ScoreCraft's own: Edwin dynamics and hairpins); a short delay after the
pen lifts (50 ms); the ruler (full screen width, horizontal, no turning yet); separate highlighter
swatches and palette (screenshot 28); the stylus shortcut (pen and eraser).

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | minor | M4 part 5: export with ink, how it is written; INK-47..49 |
| 2026-10-31 | minor | M4 part 4: the pen in the desktop app (pen told from a finger by pressure, library-backed scores, saved markup state); INK-43..46 |
| 2026-10-31 | minor | M4 part 3: rectangle select's details (visible, unlocked layers; drop point; ranges kept), shapes as a Markup-menu switch with its thresholds, the pen button's setting; INK-39..42 |
| 2026-10-31 | patch | INK-37, -38 for stamps (M4 part 2c) |
| 2026-10-31 | patch | INK-33 lands with text notes (M4 part 2b); notes are set in Inter, built in |
| 2026-10-31 | patch | INK-34..36 for layers and presets (M4 part 2a) |
| 2026-10-31 | minor | ANNO-8 from the owner's observation of enScore's text tool (screenshot 29): Type with Touch, new-note box and handle, plain text with no background, maximum width with wrapping, notes kept inside the page; text size 8–36 pt, 12 pt by default, on the size slider; INK-33 |
| 2026-10-31 | minor | Owner's answers to the open questions: chisel highlighter at full strength with a round-ends setting; one slider for eraser size, pressure kept by the pixel eraser; stamps are ScoreCraft's own (Edwin dynamics, hairpins with handles); 50 ms palm delay after the pen lifts; the ruler spans the screen, horizontal, turning moved to ANNO-18; separate highlighter swatches and palette; stylus shortcut switches pen and eraser; INK-29..32 |
| 2026-10-31 | minor | Owner's corrections: the padlock is a touch lock; the ruler stays (ANNO-17) with its button hidden by default; eraser options in the bar; width per tool; changeable swatches with a fixed 30-colour palette that replaces the selected swatch; whiteout is the pen in white |
| 2026-10-31 | minor | Owner's direction: palm rejection while the pen draws, without disturbing taps and two-finger zoom; platform behaviour noted |
| 2026-10-31 | minor | Owner's direction: pen-only by default with *Draw with Touch*, the toolbar, eight swatches, custom colour and width slider, one eraser with two modes, rectangle select (move, copy, delete, snip, shortcut) replaces the lasso, no ruler |
| 2026-10-31 | major | First version for M4: pen, pressure, highlighter, whiteout, erasers, text notes, stamps, layers, presets, selection, shapes, pen shortcuts, pen only, export with ink |
