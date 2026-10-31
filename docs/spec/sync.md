# Sync

> **Last reviewed:** 2026-10-31 · **Last updated:** 2026-10-31 · **Change:** minor (part 3: syncing through a shared folder; earlier: part 2: every record, revivals, list positions, changes from edits; earlier: first version) · **Target:** enScore 3.7 (2026)

How two or more devices that change the same library while apart end up with the same library.
Written for milestone M8, from the owner's design ([architecture §9](../architecture.md#9-sync)).
This covers the merge rules and the changes devices exchange; transport (Google Drive, Wi-Fi
peers) and score files come in later parts.

## Sources

- The owner's design in architecture §9: offline first, the device is the source of truth, sync
  exchanges changes and blobs; annotations are an add-wins set with tombstones, metadata fields
  last-writer-wins registers keyed by hybrid logical clock, list order a sequence CRDT.
- Public listings S1, S3, S4: enScore syncs scores, markings, lists, bookmarks and settings across
  a user's devices.

## Terms

- **Replica**: one device's library and what it knows of the others' changes.
- **Change**: one edit as devices exchange it: who made it (device and sequence number), when (a
  hybrid logical clock), and what.
- **Converged**: every replica has received every change; their libraries are then equal.

## Requirements

### SYNC-6 Merging concurrent edits

- Every change carries a unique id (device, sequence) and a hybrid logical clock. Receiving a
  change twice, or changes in any order, gives the same library: merging is idempotent,
  commutative and associative.
- **Elements** (scores, markings, bookmarks, folders, lists and list entries) each have three
  clocks: the last change of their content, the last removal and the last revival. An element
  shows the content of its latest put, and is gone while its latest removal is later than its
  latest revival.
  - A marking added on one device and edited or added on another is kept; two devices marking
    the same page offline both keep their strokes.
  - A marking deleted anywhere is gone everywhere, and an edit of it (even a later one by the
    clock, made before the device heard of the delete) never brings it back.
  - Undoing the delete puts the same marking back as a **revival**, which wins over the earlier
    delete on every device. Likewise for a deleted score, folder, list or entry.
- **Editing an element** (move, recolour, new text, a folder's name) is last-writer-wins by the
  clock: of two concurrent edits, the later clock wins on every device; ties break by device.
- **Fields** are last-writer-wins registers:
  - a score's details (LIB-4), folder, pages (order, rotation, crop, hidden, range), layers,
    audio, MIDI on open, practice notes, and dates;
  - Recently Deleted for each score;
  - each library setting.
- **Order.**
  - Markings and bookmarks are kept in id order (ids are time-ordered), so every device draws
    them in the same order.
  - List entries are ordered by position, a fractional index (a sequence CRDT): an entry
    inserted or moved gets a new position between its neighbours', so a move is one change and
    concurrent inserts never renumber anything. Positions carry a digit from the entry's id, so
    two devices inserting in the same gap at once rarely tie; a tie orders by entry id.
- A change inside a score or list a device doesn't have yet waits until that score or list
  arrives, then applies.
- A device's edits become changes by comparing the records the edit touched with the replica's
  copy, so every engine command syncs without its own sync code.

### SYNC-3 Offline first

- A device applies its own changes at once and keeps them in its log; nothing waits on the
  network. Exchanging is "send what the other hasn't seen": each side names the changes it has
  (by device, the highest sequence received), and gets the rest.

### SYNC-1 and SYNC-7 Syncing through a shared folder (part 3)

- `sync.backend.set {kind: "folder", path}` turns sync on for the open library and syncs it
  through `path`: any folder every device can reach, such as one that Google Drive for desktop,
  OneDrive, Dropbox, iCloud Drive or Syncthing keeps in sync, or a network share. `{kind: "none"}`
  stops using the folder; the library keeps recording changes.
- Turning sync on gives the device an id, kept in the library, and records the whole library as
  changes. From then on, every command's saved edits are recorded after the command, and kept in
  the library under `sync/` as append-only blocks. When the library opens, the replica is
  rebuilt from them, and anything edited while sync wasn't recording is recorded then.
- `sync.now`:
  - writes this device's new changes to the folder as one new block file;
  - reads the blocks of other devices that it hasn't seen;
  - applies them to the library and saves them;
  - copies score files both ways, by content address.

  Each device writes only new files and never changes one, so a cloud tool never sees a
  conflict. A block may only carry its own device's changes, and a damaged block or file is
  skipped and listed in `sync.status`. Undo history for records that changed is dropped.
- `sync.status` reports whether sync is on, the device, the folder, how many changes there are,
  how many wait, when it last synced, and the score files neither side has yet.

## Acceptance tests

| Id | Given | When | Then |
|---|---|---|---|
| SYNC-T1 | Two replicas of one library | each draws a stroke on page 1 offline, then they exchange | both have both strokes, in the same order |
| SYNC-T2 | A stroke on both replicas | one deletes it while the other recolours it, then they exchange | it is gone on both, and stays gone when the recolour arrives again |
| SYNC-T3 | A score on both replicas | both rename it offline, then they exchange | both have the name with the later clock |
| SYNC-T4 | Three devices, two joining empty | random edits of every kind (markings, undo, details, bookmarks, folders, Recently Deleted, settings, list inserts, moves and drops) and random partial, out-of-order exchanges, then everyone exchanges with everyone | all libraries are equal, each device's library equals its replica's, nothing waits and nothing is left to record (property test) |
| SYNC-T5 | A change received twice or out of order | it is applied | the library is the same as receiving it once, in order |
| SYNC-T6 | A device with a library and one with none | the second joins and they exchange; then the first edits a score directly | the second has the whole library, then the edit, and nothing is left to record |
| SYNC-T7 | A marking (then a whole score) on two devices | one deletes it, then undoes the delete | it is back on both |
| SYNC-T8 | A list A B C D on two devices | one moves D to the front (one change) while the other inserts X after B and drops C | both read D A B X |
| SYNC-T9 | A replica's state saved as JSON | it is loaded and edits continue | nothing is lost and sequence numbers carry on |
| SYNC-T10 | Two devices on one folder | one imports a score and draws on it, both sync | the other has the score, its file and the stroke, and can open, render and draw; its own edits come back |
| SYNC-T11 | A score on both | both draw and one renames it offline, then they sync | both have both strokes and the new name; nothing waits |
| SYNC-T12 | A library on disk syncing through a real folder | it is closed and reopened, then edited | the device keeps its id, the log is rebuilt, and a new device on the folder gets everything |
| SYNC-T13 | Bad params, a damaged block from another device | `sync.backend.set`, `sync.now` | refused with messages; the block is skipped and listed |

## Open questions

- How enScore resolves two devices editing the same marking; ScoreCraft's later-clock rule is
  its own.
- A folder deleted on one device while another files a score in it leaves the score pointing at
  a missing folder. `library.verify` reports it; whether the merge should file such scores at
  the root is open.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | minor | Part 3: SYNC-1 and SYNC-7 through a shared folder; SYNC-T10..T13 |
| 2026-10-31 | minor | Part 2: every record of the library, revivals for undone deletes, list positions, changes worked out from edits; SYNC-T6..T9 |
| 2026-10-31 | major | First version for M8 part 1: merge rules for markings and score details, exchange by what each side has seen |
