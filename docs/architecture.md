# ScoreCraft architecture

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-11-08 · **Change:** patch (dark and sepia recolour scans too; earlier: current state: M1 foundation crates, the library store and its commands; earlier: first engine commands; earlier: page cache and pre-rendering; earlier: PDF adapter on PDFCraft 0.15.0 with the vello_cpu patch; earlier: §16 one egui on craft-ui, §17 web runtime, sync redesigned around Google Drive and device-to-device, iPad pen host; earlier: PDF backend reuses PDFCraft's crates, §15; first version) · **Target:** enScore 3.7 (2026)

## Current state (2026-10-31)

- **PDF backend (§15).** `scorecraft-pdf` takes `pdfcraft-render` at PDFCraft 0.15.0
  (`65ff3a00`, 2026-10-30) with the hayro, lopdf, `vello_common` and `vello_cpu` patches at the
  same revision. The adapter opens, reports pages, metadata and outline, renders pages and tiles,
  and extracts text. The other PDFCraft crates in §15 come in when a milestone needs them
  (`model` and `annot` for ink, `organize` and `create` for page editing).
- **Page cache (§7).** `scorecraft-render` keeps rendered pages and 512-pixel tiles in a
  byte-budget LRU keyed by page, zoom bucket (eighth-octave steps), filter and region.
  `ScoreView` renders on a miss and pre-renders the current, next and previous pages on a
  background thread with its own document handle; the web build runs the same queue on its one
  thread. Dark and sepia recolour the rendered page by lightness (black to the ink colour, white
  to the paper colour), so scanned scores change colour like engraved ones (2026-11-08; before,
  PDFCraft's reading colours left images, and so scans, unchanged).
- **Engine and CLI.** `score.open` and `view.render` are the first feature commands (READ-1).
  The session holds one open score until the library lands (M5). `view.render` renders through
  the page cache, can write a PNG through `scorecraft-codecs`, and queues the neighbouring pages
  for pre-rendering. `scorecraft-cli info` and `render` are shorthands for them.

- **Foundation (M1, 2026-10-31).** `scorecraft-geom` has points, rects, affine transforms,
  quarter-turn rotations, page boxes that map PDF user space to display pixels and back, and
  cubic Béziers with bounded flattening. `scorecraft-raster` has premultiplied RGBA bitmaps with
  checked sizes, tile grids, mip pyramids and damage regions (the render cache still uses the
  PDF adapter's bitmap; moving it over comes with the compositor). `scorecraft-doc` is the §5
  model, all of it: ids are UUID v7 from a caller-seeded generator, annotations carry hybrid
  logical clocks. `scorecraft-ops` has reversible ops, all-or-nothing transactions and per-score
  and library undo stacks (§6). `scorecraft-format` is the §8 store with a journal for
  multi-record saves; ink is one snapshot file per page until the M8 op log.
- **Library commands.** The session holds an open library: `library.*` (create, open, import,
  folders, delete and restore, verify and repair, read-only views for agents), `score.metadata.set`,
  `page.*`, `score.pageRange.set`, bookmarks, `list.*` and `edit.undo`/`edit.redo`. Each edit is
  one transaction and one undo step, saved before the command returns; a failed save is undone in
  memory too. `score.open` takes a library score; the reader still shows the file's pages, and
  page edits apply when the M3 layout lands. The engine's clock is `chrono` natively and is set
  by the shell in the browser.

Otherwise this page is the **design plan**: the workspace, the layering rules that
keep the engine independent of the UI, the document model and how rendering, input, sync and
automation fit together. Where code later differs, add a "Current state" summary here (as
PhotoCraft does) and let it win over the plan below.

---

## 1. Goals and principles

**Goals**

- A native, portable, fully open-source sheet music reader with enScore's feature surface
  ([`feature-map.md`](feature-map.md)), written only in Rust.
- One codebase on macOS, Windows, Linux, Android, iOS/iPadOS and the web (WebAssembly).
- Instant, dependable page turns on a music stand. A reader that hesitates or crashes mid-concert
  has failed, whatever else it does.
- Markings that never damage the user's PDF and never get lost (offline-first, sync optional).
- Every feature drivable by agents and scripts.

**Principles** (carried over from the Crafting Apps framework)

1. **Engine-first, headless-first.** Every feature is reachable without a GUI: from tests, the CLI
   and MCP. The GUI is one client of the engine.
2. **Everything is a command.** Each user-visible action has a stable `CommandId`
   (`view.turn.next`, `annotate.stroke`, `list.entry.add`) with typed, serializable parameters.
   Tap zones, pedals, MIDI, menus, the command palette, the CLI, the control channel and MCP all
   dispatch the same commands. Ids come from [`feature-map.md`](feature-map.md).
3. **Data describes the UI; toolkits draw it.** Tools produce overlay primitives as data; dialogs
   come from parameter schemas. A new toolkit implements one renderer for each.
4. **The source PDF is immutable.** Imported files are stored content-addressed and never
   rewritten. Page edits, crops, rotations and markings are data on top. Export writes a new file.
5. **Immutable snapshots.** Large payloads (page bitmaps, PDF bytes, stroke lists) are `Arc`-shared
   so a library or score snapshot is cheap; undo, background rendering, autosave and sync read
   snapshots without locks.
6. **Determinism.** The same score renders the same on CPU and GPU (within tolerance), across
   thread counts and between screen and export.
7. **Pure Rust by default.** C/C++ is allowed only behind Cargo features at the I/O edge
   (for example a platform MIDI or camera binding), and only in the isolated `devices` crate.
8. **Clean-room.** See [`clean-room.md`](clean-room.md). Specs come from ISO 32000, MIDI 1.0,
   HID, eSCL, SMuFL, papers and observed behaviour.
9. **Never crash.** See [`standards/never-crash.md`](standards/never-crash.md). A hostile PDF, a
   corrupt library, an odd pedal or a full disk produces an error the user can act on.

### 1.1 Rules we adopt from day one

Each would force a rewrite later if baked in as an assumption.

| Property | Rule | Where |
|---|---|---|
| **Page geometry** | Annotations are stored in the **unrotated source page's PDF user space** (points, origin bottom-left as in ISO 32000 §8.3), never in screen pixels. Rotation, crop, margins and layout are transforms applied after. | `doc`, `layout` |
| **Page identity** | A score is a list of `PageRef { source, page_index }`, not "the PDF's pages". Reorder, hide, delete, page ranges, snips and setlists are all edits of references. | `doc` |
| **Input** | A pointer sample carries position, pressure, tilt, twist, timestamp and device kind (pen, touch, mouse, eraser end) from the start, even where a platform reports only position. | `input`, `ink` |
| **Colour** | Ink colours are sRGB with alpha; display filters (dark, sepia, inverted) apply to the *page*, and ink keeps a separate, legible mapping per filter. | `render` |
| **Size** | Libraries of 10,000+ scores and scores of 1,000+ pages are normal. Nothing loads a whole library or a whole score into memory; lists are paged and indexed. | `format`, `search` |
| **Time** | Practice logs and sync clocks go through `chrono` and hybrid logical clocks; nothing hand-rolls calendar maths. | `practice`, `sync` |
| **UI toolkit** | The core never depends on the toolkit (§4). | layering |
| **Platforms** | L0–L6 compile to `wasm32-unknown-unknown` and to Android/iOS targets; filesystem, camera, MIDI and power APIs sit behind platform-service traits. | all |

The test: **adding a new page source (images, MusicXML engraving), a new input device or a new
sync backend touches a leaf crate, not the foundation.**

---

## 2. Workspace layout

```text
scorecraft/
├─ Cargo.toml                  # [workspace], shared deps, lints, profiles
├─ crates/
│  │  ── L0 foundation (standalone, wasm-safe) ──
│  ├─ geom/        scorecraft-geom      points, rects, affine transforms, page boxes, bezier
│  ├─ raster/      scorecraft-raster    page bitmaps, tiles, mip pyramids, damage regions
│  ├─ pdf/         scorecraft-pdf       thin adapter over PDFCraft's crates (§15): open, render tiles, text, outline, annotations, write; the only door to pdfcraft-*
│  ├─ codecs/      scorecraft-codecs    raster decode/encode for imported images (PNG, JPEG, TIFF, HEIC, WebP)
│  ├─ midi/        scorecraft-midi      MIDI 1.0 messages, parser, learn-mode matching; no I/O (standalone)
│  ├─ fsc/         scorecraft-fsc       forScore .4sc reader, from the black-box spec only (standalone, read only)
│  │  ── L1 document ──
│  ├─ doc/         scorecraft-doc       library model: scores, page refs, annotation layers, setlists, bookmarks, metadata (pure data + serde)
│  │  ── L2 algorithms ──
│  ├─ ops/         scorecraft-ops       reversible operations, transactions, undo/redo
│  ├─ ink/         scorecraft-ink       stroke engine: smoothing, pressure curves, highlighter, whiteout, erasers, hit testing, stamps
│  ├─ layout/      scorecraft-layout    page layout: single, two-page, half-page and alternating turns, zoom, crop, snips
│  ├─ text/        scorecraft-text      text notes and stamp glyphs (shaping via a pure-Rust stack, SMuFL fonts)
│  ├─ search/      scorecraft-search    metadata and full-text index (incremental, on-disk)
│  ├─ algo/        scorecraft-algo      imaging for scans: edge detection, perspective, deskew, clean-up, Enhance filter
│  ├─ audio/       scorecraft-audio     decode, time-stretch, pitch-shift, loop; metronome synthesis
│  ├─ practice/    scorecraft-practice  metronome scheduling, tuner pitch detection, practice log, reports
│  │  ── L3 rendering and storage ──
│  ├─ render/      scorecraft-render    CPU page compositor (the oracle): PDF raster + filters + ink + snips
│  ├─ gpu/         scorecraft-gpu       wgpu compositor for pages and live ink
│  ├─ format/      scorecraft-format    on-disk library: content-addressed blobs, score records, atomic saves, verify/repair
│  │  ── L4 I/O and services ──
│  ├─ io/          scorecraft-io        import/export orchestration: PDF, images, folders, .4sc, setlist bundles, print spooling
│  ├─ sync/        scorecraft-sync      sync protocol, CRDT merge, backends (folder, WebDAV, ScoreCraft server), backup
│  ├─ metadata/    scorecraft-metadata  title/composer detection: PDF info, text layer, OCR, optional public catalogues
│  ├─ imslp/       scorecraft-imslp     IMSLP client over its public MediaWiki API
│  ├─ scan/        scorecraft-scan      camera capture pipeline and eSCL network scanners
│  ├─ input/       scorecraft-input     pedal key maps, MIDI learn, pen shortcuts, palm rejection → commands
│  ├─ devices/     scorecraft-devices   platform MIDI, audio I/O, camera, keep-awake; the one crate where FFI may appear (feature-gated)
│  │  ── L5 the façade every frontend talks to ──
│  ├─ engine/      scorecraft-engine    Session: open library, command registry + dispatch, jobs, events, view-models, preferences
│  │  ── L6 frontends ──
│  ├─ ui-egui/     scorecraft-ui-egui   egui shell: library, reader, tools, dialogs (thin: all actions go through the engine)
│  ├─ automation/  scorecraft-automation MCP server and control-channel handlers
│  └─ testkit/     scorecraft-testkit   synthetic PDFs, stroke generators, fixtures
├─ apps/
│  ├─ scorecraft/              desktop app (eframe/wgpu), TCP control server
│  ├─ scorecraft-cli/          headless CLI (import / info / export / run / batch / commands / mcp)
│  ├─ scorecraft-web/          the same app in the browser (trunk + wasm-bindgen)
│  ├─ scorecraft-android/      Android shell (eframe android-activity)
│  ├─ scorecraft-ios/          iOS / iPadOS shell (winit + wgpu)
│  └─ scorecraft-sync-server/  self-hostable sync and backup server
└─ xtask/                      cargo xtask layers | wasm | ci | stats | corpus | test-corpus | parity | perf | scorecard
```

## 3. Dependency layering (enforced)

```text
L6  ui-egui  automation
L5  engine
L4  io  sync  metadata  imslp  scan  input  devices
L3  render  gpu  format
L2  ops  ink  layout  text  search  algo  audio  practice
L1  doc
L0  geom  raster  pdf  codecs  midi  fsc
```

- A crate may depend only on **lower** layers. `cargo xtask layers` checks every `Cargo.toml`
  against the table in `xtask/src/layers.rs`; a new crate must be registered there.
- `pdf`, `midi` and `fsc` depend on **nothing** in the workspace, so they can be published and
  fuzzed alone. `pdf` depends on PDFCraft's crates (external, pinned), and is the only
  workspace crate that may, apart from the three exceptions in §15.
- Nothing below `ui-egui` may use egui, eframe, winit or a file dialog crate.
- `devices` is the only crate that may contain FFI, behind Cargo features, with
  `unsafe_code = "deny"` and a `SAFETY:` comment on every block; every other crate forbids
  `unsafe` (PhotoCraft's `tablet` exception, generalised to one crate).
- `testkit` is a dev-dependency only.

## 4. The UI-swap seam (the most important interface)

### 4.1 `Session`: commands in, events out

```rust
pub struct Session { /* library, open readers, history, jobs, prefs */ }

impl Session {
    pub fn dispatch(&mut self, id: &str, params: serde_json::Value) -> Result<serde_json::Value, EngineError>;
    pub fn events(&self) -> impl Iterator<Item = Event>;     // ScoreChanged, PageTurned, SyncStatus, JobProgress…
    pub fn view_model(&self, reader: ReaderId) -> ReaderView; // what to draw, as data
}
```

The UI never mutates the library directly. It dispatches commands and draws view-models.

### 4.2 `CommandRegistry`

Each command is a `CommandSpec`: `id`, `label`, menu path, default shortcut, a params doc string
(`{"layer":id?,"tool":"pen|highlighter"="pen"}`, what agents read through `commands`),
an `enabled` predicate and `run`. `run` validates params, returns a JSON result and records
**exactly one** history step (strokes and drags coalesce). Commands live in area modules
(`crates/engine/src/<area>_cmds.rs` exposing `specs()`), registered in `commands.rs`.

`feature_catalog.rs` holds the feature-map rows (id, title, command ids); `cargo xtask parity`
checks which commands are registered and live and writes [`parity-checklist.md`](parity-checklist.md).

### 4.3 Tools and gestures

Tools (pen, highlighter, whiteout, erasers, text, stamp, lasso, snip, shortcut) are state
machines: pointer samples in, ops and overlay primitives out. Page-turn gestures are a tool too,
so the same zone logic serves touch, mouse and the control channel's synthetic pointer events.

### 4.4 Reader view

`layout` turns (score, page refs, mode, viewport, turn state) into a list of placed page regions:
whole pages, half pages during a half-page turn, snips. `render`/`gpu` draw that list. Turn
animations are interpolations of two layouts, so e-ink mode simply skips them.

---

## 5. Document model (`scorecraft-doc`)

Pure data, `serde`, no I/O.

```text
Library
├─ folders: tree of Folder { id, name, children }
├─ scores:  ScoreId → Score
├─ lists:   ListId → Setlist { name, entries: [Entry { score, page_range?, tempo?, note? }] }
├─ trash:   Recently Deleted (ScoreId, deleted_at)
└─ settings (library-level, synced)

Score
├─ id: ScoreId (UUID v7)            source: BlobRef (sha256 of the imported PDF / image set)
├─ metadata: title, composer, key, tags, custom fields, notes, detection provenance
├─ pages: [PageRef { source_page, rotation, hidden, crop, margins }]   page_range?
├─ layers: [AnnotationLayer { id, name, visible, locked, order }]
├─ annotations: PageKey → [Annotation]
│     Stroke { tool: Pen|Highlighter|Whiteout, colour, width, points: [InkPoint { x, y, pressure, tilt, twist, t }] }
│     Text { rect, text, style }     Stamp { glyph (SMuFL codepoint), rect, colour }
│     Snip { from: (page, rect), to: (page, rect) }   Shortcut { at: (page, rect), target: (page, rect?) }
├─ bookmarks: [Bookmark { name, page }]
├─ audio: Track? { blob, turn_recording: [(time, page)] , loop?, speed, transpose }
├─ midi_on_open: { program?, bank?, song? }
└─ practice notes
```

Every annotation has an id, a layer, a creator device and a hybrid-logical-clock stamp so sync can
merge without conflicts (§9). Erasing a part of a stroke replaces it with its remaining pieces
(new ids) and tombstones the original.

## 6. Operations and history (`scorecraft-ops`)

Reversible ops on a score or the library, grouped into transactions; one command = one undo step.
History is per score and per library, bounded in memory, and survives a reopen for the current
session. Sync applies remote ops as their own transactions, never merged into the user's undo
stack.

## 7. Rendering

- **PDF.** `scorecraft-pdf` wraps PDFCraft: `pdfcraft-cos` parses lazily and tolerantly (xref
  streams, object streams, incremental updates, xref repair, decryption through `pdfcraft-crypt`),
  and `pdfcraft-render` rasterises pages on the CPU through PDFCraft's vendored, patched `hayro`
  family (JBIG2, CCITT, DCT, JPX, Type 1/TrueType/CFF/Type 3). See §15 and decision D1 in
  [`decisions.md`](decisions.md).
- **Caching.** Rendered tiles are cached per (page, zoom bucket, filter). The current, next and
  previous pages (and the next setlist piece's first page) are pre-rendered in background jobs so a
  turn is a cache hit. Budgets live in `perf/budgets.toml` ([`scorecard.md`](scorecard.md#performance)).
- **Two compositors.** `render` is the CPU oracle (export, print, tests, wasm fallback); `gpu` draws
  pages and live ink with wgpu (Metal, Vulkan, DX12, GLES, WebGPU). They are tested against each
  other within 1/255.
- **Ink.** Live strokes are drawn on the GPU as they arrive (predicted points for latency, replaced
  by real ones); committed strokes are tessellated once and cached per page.
- **Filters.** Dark, sepia and inverted are per-page colour transforms with ink colours remapped
  for legibility; Enhance is an `algo` pass on scanned (image-only) pages, cached.

## 8. Library storage (`scorecraft-format`)

```text
<Library>.scorecraft/
├─ manifest.json            version, library id, device id
├─ blobs/sha256/ab/cdef…    imported PDFs, images, audio (immutable, content-addressed)
├─ scores/<score-id>.json   score record (metadata, page refs, layers, bookmarks)
├─ ink/<score-id>/<page>.bin annotation op log per page (append-only, compacted)
├─ lists/<list-id>.json
├─ index/                   search index (rebuildable)
└─ journal/                 crash-safe write-ahead log
```

- Saves are atomic (write a temp file, fsync, rename), per record, never a whole-library rewrite.
- `library.verify` checks every blob hash and record and rebuilds the index (LIB-12).
- The format is documented in [`file-format-parity.md`](file-format-parity.md#scorecraft-library-format)
  so other tools can read it. Adding a `doc` field must add it here with `#[serde(default)]`; the
  `format` crate fails to compile when a `doc` struct gains a field it does not map (PhotoCraft's
  rule for `.pcraft`).

## 9. Sync

Owner's design (2026-10-28). Sync has two parts, built in this order:

1. **Google Drive sync.** The whole library (score files and folders), app configuration and
   annotations sync to the user's own Google Drive. Google Drive comes first; OneDrive and
   Dropbox follow later behind the same backend trait.
2. **Device-to-device sync.** Optional sign-in links a user's devices across platforms. Devices
   sync the library, configuration and annotations directly over the local Wi-Fi network, and
   through Google Drive (one device uploads, the others download) when they aren't on the same
   network.

- **Download everything, offline first.** Every device keeps a full local copy of every score,
  annotation and setting it syncs, and downloads all of it ahead of time. Nothing is fetched on
  demand when a score opens: a musician in a concert hall never waits on the network. Sync runs
  in the background and never during performance mode.
- **The device's library is the source of truth**; sync exchanges ops and blobs.
- **Merge.** Annotations are an add-wins set with tombstones; metadata fields are last-writer-wins
  registers keyed by hybrid logical clock; list entry order uses a sequence CRDT. Two devices
  marking the same page offline both keep their strokes.
- **Backends** behind one trait: Google Drive first (its API, with the user's own Google account,
  opt-in), then OneDrive and Dropbox; local Wi-Fi peers (discovered on the LAN, paired once,
  encrypted); and later a shared folder the OS already syncs, WebDAV and the self-hostable
  `scorecraft-sync-server`. A ScoreCraft account is never required: device pairing is optional
  and works without one. Optional end-to-end encryption of ops and blobs.
- **Backup** is a snapshot of the library directory into a single archive, local or to a backend.

## 10. Input

- **Pedals.** Bluetooth page-turn pedals present as keyboards. `input` maps key events (arrows,
  Page Up/Down, Space, Enter, custom) to `view.turn.*`, with a learn mode and per-pedal profiles.
- **MIDI.** `midi` parses messages; `devices` opens ports (CoreMIDI, Windows MIDI, ALSA, Android
  MIDI, Web MIDI); `input` maps learnt messages to commands. Output sends Program Change, Bank
  Select and Song Select on score open.
- **Pen.** Pressure, tilt and eraser end come from winit where it reports them and from platform
  hooks in `devices` where it doesn't (as PhotoCraft's `tablet` crate does on macOS and Windows).
  The owner performs from a **Windows tablet**, so Windows Ink (pressure, eraser, barrel button,
  palm rejection, touch and pen together) is the first pen platform to get right. On **iPad**, a
  thin native Swift host owns the Apple Pencil (pressure, tilt, hover, predicted touches, low
  latency) and hands samples to the Rust ink engine; everything else stays in Rust and egui
  (decision D3).
- **Palm rejection.** Pen-only mode ignores touch on the page while a pen is in range.

## 11. Engine runtime model

One `Session` per process, background jobs (render, import, OCR, sync, scan) on a rayon pool with
cancellation, events to the shell. Preferences are typed, serde, versioned, and every preference
must change behaviour (the scorecard counts "settings that do nothing").

## 12. Automation and extensibility

- **CLI** (`scorecraft-cli`): `import`, `info`, `export`, `run --cmd <id> --params <json>`,
  `batch`, `commands`, `mcp`.
- **Control channel**: authenticated, loopback-only JSON lines (`scorecraft --control`) to read
  UI state, send pointer and key events, and take offscreen screenshots
  ([`control-protocol.md`](control-protocol.md)).
- **MCP**: the same commands as tools, headless or bridged to the running app.

## 13. Testing and quality

- Unit tests in every crate; integration tests per engine area; `panic_hunt` fuzzes every
  command with adversarial params.
- `fsc` gets round-trip, synthetic-generator, malformed-input and `cargo fuzz` targets; `pdf` gets
  adapter tests on `pdfcraft-testkit` fixtures and a fuzz target over the adapter (PDFCraft fuzzes
  its own parser).
- Real-file corpora (public-domain PDFs from IMSLP and Mutopia, the PDF Association's test
  suites, our own `.4sc` samples) live in a separate corpus repository, pinned and
  sha256-verified ([`development.md`](development.md#test-corpora)).
- GPU vs CPU render parity; offscreen UI snapshots for visual checks; performance budgets on a
  fixed runner.

## 14. Open decisions

Tracked with dates in [`decisions.md`](decisions.md): D1 PDF backend (**decided:** reuse PDFCraft's
headless crates, §15), D2 on-disk record encoding (JSON vs CBOR), D3 iOS shell (**decided:** thin
Swift host for the pen), D4 OCR for title detection (leaning to `pdfcraft-ocr`), D5 sync protocol
(now device-to-device over Wi-Fi and Google Drive, §9), D6 product name (**decided:** ScoreCraft).

## 15. Reusing PDFCraft

Scores are PDFs, and [PDFCraft](https://github.com/storytold/pdfcraft) (the Crafting Apps'
clean-room, pure-Rust reimplementation of Adobe Acrobat) already has a hardened PDF stack. We
reuse it for the whole PDF backend and keep ScoreCraft's own code for what a score reader adds
(library, layout and turns, ink, setlists, sync).

**Licence.** PDFCraft is **MIT OR Apache-2.0**, the same as ScoreCraft, so its crates can be
depended on, modified and redistributed with ScoreCraft under our licence. Obligations: keep
PDFCraft's copyright notice ("ArtCraft Team and the PdfCraft contributors") and its third-party
notices in our `NOTICE` (hayro Apache-2.0 OR MIT; the Foxit standard-14 fonts and Adobe CMap
resources bundled by hayro under BSD-3-Clause; lopdf MIT; the CC0 colour profile in
hayro-interpret; the OCR models' licences if `pdfcraft-ocr` ships). Not covered by the licence and
never copied: PDFCraft's ArtCraft brand files (`docs/brand/`). PDFCraft is itself clean-room
(behaviour of Acrobat from public specs), so reusing it keeps ScoreCraft clean
([clean-room §2](clean-room.md#2-allowed-sources)).

**How we consume it: git dependencies pinned by revision, not a vendored copy.** PDFCraft's crates
are not on crates.io (checked 2026-10-28). Cargo can take individual crates from its workspace:

```toml
[workspace.dependencies]
pdfcraft-cos    = { git = "https://github.com/storytold/pdfcraft", rev = "<pinned sha>" }
pdfcraft-render = { git = "https://github.com/storytold/pdfcraft", rev = "<pinned sha>" }
# … one line per crate below, all at the same rev

# PDFCraft patches hayro in its own vendor/ through [patch.crates-io]. Patches apply only at the
# workspace root, so ScoreCraft must repeat them, pointing at the same rev:
[patch.crates-io]
hayro           = { git = "https://github.com/storytold/pdfcraft", rev = "<pinned sha>" }
hayro-interpret = { git = "https://github.com/storytold/pdfcraft", rev = "<pinned sha>" }
hayro-syntax    = { git = "https://github.com/storytold/pdfcraft", rev = "<pinned sha>" }
hayro-cmap      = { git = "https://github.com/storytold/pdfcraft", rev = "<pinned sha>" }
hayro-postscript = { git = "https://github.com/storytold/pdfcraft", rev = "<pinned sha>" }
hayro-jbig2     = { git = "https://github.com/storytold/pdfcraft", rev = "<pinned sha>" }
lopdf           = { git = "https://github.com/storytold/pdfcraft", rev = "<pinned sha>" }
vello_common    = { git = "https://github.com/storytold/pdfcraft", rev = "<pinned sha>" }
```

Why not vendor: a pinned revision gets PDFCraft's fuzz and corpus fixes by bumping one SHA, and
fixes we need go upstream as PRs to PDFCraft, where its tests and reviewers check them. We vendor
a crate into `vendor/` (with its licence files and a `vendor/README.md` row naming each patch, as
PDFCraft does) only if we need a change PDFCraft won't take, or if PDFCraft becomes unavailable.
Which of the two we use is recorded per crate in [`decisions.md`](decisions.md).

**What we take** (PDFCraft 0.14.0, `main` at `20eebc1c`, 2026-10-27):

| PDFCraft crate | Its layer | What it gives ScoreCraft | Used by | Feature-map rows |
|---|---|---|---|---|
| `pdfcraft-geom` | L0 | points, rects, page transforms | `pdf` | all page geometry |
| `pdfcraft-filters` | L0 | Flate, LZW, predictors, ASCII85/Hex, RunLength | `pdf` (via cos) | READ-1 |
| `pdfcraft-crypt` | L0 | standard security handler R2–R6 (RC4, AES) | `pdf` (via cos) | encrypted scores |
| `pdfcraft-cos` | L1 | lazy, tolerant parser with xref repair, copy-on-write edits, incremental and full saves; wasm-safe, no `unsafe`, no panics | `pdf` | READ-1, READ-14, EXP-2 |
| `pdfcraft-model` | L2 | page tree with inherited attributes, `/Rotate`, `/UserUnit`, page boxes | `pdf` | READ-1..3, NAV-5 |
| `pdfcraft-content` | L2 | content-stream operators and serializer | `pdf` | PAGE-6 |
| `pdfcraft-fonts` | L2 | standard-14 metrics, encodings, shaping for generated text | `pdf` | ANNO-8 export, PRAC-9 |
| `pdfcraft-render` | L3 | CPU rasterisation (patched hayro), text and object inspection | `pdf` | READ-1, READ-14, LIB-6 |
| `pdfcraft-annot` | L3 | ISO 32000 §12.5 annotation builders, appearance streams, reading existing annots | `pdf` | EXP-2, IMP-7, ANNO-1..9 export |
| `pdfcraft-organize` | L4 | rotate, delete, move, insert pages; combine documents | `pdf` | EXP-3, PAGE-1..4 on export |
| `pdfcraft-create` | L4 | blank PDFs, PDFs from images and text | `pdf` | PAGE-6, IMP-1, IMP-3, PRAC-9 |
| `pdfcraft-print` | L4 | page selection, sizing, n-up composition (spools through `craft-print`) | `io` | EXP-1 |
| `pdfcraft-ocr` | L4 | text recognition (ocrs) as a searchable layer | `metadata`, `scan` | LIB-6, LIB-7, IMP-1 |
| `pdfcraft-testkit` | test | synthetic PDF fixtures | dev-dependency | tests |
| `pdfcraft-oracle` | test | external oracle processes for output checks | dev-dependency | PDF-5 |

**What we don't take:** PDFCraft's engine, UI, automation, platform, forms, JavaScript, XFA,
signing, redaction, preflight, accessibility, export-to-Office, editing, comparison, measuring and
optimizer crates. They serve an Acrobat-class editor, not a reader, and would pull
Acrobat-specific commands and dependencies (a JavaScript engine, native crypto) into our build.
ScoreCraft is a different app: it shares PDFCraft's framework and the implementation of what the
two have in common, nothing more. `cargo xtask layers` rejects these crates anywhere in the
resolved graph (`xtask/src/graph.rs`, `BANNED`).

**PDFCraft's patches.** PDFCraft's root `[patch.crates-io]` has two parts: path patches into its
`vendor/` (the hayro family, lopdf, vello_common and, since 0.15, vello_cpu), and git patches that swap in the storytold forks
of egui (14 crates) and winit. We repeat only the first part; the egui and winit forks are a UI
matter, settled in §16. Cargo finds the vendored crates inside the PDFCraft repository even though
its workspace excludes `vendor/` (checked 2026-10-28).

**Test build (2026-10-28).** A scratch crate taking `pdfcraft-render`, `pdfcraft-model`,
`pdfcraft-annot` and `pdfcraft-organize` at `20eebc1c` with the hayro, lopdf and vello_common
patches checks natively and for `wasm32-unknown-unknown` on Rust 1.99.1: 165 packages, no C code,
no egui, no craft-libs crates. Adding `craft-ui` (§16) still checks on both targets with one egui
and one hayro. PDFCraft's full graph (794 packages, with `aws-lc-sys` and `boa_engine`) comes only
from the crates we don't take.

**Pinned build (2026-10-31).** The workspace takes `pdfcraft-render` at `65ff3a00` (0.15.0). That
revision adds a `vello_cpu` vendor patch, which we repeat. The graph is 161 packages and checks
natively and for wasm32 with one hayro and no egui. `cargo xtask layers` also checks that no
public item in `crates/pdf/src` names a `pdfcraft` or `backend::` type.

**Rules.**

- Only `scorecraft-pdf` depends on `pdfcraft-*`, except `io` (`pdfcraft-print`), `metadata` and
  `scan` (`pdfcraft-ocr`). `cargo xtask layers` enforces the allow-list, so swapping or forking the
  backend touches those crates only. No `pdfcraft-*` type appears in `scorecraft-pdf`'s public API.
- All `pdfcraft-*` crates and the `[patch.crates-io]` entries use **one** revision. Bumping it is
  its own PR that runs `cargo xtask test-corpus` and the render budgets.
- PDFCraft's never-crash, no-`unsafe` and wasm rules match ours, so its crates count as compliant
  for our scorecard; anything that panics through them is a bug we reproduce, fix upstream and pin.
- Rendering latency for turns is ours: `scorecraft-render` caches and pre-renders tiles that
  `pdfcraft-render` produces; `gpu` uploads and composites them with ink.
- If PDFCraft's licence ever changed, the pinned revisions stay MIT OR Apache-2.0; we would vendor
  the last such revision.

## 16. UI framework: craft-ui and one egui

ScoreCraft has its **own UI**, designed for reading and performing scores, not PDFCraft's. It is
independent of the storytold suite but canonical with it: it is built from the suite's shared
framework, so it looks and behaves like a Craft app.

- **One egui in every build.** Two egui versions can't share types, so the resolved graph may
  hold exactly one egui (and one eframe, epaint, winit, wgpu). `cargo xtask layers` fails
  otherwise (`xtask/src/graph.rs`, `ONE_VERSION`).
- **ScoreCraft's egui** is the egui that `craft-ui` is built on: the suite's fork
  (`storytold/egui`, egui 0.37 at the revision craft-libs pins, with its winit fork), patched at
  our workspace root at that same revision. We follow craft-libs' revision, not PDFCraft's, and
  bump both together when they differ. ScoreCraft's customisation lives above egui, in
  `scorecraft-ui-egui`: theme tokens, the reader canvas, page-turn and ink widgets, tap zones,
  performance mode. A change egui itself needs goes upstream to `storytold/egui` first; only if it
  can't land there do we keep a ScoreCraft branch of that fork, still one egui.
- **What we take from craft-libs** (git dependencies pinned to one craft-libs revision, like
  PDFCraft): `craft-ui` (controls, icons, fonts, docking), `craft-i18n` (translations, M11),
  `craft-keymap` (shortcuts), `craft-settings` (config storage), `craft-atomic` (crash-safe saves,
  M1), `craft-print` (via `pdfcraft-print`) and `craft-quit`/`craft-menu` for the native macOS
  menu bar. Only `ui-egui` and apps take the egui-facing ones; the data-only ones (`atomic`,
  `settings`, `i18n`, and `keymap` without its `egui` and `ui` features) may be used from their
  layer; `cargo xtask layers` catches an egui reached below L6.
- **What we never take:** `craft-update` (ScoreCraft does not self-update through the suite's
  updater; releases go through the platform stores and packages), and PDFCraft's `pdfcraft-ui-egui`
  and `pdfcraft-engine`. Both are in `BANNED`.
- **craft-pdf.** craft-libs also has `craft-pdf`, the suite's shared PDF reader and renderer. It
  patches hayro from its own `vendor/`, so it can't be combined with PDFCraft's hayro patches in
  one build. We use PDFCraft's crates now because ScoreCraft also needs annotation export,
  page organising and PDF creation, which only PDFCraft has; when PDFCraft moves onto `craft-pdf`,
  we follow it.

## 17. Platforms and the web runtime

- **Windows tablet first.** The owner performs from a Windows tablet: pen and touch on Windows
  are the first platform to get right, then macOS and Linux desktops, Android tablets, and iPad
  (with the Swift pen host, §10).
- **The web build is a nice-to-have of medium priority.** It ships after the desktop reader, and
  every L0–L6 crate keeps checking for `wasm32-unknown-unknown` so it stays possible.
- **No crash guard on the web.** On `wasm32-unknown-unknown` a panic aborts; it can't be caught.
  The native last-resort guard around dispatch, import and export does not exist there, so on the
  web the never-crash rules are the only defence. Panic-free code (lints, `panic_hunt`, fuzzing)
  is what keeps the browser build on stage.
- **No threads by default on the web.** Background jobs (rendering the neighbouring pages,
  import, OCR, sync) run on a rayon pool natively. In the browser they run in web workers, or in
  small slices on the main thread between frames, so a page turn still never waits on rendering.
  The job API in `engine` hides which one runs. Turn budgets get a web column in
  `perf/budgets.toml` once the web reader exists.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-08 | patch | Current state: dark and sepia recolour the rendered page by lightness, so scanned scores change colour too |
| 2026-10-31 | patch | Current state: M1 foundation crates (geom, raster, doc, ops, format) and the library commands |
| 2026-10-31 | patch | Current state: `score.open` and `view.render` in the engine, CLI `info` and `render` |
| 2026-10-31 | patch | Current state: `scorecraft-render` page cache, pre-rendering and dark/sepia filters |
| 2026-10-31 | patch | Current state: `scorecraft-pdf` on `pdfcraft-render` 0.15.0 (`65ff3a00`) with the `vello_cpu` patch; §15 pinned build |
| 2026-10-28 | minor | §15: headless PDFCraft crates only, test build results, banned crates; §16: own UI on craft-ui with exactly one egui (the suite's fork), no craft-update; §17: Windows tablet first, web runtime limits; §9: sync is Google Drive plus device-to-device, download everything; §10: Windows Ink first, Swift pen host on iPad |
| 2026-10-28 | minor | §15: the PDF backend reuses PDFCraft's crates (git dependencies pinned by revision); `pdf` becomes an adapter |
| 2026-10-28 | major | First version: design plan adapted from PhotoCraft's architecture for a score reader |
