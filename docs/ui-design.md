# UI design system

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-11-01 · **Change:** minor (the owner's first hands-on feedback: toolbars, the library's sidebar and floating toolbar, painted icons; earlier: first version) · **Target:** enScore 3.7 (2026)

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
never hard-coded. Themes:

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
Practice, Search, Recently Deleted, a gap, Settings); a floating toolbar anchored on the right
(More, Filter, Sort, New Folder, Import); a grid of folders and covers or a dense list; a
right-click menu on each score (Edit Details, Add to list, Move to, Select, Share, Print, Delete);
multi-select for moves, lists and deleting. Spec: `docs/spec/library.md` LIB-14.

## Widgets

Shared widgets in `ui-egui/src/widgets/` (buttons, icon buttons, sliders with numeric entry,
segmented controls, colour swatches, list rows with swipe actions). Icons are ScoreCraft's own,
painted with egui shapes in `ui-egui/src/icons.rs` so they never depend on an emoji font (glyphs
such as 📁 and ↻ showed as empty boxes, owner 2026-11-01); none is copied from another app.

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
| 2026-11-01 | minor | Owner's first hands-on feedback: what stays off the score view, the library's sidebar, floating toolbar and score menu, painted icons |
| 2026-10-28 | major | First version |
