<h1 align="center">ScoreCraft</h1>

<p align="center">
  <b>Sheet music reading; an open-source, clean-room reimplementation of enScore, rebuilt in pure Rust.</b><br>
  Read, mark up, organise and perform from your PDF scores, with half-page turns, pedals, setlists<br>
  and sync, in a native app written entirely in Rust. Open source, offline, no account, and yours.
</p>

<p align="center">
  <img alt="100% Rust" src="https://img.shields.io/badge/100%25-Rust-b7410e?style=flat-square&logo=rust">
  <img alt="macOS · Windows · Linux · Android · iOS · Web" src="https://img.shields.io/badge/macOS%20%C2%B7%20Windows%20%C2%B7%20Linux%20%C2%B7%20Android%20%C2%B7%20iOS%20%C2%B7%20Web-native-2f7bf5?style=flat-square">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-3a3a3a?style=flat-square">
  <img alt="Status: pre-alpha" src="https://img.shields.io/badge/status-pre--alpha-d69e2e?style=flat-square">
</p>

<p align="center">
  <a href="#features">Features</a> ·
  <a href="#everything-in-the-box">Everything in the box</a> ·
  <a href="#clean-room">Clean-room</a> ·
  <a href="#built-for-agents">Agents</a> ·
  <a href="#under-the-hood">Under the hood</a> ·
  <a href="#get-started">Get started</a> ·
  <a href="#documentation">Documentation</a>
</p>

> [!IMPORTANT]
> **Status: pre-alpha, documentation only.** This repository holds ScoreCraft's process, clean-room
> rules, architecture and feature map. **There is no code yet.** Everything under
> [Features](#features) is the plan, not something you can download. Where we stand, honestly and
> dimension by dimension, is in the [roadmap](ROADMAP.md).

<br>

<table>
  <tr>
    <td width="25%" valign="top">
      <h3>🎼 Made for the stand</h3>
      Half-page turns that show the next system early, alternating turns for awkward breaks, and a page that fills the screen. Turn by tap, swipe, keyboard, Bluetooth pedal or MIDI.
    </td>
    <td width="25%" valign="top">
      <h3>⚡ Native and instant</h3>
      A pure-Rust PDF engine and a wgpu compositor that pre-render your next pages, so a turn never waits. No Electron, no web view.
    </td>
    <td width="25%" valign="top">
      <h3>🔓 Free, no account</h3>
      Setlists, annotation layers, shortcuts, snips, IMSLP and sync are all included. Everything works offline. Your PDFs are never modified.
    </td>
    <td width="25%" valign="top">
      <h3>🤖 Agent-ready</h3>
      Every action is a command, so you can drive the same engine from the UI, the CLI, a JSON control channel or an MCP server.
    </td>
  </tr>
</table>

## Features

The features ScoreCraft is being built to match, from enScore's public descriptions. Each one has a
row, a command id and an estimate in the [feature map](docs/feature-map.md).

**Read anything.** Open any PDF, scanned or engraved, crisp at every zoom. Single-page, two-page and
full-screen layouts; dark, sepia and inverted filters for dim stages; an Enhance filter for tired
photocopies; two scores side by side; the screen stays awake for the whole concert.

**Turn without thinking.** Half-page turns, alternating turns, tap zones, swipes, arrow keys,
Bluetooth foot pedals and MIDI controllers. Record the turns against a recording once, and the score
turns itself next time.

**Mark it up.** Pen and finger ink with pressure, highlighters, whiteout to cover a printed bowing
and write your own, typed notes, fingering and dynamics stamps, stroke and pixel erasers, and
annotation layers you can hide for the performance. Markings never touch your PDF.

**Keep a real library.** Import files, folders and photos. Folders, composer, key and tags, all
searchable, plus full-text search inside your PDFs and practice notes. Titles and composers can be
detected from generic scan names. Bookmarks, a Recently Deleted folder, and a library check that
repairs what a crash or a sync hiccup broke.

**Edit pages, not files.** Reorder, rotate, hide and delete pages; crop and straighten scans; limit a
score to a page range; Score Snips to paste the bars after a turn before it; blank manuscript paper
with single staves, grand staves or tab.

**Perform a set.** Setlists with a page range and a tempo per entry that flow from piece to piece.
Score Shortcuts jump from a D.S. or a repeat to its target with one tap. Share a whole setlist,
PDFs and markings included, as one file.

**Practise.** A metronome with tap tempo, subdivisions and nine sounds; a tuner; recordings attached
to scores, slowed down, looped or transposed; a practice log with streaks, goals and a PDF report.
MIDI program changes when a score opens, with different sounds per setlist.

**Bring your music.** Camera scanning that straightens and cleans pages, Wi-Fi scanners, IMSLP search
and import, and migration from forScore's `.4sc` files with pages, ink, stamps and layers.

**Sync your way.** Sync scores, markings, setlists and bookmarks through a folder you already sync
(iCloud Drive, OneDrive, Dropbox, Syncthing), WebDAV, or a sync server you host, or mirror one
folder two ways with your computer. No subscription.

## Everything in the box

<table>
  <tr>
    <td width="33%" valign="top">
      <h4>📖 Reader</h4>
      Single · two-page · full screen · half-page turns · alternating turns · zoom and pan · landscape page zoom · light, dark, sepia, inverted and Enhance filters · side by side · multiple windows · keep awake · e-ink mode
    </td>
    <td width="33%" valign="top">
      <h4>✍️ Markup</h4>
      Pen · pressure · highlighter · whiteout · stroke eraser · pixel eraser · text notes · SMuFL stamps · lasso · shapes · colours and presets · layers · undo and redo · palm rejection
    </td>
    <td width="33%" valign="top">
      <h4>🗂️ Library</h4>
      Files, folders and photos · folders · title, composer, key, tags · search and full-text search · title detection · sort · bookmarks · Recently Deleted · verify and repair · page editing · page ranges · manuscript paper
    </td>
  </tr>
  <tr>
    <td width="33%" valign="top">
      <h4>🎤 Performance</h4>
      Setlists · page range and tempo per entry · continuous flow · shared setlist bundles · Score Shortcuts · Score Snips · Bluetooth pedals · MIDI pedals · recorded turns
    </td>
    <td width="33%" valign="top">
      <h4>🎧 Practice</h4>
      Metronome (tap tempo, subdivisions, nine sounds) · tuner · audio attach, time-stretch, pitch-shift, loops · practice log, streaks, goals · PDF reports · MIDI program, bank and song select, per setlist
    </td>
    <td width="33%" valign="top">
      <h4>🔄 Files and sync</h4>
      PDF in and out with ink · PDF annotations · images · forScore <code>.4sc</code> import · camera and Wi-Fi scanning · IMSLP · print · folder, WebDAV and self-hosted sync · backup and restore
    </td>
  </tr>
</table>

## Clean-room

ScoreCraft is a **clean-room** reimplementation. It rebuilds what enScore does for a musician from
public information (store listings, release notes, the developer's site, reviews) and from
observation as a user, written down as [observation notes](docs/observations/README.md) and turned
into [behavioural specs](docs/spec/README.md) before any code is written. We never decompile,
unpack, inspect or sniff enScore or any other score reader, and never copy their code, assets,
sounds or text. Formats without a public specification, like forScore's `.4sc`, are learned
black-box from sample files by one person and implemented from the written description by
another. The full rules are in [`docs/clean-room.md`](docs/clean-room.md).

ScoreCraft is an independent project. It is not affiliated with, endorsed by or sponsored by
enScore, forScore, IMSLP or any hardware maker. "enScore" and "forScore" are their owners'
trademarks and are used here only to describe what ScoreCraft measures itself against and
interoperates with.

## Built for agents

Every library action, page turn, stroke and setting runs a command from one registry. The UI, the
CLI, the JSON control channel and the MCP server all call the same commands, so anything you can
tap, a script or an AI agent can do too.

```sh
# Headless: import a folder of scans and detect titles
scorecraft-cli import ./scans --library ~/Scores.scorecraft --detect-metadata

# Build a setlist and export it as one PDF with the Performance layer
scorecraft-cli run ~/Scores.scorecraft \
  --cmd list.new       --params '{"name":"Spring recital"}' \
  --cmd list.entry.add --params '{"list":"Spring recital","score":"Bach BWV 1007","tempo":72}' \
  --cmd list.export.pdf --params '{"list":"Spring recital","layers":["Performance"],"out":"recital.pdf"}'

# Let an agent drive it over MCP (headless, or bridged to the running app)
scorecraft-cli mcp
```

The desktop app also offers an authenticated, loopback-only control channel
(`scorecraft --control`) to inspect UI state, send pointer, pedal and MIDI events, and take
offscreen screenshots. See [`docs/control-protocol.md`](docs/control-protocol.md).

## Under the hood

- **Engine first:** a pure-data library model and a command engine, with a thin egui UI on top.
  Layering is enforced at build time across about 30 crates ([architecture](docs/architecture.md)).
- **PDF engine from PDFCraft:** parsing, rendering, annotations, page assembly, printing and OCR
  come from [PDFCraft](https://github.com/storytold/pdfcraft)'s hardened, MIT/Apache-licensed
  crates, so ScoreCraft's own code goes into what a score reader adds.
- **The PDF is sacred:** imported files are stored content-addressed and never rewritten; every
  edit and marking is data, in source-page coordinates, so it survives rotation, crops and reorders.
- **Two compositors:** a CPU compositor is the reference oracle; a wgpu compositor puts pages and
  live ink on the GPU. They are tested against each other.
- **Offline-first sync:** markings merge without conflicts (CRDTs), and backends are storage you
  already have.
- **Runs everywhere:** the same Rust on macOS, Windows, Linux, Android, iOS and, through
  WebAssembly, the browser.
- **Never crash:** no panics and no `unsafe` in production code, fuzzed parsers, and a
  last-resort guard that keeps your page on screen
  ([standard](docs/standards/never-crash.md)).

## Get started

There is nothing to build yet. The first milestone is the workspace skeleton
([M0](docs/roadmap.md#milestone-definitions)). Once it lands:

```sh
git clone https://github.com/Brinsleym/enscore-cleanroom
cd enscore-cleanroom
cargo run --release -p scorecraft -- score.pdf   # the desktop app
cargo test --workspace                           # the test suite
```

**New contributors and AI agents:** start with [`AGENTS.md`](AGENTS.md), then
[`docs/clean-room.md`](docs/clean-room.md), then pick work from [`docs/gaps.md`](docs/gaps.md).

## Documentation

| | |
|---|---|
| [`AGENTS.md`](AGENTS.md) | Guide for agents and contributors: rules, workspace map, checks before you finish |
| [`ROADMAP.md`](ROADMAP.md) | One-page status: stage, parity numbers, what's next |
| [`docs/clean-room.md`](docs/clean-room.md) | The clean-room methodology |
| [`docs/feature-map.md`](docs/feature-map.md) | All 124 enScore features with ids, commands, crates, priorities, estimates |
| [`docs/architecture.md`](docs/architecture.md) | Crates, layers, document model, rendering, sync |
| [`docs/target-app-parity.md`](docs/target-app-parity.md), [`docs/gaps.md`](docs/gaps.md), [`docs/roadmap.md`](docs/roadmap.md) | Parity assessment, ranked gaps, milestones and alpha gate |
| [`docs/contributing.md`](docs/contributing.md), [`docs/development.md`](docs/development.md) | How to contribute, build, test and drive the app |
| [`book/`](book/src/SUMMARY.md) | The ScoreCraft documentation book (architecture, formats, automation, security) |
| [`rfcs/`](rfcs/0000-decision-process.md) | How project-wide decisions are made |

## Security

Security architecture, the threat model, parser hardening and vulnerability reporting are covered in
the [security documentation](book/src/security/overview.md) and the [security policy](SECURITY.md).

## The Crafting Apps

ScoreCraft follows the framework and methodology of the **Crafting Apps** from the
[ArtCraft](https://getartcraft.com/) team, such as [PhotoCraft](https://github.com/storytold/photocraft),
a clean-room reimplementation of Adobe Photoshop: clean-room and pure Rust, native on every
platform, in the browser via WebAssembly, and fully drivable by agents. ScoreCraft is an
independent, community project and is not published by the ArtCraft team.

## License

ScoreCraft is dual-licensed under the [MIT License](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option. See [`NOTICE`](NOTICE) and
[`ATTRIBUTION.md`](ATTRIBUTION.md) for third-party material.
