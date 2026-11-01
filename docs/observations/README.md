# Observation notes

Plain-language records of what enScore does, written as the first step of the
[clean-room process](../clean-room.md#4-the-two-step-process-observe-specify-implement).
Behavioural specs in [`../spec/`](../spec/README.md) link to these notes.

## Rules

- Describe behaviour in your own words. No code, no assets, no screenshots committed (keep them
  locally under [`screenshots/`](screenshots/README.md), which is gitignored, and cite them by number), no copied UI
  text beyond a short quote of public marketing copy (put it in quotes and link it).
- Observe only as a user would: the screen, the sounds, the files you export. Never the app's
  bundle, data directory, logs, memory or network traffic ([clean-room §3](../clean-room.md#3-forbidden-sources-and-actions)).
- One topic per file: `docs/observations/<area>-<topic>.md`, where `<area>` is a feature-map
  prefix in lower case (`read`, `turn`, `anno`, `lib`, `page`, `nav`, `list`, `play`, `midi`,
  `prac`, `imp`, `exp`, `sync`, `ui`, `plat`, `i18n`).

## Template

```markdown
# <Area>: <topic>

- **Feature-map ids:** READ-5, TURN-4
- **Source:** observed in enScore <version> on <device, OS version>, <date>
  (or: <link to a public page>, retrieved <date>)
- **Observer:** @github-username

## What happens

Numbered steps: what the user does, what enScore does in response.

## Edge cases seen

- …

## Open questions

- …
```

## Index

| File | Feature-map ids | Source |
|---|---|---|
| [public-listings-2026-10-28.md](public-listings-2026-10-28.md) | all | Public store listings and partner pages |
| [screenshots/README.md](screenshots/README.md) | LIB, LIST, IMP, ANNO | Index of 23 user screenshots (Library, Lists, Markup) and 6 store promo images (images not committed) |
| [anno-markup-toolbar.md](anno-markup-toolbar.md) | ANNO-1, -3, -4, -6..8, -12, -14, PAGE-7, NAV-1 | Owner's observation of the markup toolbar on Windows, 2026-10-31 |
