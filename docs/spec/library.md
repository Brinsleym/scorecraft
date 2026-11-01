# Library

> **Last reviewed:** 2026-10-31 · **Last updated:** 2026-11-01 · **Change:** minor (the owner's first hands-on feedback: sidebar, floating toolbar, the score menu, Settings in the library; earlier: Recently Deleted with folders; earlier: the owner's answers on search and sort; earlier: M5 part 2: covers and the window; earlier: first version) · **Target:** enScore 3.7 (2026)

Finding scores in the library: search, sort, the recently opened list, importing a whole folder,
the library window with covers, and Recently Deleted. Written for milestone M5; Recently Deleted
(LIB-10) added on 2026-11-01. Importing files, folders as places to file scores, metadata and
Verify & Repair are M1's ([feature map](../feature-map.md#lib-library-and-organisation), rows
LIB-1, -3, -4, -12).

## Sources

- Public listings S1, S3, S4 and S8:
  [../observations/public-listings-2026-10-28.md](../observations/public-listings-2026-10-28.md)
  ("Library"). They name search across title, composer, tags and key, sorting by title, composer
  and date, a recently opened list, importing a whole folder, and a library that stays fast with
  thousands of scores. They don't say how search matches or ranks; this spec picks behaviour
  that suits a musician and lists the choices under [Open questions](#open-questions).

## Terms

- **Library scores**: every score in the library that isn't in the trash.
- **Search fields**: every field of Edit Details (title, composer, tags, key, arranger, lyricist,
  instruments and voices, genre, difficulty as "N stars", year composed, catalogue number,
  publisher or edition, description), the file name, and the name of the folder the score is
  filed in (and the folders above it).

## Requirements

### LIB-5 Search

- A query is split into words at spaces and punctuation. A score matches when **every** word of
  the query is inside some word of its search fields ("moz son" finds *Sonata in A* by
  *W. A. Mozart*, and so does "nata"). Words are compared without case and without accents (*Dvořák* is found by
  "dvorak", *Fauré* by "faure"); `ß` matches "ss".
- Results are ranked: a word matching a whole word counts more than one matching its start, and
  that more than one inside a word; a match in the title more than in the composer, and the composer more than the other fields.
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
  before *Etude 10*), and keeps leading articles: *The Swan* sorts under T (owner, 2026-11-01).
  A score with no title sorts by its file name.
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

### LIB-6 Search inside scores

- *Inside scores* searches the words printed in every score's PDF and its notes (practice notes
  and the notes field), with the same matching as LIB-5. A result names the page of the file
  where the first query word appears and shows the words around it.
- Each file's text is read once and kept in the library's cache (`cache/text/`, never part of
  the model, safe to lose). A search reads files it hasn't seen yet within a time budget (1.5 s
  for agents, 0.1 s per frame in the app, which asks again while files are waiting) and says
  how many are still waiting. Text is kept up to 100,000 characters a page and 2 million a score.
- Scanned pages have no text until OCR (LIB-7) comes.

### LIB-14 The library window

The owner's direction (2026-11-01), with enScore's sidebar and toolbar as the arrangement to
follow (observed in the owner's screenshots 2 and 3; nothing copied):

- **No full-width toolbar.** The library has no score-view toolbar above it. A compact,
  **resizable left sidebar** lists, one per row with its icon: **Library**, **Lists**,
  **Recordings**, **Practice**, **Search**, **Recently Deleted**, then a gap, then **Settings** at
  the bottom. While a score is open, a row at the top goes back to it. Recordings and Practice are
  placeholders until they are specified; Search finds scores in the library (by their details or
  inside them, LIB-5, LIB-6), and searching IMSLP for new scores comes later.
- **Library** shows the folder's name (or *Library*) with a way back up, and a **floating toolbar
  anchored on the right**: **More** (Select; covers or a list; sync), **Filter** (only the scores
  matching what is typed, optionally inside them), **Sort** (*By Title*, Composer, Date added,
  Recently opened, and reverse order), **New Folder** (named in place, made in the folder shown),
  and **Import** (PDF files, a folder of scores, photos as one score, and new blank manuscript,
  piano or tablature paper).
- Below it, a grid of folders (each with a folder icon and its name) and covers (each score's
  first shown page, with its title and composer under it), or a list (title, composer, pages,
  date added). Tapping a folder opens it; tapping a score opens it in the reader.
- **Right-clicking a score** (or a long press) opens its menu, in this order: **Edit Details**
  (the score's details, pages and bookmarks), **Add to list** (any list, or a new one), **Move to**
  (a folder, or the library's top level), **Select** (starts selecting, with this score chosen),
  **Share** (the score as a PDF with its ink drawn in, or as PDF annotations; EXP-2), **Print**
  (shown but not built yet, EXP-1) and **Delete** (to Recently Deleted). Right-clicking a folder
  offers *Delete folder*.
- **Selecting:** taps choose scores instead of opening them; a bar shows how many, with *Add to
  list*, *Move to*, *Delete* and *Done*.
- **Settings** is a page of the library, never of the score view: appearance (light or dark),
  keeping the screen awake while a score is open (on by default), page turning (swipe to turn,
  the default layout and turns for every score), markup (the ruler button, round highlighter
  ends, shapes, what the pen's button switches to) and sync.
- The score view's toolbar keeps only what is used while playing; it has no Score, New, Export,
  Turning, theme or settings controls. Its page layout drop-down has *Use my defaults* to forget
  the score's own layout and turns.
- Covers are rendered in the background and cached; the grid scrolls while they arrive, showing
  a placeholder until each is ready.

### LIB-10 Recently Deleted (scores and folders)

- Deleting a score or a folder sends it to Recently Deleted, never straight to oblivion. A
  deleted folder takes the folders inside it and every score in them, all marked with the same
  time and keeping their places; the library window, search and the folder list leave them out.
- Everything in Recently Deleted is **read-only** until restored: editing a deleted score
  (details, pages, ink, its folder), renaming or moving a deleted folder, and filing anything into
  one are refused with "… is in Recently Deleted: restore it to change it".
- *Restore* on a folder brings back it and what went with it (the folders and scores deleted at
  the same moment); deleted folders above it come back too, as folders only. Restoring a score
  filed in a deleted folder brings that folder back (only the folder) so it has somewhere to be.
- After **30 days** a score or folder is removed for good. This is checked when a library opens
  and by `library.trash.purge`; it is not an undo step. A score still in use inside a removed
  folder moves up to the folder above it. *Empty Recently Deleted* removes everything now.
- Recently Deleted syncs (sync spec): a deleted folder is a folder record with its deletion time.
  A score another device filed into a folder deleted here stays in use and points at it;
  `library.verify` and `library.info` list it under `inDeletedFolders` with the folder to restore,
  verify is not clean, and the library window asks *Restore folder*.

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
| LIBS-11 | Two scores whose pages print their numbers (3 pages and 2) | "3" is searched inside scores; then "2"; then again after the session forgets its index | the 3-page score, page 3, snippet "3"; both; the first again, from the library's cache |
| LIBS-12 | A score whose notes say "Take the coda with rubato" | "RUBATO" is searched inside scores | that score, with the words around it |
| LIBS-9 | A portrait score | its cover is asked for at 200 px, then after its first page is turned | 200 px tall, narrower than that; then turned, width and height swapped |
| LIBS-13 | A score with every Edit Details field filled | each field's text is searched | the score is found by each; difficulty is 0 to 5 and 0 clears it |
| LIBS-14 | Folder *Horn* with *Etudes* inside, a score in each, a third score at the root | *Horn* is deleted; the first score's details, *Etudes*' name and filing the third score in *Horn* are tried; *Horn* is restored; it is deleted again and the library reopened 29 days later; then, with the etude left filed but in use, 30 days later | two scores and two folders in Recently Deleted, the third score alone in the library; all three refused; everything back in place; nothing removed; verify reports the etude with *Horn* to restore, then the concerto and both folders are removed for good and the etude moves to the root |
| LIBS-15 | The app with a library score, shown in the library | Settings is tapped at the bottom of the sidebar; then the first row | the Settings page shows; the first row goes back to the score, closing the library |
| LIBS-16 | The app with a library score | the score menu adds it to a new list *Gig*, moves it to folder *Bach*, then deletes it | *Gig* holds it; *Bach* holds it; it is in Recently Deleted; no errors |
| LIBS-17 | A library score that is not open | Share exports it, with and without ink | a PDF of its pages, the marked one with its markings; with no score open and no id, nothing is exported; a bad id is refused |
| LIBS-10 | The app with a library score | the library window is shown | the score's cover arrives without blocking a frame; list view works; opening a score closes the window |

## Not in scope

- Title and composer detection (LIB-7, -8): with `pdfcraft-ocr`, later.

## Open questions

Answered by the owner on 2026-11-01, see [the Edit Details note](../observations/library-edit-details.md). All three are built.

- **Search matches inside words**: "nata" finds *Sonata*. A match at a word start still ranks
  above one inside a word. Built (LIBS-1, LIBS-2). The full-text search inside scores (LIB-6)
  still matches word starts, through its sorted index.
- **Search covers every Edit Details field**: title, composer, tags, key, arranger, lyricist,
  instruments and voices, genre, difficulty, year composed, catalogue number, publisher or
  edition, description. Ranking stays ScoreCraft's own: title, then composer, then the rest
  Built: `doc::Metadata` has the new fields, `score.metadata.set` takes them, and the Score
  panel edits them (LIBS-13).
- **Sorting does not ignore leading articles**: *The Lark Ascending* sorts under T. Built
  (LIBS-4).

Still open: the questions in the [Edit Details note](../observations/library-edit-details.md#open-questions).

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-01 | minor | Owner's first hands-on feedback: the library's sidebar (Library, Lists, Recordings, Practice, Search, Recently Deleted, Settings), the floating toolbar on the right, the score's right-click menu in the owner's order, selecting, Settings as a library page, folder icons; LIBS-15..17 |
| 2026-11-01 | minor | LIB-10: folders in Recently Deleted with their scores, read-only, restored together, removed after 30 days; scores left in one are reported with a restore prompt; LIBS-14 |
| 2026-11-01 | minor | Built: search inside words and every Edit Details field (LIBS-13), articles kept in sort |
| 2026-11-01 | minor | Owner's answers: search inside words and in every Edit Details field; no article stripping in sort |
| 2026-10-31 | patch | LIB-6 search inside scores: PDF text and notes, a lazy on-disk text cache; LIBS-11, -12 |
| 2026-10-31 | patch | M5 part 3: LIB-13 names perf scenarios P6 and P7 (10,000 scores), first measurement |
| 2026-10-31 | patch | M5 part 2: LIBS-9, -10 for covers and the library window |
| 2026-10-31 | major | First version for M5: search, sort, recently opened, folder import, the library window, speed |
