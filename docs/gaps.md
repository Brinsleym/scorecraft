# Where ScoreCraft falls short of enScore

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-10-31 · **Change:** patch (G3: library-loader mutation test; earlier: merged with M3 done on main: M3: G2 and G6 progress from the Windows check; earlier: M4 part 1: G4 stroke engine, highlighter, whiteout, erasers, palm rejection and ruler in the engine; earlier: M1: G3 atomic saves and crash recovery done, G5 store done, G7 setlist model done; earlier: G1: M2 reading done, PDFCraft render gaps listed; earlier: G3: never-crash lints on every crate and `panic_hunt`; earlier: Microsoft Store description: G9, G10, G11, G12, G15 updated; G1 becomes integration of PDFCraft's crates) · **Target:** enScore 3.7 (2026)

Every known shortfall against enScore 3.7, one entry each, ranked by user impact on the way to
alpha and beta. This is the work list agents pick from: take the highest-ranked entry nobody is
working on, and when you close part of one, update its evidence and estimate here, the matching
parity doc and the feature-map rows. Numbers come from [`target-app-parity.md`](target-app-parity.md);
ids like `READ-5` are rows in [`feature-map.md`](feature-map.md); `#n` are GitHub issues.

"Blocks alpha" and "blocks beta" mark what stands between stages ([roadmap](roadmap.md#alpha-gate)).
Hours are agent-hours, one agent working sequentially, including tests, uncalibrated. **Human**
marks what an agent cannot finish alone (real devices, observation of enScore, legal judgement).

## Summary

| # | Gap | Kind | Impact | Estimate | Parity doc |
|---|---|---|---|---:|---|
| G1 | PDF engine: parse, render, write (reusing PDFCraft). Reading is in (M2); PDF-2 is partial on six PDFCraft render gaps (blank JBIG2 halftone, rotated-page image placement, short page tree, faint hairlines, light 1-bit downscales, sampling phase), fixed upstream then re-pinned | file format | blocks alpha | 15–30 h | [file-format-parity](file-format-parity.md#corpus-results-2026-10-31-pdfcraft-65ff3a00) |
| G2 | Reader layout and page turns. Mostly done in M3; split snapping, first-render cost and other platforms' keep-awake remain | features, UI | blocks alpha | 15–30 h | [ui-parity](ui-parity.md) |
| G3 | Stability and data safety | stability | blocks alpha | 50–100 h | [scorecard](scorecard.md#reliability) |
| G4 | Ink engine and markup tools. Ink, erasers, palm rejection, ruler, layers, presets, text notes and stamps are in the engine (M4 parts 1 and 2); the pen in the app, selection, shapes and export are not | features, UI | blocks alpha | 45–100 h + human | [annotation-parity](annotation-parity.md) |
| G5 | Library store, organisation and search | features | blocks alpha | 40–80 h | [file-format-parity](file-format-parity.md#scorecraft-library-format) |
| G6 | Foot pedals and keyboard turning. Key maps, profiles and debounce done in M3; learn mode and more pedals remain | hardware | blocks alpha | 6–12 h + human | [hardware-parity](hardware-parity.md) |
| G7 | Setlists, Score Shortcuts and Score Snips | features | blocks beta | 30–65 h | [feature-map](feature-map.md#list-setlists) |
| G8 | Import, export, print and forScore migration | file format | blocks beta | 70–140 h + human | [file-format-parity](file-format-parity.md) |
| G9 | Mobile platforms | platforms | blocks beta | 80–200 h + human | [hardware-parity](hardware-parity.md) |
| G10 | Sync and backup | sync | blocks beta | 130–250 h | [sync-parity](sync-parity.md) |
| G11 | MIDI | hardware | medium | 20–40 h + human | [hardware-parity](hardware-parity.md#midi) |
| G12 | Audio playback and practice tools | features | medium | 60–120 h | [practice-parity](practice-parity.md) |
| G13 | Scanning | features | medium | 36–64 h + human | [file-format-parity](file-format-parity.md#scanning) |
| G14 | IMSLP and automatic metadata | features, online | medium | 30–60 h | [feature-map](feature-map.md#lib-library-and-organisation) |
| G15 | Localization | localization | medium | 40–80 h + human | [localization-parity](localization-parity.md) |
| G16 | E-ink, accessibility and multi-window | UI, platforms | low-medium | 30–60 h + human | [ui-parity](ui-parity.md) |
| G17 | Unknowns: behaviour not yet observed | process | medium | human | [observations](observations/README.md) |

## Feature gaps

### G1. PDF engine: parse, render, write

- **Plan (2026-10-28):** reuse PDFCraft's crates ([architecture §15](architecture.md#15-reusing-pdfcraft),
  [decision D1](decisions.md#2026-10-28-d1-reuse-pdfcrafts-pdf-crates)). What is left is ours: the
  `scorecraft-pdf` adapter, tile caching and pre-rendering for turns, the pinned-revision setup with
  hayro patches, and a score corpus run against PDFCraft's renderer.
- **Missing:** the adapter. Through PDFCraft, `scorecraft-pdf` must parse real-world PDFs (xref streams, object
  streams, incremental updates, broken xref repair, encryption with empty user password), render
  vector and scanned pages (Flate, DCT, JBIG2, CCITT G4, JPX images; Type 1, TrueType, CFF and
  Type 3 fonts), read the outline and text layer, and write new PDFs (export with ink, manuscript
  paper, reports).
- **Rows:** READ-1, READ-14, NAV-5, LIB-6, EXP-2, PAGE-6, IMP-7.
- **Decided:** D1, reuse PDFCraft. Scanned-score quality (JBIG2, CCITT at 600 dpi) is checked on our
  corpus; render bugs go upstream to PDFCraft.
- **Impact:** nothing else works without it. Scanned scores (JBIG2/CCITT) are common on IMSLP.

### G2. Reader layout and page turns

- **Done in M3 (2026-10-31):** single and two-page layouts, half-page and alternating turns,
  full screen, zoom and pan, landscape fit, keep-awake (Windows), turns from cache well inside
  P1 (0.06 ms on real library scores, WIN-RB), a desktop window that renders real scores
  correctly, including rotated landscape pages.
- **Done (2026-10-31):** the half-page split snaps to the gap between systems on the page on
  screen (blank bands in a small render's row profile), within 15% of the page height.
- **Missing:** a split per page (it is one fraction for the score, snapped on the page shown when
  half-page turns are turned on); system detection that copes with very sparse or very dense
  pages. First render of a big scanned page takes seconds: native builds no longer block the
  window on it (the page appears when ready), but the render cost itself is G1 territory.
  Keep-awake on macOS, Linux and the web.
- **Rows:** READ-2..6, READ-9, READ-12, READ-15, TURN-1..3, TURN-8.
- **Open questions:** tap-zone proportions and half-page split behaviour (G17).
- **Impact:** the core experience. A turn must never wait on rendering.

### G4. Ink engine and markup tools

- **Spec (2026-10-31):** [`spec/ink.md`](spec/ink.md), with the owner's observation of the markup
  toolbar ([`observations/anno-markup-toolbar.md`](observations/anno-markup-toolbar.md)): pen only
  by default with Draw with Touch, touch lock, eight changeable swatches and a fixed palette, a
  width per tool, one eraser with stroke and pixel modes, rectangle select (move, copy, delete,
  snip, shortcut) and a ruler hidden unless a setting shows it.
- **Done (2026-10-31, M4 part 1, engine only):** `scorecraft-ink` (sample cleaning, pressure
  curve, corner-keeping smoothing, outlines, hit testing, stroke and pixel erasers, ruler), ink on
  the CPU in `scorecraft-render` (multiply highlighter without build-up, whiteout in the filter's
  paper colour, dark-filter ink mapping), the palm filter in `scorecraft-input`, and engine
  commands for the tools, swatches, Draw with Touch, touch lock, ruler and `annotate.stroke` with
  one undo step per gesture; ink shows in the reader frame and `view.render` (ANNO-1..3, -5..7,
  -14, -15, -17 partial).
- **Owner's answers (2026-10-31):** chisel highlighter at full strength with a round-ends
  setting, separate highlighter swatches and palette, a 50 ms palm grace after the pen lifts,
  the ruler as a band across the screen (turning it is ANNO-18, last of all), stylus shortcut
  pen ↔ eraser, stamps as ScoreCraft's own (Edwin dynamics and hairpins), and the observed text
  tool (Type with Touch, wrapping, notes kept inside the page): all in the spec; the highlighter,
  colours, palm grace and ruler changes are in the engine.
- **Done since (M4 part 2, engine):** layer commands, presets, text notes (Inter, wrapped and
  kept inside the page) and stamps (Edwin dynamics, hairpins with handles).
- **Missing:** the pen in the app (live ink, pen input and pressure from the platform, the
  toolbar), rectangle select, shapes, pen shortcuts, export with ink.
- **Rows:** ANNO-1..17.
- **Human:** ink feel must be judged on real tablets with real styluses (Apple Pencil, Surface Pen,
  S Pen, Wacom).
- **Impact:** marking up parts is the second reason musicians use a reader.

### G5. Library store, organisation and search

- **Done (2026-10-31, M1):** the on-disk library (`scorecraft-format`, format v1), PDF import from
  paths, folders, metadata, bookmarks, Recently Deleted with restore and empty, Verify & Repair,
  all as engine commands with undo (LIB-1, -3, -4, -10, -11, -12 partial: no UI yet).
- **Missing:** search, sort, recently opened list, folder and image import, thumbnails, the
  library UI, performance at 10,000 scores.
- **Rows:** LIB-1, LIB-3..5, LIB-9..15.

### G7. Setlists, Score Shortcuts and Score Snips

- **Done (2026-10-31, M1):** setlists with entries, placeholders, page ranges and tempos as
  engine commands with undo (LIST-1, -2, -3, -7 partial); score page range (PAGE-8 partial).
- **Missing:** continuous flow, shared setlist bundles, the lists UI;
  tappable repeat/coda jumps; snips that paste the bars after a turn before it.
- **Rows:** LIST-1..7, NAV-1..4, PAGE-7, PAGE-8, TURN-7.
- **Note:** paid (Pro) in enScore; free in ScoreCraft.

### G8. Import, export, print and forScore migration

- **Missing:** file, folder and image import; import report; PDF export with ink as flattened
  content or real annotations; print; forScore `.4sc` import.
- **Rows:** LIB-1, LIB-2, IMP-3, IMP-5..7, EXP-1..4.
- **Human:** the `.4sc` analysis needs someone who owns forScore to make sample files, and a
  different person to implement ([clean-room §5](clean-room.md#5-interchange-formats-without-a-public-spec)).

### G12. Audio playback and practice tools

- **Missing:** recordings attached to scores, time-stretch, pitch-shift, loops, recorded turns;
  recordings that start when the score opens; metronome with subdivisions and sounds; tuner;
  practice log, goals, report.
- **Rows:** PLAY-1..7, TURN-6, PRAC-1..11.

### G13. Scanning

- **Missing:** camera capture with edge detection and perspective correction; eSCL network scanners.
- **Rows:** IMP-1, IMP-2, PAGE-5, READ-8.
- **Human:** camera and scanner testing needs devices.

### G14. IMSLP and automatic metadata

- **Missing:** IMSLP search/import through the public API; title and composer detection.
- **Rows:** IMP-4, LIB-7, LIB-8.
- **Note:** enScore's detection uses its own online service. Ours must work on-device first and
  use public catalogues only with the user's consent ([clean-room §7](clean-room.md#7-online-services)).
  Matching enScore's hit rate may need an OCR model (decision D4).

## UI / UX gaps

### G16. E-ink, accessibility and multi-window

- **Missing:** e-ink mode, screen-reader labels, reduced motion, two scores side by side, multiple
  windows.
- **Rows:** PLAT-2, PLAT-4, READ-10, READ-11.

## Stability gaps

### G3. Stability and data safety

- **Done (2026-10-28):** never-crash lints on every crate (workspace `[lints]`, crate-root
  `#![deny(...)]`, `clippy.toml`), checked by `cargo xtask layers` (measured: 36 of 36 packages).
- **Done (2026-10-28):** `panic_hunt` dispatches every registered command with 267 adversarial
  params and runs in CI.
- **Done (2026-10-31, M1):** atomic per-file saves with a journal for multi-record saves; a test
  crashes a save at every write and reopens to exactly the old or the new library; damaged
  records are skipped on open and repaired by `library.verify`; `panic_hunt` also runs every
  command against an open library with real ids and checks the library stays sound.
- **Done (2026-10-31):** a deterministic stand-in for a library-loader fuzz target
  (`crates/format/tests/mutate.rs`): every file of a stored library damaged a dozen ways, one
  at a time and many together, never panics the loader, and checking what it opens never panics.
  The desktop app's frame has a last-resort panic guard (M3).
- **Missing:** fuzz targets for `fsc`, `midi` and setlist bundles (with those formats); a real
  kill-the-process save test on each OS.
- **Impact:** a crash on stage or lost markings ends trust permanently.

## Hardware gaps

### G6. Foot pedals and keyboard turning

- **Done in M3 (2026-10-31):** key maps with four profiles, repeat and bounce filter, taps and
  swipes. One Bluetooth pedal, tested by the owner on Windows: one turn per press, correct
  direction, both in its key mode and in its touch (swipe) mode.
- **Missing:** learn mode, more pedal models (AirTurn, PageFlip, Donner), other platforms.
- **Rows:** TURN-3, TURN-4.
- **Human:** test with at least three real pedal models on each platform.

### G11. MIDI

- **Missing:** MIDI input for turns, output of Program Change, Bank Select and Song Select (one
  message or a list) when a score opens, different messages for the same score in different
  setlists, device selection (USB, BLE MIDI, network).
- **Rows:** TURN-5, MIDI-1..5.

## Platform gaps

### G9. Mobile platforms

- **Missing:** Android and iOS/iPadOS shells, share-sheet import, Files integration, pen input,
  keep-awake, app store packaging.
- **Rows:** PLAT-1, PLAT-5 (Apple Vision runs the iPad build).
- **Human:** store accounts, signing, device testing. iOS support in egui/winit is the least mature
  target; decision D3.

### G10. Sync and backup

- **Missing:** op-log sync with CRDT merge; folder, WebDAV and self-hosted server backends; backup
  and restore; account-free operation; enScore's free two-way folder mirror with the computer or
  OneDrive (SYNC-8, shares the folder backend with SYNC-7).
- **Rows:** SYNC-1..8.

## Localization gaps

### G15. Localization

- **Missing:** every language but English. enScore offers fifteen languages.
- **Rows:** PLAT-3. Detail in [`localization-parity.md`](localization-parity.md).
- **Human:** native-speaker review per language.

## Process gaps

### G17. Unknowns: behaviour not yet observed

- The feature map is built from public listings only. Exact gestures, defaults and edge cases need
  observation notes from contributors who use enScore, and enscore.app needs reading.
- Open questions are listed in [`observations/public-listings-2026-10-28.md`](observations/public-listings-2026-10-28.md#open-questions-need-observation-or-the-developer-site).

## Not clean-room feasible (so far)

| Item | Why | What we do instead |
|---|---|---|
| Talking to enScore's servers (sync, detection) | Undocumented private service; using it would mean traffic analysis | Our own sync and on-device detection |
| Writing `.4sc` files | No way to verify output without the other app's code; black-box writing risks corrupting a user's forScore library | Read only, until someone can verify written files in forScore |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | patch | M3: G2 (40–80 → 15–30 h) and G6 (10–20 → 6–12 h) after the Windows check; half-page split snapping recorded |
| 2026-10-31 | patch | M1: G3 crash-safe saves (60–120 → 50–100 h), G5 library store and commands (60–120 → 40–80 h), G7 setlist model and commands (40–80 → 30–65 h) |
| 2026-10-31 | patch | G1: reading lands in M2; six PDFCraft render gaps listed, estimate 15–30 h |
| 2026-10-28 | minor | G3: `panic_hunt` lands with the engine command registry |
| 2026-10-28 | minor | G3: never-crash lints land on every crate with the M0 workspace; estimate unchanged |
| 2026-10-28 | minor | Microsoft Store description (S8): G9 adds PLAT-5, G10 adds SYNC-8 (120–240 → 130–250 h), G11 adds MIDI-5, G12 adds PLAY-7, G7 snip wording corrected |
| 2026-10-28 | minor | G1 re-planned as PDFCraft integration (80–160 → 20–40 h) |
| 2026-10-28 | major | First version: G1–G17 from the feature map |
