# ScoreCraft roadmap

**Stage: pre-alpha** · next: alpha, the [alpha gate](docs/roadmap.md#alpha-gate) (~105–287 h away)

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-10-31 · **Change:** patch (M6 part 3: shortcuts and snips, breadth 53%; earlier: M6 part 2: the metronome, breadth 52%; earlier: M6 part 1: playing a list, breadth 51%; earlier: M5 part 2: the library window, breadth 50%; earlier: M5 part 1: library search and sort, breadth 49%; earlier: M4 part 5: export with ink, breadth 47%; earlier: M4 part 4: the pen in the desktop app; earlier: merged with M3 done on main: M3 reader app done; earlier: M4 part 3: rectangle select, shapes and the pen button, breadth 46%; earlier: M4 part 2c: stamps, breadth 43%; earlier: M4 part 2b: text notes, breadth 42%; earlier: M4 part 2a: layers and presets, breadth 41%; earlier: owner's answers on markup, 124 rows, breadth 38%; earlier: M4 part 1: the ink engine and markup commands, breadth 39%; earlier: M1 foundation and M2 PDF reading done; earlier: owner's priorities after the M0 review; earlier: M0 skeleton done; earlier: 4 features added from the Microsoft Store description; PDF backend reuses PDFCraft) · **Target:** enScore 3.7 (2026)

ScoreCraft's one-page summary of how close it is to enScore 3.7 (measured from public listings on
2026-10-28). Detail: [target-app parity](docs/target-app-parity.md) (methodology, weights,
evidence), [gaps](docs/gaps.md) (the ranked work list), [roadmap](docs/roadmap.md) (milestones,
current focus), [feature map](docs/feature-map.md) (all 124 features), [scorecard](docs/scorecard.md)
and [parity checklist](docs/parity-checklist.md) (generated once the engine exists),
[architecture](docs/architecture.md), [clean-room methodology](docs/clean-room.md).

Hours are agent wall-clock hours for one agent working sequentially, **uncalibrated** until this
repository has merged-PR history. About 75% of the work parallelizes.

## Headline numbers

| | Value | Kind |
|---|---|---|
| **Feature breadth** | **54%** | measured: 58 of 124 feature-map rows live (M3's reader app and M4's markup) |
| **Ready for real work** | **5%** | estimated: you can read a PDF and turn pages with a pedal on Windows; the pen marks up in the desktop app, unchecked on a tablet |
| Practising musician | 0% | estimated ([method](docs/target-app-parity.md#practising-musician)) |
| Essentials user | 0% | estimated ([method](docs/target-app-parity.md#essentials-user)) |
| Remaining to **alpha** | **~105–287 h** | estimated |
| Remaining to **beta** | **~520–1,105 h** | estimated |
| Remaining to **full parity** | **~747–1,551 h** | estimated |

**Why pre-alpha.** The project has its process, clean-room rules, architecture and feature map, and
the M0 skeleton, PDF reading over PDFCraft (M2) and the foundation (M1): the document model,
undo history and a crash-safe on-disk library, driven by about 35 engine commands (import,
folders, metadata, page edits, bookmarks, setlists, undo, verify). M3 added the first app: a
Windows desktop reader with single, two-page and half-page turns, zoom, full screen,
keep-awake, Bluetooth pedal and key turning, and a control channel agents use to drive it. There
is no library screen and no ink yet, so it is not ready for real work; next is ink (M4).

## By dimension

| Dimension | % ready | Hours to full | Doc |
|---|---:|---:|---|
| Features (depth) | 0% | 840–1,690 | [feature-map](docs/feature-map.md) |
| Page-turn reliability and latency | 60% | 20–40 | [ui-parity](docs/ui-parity.md) |
| Ink feel | 0% | 30–70 | [annotation-parity](docs/annotation-parity.md) |
| PDF fidelity (engine reused from [PDFCraft](https://github.com/storytold/pdfcraft)) | 0% | 20–40 | [file-format-parity](docs/file-format-parity.md) |
| Stability | 0% | 50–100 | [gaps G3](docs/gaps.md#g3-stability-and-data-safety) |
| Hardware (pedals, MIDI, stylus, e-ink) | 10% | 45–90 | [hardware-parity](docs/hardware-parity.md) |
| Platforms | 0% | 80–200 | [gaps G9](docs/gaps.md#g9-mobile-platforms) |
| Sync | 0% | 130–250 | [sync-parity](docs/sync-parity.md) |
| Practice and audio | 0% | 60–120 | [practice-parity](docs/practice-parity.md) |
| Localization | 0% | 40–80 | [localization-parity](docs/localization-parity.md) |

## Languages

enScore lists fifteen languages. ScoreCraft ships English first; the rest follow at M11
([localization-parity](docs/localization-parity.md)).

## Upcoming

| Rank | Milestone | Estimate |
|---|---|---:|
| 1 | M4 ink: code complete; a check on a Windows tablet, then fixes from it | 5–20 h |
| 2 | M5 library: code complete; P6 and P7 on the reference device, polish from use | 5–15 h |
| 3 | M3 follow-ups: half-page split that snaps between systems, first-render cost on big scans, more pedals | 20–40 h |

Detail: [docs/roadmap.md](docs/roadmap.md).

## Progress log

| Date | Entry |
|---|---|
| 2026-10-31 | LIB-6: search inside scores (the words in their PDFs and notes), indexed lazily into the library's cache; parity 58 of 124 |
| 2026-10-31 | M3 follow-up: the half-page split snaps to the gap between systems |
| 2026-10-31 | The metronome's click: sample-accurate synthesis (`scorecraft-audio`) through the default audio output on Windows and macOS (`scorecraft-devices`, cpal); every alpha workflow is now code complete |
| 2026-10-31 | The Score panel in the app: details (title, composer, tags…), the page editor (turn, hide, reorder, delete) and bookmarks; the alpha gate re-graded: code complete but for the metronome click, fuzzing and the tablet checks |
| 2026-10-31 | M6 part 4: shortcut outlines in markup mode; snips exported as vector copies of their source page; snip resizing |
| 2026-10-31 | M6 part 4: lists in the app: the Lists panel (make, fill, reorder, play) and a now-playing strip; each next score keeps the window and layout |
| 2026-10-31 | M6 part 3: Score Shortcuts (tap to jump) and Score Snips (another page's bars drawn into this one), from the rectangle tool; parity 57 of 124 |
| 2026-10-31 | M6 part 2: the metronome: 20–300 BPM in 2, 3, 4 or 6, tap tempo, entry tempos, a pulse along the reader's top edge (the click waits on audio output); parity 55 of 124 |
| 2026-10-31 | M6 part 1: [performance spec](docs/spec/performance.md); playing a list (pieces flow into one another, entry page ranges, breaks reported), one score in many lists, the import report; parity 54 of 124 |
| 2026-10-31 | M5 part 3: library speed measured: 10,000 scores open and list in 172 ms, search in 22 ms (development container) |
| 2026-10-31 | M5 part 2: the library window: covers rendered off the frame, or a list; search box, sort, folders, folder import; `library.cover`; parity 53 of 124 |
| 2026-10-31 | M5 part 1: [library spec](docs/spec/library.md); search (word starts, no case or accents, ranked), sort (natural, articles ignored), recently opened, folder import in the engine; parity 52 of 124 |
| 2026-10-31 | M4 part 5: export with ink, flattened or as annotations, chosen layers, in the score's page order, rotation and crop; parity 48 of 124 |
| 2026-10-31 | M4 part 4: the pen in the desktop app: markup bar, pen and touch through the palm filter, live strokes, ruler, selection; scores open through the library; markup settings kept |
| 2026-10-31 | M4 part 3: rectangle select (move, copy, delete, scale, recolour), shapes (lines and ellipses) and the pen button in the engine; parity 47 of 124 |
| 2026-10-31 | M4 part 2c: stamps (dynamics pp–ff in Edwin, crescendo and diminuendo hairpins with handles) in the engine and renderer; M4 part 2 done; parity 44 of 124 |
| 2026-10-31 | M4 part 2b: text notes (Inter built in, wrapped at 86% of the page, kept inside it, 8–36 pt) in the engine and renderer; `scorecraft-text`; parity 43 of 124 |
| 2026-10-31 | M4 part 2a: annotation layers (new, rename, show, lock, reorder, delete, active) and presets in the engine; parity 42 of 124 |
| 2026-10-31 | M4 part 1: [ink spec](docs/spec/ink.md) from the owner's toolbar observation; `scorecraft-ink`, CPU ink with a multiply highlighter, palm filter, 16 markup commands (pen, highlighter, whiteout, erasers, swatches and palette, Draw with Touch, touch lock, ruler); ANNO-17 Ruler added; parity 40 of 123; then the owner's answers (chisel highlighter, highlighter colours, palm grace, screen ruler, text and stamps specified), 124 rows |
| 2026-10-31 | M3 done: desktop reader on craft-ui (layouts, half-page and alternating turns, zoom, full screen, keep-awake), pedal and key maps, control channel; checked on Windows with the owner's Bluetooth pedal and real scores; parity 32 of 122 |
| 2026-10-31 | M1 done: `geom`, `raster`, `doc`, `ops` and `format` v1, about 35 library, page, list and undo commands; a save crashed at any write reopens intact; parity 17 of 122 |
| 2026-10-31 | M2 done: PDF reading over PDFCraft 0.15.0, page cache and pre-rendering, `score.open` and `view.render`; render gaps tracked in PDFCraft |
| 2026-10-28 | M0 review acted on: Rust 1.95 (PDFCraft's) with a pinned toolchain, one egui on craft-ui checked in CI, parity counts only implemented rows, CLI keeps partial results; owner's answers recorded (Windows tablet first, pedal in M3, Google Drive sync, Swift pen host on iPad) |
| 2026-10-28 | M0 done: generated scorecard (`cargo xtask scorecard`), testkit synthetic and hostile PDFs |
| 2026-10-28 | M0: engine command registry and `Session::dispatch`, generated parity checklist (measured 0 of 122), `panic_hunt` in CI, CLI `commands` and `run` |
| 2026-10-28 | M0 started: Cargo workspace with all 29 crates and 6 apps as stubs, never-crash lints on every crate, `cargo xtask layers` and `cargo xtask wasm` green |
| 2026-10-28 | Feature map reconciled with the Microsoft Store description: 4 features added (per-setlist MIDI, recording at score open, two-way folder mirror, Apple Vision), Score Snips corrected |
| 2026-10-28 | PDF backend to reuse PDFCraft's crates (decision D1); 13 features marked as reuse |
| 2026-10-28 | Documentation and process: clean-room rules, feature map (118 rows), architecture, parity docs, RFC process |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | patch | M6 part 3: breadth 52% → 53% (57 of 124); beta ~525–1,115 h, full ~752–1,561 h |
| 2026-10-31 | patch | M6 part 2: breadth 51% → 52% (55 of 124); alpha ~105–287 h, beta ~540–1,140 h, full ~767–1,586 h |
| 2026-10-31 | patch | M6 part 1: breadth 50% → 51% (54 of 124); alpha ~110–297 h, beta ~545–1,150 h, full ~772–1,596 h |
| 2026-10-31 | patch | M5 part 2: breadth 49% → 50% (53 of 124); beta ~551–1,162 h, full ~778–1,608 h |
| 2026-10-31 | patch | M5 part 1: breadth 47% → 49% (52 of 124); alpha ~116–309 h, beta ~556–1,172 h, full ~783–1,618 h |
| 2026-10-31 | patch | M4 part 5, export with ink: breadth 46% → 47% (48 of 124); alpha ~121–319 h, beta ~565–1,189 h, full ~792–1,635 h |
| 2026-10-31 | patch | M4 part 4, the pen in the desktop app: alpha ~135–345 → ~125–325 h, beta ~569–1,195 h, full ~796–1,641 h; breadth unchanged (46%) |
| 2026-10-31 | patch | Merged with M3 done on main: M3's reduction (alpha −35/−50 h on top of M4's, beta −50/−95, full −50/−95) and its 5% ready for real work kept; breadth stays the measured 46% (47 of 124); alpha ~135–345 h, beta ~579–1,215 h, full ~806–1,661 h |
| 2026-10-31 | patch | M4 part 3: breadth 43% → 46% (47 of 124 rows); beta ~629–1,310 h, full ~856–1,756 h; alpha unchanged |
| 2026-10-31 | patch | M4 part 2c, stamps: breadth 42% → 43% (44 of 124 rows); M4 part 2 done: alpha ~200–460 → ~185–440 h, beta and full 15–20 h lower |
| 2026-10-31 | patch | M4 part 2b, text notes: breadth 41% → 42% (43 of 124 rows) |
| 2026-10-31 | patch | M4 part 2a, layers and presets: breadth 38% → 41% (42 of 124 rows) |
| 2026-10-31 | patch | Owner's answers on markup: ANNO-18 ruler angle added (124 rows), breadth 39% → 38% |
| 2026-10-31 | minor | M4 part 1 (on M3): breadth 14% → 39% (40 of 123 rows); alpha ~220–490 → ~200–460 h, beta and full parity lower; ink feel 40–80 → 30–70 h |
| 2026-10-31 | minor | M3 done: breadth 14 → 30%, ready for real work 0 → 5%, alpha ~220–490 → ~170–395 h, beta and full ~50–95 h lower, page turns 60–120 → 20–40 h, hardware 50–100 → 45–90 h |
| 2026-10-31 | minor | M1 and M2 done: breadth 0 → 14%, alpha ~250–530 → ~220–490 h, beta and full parity 40 h lower, stability 60–120 → 50–100 h |
| 2026-10-28 | minor | Owner's priorities: pedal mapping moves into M3; headline numbers unchanged |
| 2026-10-28 | minor | M0 done: remaining estimates drop by the M0 skeleton (60–100 h): alpha ~250–530 h, beta ~710–1,430 h, full ~940–1,880 h; upcoming starts at M2 |
| 2026-10-28 | minor | M0 started (workspace skeleton); numbers unchanged |
| 2026-10-28 | minor | Microsoft Store description: 122 rows; beta and full estimates up slightly |
| 2026-10-28 | minor | PDFCraft reuse: estimates lowered |
| 2026-10-28 | major | First version, per [`docs/standards/progress-docs.md`](docs/standards/progress-docs.md) |
