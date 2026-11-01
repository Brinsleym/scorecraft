# Annotation parity: ink, tools and layers

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-10-31 · **Change:** patch (M4 part 5: export with chosen layers; earlier: M4 part 3: ANN-17 rectangle select, ANN-18 shapes and ANN-19 pen button partial; earlier: minor (owner's answers and text observation: chisel highlighter, own highlighter colours, stamps ours, ruler across the screen, ANN-21 ruler angle; earlier: M4 part 1: stroke engine, highlighter, whiteout, erasers, pen only, touch lock and ruler partial in the engine; earlier: first version, all rows missing) · **Target:** enScore 3.7 (2026)

How ScoreCraft's markup compares with enScore 3.7 (PhotoCraft's brush-parity checklist, for a score
reader). Status and evidence as in [`ui-parity.md`](ui-parity.md).

**Summary (estimated):** about 15% ready (2026-10-31): the stroke engine, the highlighter, whiteout, both
erasers, pressure, palm rejection logic and the ruler work in the engine and the CPU renderer with
tests ([spec](spec/ink.md)); nothing is drawn by a pen in the app yet (M4 part 4), so no ink-feel
row can be judged. Every row has engine commands; the pen draws in the desktop app (M4 part 4) and exports with its ink (part 5); feel is judged on a device.

## Ink feel

| Id | Target | Rows | Status | Evidence |
|---|---|---|---|---|
| ANN-1 | Pen-down to first visible ink ≤ 1 frame at 120 Hz on supported tablets (predicted points, replaced by real ones) | ANNO-1 | missing | |
| ANN-2 | Pressure maps to width with a configurable curve; no jumps at stroke start | ANNO-2 | partial | `PressureCurve` (floor 0.25, γ 0.6) and smoothed start pressure, tested in `scorecraft-ink` (INK-4); not yet on a device |
| ANN-3 | Stroke smoothing that keeps sharp corners of handwritten fingerings | ANNO-1 | partial | Corner-aware smoothing and Catmull–Rom subdivision; a jittered 90° corner survives within 1 pt (INK-15) |
| ANN-4 | Strokes stay crisp at every zoom (vector, not bitmap) | ANNO-1 | partial | Stored as page-space samples, drawn per render scale from the outline; cached per page and scale |
| ANN-5 | Palm rejection while the pen draws and for 50 ms after it lifts; pen only by default with Draw with Touch; touch lock | ANNO-14 | partial | `PalmFilter` in `scorecraft-input` (INK-20..23, -31), `input.pen.only` and `input.touch.lock` in the engine (INK-17, -18, -24); the shell doesn't route pointers through it yet |

## Tools

| Id | Target | Rows | Status |
|---|---|---|---|
| ANN-10 | Pen with colours, widths and presets; eight changeable swatches and a fixed 30-colour palette per tool (pen and highlighter differ); width per tool | ANNO-1, ANNO-4 | partial (swatches, palettes, widths and presets in the engine, INK-25, -32, -36; no UI) |
| ANN-11 | Highlighter: upright chisel nib (round ends as a setting), multiplied at full strength, no build-up within a stroke | ANNO-3 | partial (INK-5, -6, -29, -30) |
| ANN-12 | Whiteout (opaque page-coloured ink, then write over) | ANNO-5 | partial (filter paper colour, INK-7, -28) |
| ANN-13 | Stroke eraser | ANNO-6 | partial (INK-8) |
| ANN-14 | Pixel eraser splitting strokes | ANNO-7 | partial (INK-9) |
| ANN-15 | Text notes: plain text, wrapped to a maximum width, kept inside the page, grip handle to move; Type with Touch; size 8–36 pt (12 pt default) on the slider; colour, edit | ANNO-8 | partial (engine and CPU renderer, Inter, INK-33; the box and handle come with the UI) |
| ANN-16 | Stamps (ScoreCraft's own): dynamics pp to ff in Edwin, crescendo and diminuendo hairpins with stretch and move handles | ANNO-9 | partial (engine and CPU renderer, INK-37, -38; handles drawn with the UI) |
| ANN-17 | Rectangle select: move, copy, resize, recolour, delete; starts snips and shortcuts | ANNO-12 | partial (move to the same or another page, copy, delete, scale 0.1–10×, recolour, each one undo step, INK-39, -40; snips and shortcuts with setlists in M6) |
| ANN-18 | Shapes and straight lines | ANNO-16 | partial (nearly straight strokes become lines, closed round ones ellipses, INK-41) |
| ANN-19 | Stylus button / double-tap switches pen and eraser | ANNO-11 | partial (`annotate.pen.button` switches and returns, the pair is a setting, INK-42; the app's stylus events in part 4) |
| ANN-20 | Ruler across the whole screen width, moved up and down, its button hidden unless a setting shows it | ANNO-17 | partial (ruling along either edge, INK-27; no UI) |
| ANN-21 | Turn the ruler, angle shown (lowest priority) | ANNO-18 | missing |

## Layers and history

| Id | Target | Rows | Status |
|---|---|---|---|
| ANN-30 | Layers: new, rename, reorder, show/hide, lock; hidden layers excluded from performance view and optionally from export | ANNO-10, EXP-2 | partial (all in the engine with undo, INK-11, -34, -35; export of chosen layers, INK-49) |
| ANN-31 | Undo/redo per score, one step per stroke | ANNO-13 | partial (one step per stroke and per eraser gesture, INK-2, -8, -9) |
| ANN-32 | Markings never alter the PDF; survive rotate, crop, reorder and page-range changes | ANNO-15 | partial (page space, rotate and reorder tested, INK-3, -12; crop isn't applied by the reader yet) |
| ANN-33 | Markings imported from `.4sc` and from PDF annotations land in the right layers | IMP-5, IMP-7 | missing |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | patch | M4 parts 4 and 5: the pen in the desktop app; export with chosen layers |
| 2026-10-31 | patch | M4 part 3: ANN-17 rectangle select, ANN-18 shapes, ANN-19 pen button partial |
| 2026-10-31 | patch | M4 part 2c: ANN-16 stamps partial |
| 2026-10-31 | patch | M4 part 2b: ANN-15 text notes partial |
| 2026-10-31 | patch | M4 part 2a: ANN-30 layers and ANN-10 presets partial |
| 2026-10-31 | minor | Owner's answers and text observation: ANN-11 chisel at full strength, ANN-10 separate highlighter colours, ANN-15 observed text behaviour, ANN-16 stamps ours, ANN-19 pen and eraser, ANN-20 ruler across the screen, ANN-21 added |
| 2026-10-31 | minor | M4 part 1: ANN-2..5, -10..14, -31, -32 partial; ANN-20 ruler added; ANN-17 becomes rectangle select (owner's observation) |
| 2026-10-28 | major | First version |
