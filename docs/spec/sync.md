# Sync

> **Last reviewed:** 2026-10-31 · **Last updated:** 2026-10-31 · **Change:** major (first version) · **Target:** enScore 3.7 (2026)

How two or more devices that change the same library while apart end up with the same library.
Written for milestone M8, from the owner's design ([architecture §9](../architecture.md#9-sync)).
This first version covers the merge rules and the changes devices exchange; transport (Google
Drive, Wi-Fi peers) and blobs come in later parts.

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

### SYNC-6 Merging concurrent edits (part 1: markings and details)

- Every change carries a unique id (device, sequence) and a hybrid logical clock. Receiving a
  change twice, or changes in any order, gives the same library: merging is idempotent,
  commutative and associative.
- **Markings** (ANNO-*) are an add-wins set with tombstones. A marking added on one device and
  edited or added on another is kept; a marking deleted anywhere is gone everywhere and never
  comes back, even if an edit of it arrives later. Two devices marking the same page offline
  both keep their strokes.
- **Editing a marking** (move, recolour, text) is a last-writer-wins register keyed by the
  clock: of two concurrent edits, the later clock wins on every device; ties break by device.
- **Order on a page** is by marking id (time-ordered), so every device draws them in the same
  order.
- **Score details** (title, composer, tags, …, LIB-4) are a last-writer-wins register per score.
- A change for a score a device doesn't have yet waits until that score arrives, then applies.
- Pages, folders, lists, bookmarks and settings follow in part 2 with the same rules (lists with
  a sequence CRDT for order).

### SYNC-3 Offline first

- A device applies its own changes at once and keeps them in its log; nothing waits on the
  network. Exchanging is "send what the other hasn't seen": each side names the changes it has
  (by device, the highest sequence received), and gets the rest.

## Acceptance tests

| Id | Given | When | Then |
|---|---|---|---|
| SYNC-T1 | Two replicas of one library | each draws a stroke on page 1 offline, then they exchange | both have both strokes, in the same order |
| SYNC-T2 | A stroke on both replicas | one deletes it while the other recolours it, then they exchange | it is gone on both, and stays gone when the recolour arrives again |
| SYNC-T3 | A score on both replicas | both rename it offline, then they exchange | both have the name with the later clock |
| SYNC-T4 | Three replicas | random edits and random partial exchanges, then everyone exchanges with everyone | all three libraries are equal (property test) |
| SYNC-T5 | A change received twice or out of order | it is applied | the library is the same as receiving it once, in order |

## Open questions

- How enScore resolves two devices editing the same marking; ScoreCraft's later-clock rule is
  its own.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | major | First version for M8 part 1: merge rules for markings and score details, exchange by what each side has seen |
