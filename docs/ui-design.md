# UI design system

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-11-08 · **Change:** minor (styling rules: Tabler icons, Segoe UI, the type scale; Search leaves the sidebar; earlier: the library ported from the owner's Stitch concept) · **Target:** enScore 3.7 (2026)

ScoreCraft's look is its own. We match enScore's *conventions* where a musician's hands expect them
(tap to turn, pen to mark, the score filling the screen), never its visual identity, icons or copy
([clean-room §1](clean-room.md#1-what-we-copy-and-what-we-never-copy)).

## Principles

1. **The score is the interface.** In the reader, chrome hides by default; the page fills the
   screen. Controls appear on a tap in the top band and disappear on the next turn.
2. **Nothing moves under a performer.** No layout shifts, no pop-ups or toasts over the page while
   performing, no update prompts, no sync banners. Errors in performance mode wait in a
   non-blocking indicator until the performer leaves the reader.
3. **Reachable on a stand.** Touch targets ≥ 44 pt (48 dp on Android); primary reader actions within
   thumb reach at the screen edges; every action also has a key and a command id.
4. **Legible at arm's length.** Page filters never reduce contrast of notation below WCAG AA for
   text-equivalent content; ink colours are remapped per filter to stay visible.
5. **Calm by default.** Animations are short (≤ 150 ms), optional, and off in e-ink mode and when
   the OS asks for reduced motion.

## Themes and tokens

All colours, radii, spacings and font sizes come from `theme::Tokens` (`ui-egui/src/theme.rs`),
never hard-coded. The accent in every theme is ScoreCraft's brand colour, `#c45821` (owner,
2026-11-08; see [decisions](decisions.md)). Themes:

| Theme | Use |
|---|---|
| **Stage** (dark) | Default for the app chrome; the page keeps its own filter |
| **Studio** (light) | Daylight practice |
| **E-ink** | Pure black and white, no gradients or shadows, no animation, large targets |
| **High contrast** | Follows the OS setting |

Page filters (READ-7) are separate from the app theme: Original, Dark (inverted luminance with
hue kept), Sepia (with intensity), Inverted.

## Reader layout

- **Turn zones.** By default the right 60% of the page turns forward and the left 25% turns back,
  the top 12% band shows the toolbar (exact proportions to be fixed in `docs/spec/page-turning.md`
  from observation, and configurable).
- **Half-page turns.** A thin divider marks the split; the incoming half fades in no faster than
  the turn animation.
- **Toolbar.** Library, setlist position, layout, filters, markup, bookmarks, shortcuts, metronome,
  audio, more. Each button dispatches a command; the command palette (⌘K / Ctrl+K) lists every one.
- **Markup bar.** Appears when the pen touches or the markup button is tapped: pen, highlighter,
  whiteout, erasers, text, stamps, lasso, colours, widths, layer picker, undo/redo, done.

- **What stays off the score view** (owner, 2026-11-01): no Open (scores are imported into the
  library), no Export (Share is in the library's score menu), no Score panel (Edit Details is in
  the score menu), no Keep Screen Awake button and no settings of any kind (Settings is a library
  page). The page layout is a drop-down (Automatic, Single page, Two pages); Clear Ink sits next
  to Markup. Full screen keeps the toolbars.

## Library layout

No full-width toolbar. A compact, resizable left sidebar of rows (Library, Lists, Recordings,
Practice, Recently Deleted, a divider, Settings); the section shown is filled, with the
accent on its left edge and icon. Beside it, a large page title with its buttons on the right
(More, Search, Sort with a chevron, New Folder, and Import filled with the accent; Search is
the header's button, not a sidebar row, owner 2026-11-08);
a grid of folders and covers or a dense list; a right-click menu on each score (Edit Details, Add
to list, Move to, Select, Share, Print, Delete); multi-select for moves, lists and deleting. Spec:
`docs/spec/library.md` LIB-14.

The look follows the owner's Stitch concept (2026-11-08): cards at least 118 pt wide fill each
row with 24 pt between columns and 32 pt between rows; covers are 4:5 white paper with 8 pt
corners and a soft shadow, titled underneath in up to two lines; folders are tiles with the accent
folder icon, their name and an item count (`library.info` `folders[].items`). Header buttons and
sidebar rows are 36 pt high (desktop sizes; tablets still need the 44 pt targets of principle 3).

## Widgets

Shared widgets in `ui-egui/src/widgets/` (buttons, icon buttons, sliders with numeric entry,
segmented controls, colour swatches, list rows with swipe actions). Icons follow the styling rules
below; never draw an icon with an emoji or symbol character in a label (glyphs such as 📁 and ↻
show as empty boxes, owner 2026-11-01).

## Styling rules (every screen)

The owner's rules (2026-11-08), taken from the Stitch concept. They apply to every element, not
only the library.

- **Icons: [Tabler Icons](https://tabler.io/icons), outline, stroke 2.** Use the most typical
  Tabler name for the meaning (`search`, `trash`, `settings`, `x`, `plus`, `chevron-down`…). Fixed
  choices: Library `books`, Lists `menu-2`, Recordings `microphone`, Practice `chart-line`.
  Filled variants (`star-filled`) only for an on state. Icons are glyphs of a subset of Tabler's
  icon font in `crates/ui-egui/assets/tabler/`, one `Icon` variant per glyph in `icons.rs`; to
  add one, add its codepoint there and re-cut the subset (the README beside the font).
- **Font: Segoe UI.** Regular for everything, Semibold (`theme::semibold()`) for page titles and
  group headings. The concept's *medium* (500) has no Segoe UI face and renders Regular, as in a
  browser. Segoe UI is Microsoft's and may not be bundled: Windows loads it from the system
  (`Platform::ui_fonts`); elsewhere egui's own fonts stand in.
- **Type scale (points):** 13 body, buttons and sidebar rows (`theme::TEXT`); 12 card titles
  (`SMALL`); 11 counts and hints (`CAPTION`); 30 page titles (`TITLE`). Egui's `TextStyle`s carry
  these, so plain labels follow without extra code.
- **Letter spacing:** page titles and card titles are *tight*, −0.025 em (`theme::TIGHT` × size);
  everything else is normal.
- **Icon to text:** an icon beside text is 16/13 of the text's size (`theme::icon_for`: 16 beside
  13). Header buttons are 36 high with a 16 icon; reader toolbar buttons are 44 with a 20 icon
  (`TOOLBAR_ICON`); a menu chevron is 12; a folder tile's icon is about 0.39 of the tile's width.
- **Colours** come only from `theme::Tokens`; the accent is the brand colour.

## Localisation

Every string goes through `tl!` and `ui-egui/src/i18n/<lang>.tsv`. Translations are clean-room:
written from the meaning of our English text, never copied from another app's translations. Layouts
must survive 40% longer strings and right-to-left scripts.

## Automation for visual checks

`cargo run -p scorecraft-ui-egui --example snapshot` renders any UI state offscreen, and
`ui.screenshot` over the control channel captures the live app. Every UI PR attaches before/after
PNGs of public-domain scores.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-08 | minor | Styling rules: Tabler icons (outline, stroke 2), Segoe UI, type scale, tight titles, icon-to-text sizes; Search leaves the sidebar |
| 2026-11-08 | minor | Library ported from the owner's Stitch concept: brand accent `#c45821`, title-row buttons, filled card grid with item counts |
| 2026-11-01 | minor | Owner's first hands-on feedback: what stays off the score view, the library's sidebar, floating toolbar and score menu, painted icons |
| 2026-10-28 | major | First version |
