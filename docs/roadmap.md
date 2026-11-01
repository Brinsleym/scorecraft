# Roadmap

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-10-31 · **Change:** patch (M6 🟡 part 3: shortcuts and snips; earlier: M6 🟡 part 2: the metronome; earlier: M6 🟡 part 1: playing a list; earlier: M5 🟡 part 2: the library window; earlier: M5 🟡 part 1: library search and sort; earlier: M4 part 5: export with ink; earlier: M4 part 4: the pen in the desktop app; earlier: merged with M3 done on main: M3 done: Windows check passed; earlier: M4 in progress: part 3, rectangle select, shapes and the pen button; earlier: M4 in progress: ink engine and markup commands; earlier: M3 in progress: control channel; earlier: reader window; earlier: reader layouts and turn commands; earlier: M1 done: geom, raster, doc, ops, format and the library commands; earlier: M2 done, PDF-2 partial on PDFCraft render gaps; earlier: M2 in progress: PDF adapter; earlier: owner's priorities: pedal moves to M3, Windows tablet first, Google Drive sync; earlier: M0 skeleton complete: workspace, CI, command registry, parity, scorecard, testkit; earlier: new S8 rows placed in M8–M10; M2 becomes PDFCraft integration) · **Target:** enScore 3.7 (2026)

Forward-looking plan: milestones, the **Current focus** list, and what's next with estimates.
Where we stand today is in [`target-app-parity.md`](target-app-parity.md) and the ranked work list
in [`gaps.md`](gaps.md); the one-page summary is [`ROADMAP.md`](../ROADMAP.md).

Status legend: ✅ done · 🟡 in progress · ⬜ not started. Hours are agent wall-clock hours for one
agent working sequentially, uncalibrated ([calibration](target-app-parity.md#remaining-effort-and-calibration)).

**Stage: pre-alpha** (engine foundation, PDF reading, the library store and a Windows reader app with pedal turns; ink in the desktop app, unchecked on a tablet; no library UI yet). Alpha needs the gate below: ~105–287 h.

## Alpha gate

The workflows a performing musician runs every week, checked end to end on the main platform
(desktop first, then one tablet), including quitting and reopening with everything intact. Any
**no**, or a **partial** that blocks the workflow, keeps ScoreCraft pre-alpha.

| Core workflow | Works end to end? | Rows | Hours to pass |
|---|---|---|---:|
| Build a library: import PDFs and a folder, set title/composer/tags, find a score by search | code complete: import, folders, search and the library window, and the Score panel's details (title, composer, arranger, key, genre, tags); needs the device run-through | LIB-1, LIB-3..5, LIB-9, IMP-6 | 40–80 |
| Read: open a score, single and two-page layouts, half-page turns, turn by tap, keys and a Bluetooth pedal, screen stays awake | yes on Windows with a pedal (M3 check); the tablet check is left | READ-1..5, READ-9, READ-15, TURN-1..4 | 60–120 |
| Mark up: pen with pressure, highlighter, stroke eraser, text note, undo; quit, reopen, markings intact and the PDF untouched | code complete (M4); needs the tablet check with a pen | ANNO-1..3, ANNO-6, ANNO-8, ANNO-13..15 | 60–110 |
| Perform a setlist: build a list, play it through with the pedal, pieces flow, no crash | code complete (M6: Lists panel, `list.play`); needs a run-through on the device | LIST-1, LIST-4, LIST-6, NAV-5 | 15–30 |
| Edit pages: reorder, rotate, hide, delete; bookmarks | code complete: the Score panel turns, hides, reorders and deletes pages and keeps bookmarks; needs the device run-through | PAGE-1..4, LIB-11 | 10–20 |
| Share: export a PDF with markings that opens in other PDF readers | code complete (M4 part 5); check the files in other readers | EXP-2 (♻ pdfcraft-annot) | 4–8 |
| Count in: metronome with tap tempo | code complete: pulse, tap tempo and the click (Windows and macOS); check the click on the device | PRAC-1 | 6–12 |

Plus the skeleton (M0, done 2026-10-28) and the stability floor: never-crash lints on every crate,
`panic_hunt` green, fuzzed `pdf` and library loader.

**Result: does not pass** (2026-10-31: code complete for reading, markup, setlists and export; left: the end-to-end checks on the Windows tablet; the `pdf` and library loaders have deterministic mutation tests (`cargo-fuzz` targets on nightly are the next step)).

## Milestones

| M | Status | Name | Where we are |
|---|---|---|---|
| M0 | ✅ | Skeleton | workspace, 29 crate and 6 app stubs, never-crash lints, `cargo xtask layers`, `wasm` and `ci`; CI on Linux, macOS, Windows, wasm32 and MSRV 1.85; engine command registry, feature catalog, `xtask parity` (0 / 122) and `panic_hunt`; CLI `commands` and `run`; generated scorecard; testkit synthetic and hostile PDFs (2026-10-28, PRs #5–#8) |
| M1 | ✅ | Foundation | `geom` (transforms, page boxes, curves), `raster` (bitmaps, tiles, pyramids, damage), `doc` (the whole §5 model), `ops` (reversible ops, transactions, per-score and library undo), `format` v1 (blobs, per-record JSON, atomic saves with a journal, verify and repair); about 35 library, page, list and undo commands; parity 17 / 122. DoD: property tests on history (random edits stay sound, undo-all restores, redo-all replays) and on the store; a save crashed at every write reopens to the old or new library; an unmapped `doc` field fails to compile in `format` (2026-10-31) |
| M2 | ✅ | PDF integration (PDFCraft) | `scorecraft-pdf` adapter over `pdfcraft-render` 0.15.0 (`65ff3a00`): open, pages, metadata, outline, page and tile render, text; no PDFCraft type in its API (checked by `xtask layers`); page and tile cache with background pre-rendering and dark/sepia filters in `scorecraft-render`; `score.open` and `view.render` live (parity 1 / 122), CLI `info` and `render` to PNG. fuzz target (`crates/pdf/fuzz`, plus 600 mutations in CI); corpus harness (`cargo xtask test-corpus`): PDF-1 done, engraved scores all match, 15/20 overall, renderer gaps reported for PDFCraft ([file-format-parity](file-format-parity.md#corpus-results-2026-10-31-pdfcraft-65ff3a00)); 300 dpi render 41 ms on the tablet (P13 ≤ 300 ms). Closed 2026-10-31 by the owner with PDF-2 partial: the gaps are PDFCraft's and are tracked in G1; re-run `cargo xtask test-corpus` on every PDFCraft pin bump |
| M3 | ✅ | Reader app | Part 1: [page-turning spec](spec/page-turning.md); `scorecraft-layout` (single and two-page layouts with first page alone, half-page and alternating turns, fit page or width, zoom and pan); 13 reader commands (`view.turn.*`, `view.layout.*`, `view.page.goto`, `view.zoom.*`, `view.pan`, `view.outline.goto`, `view.viewport.set`, `view.state`); the reader follows the library score's page order, hidden pages, rotation and page range; P1 turn from cache 0.1 ms (synthetic, cloud); parity 25 / 122. Part 2: `scorecraft-input` (key chords, pedal map with four profiles, repeat and bounce filter, tap thirds, swipe classifier) and `input.pedal.map`, `input.key`, `input.tap`, `input.swipe`; parity 29 / 122. Part 3: the `scorecraft` desktop window on craft-ui (one egui fork), reader canvas with key, tap, swipe, pinch and drag input, toolbar, full screen, light and dark themes, keep-awake on Windows, saved preferences and last score, `ui.*` shell commands, `view.snapshot` (CPU picture of the reader); parity 32 / 122. Part 4: control channel (`scorecraft --control`, [protocol](control-protocol.md)): `command`, `command_batch`, `command_list`, `ui.state`, `ui.set`, `ui.key`, `ui.pointer`, `ui.screenshot`; an agent turns a page and screenshots it in a test. Windows check (WIN-RB, 2026-10-31): the owner's Bluetooth pedal turns one page per press in key and swipe modes; real library scores render correctly in single, two-page, half-page, full-screen, dark and rotated landscape views; turns from cache 0.06 ms (P1); `view.turn.next` round trip over the channel 17 ms median. Follow-ups in [gaps](gaps.md) G2 and G6 |
| M4 | 🟡 | Ink | Part 1: [ink spec](spec/ink.md) with the owner's [toolbar observation](observations/anno-markup-toolbar.md); `scorecraft-ink` (cleaning, pressure curve, corner-keeping smoothing, outlines, erasers, ruler), CPU ink in `scorecraft-render` (multiply highlighter without build-up, whiteout, dark-filter ink), palm filter in `scorecraft-input`; 16 markup commands (`annotate.tool.*`, `annotate.stroke`, swatches and palette, `input.pen.only`, `input.touch.lock`, ruler, `annotate.mode.set`); ink in the reader frame, `view.snapshot` and `view.render`; parity 40 / 124 (ANNO-17 Ruler and ANNO-18 ruler angle added). Owner's answers folded in: chisel highlighter at full strength with a round-ends setting, separate highlighter swatches and palette, a 50 ms palm grace after the pen lifts, the ruler as a band across the screen; text notes and stamps (ScoreCraft's own, Edwin) specified for part 2. Part 2a: layer commands (new, rename, show, lock, reorder, delete, active; `MoveLayer` op) and presets; parity 42 / 124. Part 2b: `scorecraft-text` (Inter built in through `scorecraft-pdf`, Edwin from the fonts directory; measuring, wrapping, glyph outlines), text notes drawn by the CPU renderer, `annotate.tool.text`, `annotate.text.add`, `annotate.text.edit`; parity 43 / 124. Part 2c: stamps, dynamics (Edwin, fallback Inter) and hairpins (`Hairpin` in the model) with `annotate.tool.stamp`, `annotate.stamp.place`, `annotate.stamp.edit`; parity 44 / 124. Part 3: rectangle select (`annotate.tool.select`, `annotate.select.*`: move to any page, copy, delete, scale, recolour), shapes (`scorecraft-ink` straightens lines and ellipses; `annotate.shapes`) and the pen button (`annotate.pen.button`, `annotate.pen.buttonTool`); parity 47 / 124. Part 4: the pen in the desktop app: a markup bar (Draw with Touch, touch lock, tools, eraser modes, stamps, swatches and palette, width slider, selection actions, undo, settings for the ruler button, round highlighter ends, shapes and the pen button), pen, touch and mouse through the palm filter, live strokes kept until the page redraws, the ruler band, the selection and the new note's box; scores open through the library (`library.openFile`) and markup settings are kept between launches. Part 5: export with ink (`score.export.pdf`: flattened or as `/Ink`, `/FreeText`, `/Stamp` annotations with appearances, chosen layers, the score's order, rotation and crop; `scorecraft-pdf` on `pdfcraft-cos` and `pdfcraft-organize`); parity 48 / 124. Code complete; left: a check on a Windows tablet with a pen, and the fixes it brings |
| M5 | 🟡 | Library | Part 1: [library spec](spec/library.md); `scorecraft-search` (folding without case or accents, word-start matching ranked by field, natural sort keys); `library.search`, `library.sort.set` (title, composer, date added, recently opened; kept between launches), `library.recent.list`, `library.import.folder` (subfolders, keep folders, skips files already imported, one undo step); `library.scores` follows the sort; parity 52 / 124. Part 2: the library window (`ui.library.show`, `ui.library.view`): covers rendered on a worker thread (`library.cover`, `Session::cover_job`) or a list, the search box, sort, folders and folder import; parity 53 / 124. Part 3: `library_perf` measures P6 and P7 (10,000 scores: 172 ms to open and list, 22 ms to search on the development container; the reference device is the owner's). Code complete; left: P6 and P7 on the reference device |
| M6 | 🟡 | Performance (→ alpha) | Part 1: [performance spec](spec/performance.md); `list.play`, `list.stop`, `list.state`: turns past an entry's end open the next entry (back: the previous at its last page), entries show their page range, breaks are passed and reported, opening a score by hand stops; one score in many lists tested (LIST-6); the app reports every import (IMP-6); parity 54 / 124. Part 2: the metronome (`scorecraft-practice`; `practice.metronome.toggle`, `.tap`, `.set`, `.state`; entry tempos set it; a pulse along the reader's top edge, the click waits on audio output); parity 55 / 124. Part 3: Score Shortcuts (`nav.shortcut.add`, `nav.shortcut.follow`; a tap on one jumps) and Score Snips (`page.snip.create`, `page.snip.place`; drawn into the page from the source page at the same scale), both from the rectangle tool, and `annotate.delete`; parity 57 / 124. Part 4: the Lists panel in the library window (make, fill, reorder and play lists; a cover's menu adds it) and a now-playing strip with *Stop list* in the toolbar; a list's next score keeps the window, layout, turn mode and fit. Then shortcut outlines in markup mode, snips in export (vector, from the source page) and snip resizing. Left for the alpha gate: the metronome click (audio output) and the gate's end-to-end checks on the Windows tablet |
| M7 | ⬜ | Mobile | |
| M8 | ⬜ | Sync | |
| M9 | ⬜ | Interop (→ beta with M7, M8) | |
| M10 | ⬜ | Practice and MIDI | |
| M11 | ⬜ | Parity | |

## Current focus

Ranked; pick from the top unless an issue is assigned to you.

1. ~~**M0 skeleton**~~ ([G3](gaps.md#g3-stability-and-data-safety)), done 2026-10-28: ~~workspace, every crate stub
   registered in `xtask/src/layers.rs`, lints, `clippy.toml`, `rustfmt.toml`, `xtask layers` and
   `wasm`, `xtask ci`, CI on Linux, macOS, Windows and wasm, `xtask parity`, the engine command registry with `panic_hunt`~~ (done
   2026-10-28), `xtask scorecard` and testkit with a synthetic PDF generator~~.
2. **PDFCraft integration** ([G1](gaps.md#g1-pdf-engine-parse-render-write),
   [decision D1](decisions.md#2026-10-28-d1-reuse-pdfcrafts-pdf-crates)): pin a PDFCraft revision,
   repeat its hayro, lopdf and vello_common patches (not its egui and winit ones; architecture §15),
   write the `scorecraft-pdf` adapter, and run a 200-file public-domain score corpus through it
   (correctness, speed, wasm size). A test build on 2026-10-28 showed the crates we take check
   natively and for wasm32 with no C code.
3. ~~**M1 foundation**~~, done 2026-10-31: the model, history and library store under the reader.
4. **Page turns and the pedal next** (owner's priority, 2026-10-28): M3 includes Bluetooth pedal
   mapping (TURN-4), on the owner's Windows tablet first. Start with `docs/spec/page-turning.md`,
   then `ink.md`, `library.md`, `setlists.md` from the public listings plus observation passes ([G17](gaps.md#g17-unknowns-behaviour-not-yet-observed)).
5. **Create the corpus repository** with pinned public-domain scores and hostile synthetic PDFs.

## What's next, with estimates

| Rank | Milestone | Estimate |
|---|---|---:|
| 1 | ~~M3 reader app~~ (done 2026-10-31) | — |
| 2 | M4 ink: code complete (engine, the pen in the desktop app, export); a Windows tablet check left | 5–20 h |
| 3 | M5 library: code complete; P6 and P7 on the reference device, polish from use | 5–15 h |
| 4 | M6 performance: code complete; the gate checks on the device left | 5–15 h |
| Later | M7 mobile, M8 sync, M9 interop, M10 practice and MIDI, M11 parity; last of all, turning the ruler (ANNO-18) | 490–980 h |

## Milestone definitions

Each milestone has a **definition of done (DoD)** and must leave `main` green on all tier-1
platforms (macOS, Windows, Linux) plus a wasm build.

| M | Name | Scope (key items) | DoD / acceptance |
|---|---|---|---|
| **M0** | Skeleton | Workspace, all crate stubs, lints, `xtask` (layers, wasm, ci, parity, scorecard), CI matrix, testkit | `cargo test --workspace` green; `cargo xtask wasm` green; layering check passes; `docs/parity-checklist.md` generated (0 / 122) |
| **M1** | Foundation | geom, raster (tiles, pyramids), doc (full model incl. layers, snips, shortcuts, setlists), ops (history), format v1 (blobs, records, atomic saves) | Property tests on history and the store; crash-during-save test; format fails to compile on an unmapped doc field |
| **M2** | PDF integration | `scorecraft-pdf` adapter over PDFCraft's `cos`, `model`, `render`, `annot`, `organize`, `create` at one pinned revision with its hayro patches; tile cache and pre-rendering; outline and text access | ≥ 95% of the pinned score corpus renders within tolerance; adapter fuzz target; no `pdfcraft-*` type in the adapter's public API; 300 dpi page render within budget |
| **M3** | Reader app | egui shell on `craft-ui` (one egui, architecture §16), library-less open, single/two-page, half-page and alternating turns, zoom, full screen, keep-awake, tap/key turns, Bluetooth pedal mapping (TURN-4), control channel, CLI `run`, Windows tablet and desktop first, web after | Turn latency within budget from cache; agent drives a turn and screenshots it over the control channel |
| **M4** | Ink | ink engine, pen/highlighter/whiteout/erasers, text notes, stamps, layers, lasso, undo/redo, export with ink (♻ pdfcraft-annot) | Ink latency budget on desktop; strokes survive rotate/crop/reorder; exported PDF annotations open in two other readers |
| **M5** | Library | import files/folders/images, folders, metadata, search (incl. full text), sort, bookmarks, Recently Deleted, verify/repair, thumbnails | 10,000-score synthetic library opens and searches within budget |
| **M6** | Performance | setlists, page ranges, tempos, Score Shortcuts, Score Snips, metronome | **Alpha gate passes** |
| **M7** | Mobile | Android and iOS/iPadOS shells (iPad: a thin Swift host for the Apple Pencil, decision D3), pen pressure, share-sheet import, store packaging; the iPad build on Apple Vision (PLAT-5) | Alpha gate passes on one Android tablet and one iPad |
| **M8** | Sync | op-log sync, CRDT merge, Google Drive first (library, configuration, annotations), then device-to-device over Wi-Fi and via Drive with optional sign-in, download everything (architecture §9), backup/restore; OneDrive, Dropbox and the two-way folder mirror (SYNC-8) later | Randomised two-device histories converge; offline edits on both devices merge without loss |
| **M9** | Interop | forScore `.4sc` import, camera and eSCL scanning, IMSLP, title/composer detection (♻ pdfcraft-ocr), print (♻ pdfcraft-print) | Two-team `.4sc` spec and import of the sample corpus; **beta** (with M7 and M8) |
| **M10** | Practice and MIDI | audio attach and start at open, time-stretch, pitch-shift, loops, recorded turns, tuner, practice log, MIDI in/out with per-setlist messages | Practice workflow end to end; MIDI learn with two controllers |
| **M11** | Parity | localization (15 locales), e-ink mode, multi-window, side by side, remaining P2 rows | Breadth ≥ 95% of the feature map |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | patch | Alpha gate re-graded: reading, markup, setlists, export, library and page editing code complete in the app; the Score panel (details, pages, bookmarks) added |
| 2026-10-31 | patch | M6 🟡: part 3, shortcuts and snips; parity floor 57 |
| 2026-10-31 | patch | M6 🟡: part 2, the metronome; parity floor 55 |
| 2026-10-31 | patch | M6 🟡: part 1, playing a list, one score in many lists, import report; parity floor 54 |
| 2026-10-31 | patch | M5 🟡: part 3, P6 and P7 measured on the development container |
| 2026-10-31 | patch | M5 🟡: part 2, the library window with covers; parity floor 53 |
| 2026-10-31 | patch | M5 🟡: part 1, search, sort, recent, folder import; parity floor 52 |
| 2026-10-31 | patch | M4 🟡: part 5, export with ink; code complete, awaiting a tablet check; parity floor 48 |
| 2026-10-31 | patch | M4 🟡: part 4, the pen in the desktop app |
| 2026-10-31 | patch | Merged with M3 ✅ on main; alpha ~135–345 h |
| 2026-10-31 | patch | M4 🟡: part 3, rectangle select, shapes, pen button; parity floor 47 |
| 2026-10-31 | patch | M4 🟡: part 2c, stamps; part 2 done; parity floor 44 |
| 2026-10-31 | patch | M4 🟡: part 2b, text notes; parity floor 43 |
| 2026-10-31 | patch | M4 🟡: part 2a, layers and presets; parity floor 42 |
| 2026-10-31 | patch | M4 🟡: owner's answers in part 1 (chisel highlighter, highlighter colours, palm grace, screen ruler); ANNO-18 ruler angle placed last in Later; 124 rows |
| 2026-10-31 | patch | M4 🟡: part 1, ink engine and markup commands; parity floor 40 of 123; M4 80–150 → 60–120 h |
| 2026-10-31 | patch | M3 ✅: Windows check passed (pedal, real scores, P1); follow-ups in gaps G2, G6 |
| 2026-10-31 | patch | M3 🟡: control channel for agents (turn and screenshot) |
| 2026-10-31 | patch | M3 🟡: desktop reader window on craft-ui, keep-awake, snapshot; parity floor 32 |
| 2026-10-31 | patch | M3 🟡: pedal and key map, taps and swipes; parity floor 29 |
| 2026-10-31 | patch | M3 🟡: page-turning spec, layout crate and reader commands; parity floor 25 |
| 2026-10-31 | minor | M1 ✅: foundation crates and library commands; parity floor 17; alpha ~250–530 → ~220–490 h; M5 60–120 → 40–80 h |
| 2026-10-31 | patch | M2 ✅: fuzz, corpus and perf done; PDF-2 partial, its PDFCraft render gaps tracked in G1 |
| 2026-10-31 | patch | M2 🟡: `score.open` and `view.render` live; parity floor 1 |
| 2026-10-31 | patch | M2 🟡: page cache and pre-rendering in `scorecraft-render` |
| 2026-10-31 | patch | M2 🟡: `scorecraft-pdf` adapter over `pdfcraft-render` 0.15.0 |
| 2026-10-28 | minor | Owner's answers to the M0 review: page turns and the pedal come first after M2, so TURN-4 moves from M6 to M3 (M3 66–122 h, M6 44–88 h; totals unchanged); M3 targets the Windows tablet first; M7 iPad uses a Swift pen host; M8 is Google Drive first, then device-to-device |
| 2026-10-28 | minor | M0 done; alpha ~310–630 → ~250–530 h: `cargo xtask scorecard` generates the scorecard from `perf/budgets.toml` and `scorecard/*.toml`; testkit writes synthetic and hostile PDFs |
| 2026-10-28 | minor | M0: engine `Session::dispatch` and command registry, generated feature catalog and parity checklist (0 of 122, floor 0), `panic_hunt`, CLI `commands`/`run` |
| 2026-10-28 | minor | M0: GitHub Actions CI (fmt, layers, clippy and tests on three OSes, wasm32, MSRV) and `cargo xtask ci` |
| 2026-10-28 | minor | M0 in progress: Cargo workspace (edition 2024, MSRV 1.85), every crate in the architecture map as a stub, workspace never-crash lints, `cargo xtask layers` and `cargo xtask wasm` |
| 2026-10-28 | minor | S8 rows (PLAY-7, MIDI-5, SYNC-8, PLAT-5) placed in M7, M8 and M10; later milestones 480–950 → 490–980 h; alpha gate unchanged |
| 2026-10-28 | minor | M2 becomes PDFCraft integration (80–160 → 20–40 h); alpha ~350–720 → ~310–630 h |
| 2026-10-28 | major | First version |
