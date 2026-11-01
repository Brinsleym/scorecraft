# Library

> **Last reviewed:** 2026-10-31 · **Last updated:** 2026-10-31 · **Change:** patch (M5 part 2: covers and the window; earlier: first version) · **Target:** enScore 3.7 (2026)

Finding scores in the library: search, sort, the recently opened list, importing a whole folder,
and the library window with covers. Written for milestone M5. Importing files, folders as places
to file scores, metadata, the trash and Verify & Repair are M1's
([feature map](../feature-map.md#lib-library-and-organisation), rows LIB-1, -3, -4, -10, -12).

## Sources

- Public listings S1, S3, S4 and S8:
  [../observations/public-listings-2026-10-28.md](../observations/public-listings-2026-10-28.md)
  ("Library"). They name search across title, composer, tags and key, sorting by title, composer
  and date, a recently opened list, importing a whole folder, and a library that stays fast with
  thousands of scores. They don't say how search matches or ranks; this spec picks behaviour
  that suits a musician and lists the choices under [Open questions](#open-questions).

## Terms

- **Library scores**: every score in the library that isn't in the trash.
- **Search fields**: title, composer, arranger, key, genre, tags, the file name, and the name of
  the folder the score is filed in (and the folders above it).

## Requirements

### LIB-5 Search

- A query is split into words at spaces and punctuation. A score matches when **every** word of
  the query is the start of some word in its search fields ("moz son" finds *Sonata in A* by
  *W. A. Mozart*). Words are compared without case and without accents (*Dvořák* is found by
  "dvorak", *Fauré* by "faure"); `ß` matches "ss".
- Results are ranked: a word matching a whole word counts more than one matching its start; a
  match in the title more than in the composer, and the composer more than the other fields.
  Ties keep the current sort order. Up to 500 results are returned; the count says how many
  matched.
- Search can be limited to one folder (with the folders inside it) and can include the trash.
- An empty query matches every score, in the current sort order.
- A query is at most 256 characters and 16 words; longer queries are refused, never truncated
  silently.

### LIB-9 Sort

- The library lists scores by **title** (A to Z, the default), **composer** (A to Z, then title),
  **date added** (newest first) or **recently opened** (most recent first; never-opened scores
  last, by title). Each can be reversed.
- Comparing names ignores case and accents, compares runs of digits as numbers (*Etude 2*
  before *Etude 10*), and ignores a leading "The", "A" or "An" (English titles only). A score
  with no title sorts by its file name.
- The chosen sort is kept between launches; search results use it to break ties.

### LIB-15 Recently opened

- The scores opened most recently, newest first, up to 20 by default (at most 100); trashed
  scores are left out. Opening a score puts it at the top.

### LIB-2 Import a folder

- Importing a folder imports every PDF in it (by extension, any case) and, if asked, in the
  folders inside it, at most 10 levels down and 2,000 files per import. Files already in the
  library (the same bytes) are skipped and counted, not imported twice. Other files are ignored.
- With *keep folders*, each folder becomes a library folder of the same name under the target
  folder, and its scores are filed there; otherwise every score goes into the target folder.
- The whole import is one undo step. A file that can't be read is listed with the reason and the
  rest still import.

### LIB-14 The library window

- A grid of covers (each score's first shown page, with its title and composer under it) or a
  list (title, composer, pages, date added), switched from the toolbar. The search box and the
  sort sit above it; folders show first, and opening one shows its scores with a way back up.
- Covers are rendered in the background and cached; the grid scrolls while they arrive, showing
  a placeholder until each is ready. Opening a score opens it in the reader.

### LIB-13 Fast with thousands of scores

- With 10,000 scores, opening the library and listing its first screen takes at most 1.5 s on
  the reference desktop (perf scenario P6), and a metadata search answers in at most 50 ms (P7).
  `crates/engine/examples/library_perf.rs` measures both (`cargo xtask perf`); on the
  development container (not the reference device) 10,000 synthetic scores open in 172 ms and
  search in 22 ms (median).

## Acceptance tests

| Id | Given | When | Then |
|---|---|---|---|
| LIBS-1 | *Sonata in A* by W. A. Mozart and *Étude* by Chopin | "moz son", then "etude", then "chopin xyz" are searched | the sonata; the étude; nothing |
| LIBS-2 | "Rondo" in one score's title and another's composer field | "rondo" is searched | the title match ranks first |
| LIBS-3 | A query of 300 characters, or of 17 words | it is searched | it is refused |
| LIBS-4 | *Etude 10*, *Etude 2*, *The Swan*, *apple* | sorted by title | *apple*, *Etude 2*, *Etude 10*, *The Swan* (as "Swan") |
| LIBS-5 | Three scores opened in turn, one never | sorted by recently opened | the last opened first, the never-opened last |
| LIBS-6 | Two scores opened, one then trashed | the recent list is asked for | only the other |
| LIBS-7 | A folder with two PDFs, a text file and a subfolder with one PDF, one already imported | it is imported with subfolders and *keep folders* | two new scores, one skipped, the text file ignored; the subfolder's score is in a library folder of its name; one undo removes them all |
| LIBS-8 | A score filed in folder *Bach* | "bach" is searched | it is found |
| LIBS-9 | A portrait score | its cover is asked for at 200 px, then after its first page is turned | 200 px tall, narrower than that; then turned, width and height swapped |
| LIBS-10 | The app with a library score | the library window is shown | the score's cover arrives without blocking a frame; list view works; opening a score closes the window |

## Not in scope

- Full-text search inside PDFs and practice notes (LIB-6, P1): after the text index exists.
- Title and composer detection (LIB-7, -8): with `pdfcraft-ocr`, later.

## Open questions

- Whether enScore's search matches inside words ("nata" finding *Sonata*) or only at their
  starts; ScoreCraft matches word starts.
- Which fields enScore searches, and how it ranks; ranking here is ScoreCraft's own.
- Whether enScore ignores leading articles when sorting.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | patch | M5 part 3: LIB-13 names perf scenarios P6 and P7 (10,000 scores), first measurement |
| 2026-10-31 | patch | M5 part 2: LIBS-9, -10 for covers and the library window |
| 2026-10-31 | major | First version for M5: search, sort, recently opened, folder import, the library window, speed |
