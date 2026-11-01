# Library: the Edit Details dialog

- **Feature-map ids:** LIB-4, LIB-5, LIB-7
- **Source:** observed in enScore on Windows by the project owner, 2026-11-01 (screenshots 30 to
  32), with the owner's answers to the library spec's open questions the same day. Written in our
  own words under [clean-room §2.4](../clean-room.md#2-allowed-sources).

## What happens

1. **Opening it.** Edit Details is a modal dialog over the library, titled *Edit Details*, with
   *Save* (accent colour) and *Cancel* in a fixed footer. The body scrolls when it is taller than
   the window (screenshots 31 and 32).
2. **Suggestion bar.** When detection has a value for an empty field, a highlighted bar at the top
   (sparkle icon, "Suggested") shows the field and the value, here *Composer* with a name, and an
   *Apply* button that fills the field in. It is a suggestion only: nothing is written until Apply,
   then Save (LIB-7).
3. **Always-shown fields.** A small cover thumbnail of the first page sits beside *Title* and
   *Composer*, both single-line text boxes. The title keeps non-Latin text as typed (screenshot 30
   shows Japanese). Below is *Tags*, a box with the hint "Separate tags with commas".
4. **Tags as chips.** Typing a comma turns the text before it into a chip with an × to remove it;
   the box keeps the text after the last comma for the next tag (screenshot 31).
5. **Add details.** Under the heading *Add details* is a row of pill buttons, each with a plus,
   for the optional fields: *Key*, *Arranger*, *Lyricist*, *Instruments & voices*, *Genre*,
   *Difficulty*, *Year composed*, *Catalogue number*, *Publisher or edition*, *Description*.
   Pressing one adds that field to the form and removes its pill. A score with a value in a field
   shows that field without needing its pill.
6. **Field kinds** (screenshots 31 and 32):
   - *Key*: a drop-down of keys (shown: "E Major").
   - *Arranger*, *Lyricist*, *Genre*, *Publisher or edition*: single-line text.
   - *Instruments & voices*: single-line text with the hint "Piano, Violin, SATB…" (a list).
   - *Difficulty*: five stars, filled up to the chosen one (shown: three of five).
   - *Year composed*: a short text box sized for a year.
   - *Catalogue number*: single-line text with the hint "Op. 27 No. 2, BWV 1007".
   - *Description*: a multi-line text area.
7. **Focus.** The focused field gets an accent-coloured underline (screenshot 31, *Lyricist*).

## Owner's answers (2026-11-01)

- **Search covers every field in this dialog**, not only title, composer, tags and key.
- **Search matches inside words**: "nata" finds *Sonata*.
- **Sorting does not ignore leading articles**: "The …" sorts under T.

## Decisions taken from this note

- `doc::Metadata` gains the fields it lacks: lyricist, instruments and voices (a list), difficulty
  (0 to 5), year composed, catalogue number, publisher or edition, and description (today
  `notes`). `custom` stays for user-defined fields. A format v1 migration fills the new fields
  empty.
- The details form in ScoreCraft's Score panel follows the same shape: fixed title, composer and
  tags; optional fields added from pills; detection shown as a suggestion that needs Apply.
- `library.search` searches all of these fields, ranked title first, then composer, then the rest.
- The sort key stops dropping leading articles.

## Open questions

- Whether *Year composed* accepts ranges or approximate years ("c. 1720", "1801–02").
- Whether the key list includes modes and minor keys spelled both ways, and a "none" entry.
- Whether enScore can filter the library by difficulty or by instrument (screenshot 6 shows a
  filter sidebar).
