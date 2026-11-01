# Page turning

> **Last reviewed:** 2026-10-31 · **Last updated:** 2026-10-31 · **Change:** minor (the half-page cut snaps between systems; earlier: first version) · **Target:** enScore 3.7 (2026)

How the reader lays pages out and turns them, and how taps, swipes, keys and foot pedals turn
them. Written for milestone M3.

## Sources

- Public listings S1, S2, S4 and S8: [../observations/public-listings-2026-10-28.md](../observations/public-listings-2026-10-28.md)
  ("Reading" and "Page turning"). They name the capabilities: two-page, single-page and full-screen
  layouts, half-page turns that show the next line early, alternating turns for awkward breaks,
  Landscape Page Zoom, and turning by tap, swipe, arrow keys, Bluetooth pedals and MIDI.
- The listings don't say how these behave in detail. Where they are silent, this spec picks
  behaviour that suits a performer, and lists the choice under [Open questions](#open-questions)
  so an observation pass ([G17](../gaps.md#g17-unknowns-behaviour-not-yet-observed)) can check it.

## Terms

- **Score pages**: the pages the reader turns through. For a library score these are its pages in
  the order the user set (PAGE-1), without hidden pages (PAGE-3) and outside its page range
  (PAGE-8), each turned by its own rotation (PAGE-2). For a file opened outside the library, the
  file's pages.
- **Spread**: what one full screen shows: one page, or two facing pages.
- **View**: one step in turn order: a whole spread, or (during a half-page turn) the top part of
  one spread above the bottom part of the spread before it.

## Requirements

### READ-2 Single-page layout

- One page at a time, as large as fits the reader area, centred.

### READ-3 Two-page layout

- Two facing pages side by side at the same height, the whole pair fitted and centred.
- Pairs are 1–2, 3–4, … by default. With *first page alone* (for a title page or a score whose
  page 2 is a left-hand page), page 1 stands alone and pairs are 2–3, 4–5, ….
- A last page without a partner stands alone.

### READ-5 Half-page turns

- From a full view, the first turn replaces the **top** part of the screen with the top part of the
  next spread; the bottom part, which the player is reading, stays where it is. The next turn
  completes the turn.
- The cut is at 50% of the page height by default, adjustable from 20% to 80%. Turning half-page
  turns on in the app snaps the cut to the gap between systems on the page on screen: a small
  render of the page shows its blank bands (rows with almost no ink, at least 1.2% of the page
  tall, not touching the edges), and the cut moves to the middle of the nearest one within 15%
  of the page height. A score page turned sideways, or with no gap near, keeps the cut as set
  (`view.turn.halfPage` with `snap`).
- Turning back during a half-page turn undoes it. Turning back from a full view goes to the half
  view before it.
- Works in both layouts.

### READ-6 Alternating turns

- In the two-page layout, each turn replaces only the page the player has finished: 1–2, then 3
  beside 2, then 3–4, then 5 beside 4. The page being read never moves.
- With *first page alone*: 1, then 2–3, then 4 beside 3, ….
- In the single-page layout it turns like a full turn.

### READ-12 Landscape Page Zoom, READ-15 zoom and pan

- Zoom is relative to a *fit*: the whole spread on screen (*page*), or the spread filling the
  width and scrolling up and down (*width*, Landscape Page Zoom).
- Zoom goes from 100% to 800% of the fit. Content larger than the screen pans; content smaller is
  centred.
- A turn keeps the zoom and shows the top of the new view.

### TURN-8 Go to page

- Go to any score page by number, or to the first or last. The reader lands on the first full view
  that shows that page.

### NAV-5 Contents

- Choosing an entry of the PDF's outline goes to its page. An entry whose page is hidden or outside
  the page range is an error the user sees, not a jump to a different page.

### Changing layout or mode

- Changing the layout or the turn mode keeps the page being read on screen (the lowest-numbered
  page of the full view, or of the bottom part during a half-page turn).
- Editing the score's pages while it is open (hide, move, rotate, range) keeps the same file page
  on screen when it is still shown.

### TURN-1 Tap zones, TURN-2 swipes, TURN-3 keys, TURN-4 pedals

- Tapping the right third of the page turns forward, the left third turns back; the middle third
  shows or hides the controls. In a two-page layout the thirds are of the whole reader area.
- Swiping left turns forward, swiping right turns back. A swipe needs at least 60 pixels of travel
  and more horizontal than vertical movement; a shorter movement is a tap.
- Keys: Right, Down, Page Down and Space turn forward; Left, Up, Page Up and Shift+Space turn back;
  Home and End go to the first and last page.
- Bluetooth foot pedals that act as keyboards work out of the box when they send any of those keys.
  The user can map any key to forward, back, first or last page, and the map is kept between
  launches. A pedal profile lists common pedal key pairs (arrows, Page Up/Down) to pick from.
- Holding a key or pedal down never turns more than one page per press.

### Speed (P1)

- A turn to a page that is already pre-rendered shows within one frame (≤ 16 ms from input). After
  every turn the views one turn forward and one back are pre-rendered in the background.

## Acceptance tests

| Id | Given | When | Then |
|---|---|---|---|
| PT-1 | A 5-page score, two-page layout | the reader opens | pages 1–2 show, then 3–4, then 5 alone |
| PT-2 | The same, first page alone | the user turns through it | 1, 2–3, 4–5 |
| PT-3 | A 3-page score, single layout, half-page turns | the user turns forward once | top part shows page 2, bottom part still page 1 |
| PT-4 | PT-3 after one turn | the user turns back | page 1 fills the screen again |
| PT-5 | 5 pages, two-page, alternating | the user turns three times | 3 beside 2, then 3–4, then 5 beside 4 |
| PT-6 | Page 6 of 10 on screen, single | the user switches to two-page | pages 5–6 show |
| PT-7 | Zoom 200%, panned to the bottom | the user turns | the next page shows at 200% from its top |
| PT-8 | A library score with page 2 hidden | the user turns from page 1 | the file's page 3 shows |
| PT-9 | Any view | the last view is on screen and the user turns forward | nothing changes, the reader reports it didn't turn |
| PT-10 | A pre-rendered next page | the user turns | the page is on screen within 16 ms |
| PT-11 | A pedal sending Page Down | the user presses it | the reader turns forward once |

PT-1 to PT-9 are engine tests (`crates/layout`, `crates/engine/src/reader_cmds.rs`,
`crates/engine/tests/library.rs`); PT-10 is perf scenario P1; PT-11 comes with the pedal map.

## Not in scope

- Turning across pieces in a setlist (TURN-7), MIDI turns (TURN-5), recorded turns (TURN-6) and
  repeat-aware turning (NAV-4): later milestones.
- Scroll layout (continuous vertical scrolling): not in the public listings.
- Turn animations: turns are instant; an optional animation is a later preference.

## Open questions

- Which part does enScore replace in a half-page turn, top or bottom, and is the split adjustable
  per page? (Listing S8 says only "show you the next line before you need it".)
- Exact tap-zone layout and whether it is configurable.
- Which keys enScore accepts from pedals by default.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | minor | The half-page cut snaps to the gap between systems (gap G2); PERF-15 in `crates/engine/src/reader_cmds.rs`, `scorecraft-layout` test in `snap.rs` |
| 2026-10-31 | major | First version for M3: layouts, half-page and alternating turns, zoom, go to page, contents, tap, swipe, keys and pedals |
