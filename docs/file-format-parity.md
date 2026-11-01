# File-format parity

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-10-31 · **Change:** patch (hairpins and text stamps; earlier: strokes record their nib; earlier: library format v1 written down as built in M1; earlier: corpus results: PDF-1 done, PDF-2 and PDF-3 partial; earlier: PDF reading is partial; earlier: PDF rows reuse PDFCraft's crates; earlier: major, first version) · **Target:** enScore 3.7 (2026)

Which files ScoreCraft reads and writes, against what enScore's public listings say it handles.
Status: **done** (with round-trip or corpus tests), **partial**, **missing**. Directions: R read,
W write.

**Summary (estimated):** 0% ready.

## enScore's formats

| Format | enScore | ScoreCraft target | Crate | Spec | Rows | Status |
|---|---|---|---|---|---|---|
| PDF | R, W (export with ink, print, reports) | R, W | `pdf` ♻ pdfcraft-cos, -render, -create, -organize | ISO 32000-1 / 32000-2 | READ-1, EXP-2, PRAC-9 | partial (2026-10-31: open, render, text via pdfcraft-render 0.15.0; writing not started) |
| PDF annotations (Ink, Highlight, FreeText, Square/Circle) | export only (inferred) | R, W | `pdf` ♻ pdfcraft-annot | IMP-7, EXP-2 | missing |
| Images (JPEG, PNG, HEIC, TIFF) as pages | R (Photos, camera) | R | `codecs`; ♻ pdfcraft-create (PNG, JPEG, TIFF) | format specs | IMP-3 | missing |
| forScore `.4sc` | R | R only ([why](gaps.md#not-clean-room-feasible-so-far)) | `fsc` | black-box, [two-team](clean-room.md#5-interchange-formats-without-a-public-spec) | IMP-5 | missing |
| enScore setlist / library share file | R, W (container not described publicly) | R, W our own bundle; enScore's own file read once observed | `io` | ours below | LIST-5 | missing |
| Audio recordings | R (attach) | R: WAV, FLAC, MP3, AAC/M4A, Ogg/Opus | `audio` | format specs | PLAY-1 | missing |

## Formats enScore doesn't have (ours only)

| Format | Direction | Why | Rows |
|---|---|---|---|
| ScoreCraft library (directory) | R, W | open, documented, sync-friendly | all |
| ScoreCraft setlist bundle `.scoreset` | R, W | share a setlist with its PDFs and markings | LIST-5 |
| MusicXML / MIDI rendered to pages | R | P3, needs an engraver | IMP-8 |
| Plain CSV/JSON metadata export | W | spreadsheets, migrations | ours |

## ScoreCraft library format

A directory, `<Name>.scorecraft/`, readable without ScoreCraft: plain JSON records plus the
imported files. Version 1, as built in M1 (`crates/format`, 2026-10-31):

```text
<Name>.scorecraft/
├─ manifest.json              {"format":"scorecraft-library","version":1,"library":"<uuid>","createdMs":…}
├─ library.json               folders, Recently Deleted (score → deleted-at ms), settings
├─ scores/<uuid>.json         a score without its annotations: source, metadata, folder, pages,
│                             pageRange, layers, bookmarks, audio, midiOnOpen, practiceNotes, times
├─ ink/<uuid>/<page>.json     {"page":n,"annotations":[…]} for one source page (0-based)
├─ lists/<uuid>.json          a setlist: name and entries (score or placeholder)
├─ blobs/sha256/<2 hex>/<62>  imported files, byte-for-byte, never modified
├─ journal/pending.json       only while a save that changes several records is in flight
└─ damaged/                   records Verify & Repair could not read, kept for recovery
```

- **Ids** are UUID v7 strings; times are Unix milliseconds; colours are `#rrggbbaa`; rotations
  are degrees; blob ids are 64 hex digits; JSON keys are camelCase.
- **Annotations** are in PDF user space of the source page, so page edits never move them.
  Annotation objects carry `"type"`: `stroke`, `text`, `stamp`, `hairpin`, `snip` or
  `shortcut`. A dynamic stamp has `text` (`pp` … `ff`) and `size`; a SMuFL stamp has `glyph`.
  A hairpin has `from`, `to` (points), `crescendo`, `opening`, `width` and `color`. A
  stroke has `tool` (`pen`, `highlighter`, `whiteout`), `color`, `width`, `points` and, for a
  chisel highlighter, `"nib": "chisel"` (left out for the round nib, so older files read as
  round).
- **Writes** are atomic per file (temp file, flush, rename). A save that changes more than one
  file writes every new record to `journal/pending.json` first; on open, a journal is replayed
  and removed, so the library is always as before or after a save. Only files whose bytes
  changed are written.
- **Reading** skips a record it can't parse and reports it; the rest of the library opens.
  `library.verify` checks references and blob hashes; with `repair` it moves damaged records to
  `damaged/`, fixes dangling references, and removes unused blobs.
- **Compatibility:** optional fields default when missing, unknown fields are ignored. A newer
  `version` is refused with an error, never guessed at. Breaking changes bump `version` and ship
  a migration plus a reader for the previous version. The record mapping destructures every
  `doc` field, so adding one fails to compile until it is mapped here.
- The per-page ink files are whole snapshots for now; the append-only op log the sync design
  needs (architecture §9) replaces them in M8.

## ScoreCraft setlist bundle

`.scoreset` is a zip (stored, not deflated, for PDFs) containing `setlist.json` (order, page
ranges, tempos), `scores/<uuid>.json`, the referenced `blobs/` and `ink/`. Import de-duplicates
blobs by hash and asks before overwriting markings.

## Scanning

| Source | Output | Crate | Rows | Status |
|---|---|---|---|---|
| Camera | multi-page PDF with straightened, cropped, cleaned pages; originals kept | `scan`, `algo` | IMP-1 | missing |
| eSCL network scanner | PDF from flatbed or feeder | `scan` | IMP-2 | missing |

## PDF fidelity targets

The PDF engine is PDFCraft's ([architecture §15](architecture.md#15-reusing-pdfcraft)). These
targets are checked on our score corpus through the `scorecraft-pdf` adapter; failures are
reported and fixed upstream in PDFCraft, then pinned.

| Id | Target | Status |
|---|---|---|
| PDF-1 | ≥ 99% of the pinned public-domain score corpus opens; none crash | done |
| PDF-2 | ≥ 95% render within tolerance of a reference rasteriser at 150 and 300 dpi | partial |
| PDF-3 | Scanned scores: JBIG2, CCITT G4, DCT, JPX images decode | partial |
| PDF-4 | Damaged files: broken xref and truncated streams are repaired or the bad page shows a placeholder | missing |
| PDF-5 | Exported PDFs pass a validator (veraPDF or qpdf --check) and open in three other readers | missing |
| PDF-6 | Encrypted PDFs with an empty user password open; others ask for the password | missing |

### Corpus results (2026-10-31, PDFCraft `65ff3a00`)

Run with `cargo xtask test-corpus` on the owner's Windows tablet, against the 20 pinned
public-domain scores and the owner's private library (about 300 distinct scores, reported by
count only; see [development](development.md#test-corpora)).

- **PDF-1:** 20/20 public-domain scores and every readable library score open or ask for a
  password; none crash. Two library files that PDFium can't open either are not counted.
- **PDF-2:** 15/20 public-domain scores match (all 12 engraved ones); about two thirds of the
  library. Every remaining failure is in PDFCraft's renderer (hayro), to fix upstream and re-pin:
  1. A full-page JBIG2 halftone image is drawn blank, with no error (PDF-3). Likely covered by
     storytold/pdfcraft#2134.
  2. An image on a page with /Rotate 90 and an offset crop box lands a few pixels off and
     slightly mis-scaled. Not yet tracked upstream.
  3. 1-bit images downscaled about 3× come out 5–8% lighter than PDFium's: thin lines are
     point-sampled away. Tracked: storytold/pdfcraft#2393, #3858.
  4. Strokes thinner than a device pixel are drawn at their exact coverage (up to 32% lighter
     staff lines); PDFium widens them to one pixel. Not yet tracked upstream.
  5. Unsmoothed images pick different source pixels from PDFium (1-px flips along edges). The
     ink and layout match; only the 4 × 4 tolerance sees it. Tracked with item 3.
  6. A damaged file yields one page where PDFium finds six (Acrobat recovers it, other readers
     fail to open it). Damaged-file recovery, PDF-4, rather than a rendering bug.
- Images are not smoothed when they are scaled, because hayro honours each image's /Interpolate
  flag. Scans therefore look harder-edged than in PDFium-based readers. An "always smooth
  images" render option in PDFCraft would fix that and items 3 and 5.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | patch | Library format: `hairpin` annotations; stamps carry `text` and `size` (optional, backward compatible) |
| 2026-10-31 | patch | Library format: strokes record their nib (`chisel` for the highlighter's default), optional and backward compatible |
| 2026-10-31 | patch | Library format v1 as built (M1): records, ink per page, journal, verify and repair |
| 2026-10-31 | patch | Corpus results: PDF-1 done; PDF-2 and PDF-3 partial, with the renderer gaps listed |
| 2026-10-31 | patch | PDF reading is partial: the adapter opens, renders and extracts text |
| 2026-10-28 | minor | PDF rows reuse PDFCraft's crates |
| 2026-10-28 | major | First version |
