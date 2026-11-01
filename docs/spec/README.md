# Behavioural specs

Step two of the [clean-room process](../clean-room.md#4-the-two-step-process-observe-specify-implement):
what ScoreCraft must do, in our own words, written from [observation notes](../observations/README.md)
and public standards. Implementers work from these specs.

## Rules

- One file per area: `docs/spec/<area>.md` (for example `page-turning.md`, `ink.md`).
  Proprietary interchange formats without a public spec go in [`formats/`](formats/README.md) and
  follow the two-team rule in [clean-room §5](../clean-room.md#5-interchange-formats-without-a-public-spec).
- Every requirement has a feature-map id from [`feature-map.md`](../feature-map.md) and at least
  one acceptance test written as Given / When / Then.
- Link sources: observation notes, standards (with section numbers), papers.
- A spec says *what*, not *how*: inputs, outputs, timing, edge cases, failure behaviour. Crate and
  type names belong in the PR, not here.
- Specs are versioned with the progress-docs header ([`../standards/progress-docs.md`](../standards/progress-docs.md)).

## Template

```markdown
# <Area>

> **Last reviewed:** YYYY-MM-DD · **Last updated:** YYYY-MM-DD · **Change:** major (first version) · **Target:** enScore 3.7 (2026)

## Sources

- Observation: [../observations/<file>.md](../observations/<file>.md)
- Standard: ISO 32000-2 §12.5 (annotations)

## Requirements

### READ-5 Half-page turns

- Given a single-page layout, when the user turns forward, the bottom half of the screen shows the
  top half of the next page while the top half keeps the bottom of the current one.
- …

## Acceptance tests

| Id | Given | When | Then |
|---|---|---|---|

## Not in scope

## Revision history
```

## Index

| Spec | Feature-map ids | Status |
|---|---|---|
| [page-turning.md](page-turning.md) | READ-2, READ-3, READ-5, READ-6, READ-12, READ-15, TURN-1..4, TURN-8, NAV-5 | first version 2026-10-31 |
| [ink.md](ink.md) | ANNO-1..17, EXP-2 | first version 2026-10-31 |
| [library.md](library.md) | LIB-2, LIB-5, LIB-9, LIB-13, LIB-14, LIB-15 | first version 2026-10-31 |
| [performance.md](performance.md) | LIST-4, LIST-6, IMP-6, PRAC-1, NAV-1, PAGE-7 | first version 2026-10-31 |
