# Annotation: the markup toolbar

- **Feature-map ids:** ANNO-1, ANNO-3, ANNO-4, ANNO-6..9, ANNO-11, ANNO-12, ANNO-14, ANNO-17, ANNO-18, PAGE-7, NAV-1
- **Source:** observed in enScore on Windows by the project owner, 2026-10-31 (screenshots 24 to
  29, [index](screenshots/README.md#markup)), plus the owner's description of what ScoreCraft's
  markup should do
- **Observer:** Brinsleym

## What happens

1. Entering markup on a score shows one toolbar strip across the top of the reader. From left to
   right it holds:
   1. A **Draw with Touch** toggle (a hand icon with its label).
   2. A padlock button (the **touch lock**, below).
   3. The pen, shown selected (highlighted in an accent colour) by default.
   4. A highlighter.
   5. An eraser.
   6. A text tool (two letter "A"s of different sizes).
   7. A rectangle-selection tool (a dashed square).
   8. A ruler.
   9. Eight round colour swatches: black (selected, ringed in the accent colour), red, blue,
      green, amber, purple, brown and grey.
   10. A palette button that opens a custom colour picker.
   11. A slider that sets the width.
2. The owner's account of how it behaves and what ScoreCraft should do:
   - In markup, **only a pen draws** by default. A mouse (and a finger) doesn't mark up.
     **Draw with Touch** lets the mouse and touch draw ink too.
   - The rectangle tool draws a rectangle on the page. The rectangle is then used to **snip and
     paste** bars (Score Snips), to **create a shortcut** (Score Shortcuts), or to **move, copy or
     delete** the ink inside it.
   - The **padlock is a touch lock**: when it is on, every touch input is ignored. It can only be
     toggled while markup (ink) mode is on.
   - The **eraser** has *stroke* and *pixel* options, which appear in the bar when the eraser is
     selected.
   - The **width slider is per tool**: each tool keeps its own width.
   - The **eight swatches can be changed**. The palette button opens a fixed grid of extra colours
     (screenshot 25): six columns by five rows, roughly black, white and four greys; crimson,
     red, orange-red, orange, amber and yellow; lime, green, sea green, cyan, blue and indigo;
     violet, deep purple, peach, tan, brown and dark brown; then pastel pink, orange, yellow,
     green, blue and lavender. It offers only these colours, not a free colour picker. Clicking
     one replaces the **currently selected swatch** with that colour.

3. Second pass, the owner's answers to the spec's open questions (2026-10-31):
   - **Highlighter strokes** (screenshot 26: shapes drawn with the default highlighter). Each
     band keeps the same height whichever way it runs. Horizontal strokes are thick, vertical
     strokes thin, and the ends are cut straight up and down rather than across the stroke. A tap
     leaves an upright rectangle about three times taller than wide. So the nib is a chisel held
     upright, swept along the path. Where a stroke crosses itself the colour is even (no
     build-up). The yellow shows at the swatch's full strength over white paper.
   - The eraser's **size** is set by the same single width slider. The pixel eraser leaves the
     remaining stroke's pressure profile as it was.
   - **Stamps are not an enScore feature.** The owner wants them in ScoreCraft anyway: see the
     decisions below.
   - **Ruler** (screenshot 27). A light band with tick marks along both long edges spans the whole
     screen width; enScore shows the current angle (0°) in a small badge at the centre.
   - **Highlighter swatches** (screenshot 28). The highlighter has its own eight swatches (yellow,
     pink, light green, light blue, orange, lavender, coral, aqua) and its own palette of 30
     highlighter colours, six by five: pale to deep yellows and golds; peaches, oranges, salmon and
     coral; pinks, roses, magenta and orchid; yellow-greens, greens, mint and turquoise; pale and
     sky blues, blue, lavenders and violet.
   - The **stylus shortcut** (barrel button or double-tap) switches between the pen and the
     eraser by default.

4. **Text** (screenshot 29: the owner added "Sample Text" notes of many lengths to a score's
   title page):
   - With the text tool selected (its button highlighted), the first toggle reads **Type with
     Touch** instead of *Draw with Touch*; in the screenshot it is on.
   - Notes are plain black text in a sans-serif face, about 10–11 pt for the page, with no box,
     border or background: they sit straight over the music and the title art, which shows
     through around the letters.
   - A long note wraps at word boundaries to a **maximum width**. In the screenshot every line
     of the long block starts at the same left edge and wraps at the same right edge, about 86%
     of the page's width (705 of roughly 824 pixels). Lines are set close, about 1.5 times the
     text height apart. Short notes ("Sample Text Sample Text", even a lone "a") are only as wide
     as their text.
   - **Boundary limits**: notes stay inside the page. The long block starts exactly at the page's
     top-left corner, and a note near the right edge ends at the page's right edge rather than
     running off it.
   - **Text size** (the owner, 2026-10-31): the slider sets it while the text tool is active,
     from 8 pt to 36 pt, 12 pt by default.
   - **A new or selected note** shows a small empty box outlined in red (the accent colour) where
     the text goes, with a dark rounded **handle** on its left marked with grip lines (for
     dragging the note).

## Decisions taken from this note

- The **ruler** keeps its function, but its button is **hidden by default**; a setting shows it
  (owner, 2026-10-31, correcting an earlier "leave it out").
- The rectangle tool replaces the freehand lasso as the way to select ink.
- A setting switches the highlighter to **round ends** instead of the chisel.
- **Stamps** (ScoreCraft's own, not enScore's): dynamics pp, p, mp, mf, f and ff in the **Edwin**
  font ([MuseScoreFonts/Edwin](https://github.com/MuseScoreFonts/Edwin), SIL OFL 1.1), with **p**
  the default; a ">" button at the end opens crescendo and diminuendo **hairpins**, each with a
  handle at both ends to stretch it and one in the middle to move it.
- **Palm rejection after the pen lifts**: touches that start within a very short moment after the
  pen lifts are ignored, short enough to catch a landing hand but never a deliberate tap.
- The **ruler** starts horizontal across the whole screen width. Turning it to other angles (and
  the angle badge) waits at the bottom of the low-priority list.

## Open questions

- Whether the text tool has colour or font options (none show in screenshot 29), and how
  the maximum width is set: a fixed share of the page, or a fixed size.
- Whether a note dragged towards an edge stops at it or is pushed back inside when dropped.
- Whether the chisel turns with the page when a page is rotated (ScoreCraft keeps it upright on
  the page, like a marker on paper).
- How strokes follow enScore's ruler edges, and whether the ruler can be moved up and down.
