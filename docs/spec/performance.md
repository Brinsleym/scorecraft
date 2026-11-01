# Performance: setlists on stage, shortcuts, snips and the metronome

> **Last reviewed:** 2026-10-31 · **Last updated:** 2026-11-01 · **Change:** minor (the owner's answers on placeholders and page ranges; earlier: M6 part 4: the Lists panel; earlier: M6 part 3: shortcuts and snips; earlier: M6 part 2: metronome; earlier: first version) · **Target:** enScore 3.7 (2026)

What a musician needs on stage, beyond reading one score: playing a setlist from piece to piece,
jumping at repeats and codas, seeing the bars after a turn before it, and a metronome. Written
for milestone M6, which closes the [alpha gate](../roadmap.md#alpha-gate). Setlists as data
(create, reorder, page ranges, tempos, placeholders) are M1's (LIST-1..3, LIST-7).

## Sources

- Public listings S1, S2, S3, S4 and S8:
  [../observations/public-listings-2026-10-28.md](../observations/public-listings-2026-10-28.md)
  ("Lists", "Score Shortcuts", "Score Snips", "Practice"). They name lists whose pieces flow into
  one another with the screen kept awake, tappable shortcuts on repeat and coda signs, snips of
  the bars after a turn pasted before it, and a metronome with tap tempo. How each behaves in
  detail is ScoreCraft's choice here, listed under [Open questions](#open-questions).

## Requirements

### LIST-4 Playing a list

- Playing a list opens its first entry's score (or the entry the player starts from) at the
  first page of its page range. Turning past the last page opens the next entry at its first
  page; turning back from the first page opens the previous entry at its last page. Pedals,
  keys, taps and swipes all turn this way.
- A placeholder (LIST-7) has a page of its own (see below) and turns stop on it like on any
  score. Placeholders made before they had pages are passed over; the turn that passes one
  reports its label so the reader can show it briefly. Entries whose score is in the trash or
  gone are passed over too.
- While a list plays, each entry shows only its page range (LIST-2); stopping shows the whole
  score again. Opening another score by hand stops the list.
- The screen stays awake from the first entry to the end (READ-9 applies throughout, as a score
  is always open).
- The reader can show where the player is: the list's name, entry *n* of *m*, the score's title,
  its tempo (LIST-3) and the next score's title. The app shows this in the toolbar with a *Stop
  list* button.
- Each score opens in the same window size, layout, turn mode and fit as the one before it.
- The library window has a Lists panel: the lists, a new-list box, and for the list picked its
  entries with play-from-here, up, down and remove, *Add break*, *Delete list* and *Play*. A
  score's cover menu adds it to the picked list.

### LIST-7 Placeholders are pages (ScoreCraft's own)

- Adding a placeholder ("Interval", "Speech") makes a one-page score from the blank A4 template
  (595 × 842 pt) with the name as a 36 pt text note centred on the page. The name is cleaned
  first: control characters and invisible format characters (bidi overrides, zero-width
  characters) are dropped, runs of white space become one space, and it is cut to 80
  characters; a name with nothing visible left is refused. It is also the page's title and,
  with characters no file system allows replaced, its file name.
- The page belongs to the list: it is left out of the library's listings, search, full-text
  search and recent list. The pen, highlighter, text and every other markup tool work on it,
  and the name is an ordinary note that can be moved, edited or deleted.
- Removing the entry, or deleting the list, sends its page to Recently Deleted with its ink; undo
  brings both back. Adding a placeholder is one undo step.

### LIST-2 A library page range comes with the score

- When a score whose library entry has a page range (PAGE-8) is added to a list, the new entry
  copies that range. The answer says so (`pageRange` with `fromLibrary`), and the Lists panel
  shows a quiet one-line notice under the list's buttons with the range to change (*Apply*),
  *All pages* to clear it, or *OK*. From then on the entry's range and the library's are
  independent: changing one leaves the other alone.

### LIST-6 One score, many lists

- A list entry refers to a library score; it never copies it. The same score may be in any
  number of lists, and more than once in one. Editing the score (metadata, pages, markings)
  shows in every list; deleting a list never deletes a score.

### IMP-6 Import report

- Every import reports what it did: how many scores were imported, how many files were already
  in the library (the same bytes) and skipped, how many were not PDFs, and each file that
  couldn't be read with the reason (the first five named, the rest counted).

### PRAC-1 Metronome with tap tempo

- A metronome at 20–300 beats per minute, in 2, 3, 4 or 6, with an accent on the first beat, as
  a click and a light pulse along the top edge of the reader (brighter, in the accent colour, on
  the first beat of the bar, fading within the first 40% of the beat). The click is a 30 ms sine
  burst (1,760 Hz on the first beat, 1,100 Hz on the others), synthesised sample-accurately by
  `scorecraft-audio` and played through the default output on Windows and macOS
  (`scorecraft-devices`, `audio` feature); a build or device without audio output shows why in
  the metronome menu and keeps the pulse. Changing the tempo while it runs keeps the beat count
  going. Tapping the tempo button four times sets
  the tempo from the average of the last three intervals; taps more than 2 s apart start again.
  A playing list's entry tempo sets it when the entry opens.

### NAV-1 Score Shortcuts

- A shortcut is a rectangle on a page (drawn with the rectangle tool, ANNO-12) that jumps to a
  target page, and optionally a part of it, when tapped. It is drawn as a faint outline in markup
  mode and invisible otherwise; tapping it in the reader jumps instead of turning.
- With the rectangle tool, any rectangle (even one with no markings in it) offers *Shortcut to
  page n*. A target page that is hidden or outside the score's range is not jumped to; the tap
  turns as usual. Shortcuts are deleted like any marking, one undo step. In markup mode each
  shows as a thin blue outline labelled with its target page ("→ hidden" when that page isn't
  shown).

### PAGE-7 Score Snips

- A snip copies a rectangle of one page and shows it at a place on another, typically the first
  bars of the next page at the foot of this one, so the turn can wait. It is drawn into the page
  like ink, on its own layer, and can be moved, resized and deleted.
- With the rectangle tool, *Snip* then a tap places the rectangle's copy with its top-left corner
  at the tap, kept inside the page, at the same size, framed by a thin grey line. It is drawn
  from the source page rendered at the reader's scale, under the page's filter. Export (EXP-2)
  writes it as the source page's own vector content, clipped to the rectangle over white paper,
  with the same frame, in both export modes. Moving keeps it inside its page; resizing keeps its
  shape, from 10% to 4 times the snipped width.

## Acceptance tests

| Id | Given | When | Then |
|---|---|---|---|
| PERF-1 | A list: score A (2 pages), a placeholder "Interval", score B (3 pages) | it is played and the pedal pressed on A's last page, turned again, then turned back twice | the interval's page; B at page 1; back to the interval, then A at page 2; past B's end nothing turns |
| PERF-14 | A list | a placeholder named "  Inter<U+202E>val\n\t talk  " is added, played and written on; the entry is removed and the removal undone; a name of only invisible characters is added | a one-page A4 score titled "Interval talk" with that note centred, 36 pt, not listed or found in the library; playing stops on it and the stroke lands on it; removing trashes it, undo restores it; the last is refused |
| PERF-15 | A 5-page score with library range 2–4 | it is added to a list; the entry's range is set to 3–3 and the library's to 1–5 | the answer reports range 2–4 from the library; the entry keeps 3–3 |
| PERF-2 | An entry with pages 2–3 of a 5-page score | the list is played, then stopped | the reader shows 2 pages, then all 5 |
| PERF-3 | One score twice in each of two lists | the score is renamed and a list deleted | both lists show the new title; the library still has one score |
| PERF-4 | An empty list; a list with one entry | the first is played; the second from entry 5; then a score opened by hand | refused; refused; the list stops |
| PERF-6 | 120 BPM in 3, started at t = 1 s | the beat is asked for at 1.0 s, 1.75 s and 2.5 s | beat 1 (accented, next at 1.5 s); beat 2 half way; beat 1 of the next bar |
| PERF-7 | Taps at 0, 500, 1000, 1500 ms, then 2100, then 5000 | each tap | 120 BPM at the fourth; the fifth averages the last three intervals; after the pause, nothing until four more |
| PERF-8 | The engine's clock | the metronome is set to 120 in 3, started, read 1.1 s later, then tapped every 400 ms | the third beat of the bar; 150 BPM; out-of-range tempos and meters refused |
| PERF-9 | A shortcut on page 1 to page 3 | the page is tapped beside it, then on it; then page 3 is hidden and it is tapped again | nothing; page 3; nothing |
| PERF-10 | A snip of the top of page 2 at the foot of page 1 | it is created, moved below the page, and the move undone | same size, top at the tap; kept on the page; back where it was |
| PERF-11 | Page 1 | a snip of page 2 is placed on it | page 1's picture changes |
| PERF-12 | The app, a list "Gig" with the open score, picked in the Lists panel | *Play* | the reader shows; the list plays |
| PERF-13 | A click track at 120 BPM in 3 at 48 kHz, stereo | two seconds are filled, then it stops | loud accented clicks at frames 0 and 72,000, a quieter one at 24,000, silence between, both channels alike; then silence |
| PERF-5 | An import of two new PDFs, one already imported, three other files and one broken PDF | it finishes | "Imported 2 scores; 1 already in the library; 3 files not PDFs; 1 couldn't be read (broken.pdf: …)." |

PERF-1 to PERF-4 are engine tests (`crates/engine/src/play_cmds/tests.rs`); PERF-5 is a shell
test (`crates/ui-egui/src/app.rs`); PERF-6 and PERF-7 are `scorecraft-practice` tests, PERF-8 an
engine test (`crates/engine/src/metronome_cmds.rs`); PERF-9 to PERF-11 are engine tests
(`crates/engine/src/nav_cmds/tests.rs`); PERF-12 is a shell test (`crates/ui-egui/src/app.rs`);
PERF-14 and PERF-15 are engine tests (`crates/engine/src/play_cmds/tests.rs`).
PERF-1 also checks that the window and layout carry over to the next score.

## Open questions

None left. Answered by the owner on 2026-11-01; both are built (LIST-7, LIST-2 above).

- **Placeholders are ScoreCraft's own** (enScore has none). A placeholder in a list (for example
  "Interval") is a real, editable page: a blank A4 page made from a template, with the
  placeholder's name in large centred text. The name is sanitised before it is drawn (plain text,
  length-capped, no control characters). The pen and text tools work on it like any page, and
  playing the list stops on it like any score. Built (PERF-14).
- **Page ranges.** An entry's range limits what plays: turning past its last page goes to the next
  entry (as built). When a score that has a page range in the library is added to a list, the
  entry copies that range and an unobtrusive notice says so, with a way to change it there. From
  then on the list entry's range and the library score's range are independent. Built
  (PERF-15).

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-01 | minor | Built: placeholders are A4 pages that playing stops on (LIST-7, PERF-14); a library page range comes with the score once, with a notice (PERF-15) |
| 2026-11-01 | minor | Owner's answers: placeholders become editable A4 template pages; a library page range is copied to a new list entry with a notice, then independent |
| 2026-10-31 | patch | The metronome's click: synthesis and audio output on Windows and macOS; PERF-13 |
| 2026-10-31 | patch | M6 part 4: shortcut outlines, snips in export, snip resizing |
| 2026-10-31 | patch | M6 part 4: the Lists panel and the now-playing strip; scores in a list keep the window and layout; PERF-12 |
| 2026-10-31 | patch | M6 part 3: shortcuts and snips from the rectangle, how they behave; PERF-9..11 |
| 2026-10-31 | patch | M6 part 2: the metronome's pulse and timing, PERF-6..8 |
| 2026-10-31 | major | First version for M6: playing a list, one score in many lists, the import report; the metronome, shortcuts and snips outlined for parts 2 and 3; PERF-1..5 |
