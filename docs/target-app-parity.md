# ScoreCraft vs enScore: target-app parity

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-10-31 · **Change:** patch (M4 part 5: breadth 47%, alpha ~121–319 h; earlier: M4 part 4: alpha ~125–325 h; earlier: merged with M3 done on main: M3 reader app done: breadth 30%, alpha ~170–395 h; earlier: M4 part 3: breadth 46%, 47 of 124 rows; earlier: M4 part 2c: breadth 43%, 44 of 124 rows; earlier: M4 part 2b: breadth 42%, 43 of 124 rows; earlier: M4 part 2a: breadth 41%, 42 of 124 rows; earlier: ANNO-18 added, breadth 38%, 40 of 124 rows; earlier: M4 part 1, with M3 below it: breadth 39% measured, 40 of 123 rows, ANNO-17 added; alpha ~200–460 h; earlier: M1 foundation done: breadth 14% measured, alpha ~220–490 h; earlier: M0 skeleton done, remaining hours lowered; earlier: 4 rows added from the Microsoft Store description; estimates lowered for PDFCraft reuse) · **Target:** enScore 3.7 (2026)

The authoritative answer to "how close is ScoreCraft to enScore, and how much work is left". The
one-page summary is [`ROADMAP.md`](../ROADMAP.md); the ranked work list is [`gaps.md`](gaps.md);
the feature catalog is [`feature-map.md`](feature-map.md); the measured checklists will be
[`scorecard.md`](scorecard.md) (`cargo xtask scorecard`) and [`parity-checklist.md`](parity-checklist.md)
(`cargo xtask parity`). Detail by dimension: [UI](ui-parity.md), [annotation](annotation-parity.md),
[file formats](file-format-parity.md), [hardware](hardware-parity.md),
[practice and audio](practice-parity.md), [sync](sync-parity.md),
[localization](localization-parity.md).

## Headline (2026-10-31, engine, library store, the M3 reader window; no pen in the app yet)

| Number | Value | Kind |
|---|---|---|
| **Feature breadth** (does each enScore feature exist?) | **47%** weighted (48 of 124 feature-map rows live, all `partial`: engine commands with tests, plus the M3 Windows reader; the pen marks up in the desktop app) | measured by `cargo xtask parity` |
| **Ready for real work** (could a musician replace enScore with it on stage?) | **0%** | estimated, weights below |
| **Practising musician** (reads, marks, turns with a pedal, uses setlists) | 0% | estimated, [method](#practising-musician) |
| **Essentials user** (open a PDF, turn pages, mark it up) | 0% | estimated, [method](#essentials-user) |
| **Stage** | **pre-alpha** | the [alpha gate](roadmap.md#alpha-gate) does not pass |
| Remaining to **alpha** | **~121–319 agent-hours** | estimated, uncalibrated: P0 rows (after PDFCraft reuse); M0 (2026-10-28), M2, M1 and M3 (2026-10-31) are done; M4 parts 1 to 4 (ink, layers, presets, text, stamps, selection, the pen in the desktop app, export with ink) took 50–75 h off |
| Remaining to **beta** | **~565–1,189 agent-hours** | estimated, uncalibrated: alpha + P1 rows (350–680 h) + stability and performance (100–200 h) |
| Remaining to **full parity** | **~792–1,635 agent-hours** | estimated, uncalibrated: beta + P2 rows (233–456 h, with the ruler); P3 (ours only) adds 80–170 h |

"Agent-hours" are wall-clock hours for one agent working sequentially, including tests, as in
PhotoCraft's docs. They are **uncalibrated** until this repository has merged-PR history
([below](#remaining-effort-and-calibration)). About 75% of the work parallelizes: most feature
rows are independent command modules; `pdf` rendering, the library store and sync are the serial
parts.

## Target measured

- **enScore 3.7** on Apple platforms (3.7.0 per a public app directory, 3.5.9 per the App Store's
  version history), **3.5.15** on Android, Microsoft Store build **2026.10.21.305** on Windows,
  measured on 2026-10-28.
- **From public sources only** ([clean-room](clean-room.md#2-allowed-sources)): the store
  listings (including the full Microsoft Store description, S8), release notes and partner pages summarised in
  [`observations/public-listings-2026-10-28.md`](observations/public-listings-2026-10-28.md). Unlike
  PhotoCraft's measurement of Photoshop, **no installed package was inspected**, so items enScore
  ships without describing publicly are missing from the denominator until someone observes them
  as a user. Rows tagged `inferred` in the feature map are capabilities any reader needs (undo,
  zoom) that the listings imply but do not name.
- enscore.app was not reachable from our build environment on 2026-10-28; reading it is the first
  re-measure task.

## Feature breadth: 47% (weighted, 2026-10-31)

| Component | Weight | Ours / enScore | % | Kind |
|---|---:|---|---:|---|
| Reading, layout and page turning (READ, TURN) | 25% | 14 / 23 | 61% | measured |
| Annotation (ANNO) | 20% | 16 / 18 | 89% | measured |
| Library, page editing, navigation (LIB, PAGE, NAV) | 15% | 12 / 28 | 43% | measured |
| Setlists (LIST) | 10% | 4 / 7 | 57% | measured |
| Import, export, scanning, IMSLP, forScore (IMP, EXP) | 10% | 1 / 12 | 8% | measured |
| Practice, audio, MIDI (PRAC, PLAY, MIDI) | 10% | 0 / 23 | 0% | measured |
| Sync and backup (SYNC) | 5% | 0 / 8 | 0% | measured |
| Platforms and languages (PLAT) | 5% | 1 / 5 | 20% | measured |
| **Weighted** | 100% | | **47%** | |

Measured on the M4 branch, merged with M3 done (READ, TURN and PLAT rows come from M3; main's
30% was measured before the feature map's M4 rows went live). Annotation rows are `partial`: the engine draws, erases and stores ink with tests, but
no pen draws in the app until M4 part 4.

Weights reflect what a performing musician relies on most; they are set here once and only moved by
RFC.

## Ready for real work: 0% (weighted)

| Dimension | Weight | % ready | Remaining (agent-h) | Evidence | Detail |
|---|---:|---:|---:|---|---|
| Features in depth | 25% | 0% | 840–1,690 | no commands | [feature-map](feature-map.md) |
| Page-turn reliability and latency | 15% | 0% | 60–120 | no renderer; budgets defined in [scorecard](scorecard.md#performance) | [ui-parity](ui-parity.md) |
| Ink feel (latency, pressure, smoothing) | 10% | 0% | 30–70 | ink engine with pressure curve and corner-keeping smoothing (M4 part 1); no pen in the app to judge on a device | [annotation-parity](annotation-parity.md) |
| PDF fidelity (renders what other readers render) | 15% | 0% | 20–40 | no adapter yet; engine reused from PDFCraft | [file-format-parity](file-format-parity.md) |
| Stability (never crash, never lose markings) | 10% | 0% | 60–120 | no code | [gaps G3](gaps.md#g3-stability-and-data-safety) |
| Hardware (pedals, MIDI, stylus, e-ink) | 10% | 0% | 50–100 | none | [hardware-parity](hardware-parity.md) |
| Platforms (desktop, Android, iOS, web) | 10% | 0% | 80–200 | none | [gaps G9](gaps.md#g9-mobile-platforms) |
| Localization | 5% | 0% | 40–80 | English only planned at alpha | [localization-parity](localization-parity.md) |

## Practising musician

Weekly areas for a typical gigging or studying musician: open and read PDFs (READ-1..5), pedal turns
(TURN-4), pen and highlighter (ANNO-1..3, 6, 8), library search (LIB-3..5), setlists (LIST-1..4),
metronome (PRAC-1), export with ink (EXP-2). Ready % = the share of those rows `done`, discounted
by 25% for any open stability gap in the area. Today 0%.

## Essentials user

Open a PDF, turn pages by tap and keyboard, pen and eraser, save and reopen with markings intact.
Ready % = share of READ-1, READ-2, TURN-1, TURN-3, ANNO-1, ANNO-6, ANNO-13, ANNO-15 `done`. Today 0%.

## Feature areas

| Area | % ready | Hours | Notes |
|---|---:|---:|---|
| PDF engine (parse, render, write) | 0% | 20–40 | reused from PDFCraft (decision D1); the adapter and turn caching are ours |
| Reader layout and turns | 0% | 40–80 | half-page and alternating turns are the signature feature |
| Ink engine and markup tools | 0% | 80–150 | latency and pressure feel need real devices |
| Library store, search, metadata | 0% | 60–120 | must scale to 10,000 scores |
| Setlists, shortcuts, snips | 0% | 40–80 | Pro features in enScore, free here |
| Import / export / print | 0% | 40–80 | |
| forScore `.4sc` import | 0% | 30–60 | black-box format, two-team rule |
| Scanning (camera, eSCL) | 0% | 36–64 | |
| IMSLP | 0% | 12–24 | public API, terms |
| Audio playback and practice tools | 0% | 60–120 | time-stretch, pitch-shift, tuner |
| MIDI | 0% | 20–40 | |
| Sync and backup | 0% | 120–240 | offline-first CRDT plus three backends |
| Platforms and shells | 0% | 80–200 | iOS is the least mature egui target |
| Localization | 0% | 40–80 | |

## Remaining effort and calibration

**Calibration.** There is no history yet. The per-row estimates in the feature map use PhotoCraft's
published calibration (2–9 agent-hours per checklist item of tool or UI depth, 8–20 per format or
subsystem, from its first 1,004 merged PRs) as a prior. After about 50 merged PRs here, replace
the prior with this repository's own numbers (median PR hours by size, as PhotoCraft's
calibration table does) and re-estimate every row.

**Totals.** Sum of feature-map estimates after PDFCraft reuse: P0 250–530, P1 360–700, P2 230–450, P3 80–170
(agent-hours). Infrastructure not in the rows: M0 skeleton and xtask 60–100, stability and
performance hardening before beta 100–200.

## How to re-measure

1. `cargo xtask parity` and `cargo xtask scorecard` for the measured rows (once they exist).
2. Re-read every public source in the observation note; add rows for new capabilities (new
   release notes), with their source, and record the enScore version.
3. Observation passes by contributors who use enScore: fill the open questions in
   [`observations/public-listings-2026-10-28.md`](observations/public-listings-2026-10-28.md#open-questions-need-observation-or-the-developer-site).
4. Re-grade the estimated rows from open issues by area and corpus floors. Move a number only with
   new evidence, and add a revision-history row.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | patch | M4 part 5 (export with ink): breadth 46% → 47% (48 of 124 rows; EXP 0 → 1 of 12); alpha ~125–325 → ~121–319 h, beta ~565–1,189 h, full ~792–1,635 h |
| 2026-10-31 | patch | M4 part 4 (the pen in the desktop app): alpha ~135–345 → ~125–325 h, beta ~579–1,215 → ~569–1,195 h, full ~806–1,661 → ~796–1,641 h; breadth unchanged; ink feel stays 0% until judged on a device |
| 2026-10-31 | patch | Merged with M3 done on main: hours combine M3's reduction with M4's (alpha ~135–345 h, beta ~579–1,215 h, full ~806–1,661 h); breadth stays the measured 46% (47 of 124 rows) |
| 2026-10-31 | patch | M4 part 3 (rectangle select, shapes, pen button): breadth 43% → 46% (47 of 124 rows; ANNO 13 → 16 of 18); beta ~635–1,320 → ~629–1,310 h (ANNO-12's engine half), full ~868–1,776 → ~856–1,756 h (ANNO-11 and ANNO-16 too); alpha unchanged |
| 2026-10-31 | patch | M4 part 2c (stamps): breadth 42% → 43% (44 of 124 rows; ANNO-9 is ScoreCraft's own, P3); M4 part 2 done: alpha ~200–460 → ~185–440 h, beta ~650–1,340 → ~635–1,320 h, full ~883–1,796 → ~868–1,776 h |
| 2026-10-31 | patch | M4 part 2b (text notes): breadth 41% → 42% (43 of 124 rows) |
| 2026-10-31 | patch | M4 part 2a (layers, presets): breadth 38% → 41% (42 of 124 rows) |
| 2026-10-31 | patch | ANNO-18 ruler angle added from the owner's answers (124 rows): breadth 39% → 38% |
| 2026-10-31 | minor | M4 part 1 (on M3): breadth 14% → 39% (40 of 123 rows; ANNO-17 Ruler added from the owner's observation); alpha ~220–490 → ~200–460 h, beta ~670–1,370 → ~650–1,340 h, full ~900–1,820 → ~883–1,796 h; ink feel 40–80 → 30–70 h |
| 2026-10-31 | minor | M3 done: breadth 14 → 30% weighted (32 of 122 rows live); alpha ~220–490 → ~170–395 h, beta ~670–1,370 → ~620–1,275 h, full ~900–1,820 → ~850–1,725 h |
| 2026-10-31 | minor | M1 done (and M2 earlier the same day): breadth 0 → 14% (17 of 122 rows live, engine only); alpha ~250–530 → ~220–490 h, beta ~710–1,430 → ~670–1,370 h, full ~940–1,880 → ~900–1,820 h |
| 2026-10-28 | minor | M0 skeleton done (PRs #5–#8): alpha ~310–630 → ~250–530 h, beta ~770–1,530 → ~710–1,430 h, full ~1,000–1,980 → ~940–1,880 h |
| 2026-10-28 | minor | Microsoft Store description (S8): 118 → 122 rows; beta ~760–1,520 → ~770–1,530 h, full ~980–1,960 → ~1,000–1,980 h; alpha unchanged |
| 2026-10-28 | minor | PDFCraft reuse: alpha ~350–720 → ~310–630 h, beta ~830–1,650 → ~760–1,520 h, full ~1,070–2,120 → ~980–1,960 h |
| 2026-10-28 | major | First assessment: 0 of 118 rows, pre-alpha, estimates from the feature map |
