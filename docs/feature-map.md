# Feature map

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-10-31 · **Change:** patch (LIB-6 partial; earlier: M6 part 3: NAV-1, PAGE-7 partial; earlier: M6 part 2: PRAC-1 partial; earlier: M6 part 1: LIST-4, LIST-6, IMP-6 partial; earlier: M5 part 3: LIB-13 partial, measured; earlier: M5 part 2: LIB-14 partial; earlier: M5 part 1: LIB-2, -5, -9, -15 partial; earlier: M4 part 5: EXP-2 partial; earlier: M4 part 3: ANNO-11, ANNO-12 and ANNO-16 partial; earlier: M4 part 2c: ANNO-9 partial; earlier: M4 part 2b: ANNO-8 partial; earlier: M4 part 2a: ANNO-4 and ANNO-10 partial; earlier: owner's answers: ANNO-9 is ScoreCraft's own, ANNO-18 ruler angle added, 124 rows; earlier: M4 part 1: 8 ANNO rows partial, ANNO-17 Ruler added from the owner's observation, 123 rows; earlier: M3: READ-4, READ-9 and PLAT-4 partial: the reader window's full screen, keep-awake and theme; earlier: M3: TURN-1..4 partial: keys, pedal map, taps and swipes go through the engine; earlier: M3: 8 reader rows partial (layouts, half-page and alternating turns, zoom, go to page, contents), live in the engine, the reader window comes later in M3; summary counts recounted; earlier: M1: 16 library, page, list and undo rows partial, live in the engine with no UI yet; earlier: READ-1 partial: `score.open` and `view.render` are live; earlier: owner's sync design and platform priorities in SYNC-1, -5, -7 and PLAT-1; earlier: reconciled with the Microsoft Store description S8, 4 rows added, 1 corrected; 13 rows marked ♻ PDFCraft reuse; first version 118 rows) · **Target:** enScore 3.7 (2026)

Every user-visible enScore capability we know of, one row each, with the ScoreCraft command that
will implement it. This is ScoreCraft's equivalent of PhotoCraft's Photoshop menu catalog: it is
the **denominator** of the breadth number in [`target-app-parity.md`](target-app-parity.md), the
source of `crates/engine/src/feature_catalog.rs` once the engine exists, and what
`cargo xtask parity` will measure into [`parity-checklist.md`](parity-checklist.md).

Rows come from public sources only ([clean-room](clean-room.md)); the per-row source tags (S1–S8)
are listed in [`observations/public-listings-2026-10-28.md`](observations/public-listings-2026-10-28.md).
Add a row when a new enScore release or an observation note shows a new capability; never delete a
row, mark it `dropped` with a reason.

User screenshots of the running app are indexed by photo number in
[`observations/screenshots/`](observations/screenshots/README.md) (Library and Lists so far), with the
feature-map ids each one informs. `obs.` in the Source column means an observation note in
[`observations/`](observations/README.md) (for ANNO, [the markup toolbar](observations/anno-markup-toolbar.md)). Cite them as "screenshot 13" in observation notes and specs.

## Columns

- **Id**: stable. Prefix = area. Used in specs, scorecard rows, commits (`READ-5: half-page turns`).
- **Tier**: what enScore charges for it. `Free`, `Pro` (one-time), `Sync` (subscription), `Acct`
  (needs an account). **ScoreCraft has no tiers:** every feature is free and works without an
  account. The column exists so parity reports can compare like with like.
- **Command**: the engine command id(s), `area.noun.verb` in lowerCamelCase segments, like
  PhotoCraft's (`view.turn.next`). Pure view state that never touches a document may be a shell
  command (`ui.*`), as in PhotoCraft's `UI_COMMANDS`.
- **Crate**: where the algorithm lives. **♻** marks a row built mainly on a PDFCraft crate rather than new code ([reuse](#reuse-from-pdfcraft)) (see [`architecture.md`](architecture.md#2-workspace-layout)).
- **P**: priority. `P0` = needed for the [alpha gate](roadmap.md#alpha-gate), `P1` = needed for
  beta, `P2` = full parity, `P3` = ours only or optional.
- **Est**: agent-hours including tests, uncalibrated until we have merged-PR history
  ([calibration](target-app-parity.md#remaining-effort-and-calibration)).
- **Status**: `missing` · `partial` · `done` (behaves as specified, with a test) · `dropped`.

## Summary

| Area | Prefix | Rows | P0 | Done | Partial | Missing |
|---|---|---:|---:|---:|---:|---:|
| Reading and display | READ | 15 | 8 | 0 | 9 | 6 |
| Page turning inputs | TURN | 8 | 4 | 0 | 5 | 3 |
| Annotation | ANNO | 18 | 8 | 0 | 17 | 1 |
| Library and organisation | LIB | 15 | 6 | 0 | 13 | 2 |
| Page editing | PAGE | 8 | 4 | 0 | 6 | 2 |
| In-score navigation | NAV | 5 | 1 | 0 | 2 | 3 |
| Setlists | LIST | 7 | 3 | 0 | 6 | 1 |
| Audio playback | PLAY | 7 | 0 | 0 | 0 | 7 |
| MIDI | MIDI | 5 | 0 | 0 | 0 | 5 |
| Practice tools | PRAC | 11 | 1 | 0 | 1 | 10 |
| Import and scanning | IMP | 8 | 1 | 0 | 1 | 7 |
| Export and print | EXP | 4 | 1 | 0 | 1 | 3 |
| Sync and backup | SYNC | 8 | 0 | 0 | 0 | 8 |
| Platforms, UI and languages | PLAT | 5 | 2 | 0 | 1 | 4 |
| **Total** | | **124** | **39** | **0** | **62** | **62** |

Sum of the estimates: **~925–1,870 agent-hours** for every row, after PDFCraft reuse, the S8 rows and the ruler (P0 250–530, P1 350–680, P2 233–456, P3 92–194) (P0 alone: see [`roadmap.md`](roadmap.md#alpha-gate)).

## READ: reading and display

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| READ-1 | Open a PDF score and render its pages crisply at any zoom, fast enough to turn without a visible delay | Free | S1 | `score.open`, `view.render` | pdf ♻ pdfcraft-cos, -render; render | P0 | 10–20 | partial |
| READ-2 | Single-page layout | Free | S1, S4 | `view.layout.single` | layout | P0 | 4–8 | partial |
| READ-3 | Two-page spread layout, with correct first-page placement for spreads | Free | S1 | `view.layout.twoPage` | layout | P0 | 6–12 | partial |
| READ-4 | Full-screen (chrome hidden) performance mode | Free | S1, S4 | `ui.view.fullScreen` | ui-egui | P0 | 3–6 | partial |
| READ-5 | Half-page turns: the next system appears before the full turn | Free | S1, S2, S3 | `view.turn.halfPage` (mode) | layout | P0 | 8–16 | partial |
| READ-6 | Alternating page turns for awkward page breaks | Free | S4 | `view.turn.alternating` (mode) | layout | P1 | 4–8 | partial |
| READ-7 | Display filters: light, dark, sepia (with intensity), inverted | Free | S1, S8 | `view.filter.set` | render | P1 | 6–10 | missing |
| READ-8 | Enhance filter for scanned pages (contrast / clean-up) | Free | S1 | `view.filter.enhance` | algo | P2 | 8–16 | missing |
| READ-9 | Keep the screen awake while a score or setlist is open | Free | S1, S4 | `ui.power.keepAwake` | app shell | P0 | 2–4 | partial |
| READ-10 | Two scores side by side | Free | S1, S3 | `view.split.open` | engine, ui-egui | P2 | 8–16 | missing |
| READ-11 | Multiple windows, each with its own score | Free | S1, S8 | `ui.window.new` | app shell | P2 | 8–16 | missing |
| READ-12 | Landscape Page Zoom (fit width in landscape, scroll within a page) | Free | S1 | `view.zoom.landscapeFit` | layout | P1 | 4–8 | partial |
| READ-13 | Non-destructive crop and margin adjustment per page | Free | S1 | `page.crop.set`, `page.margins.set` | doc, layout | P1 | 6–12 | missing |
| READ-14 | Robust with damaged pages and very large scores (hundreds of pages, huge scans) | Free | S1 | n/a (quality bar) | pdf ♻ pdfcraft-cos (xref repair), -render | P0 | 4–8 | missing |
| READ-15 | Pinch / scroll zoom and pan within a page | Free | inferred | `view.zoom.set`, `view.pan` | layout, ui-egui | P0 | 4–8 | partial |

## TURN: page turning inputs

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| TURN-1 | Tap zones turn forward and back | Free | S1, S4 | `view.turn.next`, `view.turn.previous` | ui-egui | P0 | 3–6 | partial |
| TURN-2 | Swipe to turn | Free | S1, S4 | `view.turn.next`, `view.turn.previous` | ui-egui | P0 | 3–6 | partial |
| TURN-3 | Keyboard arrow keys (and Page Up / Down, Space) | Free | S4 | `view.turn.next`, `view.turn.previous` | ui-egui | P0 | 1–2 | partial |
| TURN-4 | Bluetooth foot pedals (keyboard-emulating HID pedals, configurable key mapping) | Free | S1, S4 | `input.pedal.map` | input | P0 | 6–12 | partial |
| TURN-5 | MIDI pedals and controllers turn pages | Free | S1 | `input.midi.mapTurn` | midi, input | P2 | 6–12 | missing |
| TURN-6 | Record page turns during audio playback (half-page turns included); replay them automatically | Free | S1, S8 | `play.turns.record`, `play.turns.follow` | engine, audio | P2 | 8–16 | missing |
| TURN-7 | Turn across score boundaries inside a setlist (last page → next piece) | Pro | S1 | `list.turn.next` | engine | P1 | 3–6 | missing |
| TURN-8 | Jump to first / last page, go to page number | Free | inferred | `view.page.goto` | engine | P1 | 1–2 | partial |

## ANNO: annotation

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| ANNO-1 | Pen ink with stylus, finger ("draw with touch") or mouse, low latency | Free | S1, S2, S4 | `annotate.tool.pen`, `annotate.stroke` | ink | P0 | 16–30 | partial |
| ANNO-2 | Pressure-sensitive strokes (Apple Pencil, Windows Ink, Android stylus, Wacom) | Free | S1, S2 | `annotate.stroke` (pressure) | ink, input | P0 | 8–16 | partial |
| ANNO-3 | Highlighter that blends with the print like a real one (translucent multiply, no build-up within a stroke; stronger option) | Free | S1, S8 | `annotate.tool.highlighter` | ink | P0 | 4–8 | partial |
| ANNO-4 | Custom pen colours and widths, saved presets: eight changeable swatches, a fixed palette of extra colours that replaces the selected swatch, a width slider per tool | Free | S1, obs. | `annotate.pen.preset.set` | ink, engine | P1 | 4–8 | partial |
| ANNO-5 | Whiteout: cover printed marks, then write over them | Free | S1, S2 | `annotate.tool.whiteout` | ink | P1 | 4–8 | partial |
| ANNO-6 | Stroke eraser (removes whole strokes) | Free | S1 | `annotate.tool.strokeEraser` | ink | P0 | 4–8 | partial |
| ANNO-7 | Pixel eraser (erases part of a stroke) | Free | S1 | `annotate.tool.pixelEraser` | ink | P1 | 8–16 | partial |
| ANNO-8 | Typed text annotations, movable and editable: plain text with no background, wrapped to a maximum width, kept inside the page, a grip handle to move; Type with Touch | Free | S1, S4, obs. | `annotate.text.add`, `annotate.text.edit` | doc, text | P0 | 8–14 | partial |
| ANNO-9 | Stamps (ScoreCraft's own; not an enScore feature, per the owner's observation): dynamics pp to ff in the Edwin font, crescendo and diminuendo hairpins with stretch and move handles | n/a | obs. | `annotate.stamp.place` | ink, text | P3 | 10–20 | partial |
| ANNO-10 | Annotation layers: create, rename, show/hide individually (hide for performance) | Pro | S1, S3, S4 | `annotate.layer.new`, `annotate.layer.setVisible` | doc | P1 | 8–14 | partial |
| ANNO-11 | Pen shortcuts (stylus double-tap / barrel button switches tool) | Free | S1 | `annotate.pen.button`, `annotate.pen.buttonTool` | input, engine | P2 | 4–8 | partial |
| ANNO-12 | Select, move, copy, resize, recolour and delete existing annotations with a rectangle (the same rectangle starts a Score Snip or Score Shortcut) | Free | obs. | `annotate.select.rect`, `annotate.select.move` | ink, engine | P1 | 10–18 | partial |
| ANNO-13 | Undo / redo of every markup step | Free | inferred | `edit.undo`, `edit.redo` | ops | P0 | 6–10 | partial |
| ANNO-14 | Palm rejection while the pen draws, "pen only" drawing by default with Draw with Touch, and a touch lock | Free | S1, obs. | `input.pen.only`, `input.touch.lock` | input | P0 | 4–8 | partial |
| ANNO-15 | Annotations never alter the source PDF; stored separately per page | Free | S1 | n/a (model rule) | doc | P0 | 2–4 | partial |
| ANNO-16 | Shapes and lines (straight line, circle) snapped from strokes | Free | inferred | `annotate.shapes` | ink | P2 | 6–10 | partial |
| ANNO-17 | Ruler: a straight edge across the whole screen width to rule lines against, moved up and down; its button is hidden unless a setting shows it | Free | obs. | `annotate.ruler.show`, `annotate.ruler.set` | ink | P2 | 3–6 | partial |
| ANNO-18 | Turn the ruler to any angle, with its angle shown (lowest priority, owner 2026-10-31) | Free | obs. | `annotate.ruler.rotate` | ink | P3 | 2–4 | missing |

## LIB: library and organisation

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| LIB-1 | Import PDFs from a file picker, open-with / share sheet, drag and drop | Free | S1, S4 | `library.import.files` | io | P0 | 6–12 | partial |
| LIB-2 | Import a whole folder, with folder-level bulk add | Free | S1, S3 | `library.import.folder` | io | P1 | 4–8 | partial |
| LIB-3 | Folders | Free | S1 | `library.folder.new`, `library.score.move` | doc | P0 | 4–8 | partial |
| LIB-4 | Metadata fields: title, composer, tags, key | Free | S1 | `score.metadata.set` | doc | P0 | 4–8 | partial |
| LIB-5 | Search across title, composer, tags, key and folders | Free | S1 | `library.search` | search | P0 | 6–12 | partial |
| LIB-6 | Full-text search inside PDFs and practice notes | Free | S1 | `library.search.fullText` | search; pdf ♻ pdfcraft-render (text) | P1 | 6–12 | partial |
| LIB-7 | Automatic title and composer detection from generic scan names | Acct | S1, S2, S8 | `score.metadata.detect` | metadata ♻ pdfcraft-ocr | P2 | 8–16 | missing |
| LIB-8 | Detection for a whole folder of files at once | Acct | S1 | `library.metadata.detectAll` | metadata | P2 | 4–8 | missing |
| LIB-9 | Sort by title, composer, date added, recently opened | Free | S1 | `library.sort.set` | engine | P0 | 2–4 | partial |
| LIB-10 | Recently Deleted folder with restore | Free | S1, S3 | `library.trash.restore`, `library.trash.empty` | doc | P1 | 4–8 | partial |
| LIB-11 | Bookmarks: name a page, list and jump | Free | S1, S4 | `score.bookmark.add`, `view.bookmark.goto` | doc | P0 | 3–6 | partial |
| LIB-12 | Verify & Repair Library (check files, rebuild index) | Free | S1 | `library.verify` | format | P1 | 6–12 | partial |
| LIB-13 | Unlimited scores; library opens fast with thousands of PDFs | Free | S4 | n/a (performance budget) | format, search | P1 | 8–16 | partial |
| LIB-14 | Thumbnails / cover grid and list views | Free | inferred | `ui.library.view` | ui-egui, render | P1 | 6–12 | partial |
| LIB-15 | Recently opened list | Free | inferred | `library.recent.list` | engine | P1 | 2–4 | partial |

## PAGE: page editing

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| PAGE-1 | Reorder pages | Free | S1, S4 | `page.move` | doc | P0 | 3–6 | partial |
| PAGE-2 | Rotate pages | Free | S1, S4 | `page.rotate` | doc | P0 | 2–4 | partial |
| PAGE-3 | Hide pages (kept, not shown) | Free | S1, S4 | `page.setHidden` | doc | P0 | 2–4 | partial |
| PAGE-4 | Delete pages | Free | S1, S4 | `page.delete` | doc | P0 | 2–4 | partial |
| PAGE-5 | Crop and straighten scanned pages without altering the file | Free | S1 | `page.straighten`, `page.crop.set` | algo, doc | P1 | 8–16 | missing |
| PAGE-6 | Blank manuscript paper: single staff, grand staff or tab, any page size | Free | S1 | `score.newManuscript` | pdf ♻ pdfcraft-create | P1 | 4–8 | missing |
| PAGE-7 | Score Snips: copy bars from one place and paste them elsewhere, for example the bars after a page turn pasted before it, so the turn can come later | Pro | S1, S3, S4, S8 | `page.snip.create`, `page.snip.place` | doc, render | P1 | 12–24 | partial |
| PAGE-8 | Page Range: limit a score to a range of its PDF's pages | Free | S1 | `score.pageRange.set` | doc | P1 | 3–6 | partial |

## NAV: in-score navigation

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| NAV-1 | Score Shortcuts: tappable link on a repeat / D.S. / coda sign that jumps to its target | Pro | S1, S4 | `nav.shortcut.add`, `nav.shortcut.follow` | doc, engine | P1 | 10–18 | partial |
| NAV-2 | Create a shortcut from the viewer menu | Pro | S1 | `nav.shortcut.add` | ui-egui | P1 | 2–4 | missing |
| NAV-3 | Return from a jump (back to where the shortcut was followed) | Pro | inferred | `nav.back` | engine | P1 | 2–4 | missing |
| NAV-4 | Page-turn order follows shortcuts during a performance (repeat-aware turning) | Pro | S1, S4 | `nav.follow.mode` | engine | P2 | 8–16 | missing |
| NAV-5 | Table of contents from PDF outline | Free | inferred | `view.outline.goto` | pdf ♻ pdfcraft-model | P0 | 2–4 | partial |

## LIST: setlists

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| LIST-1 | Setlists ("Lists") for practice, lessons and concerts: create, reorder, rename | Pro | S1, S4 | `list.new`, `list.entry.add`, `list.entry.move` | doc | P0 | 6–12 | partial |
| LIST-2 | Page range per list entry | Pro | S1 | `list.entry.pageRange.set` | doc | P1 | 3–6 | partial |
| LIST-3 | Tempo per list entry (feeds the metronome) | Pro | S1 | `list.entry.tempo.set` | doc, practice | P1 | 2–4 | partial |
| LIST-4 | Pieces flow into one another; screen stays awake until the end | Pro | S1, S2 | `list.play` | engine | P0 | 4–8 | partial |
| LIST-5 | Share a whole setlist as one file (PDFs, order, page ranges, markings) and import it ready to play on another device | Pro | S1, S3, S8 | `list.export.bundle`, `list.import.bundle` | io, format | P1 | 10–18 | missing |
| LIST-6 | The same score appears in several lists without duplication | Pro | inferred | n/a (model rule) | doc | P0 | 1–2 | partial |
| LIST-7 | Placeholders / breaks in a list (intermission, spoken item) | Pro | inferred | `list.entry.addPlaceholder` | doc | P3 | 2–4 | partial |

## PLAY: audio playback

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| PLAY-1 | Attach an audio recording to a score | Free | S1 | `play.track.attach` | doc, audio | P2 | 6–12 | missing |
| PLAY-2 | Play at any speed without changing pitch (time-stretch) | Free | S1 | `play.speed.set` | audio | P2 | 12–24 | missing |
| PLAY-3 | Loop a passage (A–B) | Free | S1 | `play.loop.set` | audio | P2 | 4–8 | missing |
| PLAY-4 | Transpose playback by semitones (pitch-shift) | Free | S1 | `play.transpose.set` | audio | P2 | 10–20 | missing |
| PLAY-5 | Record page turns during playback (see TURN-6) | Free | S1 | `play.turns.record` | engine | P2 | 2–4 | missing |
| PLAY-6 | Transport controls while reading (play, pause, scrub) | Free | S1 | `play.toggle`, `play.seek` | ui-egui, audio | P2 | 4–8 | missing |
| PLAY-7 | Start the attached recording automatically when the score opens | Free | S8 | `play.autoStart.set` | doc, audio | P2 | 1–2 | missing |

## MIDI

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| MIDI-1 | Turn pages with MIDI pedals or controllers (learn mode) | Free | S1 | `input.midi.mapTurn` | midi | P2 | 6–12 | missing |
| MIDI-2 | Send Program Change and Bank Select when a score opens: one message or an ordered list | Free | S1, S8 | `score.midi.onOpen.set` | midi | P2 | 4–8 | missing |
| MIDI-3 | Send Song Select when a score opens | Free | S1, S8 | `score.midi.onOpen.set` | midi | P2 | 1–2 | missing |
| MIDI-4 | Choose MIDI input/output devices (USB, Bluetooth LE MIDI, network) | Free | S1 | `input.midi.devices` | midi | P2 | 8–16 | missing |
| MIDI-5 | Per-setlist sounds: the same score sends different MIDI messages in different setlists (list entry overrides the score's) | Pro | S8 | `list.entry.midi.set` | doc, midi | P2 | 2–4 | missing |

## PRAC: practice tools

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| PRAC-1 | Metronome with tap tempo | Free | S1, S4 | `practice.metronome.toggle`, `practice.metronome.tap` | practice | P0 | 6–12 | partial |
| PRAC-2 | Metronome subdivisions and beats per bar, accent | Free | S1 | `practice.metronome.set` | practice | P1 | 3–6 | missing |
| PRAC-3 | Nine metronome sounds (our own recordings or synthesis) | Free | S1 | `practice.metronome.sound.set` | practice | P2 | 4–8 | missing |
| PRAC-4 | Tuner (pitch detection from the microphone, reference pitch) | Free | S1, S8 | `practice.tuner.toggle` | practice | P2 | 12–24 | missing |
| PRAC-5 | Practice log kept automatically while playing: minutes per day, streaks, pieces practised and when | Free | S1, S8 | `practice.log.*` | practice, doc | P2 | 8–16 | missing |
| PRAC-6 | Daily practice goals | Free | S1, S8 | `practice.goal.set` | practice | P2 | 2–4 | missing |
| PRAC-7 | Pause and resume logging | Free | S1 | `practice.log.pause` | practice | P2 | 1–2 | missing |
| PRAC-8 | Practice notes: write a note about today's practice from inside the score (searchable, see LIB-6) | Free | S1, S8 | `score.note.add` | doc | P2 | 3–6 | missing |
| PRAC-9 | Shareable PDF practice report | Free | S1, S8 | `practice.report.export` | practice; pdf ♻ pdfcraft-create | P2 | 3–6 | missing |
| PRAC-10 | Practice log backup and restore | Free | S1 | `practice.log.export`, `practice.log.import` | practice, format | P2 | 2–4 | missing |
| PRAC-11 | Visual metronome pulse (silent performance cue) | Free | inferred | `practice.metronome.visual` | ui-egui | P3 | 2–4 | missing |

## IMP: import and scanning

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| IMP-1 | Camera scan with automatic edge detection, perspective straightening, crop and clean-up into a PDF | Free | S1, S2 | `scan.camera` | scan, algo; pdf ♻ pdfcraft-create | P2 | 18–32 | missing |
| IMP-2 | Wi-Fi flatbed and feeder scanners (eSCL / AirScan) | Free | S1 | `scan.network` | scan | P2 | 12–24 | missing |
| IMP-3 | Import photos / images (PNG, JPEG, HEIC, TIFF) as pages | Free | S1 | `library.import.images` | io ♻ pdfcraft-create | P1 | 2–4 | missing |
| IMP-4 | IMSLP search and import of public-domain scores | Pro | S1, S3, S4 | `imslp.search`, `imslp.import` | imslp | P2 | 12–24 | missing |
| IMP-5 | Import a forScore library from a `.4sc` file: pages, ink, stamps, layers | Free | S1, S4, S8 | `library.import.forscore` | fsc, io | P1 | 30–60 | missing |
| IMP-6 | Import report: what was imported, skipped, or could not be read | Free | inferred | n/a (result of import) | io | P0 | 2–4 | partial |
| IMP-7 | Import PDF annotations made in other PDF apps (ISO 32000 ink, highlight, free-text annots) | Free | ours | `library.import.files` | pdf ♻ pdfcraft-annot | P1 | 4–8 | missing |
| IMP-8 | Import MusicXML / MIDI rendered to pages | n/a | ours | `library.import.musicxml` | engrave | P3 | 80–160 | missing |

## EXP: export and print

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| EXP-1 | Print a score with or without annotations | Free | S1, S4 | `score.print` | io ♻ pdfcraft-print | P1 | 3–6 | missing |
| EXP-2 | Export a PDF with ink flattened, or as real PDF annotations, choosing which layers | Free | S1, S4 | `score.export.pdf` | pdf ♻ pdfcraft-cos, -organize | P0 | 4–8 | partial |
| EXP-3 | Export a setlist as a single combined PDF | Pro | inferred | `list.export.pdf` | pdf ♻ pdfcraft-organize | P2 | 1–2 | missing |
| EXP-4 | Export a page as an image | Free | inferred | `page.export.image` | render, codecs | P2 | 2–4 | missing |

## SYNC: sync and backup

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| SYNC-1 | Sync scores, markings, lists, bookmarks and settings across devices: over Wi-Fi between devices, or through Google Drive; optional sign-in | Sync | S1, S3, S4 | `sync.now`, `sync.status` | sync | P1 | 60–120 | missing |
| SYNC-2 | Cloud backup of the whole library | Sync | S1, S4 | `backup.create`, `backup.restore` | sync, format | P1 | 12–24 | missing |
| SYNC-3 | Offline-first: the device is the source of truth; the server holds a copy | Sync | S1 | n/a (design rule) | sync | P1 | n/a | missing |
| SYNC-4 | Account deletion removes server copies, keeps local ones | Sync | S1 | `sync.account.delete` | sync | P2 | 2–4 | missing |
| SYNC-5 | Download everything ahead of time on every device, in the background; nothing waits on the network when a score opens | Sync | S1 | n/a (performance) | sync | P2 | 8–16 | missing |
| SYNC-6 | Conflict-free merging of concurrent markings on two devices | Sync | inferred | n/a (CRDT) | sync | P1 | 20–40 | missing |
| SYNC-7 | Sync through storage the user already has, no ScoreCraft account: Google Drive first, then OneDrive and Dropbox, a folder the OS syncs, or WebDAV | n/a | ours (S3 review); enScore's free folder mirror is SYNC-8 | `sync.backend.set` | sync | P1 | 16–30 | missing |
| SYNC-8 | Mirror a library folder two ways with a folder on the computer or in OneDrive, no account or subscription | Free | S8 | `library.folder.mirror.set` | sync, io | P1 | 8–16 | missing |

## PLAT: platforms, UI and languages

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| PLAT-1 | Same app on desktop (macOS, Windows, Linux), tablet and phone (iPadOS, iOS, Android), Chromebook, web | Free | S1–S3 | n/a | apps | P0 (Windows tablet, then desktop), P1 (Android, web), P2 (iPad, with a Swift pen host) | 80–200 | missing |
| PLAT-2 | E-ink / e-reader mode (no animation, high contrast, partial refresh friendly) | Free | S3, S6, S8 | `ui.display.eInk` | ui-egui | P2 | 8–16 | missing |
| PLAT-3 | Fifteen interface languages (see [`localization-parity.md`](localization-parity.md)) | Free | S1, S8 | n/a | ui-egui | P1 | 40–80 | missing |
| PLAT-4 | Light and dark app themes, independent of the page filter | Free | S4 | `ui.theme.set` | ui-egui | P0 | 4–8 | partial |
| PLAT-5 | Apple Vision (visionOS): the iPad build runs there, with gaze-and-pinch input for turns and tools | Free | S1, S8 | n/a | apps | P2 | 4–8 | missing |

## Reuse from PDFCraft

ScoreCraft's PDF backend reuses [storytold/pdfcraft](https://github.com/storytold/pdfcraft)'s crates
instead of writing a PDF engine ([architecture §15](architecture.md#15-reusing-pdfcraft),
[decision D1](decisions.md#2026-10-28-d1-reuse-pdfcrafts-pdf-crates)). These rows become integration
work on top of PDFCraft rather than new code. Estimates are agent-hours, before → after.

| Id | What PDFCraft provides | Before | After |
|---|---|---:|---:|
| READ-1 | pdfcraft-cos parses (lazy, tolerant), pdfcraft-render rasterises (vendored hayro, JBIG2/CCITT/DCT/JPX) | 40–80 | 10–20 |
| READ-14 | pdfcraft-cos xref repair and resource caps; hayro paint-nesting and image-size caps | 10–20 | 4–8 |
| NAV-5 | pdfcraft-model typed catalog / outline access | 3–6 | 2–4 |
| LIB-6 | text extraction from pdfcraft-render inspection; our index stays in `search` | 12–24 | 6–12 |
| LIB-7 | pdfcraft-ocr (ocrs) recognises first-page text; our heuristics pick title and composer | 16–30 | 8–16 |
| PAGE-6 | pdfcraft-create writes the blank pages; we draw the staves | 8–14 | 4–8 |
| PRAC-9 | pdfcraft-create writes the report PDF | 6–10 | 3–6 |
| IMP-3 | pdfcraft-create turns PNG/JPEG/TIFF into pages | 6–10 | 2–4 |
| IMP-7 | pdfcraft-annot reads Ink, Highlight, FreeText, Square/Circle | 10–18 | 4–8 |
| EXP-1 | pdfcraft-print composes pages; craft-print spools | 8–16 | 3–6 |
| EXP-2 | pdfcraft-annot builds ISO 32000 annotations with appearance streams; pdfcraft-cos saves incrementally or flattens | 10–18 | 4–8 |
| EXP-3 | pdfcraft-organize concatenates and reorders pages | 3–6 | 1–2 |
| IMP-1 | pdfcraft-create assembles the scanned pages; capture and straightening stay ours | 24–40 | 18–32 |
| **Total** | | **124** | **39** | **0** | **62** | **62** |

Outside the rows, the PDF engine milestone (M2, [gap G1](gaps.md#g1-pdf-engine-parse-render-write))
shrinks from 80–160 h of new code to 20–40 h of integration: the `scorecraft-pdf` adapter, page-tile
caching for instant turns, and corpus checks.

## Reconciled with the Microsoft Store description (S8)

On 2026-10-28 every row was checked against the full Microsoft Store description
([observation note](observations/public-listings-2026-10-28.md#what-the-microsoft-store-description-adds-s8)).
Most of what it lists was already here (MIDI Program Change, Bank Select and Song Select, the
practice log, the shared setlist file, e-ink, the tuner, multiple windows); those rows now cite S8.

| Change | Rows |
|---|---|
| Added | PLAY-7 (recording starts at score open), MIDI-5 (different sounds per setlist), SYNC-8 (two-way folder mirror with the computer or OneDrive), PLAT-5 (Apple Vision) |
| Corrected | PAGE-7: Score Snips paste the bars that come *after* a turn *before* it, not the other way round |
| Corrected | PLAT-3: fifteen languages, not "twelve or more" |
| Refined | TURN-6 (half-page turns are recorded), ANNO-3 (highlighter blending), MIDI-2 (a list of messages), PRAC-5 and PRAC-8 (automatic log, a note about today's practice), LIST-5 (lands ready to play) |

SYNC-8 changes one assumption: enScore offers folder mirroring for free, so a folder backend is
parity work, not only ours (SYNC-7 shares its backend). PLAT-1 already listed e-readers through
PLAT-2; Apple Vision was missing.

## Ours only (not in enScore)

Rows tagged `ours` above (IMP-7, IMP-8, SYNC-7) and these capabilities come from the Crafting Apps
framework rather than from enScore. They are counted in the scorecard, not in parity:

- **Everything is a command**, drivable from the CLI, a JSON control channel and an MCP server
  ([`control-protocol.md`](control-protocol.md)).
- **Headless batch**: `scorecraft-cli` imports, tags, exports and checks libraries without a UI.
- **Open library format** on disk ([`file-format-parity.md`](file-format-parity.md#scorecraft-library-format)),
  readable without ScoreCraft.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | patch | LIB-6 full-text search (PDF text and practice notes) partial |
| 2026-10-31 | patch | M6 part 3: NAV-1 Score Shortcuts and PAGE-7 Score Snips partial |
| 2026-10-31 | patch | M6 part 2: PRAC-1 metronome with tap tempo partial (visual pulse; the click waits on audio output) |
| 2026-10-31 | patch | M6 part 1: LIST-4 playing a list, LIST-6 one score in many lists, IMP-6 import report partial |
| 2026-10-31 | patch | M5 part 3: LIB-13 partial: 10,000 synthetic scores open in 172 ms and search in 22 ms (perf P6, P7), not yet on the reference device |
| 2026-10-31 | patch | M5 part 2: LIB-14 library window (covers or list, search, sort, folders) partial |
| 2026-10-31 | patch | M5 part 1: LIB-5 search, LIB-9 sort, LIB-15 recently opened and LIB-2 folder import partial, live in the engine |
| 2026-10-31 | patch | M4 part 5: EXP-2 export with ink (flattened or as annotations, chosen layers) partial |
| 2026-10-31 | patch | M4 part 3: ANNO-12 rectangle select (move, copy, delete, scale, recolour), ANNO-16 shapes and ANNO-11 pen button partial, live in the engine |
| 2026-10-31 | patch | M4 part 2c: ANNO-9 stamps (dynamics and hairpins) partial, live in the engine and the CPU renderer |
| 2026-10-31 | patch | M4 part 2b: ANNO-8 text notes partial, live in the engine and the CPU renderer |
| 2026-10-31 | patch | M4 part 2a: ANNO-4 (presets) and ANNO-10 (layers) partial, live in the engine |
| 2026-10-31 | patch | ANNO-8 reworded from the owner's observation of the text tool |
| 2026-10-31 | minor | Owner's answers: ANNO-9 stamps are ScoreCraft's own (P1 → P3, Edwin dynamics and hairpins); ANNO-17 ruler spans the screen without turning; ANNO-18 ruler angle added at the lowest priority (124 rows) |
| 2026-10-31 | minor | M4 part 1: ANNO-1, -2, -3, -5, -6, -7, -14 and -15 partial (strokes, pressure, highlighter, whiteout, both erasers, pen only, Draw with Touch, touch lock, live in the engine with no UI yet); ANNO-17 Ruler added from the owner's observation (123 rows) and partial; ANNO-4, -12 and -14 reworded from the observation |
| 2026-10-31 | patch | M3: READ-4, READ-9 and PLAT-4 partial (shell commands `ui.view.fullScreen`, `ui.power.keepAwake`, `ui.theme.set` in the reader window; keep-awake on Windows only so far) |
| 2026-10-31 | patch | M3: TURN-1, -2, -3 and -4 partial (`input.pedal.map` with four pedal profiles; `input.key`, `input.tap`, `input.swipe`) |
| 2026-10-31 | patch | M3: READ-2, -3, -5, -6, -12, -15, TURN-8 and NAV-5 partial (reader commands live in the engine); summary counts recounted from the rows (M1's 16 partial rows were missing from it) |
| 2026-10-31 | patch | READ-1 partial: `score.open` opens a PDF and `view.render` renders pages through the page cache; the reader UI comes in M3 |
| 2026-10-28 | minor | Owner's answers: SYNC-1, SYNC-5 and SYNC-7 describe Google Drive first, Wi-Fi device sync and download-everything; PLAT-1 puts the Windows tablet first and the iPad Swift pen host at P2; rows and estimates unchanged |
| 2026-10-28 | minor | Reconciled with the Microsoft Store description (S8): added PLAY-7, MIDI-5, SYNC-8, PLAT-5 (118 → 122 rows); corrected PAGE-7 and PLAT-3; all rows ~910–1,830 → ~920–1,860 h |
| 2026-10-28 | minor | 13 rows marked ♻ PDFCraft reuse; estimates lowered (all rows ~990–1,990 → ~910–1,830 h) |
| 2026-10-28 | major | First version: 118 rows from public listings (S1–S6) |
