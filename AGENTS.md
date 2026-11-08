# AGENTS.md: guide for AI agents and contributors

ScoreCraft is an open-source, native sheet music reader: a clean-room reimplementation of
**enScore**, written in **Rust only** (no JavaScript or TypeScript). **No Tauri, Electron or
webview shells:** the desktop and mobile apps are native egui/eframe on wgpu, and the web build is
the same Rust compiled to WebAssembly (trunk + wasm-bindgen). Never add a webview or JS UI
framework as a dependency, build step or packaging target. The one non-Rust code is the iPad's
thin Swift host for the Apple Pencil (decision D3): it forwards pen samples and holds no UI logic.
The UI is ScoreCraft's own, built on the Craft suite's `craft-ui` with exactly one egui in the
build; never take PDFCraft's UI or `craft-update` ([architecture §16](docs/architecture.md#16-ui-framework-craft-ui-and-one-egui)). The product name is always written
**ScoreCraft** in user-facing text (UI, window titles, About, installers, release names, docs
prose), following the Crafting Apps' `{Function}Craft` convention. Machine names stay lowercase:
crates (`scorecraft-*`), binaries, file names, ids (`app.scorecraft`). The goal is enScore parity
(the same capabilities, gestures and file interchange) with better performance, no paywall, no
required account, and every feature drivable by agents. Read this file first, then `docs/`.

ScoreCraft follows the process of the storytold Crafting Apps (see
[storytold/photocraft](https://github.com/storytold/photocraft) and
[storytold/craft-contrib](https://github.com/storytold/craft-contrib)). It is an independent
project, not part of the storytold organisation; the shared standards it needs are copied into
[`docs/standards/`](docs/standards/README.md) rather than referenced from a private repository.

## 1. Orientation (5 minutes)

| Read | Why |
|---|---|
| [`docs/clean-room.md`](docs/clean-room.md) | **The rule book.** What we may and may never use. Read before any feature work |
| [`docs/architecture.md`](docs/architecture.md) | Crate map, dependency layers, the engine/UI seam, document model |
| [`docs/development.md`](docs/development.md) | Build, test, run, drive the app programmatically, corpora |
| [`docs/contributing.md`](docs/contributing.md) | Rules: clean-room, tests, layering, style, commits; the "add a command" checklist |
| [`docs/feature-map.md`](docs/feature-map.md) | Every enScore feature with its id, command id, crate, priority and estimate |
| [`docs/control-protocol.md`](docs/control-protocol.md) | JSON control channel: how agents drive and screenshot the running app |
| [`docs/ui-design.md`](docs/ui-design.md) | Design tokens, themes, widgets, reader ergonomics |
| [`ROADMAP.md`](ROADMAP.md) | One-page summary: stage, parity numbers, every dimension, what's next |
| [`docs/target-app-parity.md`](docs/target-app-parity.md) | The authoritative parity assessment against enScore: numbers, weights, evidence, hours |
| [`docs/gaps.md`](docs/gaps.md) | Every known shortfall, ranked: the work list to pick from |
| [`docs/roadmap.md`](docs/roadmap.md) | Milestones, alpha gate, **current focus** |
| `docs/*-parity.md` | Per-dimension checklists: [UI](docs/ui-parity.md), [annotation](docs/annotation-parity.md), [files](docs/file-format-parity.md), [hardware](docs/hardware-parity.md), [practice and audio](docs/practice-parity.md), [sync](docs/sync-parity.md), [localization](docs/localization-parity.md) |
| [`docs/parity-checklist.md`](docs/parity-checklist.md) | Generated: every feature-map row, live or missing |
| [`docs/scorecard.md`](docs/scorecard.md) | Generated: performance budgets, corpus floors, per-area checklists, settings that do nothing |
| [`docs/spec/`](docs/spec/README.md), [`docs/observations/`](docs/observations/README.md) | Behavioural specs and the observation notes they come from |
| `crates/<name>/README.md` (where present) | Public API of that crate |

## 2. Workspace map

```text
crates/
  geom raster codecs           L0 foundation (geometry, page bitmaps, image codecs)
  pdf                          L0 adapter over PDFCraft's PDF crates (the only door to pdfcraft-*; architecture §15)
  midi fsc                     L0 standalone format crates (no workspace deps; publishable)
  doc                          L1 library model (scores, page refs, annotations, setlists: pure data)
  ops ink layout text search   L2 history, stroke engine, page layout and turns, text/stamps, index
  algo audio practice          L2 scan imaging, audio playback DSP, metronome / tuner / practice log
  render gpu format            L3 CPU compositor (the oracle), wgpu compositor, on-disk library
  io sync metadata imslp scan  L4 import/export, sync and backup, title detection, IMSLP, scanning
  input devices                L4 pedal / MIDI / pen mapping; platform services (the one FFI crate)
  engine                       L5 Session + command registry (every action is a command)
  ui-egui automation           L6 egui shell (thin: all actions go through the engine); MCP server
  testkit                      test helpers
apps/
  scorecraft                   desktop app (eframe/wgpu), TCP control server
  scorecraft-cli               headless CLI (import/info/export/run/batch/commands/mcp)
  scorecraft-web               the same app in the browser (trunk + wasm-bindgen)
  scorecraft-android           Android shell
  scorecraft-ios               iOS / iPadOS shell
  scorecraft-sync-server       self-hostable sync and backup server
xtask/                         cargo xtask layers | wasm | ci | stats | corpus | test-corpus | parity | perf | scorecard
```

**Layering is enforced** by `cargo xtask layers`. A crate may depend only on lower layers. `pdf`,
`midi` and `fsc` depend on nothing in the workspace. Only `pdf` (plus `io` for `pdfcraft-print` and
`metadata`/`scan` for `pdfcraft-ocr`) may depend on PDFCraft's crates, all at one pinned revision;
PDF bugs are fixed upstream in [PDFCraft](https://github.com/storytold/pdfcraft), then re-pinned. Nothing below `ui-egui` may use egui, eframe,
winit or a file-dialog crate. A new crate must be registered in `xtask/src/layers.rs`.

## 3. Golden rules

### Never crash (outranks feature work)

Musicians trust ScoreCraft on stage, and a crash mid-performance is the worst thing it can do. A
malformed PDF, a bad command or MCP param, a corrupt library, an odd pedal, a stray MIDI message or
a full disk must produce an error the user or agent can act on, never a panic. The standard is
[`docs/standards/never-crash.md`](docs/standards/never-crash.md).

- **Non-test code never panics.** No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!` or
  `unimplemented!`. Return the crate's error type and propagate with `?`. Unfinished features
  return an "unsupported" error. The only exception is a provably infallible literal:
  `#[allow(clippy::expect_used)]` plus `.expect("why it can't fail")`.
- **No `unsafe`.** The workspace sets `unsafe_code = "forbid"`. The one exception is
  `scorecraft-devices` (platform MIDI, pen and power APIs), with `unsafe_code = "deny"`, feature
  gates and a `SAFETY:` comment on every block. Don't add `unsafe` anywhere else.
- **Input-derived numbers are hostile.** `get()` rather than `[i]` for indices from files, params
  or arithmetic on them; `checked_*`/`saturating_*` for lengths, offsets and counts; guard
  division by zero and NaN; cap allocations sized by input (PDF object counts, image dimensions,
  decompressed stream sizes).
- **Bound recursion** with depth limits or seen-sets (PDF object graphs are often cyclic).
- **Don't cascade.** Handle lock poisoning and treat thread joins as `Result`s.
- **Last-resort guard.** The app shell catches an escaped panic around command dispatch, import
  and export, reports it and keeps the reader on screen. Keep `panic = "unwind"` natively.
- **Prove it.** Every crash fix comes with a small synthetic regression test that panicked before.
- **Enforced by clippy.** `clippy.toml` allows `unwrap`/`expect`/`panic` in tests only; crate roots
  carry `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`
  and every crate's `Cargo.toml` has `[lints] workspace = true`.

### The rules

1. **Clean-room.** Follow [`docs/clean-room.md`](docs/clean-room.md) exactly. We study enScore's
   *behaviour* from public information and use as a user only. Never decompile, unpack, inspect
   or sniff enScore or forScore. Never copy their code, assets, sounds, icons or text. Formats
   without a public spec follow the two-team rule (§5 there). Third-party assets must be
   permissively licensed, keep their licence file next to them, and get a row in
   [`ATTRIBUTION.md`](ATTRIBUTION.md) in the same change.
2. **Everything is a command.** New user-visible behaviour = a command in the engine
   (`crates/engine/src/<area>_cmds.rs`, registered in `commands.rs`) with id, label, menu path,
   shortcut, params doc, `enabled` and `run`, plus tests. Tap zones, pedals, MIDI, the UI, CLI,
   control channel and MCP all dispatch commands by id. Use the **command id from
   [`docs/feature-map.md`](docs/feature-map.md)**. Only pure view/window state belongs to the shell
   (`ui.*`).
3. **The source file is sacred.** Never rewrite an imported PDF or image. Every edit is data in the
   library; export writes a new file. Annotation coordinates are in source-page PDF space.
4. **Tests are the gate.** Every change comes with tests. Format crates use round-trip,
   synthetic-generator, oracle and fuzz tests. Keep corpus floors and the parity floor from
   regressing.
5. **The UI is thin and data-driven.** UI state lives in `ui-egui/src/state.rs` (serde) so the
   control channel can read and drive it. Colours and sizes come from `theme::Tokens`. Every
   screen follows the [styling rules](docs/ui-design.md#styling-rules-every-screen): Tabler
   icons (outline, stroke 2), Segoe UI, the type scale and its icon-to-text sizes.
6. **Verify UI changes visually.** Render offscreen (`cargo run -p scorecraft-ui-egui --example snapshot`)
   or launch with `--control` and take `ui.screenshot`. Look at the PNG. Demo scores must be
   public domain (IMSLP / Mutopia files marked public domain), never personal files. When fetching
   assets, never put a person's name or email in requests; use a generic `ScoreCraft-dev` User-Agent.
7. **Never break wasm or mobile.** L0–L6 must `cargo check --target wasm32-unknown-unknown`
   (`cargo xtask wasm`). Filesystem, camera, MIDI and power code goes through platform services.
8. **Turns are sacred, performance is a feature.** A page turn must never wait on rendering.
   Pre-render neighbours, cache per revision, never re-render a page per frame, and record
   before/after timings in the dev log. Budgets: `perf/budgets.toml`.
9. **Never panic on input** (see *Never crash*). The `panic_hunt` integration test fuzzes every
   command with adversarial params and must stay green.
10. **Time goes through `chrono`**; sync clocks are hybrid logical clocks. Never hand-roll
    calendar maths.
11. **No required account, no paywall.** Every enScore Pro and Sync feature is free in ScoreCraft
    and works offline. Online features (IMSLP, catalogue lookups, sync) are opt-in.

## 4. Picking work

Priorities: important infrastructure first, then the alpha gate, then low-hanging parity, then the
long tail.

0. **Read [`ROADMAP.md`](ROADMAP.md), then [`docs/gaps.md`](docs/gaps.md).** When your work moves a
   number or closes a gap, update the parity doc and gap entry it belongs to, with the date, and
   bump their status line and revision history ([`docs/standards/progress-docs.md`](docs/standards/progress-docs.md)).
1. **Check [`docs/scorecard.md`](docs/scorecard.md)**: each area's `missing` and `partial` rows,
   over-budget performance scenarios, settings that do nothing.
2. [`docs/roadmap.md`](docs/roadmap.md) → **Current focus**.
3. `cargo xtask parity` → [`docs/parity-checklist.md`](docs/parity-checklist.md) lists every
   feature-map row that isn't live yet. A row counts only when its commands are registered (`ui.*` ones
   in `crates/ui-egui/shell-commands.toml`) and you mark it `partial` or `done` in the feature map
   with a test; a stub returning "unsupported" never counts. Prefer `P0` rows, then `P1`.
4. If the feature has no spec in `docs/spec/`, write the spec first (from observation notes and
   public sources), in its own PR or the same one.
5. `log/devlog.md` → the "Still open" bullets of recent entries.

When parity rises, raise `FLOOR` in `crates/engine/src/parity.rs` (never lower it).

## 5. Before you finish a task

```sh
cargo test -p <crates you touched>
cargo clippy -p <crates> --all-targets -- -D warnings
cargo fmt --check
cargo xtask layers
cargo xtask wasm            # if you touched L0–L6
cargo xtask parity          # if you added commands; commit the regenerated docs/parity-checklist.md
cargo test -p scorecraft-engine --test panic_hunt -- --ignored   # if you added/changed commands
cargo xtask scorecard       # if you moved a number; commit the regenerated docs/scorecard.md
cargo xtask perf --quick    # if you touched rendering, layout, ink or the library store
cargo xtask test-corpus     # if you touched pdf, fsc, io, render, gpu or format
```

Then append a terse entry to `log/devlog.md` (what landed, numbers, what's still open). Sessions can
end abruptly, so the dev log plus a green tree is how the next agent picks up. Keep the tree
building at every step.

**Until the workspace exists** (milestone M0), documentation-only changes need only: links that
resolve, the progress-docs header and revision history updated, and the feature-map summary
counts matching the rows.

## 6. Parallel agents

- Use your own target dir (`CARGO_TARGET_DIR=target/agent-<name>`) and edit only the files you own.
  Shared files (`engine/src/lib.rs`, the `v.extend(...)` list in `engine/src/commands.rs`,
  `ui-egui/src/menus.rs`, `state.rs`, `docs/feature-map.md`) get small, surgical edits; re-read
  before editing.
- Put new commands in a **new module** (`engine/src/<area>_cmds.rs` with `specs()`).
- If someone else's in-progress edit breaks the build, wait and retry; don't fix their files.
- Keep every `Cargo.toml` valid at all times. Create or rewrite manifests atomically (write to a
  temp file outside `crates/`, then `mv`).
- For black-box format work, the analyst and the implementer must be **different sessions** that
  share only the written spec ([clean-room §5](docs/clean-room.md#5-interchange-formats-without-a-public-spec)).

## 7. Where things are tracked

- [`docs/roadmap.md`](docs/roadmap.md): milestones M0–M10, alpha gate, current focus.
- [`ROADMAP.md`](ROADMAP.md), [`docs/target-app-parity.md`](docs/target-app-parity.md),
  [`docs/gaps.md`](docs/gaps.md) and the `docs/*-parity.md` checklists: where we stand against enScore.
- [`docs/feature-map.md`](docs/feature-map.md): the feature catalog (the parity denominator).
- [`docs/parity-checklist.md`](docs/parity-checklist.md), [`docs/scorecard.md`](docs/scorecard.md): generated.
- [`docs/decisions.md`](docs/decisions.md): implementation decisions with dates.
- [`rfcs/`](rfcs/0000-decision-process.md): changes to this process and project-wide decisions.
- [`docs/releasing.md`](docs/releasing.md): cutting a release, signing, packaging.
- `plan/` (local, gitignored): research, plans, estimates. `log/` (local, gitignored): the dev log.

## 8. Keeping the library format complete

`scorecraft-format` deliberately fails to compile when a `scorecraft-doc` struct gains a field, so
nothing is silently dropped from saves or sync. When you add a doc field, map it in
`crates/format/src/` with `#[serde(default)]` so older libraries still load, add it to the sync op
set in `crates/sync`, and document it in [`docs/file-format-parity.md`](docs/file-format-parity.md#scorecraft-library-format).

## Contributor credits (About window)

- About ▸ Contributors is compiled from `contributors/contributors.json` (generated, never
  hand-edit) and [`contributors/people.toml`](contributors/people.toml) (names people chose for
  themselves). See [`docs/contributors.md`](docs/contributors.md).
- **Agents working for a contributor:** when you prepare a PR, check whether your human's GitHub
  username has a `[people.<username>]` entry. If not, ask them once whether they want to be
  credited by more than their username. If yes, add **only their own** entry, committed as them.
- Never add, edit, guess or copy anyone else's entry or name (not from git config, commit authors
  or GitHub profiles).
