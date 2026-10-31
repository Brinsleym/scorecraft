# Development guide

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-10-31 · **Change:** minor (corpus, private-library, fuzz and perf tasks; earlier: first version, written ahead of milestone M0) · **Target:** enScore 3.7 (2026)

How to build, test, run and drive ScoreCraft. Commands marked _(M0)_ arrive with the workspace
skeleton ([`roadmap.md`](roadmap.md#milestones)); until then this page is the contract M0 must meet.

## Prerequisites

- Rust stable (edition 2024; the minimum version is pinned in `Cargo.toml` `rust-version`) with
  `rustfmt` and `clippy`.
- For the web build: `rustup target add wasm32-unknown-unknown`, `cargo install trunk`.
- For Android: the Android SDK and NDK, `cargo install cargo-apk` (or `xbuild`), and
  `rustup target add aarch64-linux-android`.
- For iOS: Xcode, `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.
- Linux desktop: the usual winit/wgpu system libraries (Wayland/X11, Vulkan or GL drivers), ALSA
  headers only if you enable the `alsa-midi` feature of `scorecraft-devices`.
- Optional: `cargo install cargo-nextest cargo-fuzz mdbook`.

## Build and run _(M0)_

```sh
cargo run --release -p scorecraft -- score.pdf      # the desktop app
cargo run -p scorecraft-cli -- --help               # the headless CLI
cargo test --workspace                              # the test suite
cargo xtask ci                                      # what CI runs on Linux
(cd apps/scorecraft-web && trunk serve)             # the browser build
```

### Repository tasks

| Task | What it checks |
|---|---|
| `cargo xtask ci` | what the Linux CI jobs run: `fmt --check`, clippy with `-D warnings`, `test --workspace` (plus doctests), `panic_hunt`, `layers`, `parity --check`, `scorecard --check`, `wasm` |
| `cargo xtask layers` | every package is registered in `xtask/src/layers.rs`, depends only on lower layers, uses the workspace lints and carries the never-crash `#![deny(...)]` at its root; PDFCraft and UI-toolkit allow-lists; no PDFCraft type in `scorecraft-pdf`'s public API; the fuzz workspace patches PDFCraft's crates like ours |
| `cargo xtask parity` | regenerates `crates/engine/src/feature_catalog.rs` and [`parity-checklist.md`](parity-checklist.md) from the feature map and the registry; fails below `FLOOR`; `--check` fails if either is stale |
| `cargo xtask scorecard` | regenerates [`scorecard.md`](scorecard.md) from `perf/budgets.toml`, `scorecard/*.toml`, the parity docs' status columns and measured counts; `--check` fails if stale |
| `cargo xtask corpus` | `--all` downloads and verifies the pinned public-domain scores; `--references` renders PDFium references |
| `cargo xtask test-corpus` | the opt-in corpus test (PDF-1, PDF-2), on the pinned corpus or `--library <dir>` |
| `cargo xtask perf` | the render budgets in `perf/budgets.toml`; `--update-baseline` writes `perf/baseline.json` |
| `cargo xtask wasm` | every L0–L6 crate checks for `wasm32-unknown-unknown` |

Synthetic test PDFs come from `scorecraft_testkit::pdf` (`PdfBuilder` for valid scores of any
size, page count and rotation; `pdf::hostile::all()` for malformed files). To look at one in
another reader: `cargo run -p scorecraft-testkit --example write_sample -- out.pdf`.

### Faster test loops

- `cargo test -p <crate> --lib --tests` skips examples and doctests.
- `cargo test -p <crate> --test <file>` runs one integration-test file.
- `cargo nextest run` runs test binaries in parallel (`.config/nextest.toml`).
- Use your own target dir per agent: `CARGO_TARGET_DIR=target/agent-<name>`.

## Fonts

Fonts are never committed ([`standards/fonts.md`](standards/fonts.md)). To build with the music
symbol and UI fonts:

```sh
git clone <fonts repository> ../scorecraft-fonts
SCORECRAFT_FONTS_DIR="$PWD/../scorecraft-fonts" cargo run --release -p scorecraft
```

Without it, ScoreCraft uses system fonts and a built-in fallback for stamps.

## Driving the app programmatically

Every action is a command, so you can drive ScoreCraft without touching the UI:

```sh
# Headless: import, tag, export with ink
scorecraft-cli import ./scans --library ~/Scores.scorecraft --detect-metadata
scorecraft-cli run ~/Scores.scorecraft \
  --cmd score.metadata.set --params '{"score":"Bach BWV 1007","composer":"J. S. Bach","key":"G major"}' \
  --cmd score.export.pdf   --params '{"score":"Bach BWV 1007","layers":["Performance"],"out":"bwv1007.pdf"}'

# Every command and its params
scorecraft-cli commands --json

# Let an agent drive it over MCP (headless, or bridged to the running app)
scorecraft-cli mcp
```

`run` prints one JSON line per step. It stops at the first failing step: the results of the steps
that ran are still printed, followed by `{"error":{"code":…,"message":…,"command":…,"step":n}}`
(the control protocol's error shape), and it exits with status 1.

**Windows PowerShell** (5.1) strips the inner double quotes from `'{"x":1}'` when it calls a
native program, so the CLI receives `{x:1}`. Put params in a file, or pipe them in:

```powershell
scorecraft-cli run --cmd engine.commands.list --params-file params.json
'{"filter":"view."}' | scorecraft-cli run --cmd engine.commands.list --params-file -
```

The desktop app's control channel (`scorecraft --control`) reads UI state, sends pointer, key,
pedal and MIDI events, and takes offscreen screenshots ([`control-protocol.md`](control-protocol.md)).

## Offscreen UI snapshots (no window)

```sh
cargo run -p scorecraft-ui-egui --example snapshot -- --score corpus/public-domain/bach-1007.pdf \
  --layout twoPage --page 3 --out /tmp/reader.png
```

It composes the reader on the CPU (`view.snapshot`), so it needs no GPU. Look at the PNG before you submit a UI change; attach before/after images to the PR.

## Environment variables

| Variable | Effect |
|---|---|
| `SCORECRAFT_FONTS_DIR` | Build input: fonts checkout (see Fonts) |
| `SCORECRAFT_LOG` | Log filter (`info`, `scorecraft_pdf=debug`) |
| `SCORECRAFT_LIBRARY` | Library path override for tests and the CLI |
| `SCORECRAFT_NO_NETWORK=1` | Refuse every network call (IMSLP, catalogues, sync); CI sets it |
| `SCORECRAFT_NO_PRINT=1` | The engine refuses to spool print jobs; CI sets it |
| `SCORECRAFT_CONTROL_TOKEN` | Fixed control-channel token for local scripting (never in CI logs) |

## Logs

Native builds log to the platform log directory (`~/Library/Logs/ScoreCraft`,
`%LOCALAPPDATA%\ScoreCraft\logs`, `$XDG_STATE_HOME/scorecraft`). Logs never contain score
contents, file names outside the library, or account tokens.

## Testing strategy

| Kind | Where | What |
|---|---|---|
| Unit | every crate | algorithms, model invariants |
| Engine integration | `crates/engine/tests/it/` | each command: behaviour, undo/redo, disabled states, bad params |
| `panic_hunt` | `crates/engine/tests/panic_hunt.rs` (`--ignored`) | every command with adversarial params and states |
| Format | `crates/pdf`, `crates/fsc`, `crates/format` | round trip, synthetic generators, malformed input, fuzz |
| Render parity | `crates/gpu/tests/` | GPU vs CPU within 1/255 on synthetic and corpus pages |
| Layout goldens | `crates/layout/tests/` | half-page and alternating turns, spreads, crops, snips |
| UI snapshots | `crates/ui-egui/examples/snapshot.rs` | offscreen renders compared with goldens |
| Sync | `crates/sync/tests/` | randomised multi-device op histories converge (property tests) |
| Corpus | `--features corpus` | real public-domain PDFs and `.4sc` samples |
| Performance | `cargo xtask perf` | budgets in `perf/budgets.toml` |

Every crash fix ships with a regression test that panicked before the fix.

## Test corpora

Real-file corpora live in `corpus/` (gitignored, **never committed**). `cargo xtask corpus --all`
downloads the pinned public-domain scores in [`xtask/corpus-pins.toml`](../xtask/corpus-pins.toml)
and checks each one's SHA-256.

| Corpus | Contents | Licence rule |
|---|---|---|
| Pinned public-domain scores (`xtask/corpus-pins.toml`) | 12 engraved Mutopia pieces and 8 scans from Wikimedia Commons (JPEG, JBIG2, JPX, CCITT), 1 to 36 pages | Public domain worldwide; the pin records where that is stated |
| A private library (`--library <dir>`) | Whatever a contributor owns: the owner's real scores | Only read, never copied: results go to `corpus/`, files are named by hash |
| PDF Association test suites | Conformance and edge-case PDFs | As published |
| Synthetic | Generated in `testkit` at test time | Ours |

The corpus test (`crates/pdf/tests/corpus.rs`) is opt-in through the `corpus` cargo feature.
It checks PDF-1 and PDF-2 in [file-format-parity](file-format-parity.md#pdf-fidelity-targets).

- **PDF-1:** every distinct score opens or asks for its password.
- **PDF-2:** every page renders at 150 dpi, and the first three also at 300 dpi. Each render is compared with PDFium's after both are reduced to 4 × 4-pixel grey blocks. PDFium draws the references at exactly the requested scale (its `PdfPage.render` rounds a Letter page at 150 dpi up to 1651 rows and stretches it) and without image smoothing (`no_smoothimage`), as ScoreCraft does for every image whose /Interpolate flag is off; otherwise every scan fails on its anti-aliased edges alone.

`cargo xtask corpus --references` renders the PDFium references. It needs Python 3 with
`pip install pypdfium2 pillow`.

`cargo xtask test-corpus` fetches the corpus, renders references if they are missing, and runs the
test. The report goes to `corpus/report.md`. To test a private library, run
`cargo xtask test-corpus --library "C:\path\to\scores" --max-pages 20`, and the report goes to
`corpus/report-library.md`. Public-domain scores that fail on a known PDFCraft render gap are listed in
`crates/pdf/tests/corpus-known.txt`: they are reported but left out of the PDF-2 rate, and the list
only shrinks. A private library is held to the PDF-1 floor; its PDF-2 rate is reported only. After
every PDFCraft pin bump, run both and compare with the numbers in
[file-format-parity](file-format-parity.md#corpus-results-2026-10-31-pdfcraft-65ff3a00).
With `SCORECRAFT_CORPUS_DUMP=1` set, the test also writes ScoreCraft's
full-resolution grey render of each page that misses its reference to `corpus/diff/`.

The corpus test needs files that CI can't download, so it runs on contributors' machines. The
adapter's fuzzing runs in two places:

- `crates/pdf/tests/mutate.rs` runs 600 deterministic mutations in every CI job.
- `crates/pdf/fuzz` holds the libFuzzer target: `cargo +nightly fuzz run open_render`.

## Scorecard and performance budgets

`cargo xtask scorecard` regenerates [`scorecard.md`](scorecard.md) from `scorecard/*.toml`,
`perf/budgets.toml`, `perf/baseline.json`, corpus floors and a preferences audit. CI runs
`cargo xtask scorecard --check`. `cargo xtask perf` measures the render budgets (P2, P3, P4, P13) on the corpus, or on
synthetic scores without it; `--update-baseline` writes `perf/baseline.json` from a reference
device named by `SCORECRAFT_PERF_DEVICE`.

## Web build

`apps/scorecraft-web` is the desktop shell (`ui-egui`) on eframe's WebGL backend. To run it:

```bash
cargo build -p scorecraft-web --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir apps/scorecraft-web/pkg target/wasm32-unknown-unknown/release/scorecraft-web.wasm
python3 -m http.server -d apps/scorecraft-web 8000
```

then open <http://localhost:8000>. Install the CLI at the version in `Cargo.lock`
(`cargo install wasm-bindgen-cli --version 0.2.130 --locked`). Scores come from the file picker
(the Open button) or a drop, and open through an in-memory library: markings last until the tab
closes. Settings are kept in local storage. Export downloads the file; keep-awake uses the Screen
Wake Lock API where the browser has it. Not yet: a library kept in the browser (OPFS), Web MIDI. `cargo xtask wasm` checks it builds.

## Mobile builds

`apps/scorecraft-android` and `apps/scorecraft-ios` are thin shells around the same engine and UI.
Platform services (files, share sheet, camera, keep-awake, MIDI, pen) are implemented in
`scorecraft-devices` behind the platform-service traits. Test on a real tablet before claiming a
mobile feature done: emulators do not report pen pressure or pedal timing faithfully.

## Dev log

Append a terse entry to `log/devlog.md` (gitignored) when you finish a task: what landed, numbers,
what is still open. It is how the next agent picks up after an interrupted session.
