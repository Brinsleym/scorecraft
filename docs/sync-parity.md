# Sync and backup parity

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-10-31 · **Change:** patch (SY-1, SY-4 partial: sync through a shared folder; earlier: SY-2, SY-3 partial: the merge core; earlier: owner's sync design: Google Drive first, device-to-device over Wi-Fi, download everything; earlier: SY-12 folder mirror; first version) · **Target:** enScore 3.7 (2026)

enScore sells Sync as a subscription and requires an account, and also offers a free two-way folder mirror with the computer or OneDrive (S8). ScoreCraft's sync is free and
has two parts (owner's design, 2026-10-28): **Google Drive sync** of the whole library, app
configuration and annotations, then **device-to-device sync** over Wi-Fi and through Google Drive,
with optional sign-in. Every device downloads everything ahead of time, so nothing waits on the
network during a performance. Google Drive comes first; OneDrive and Dropbox later.
Design: [`architecture.md` §9](architecture.md#9-sync).

## Google Drive (part 1, first)

**Summary (estimated):** 0% ready.

| Id | Target | Rows | Status |
|---|---|---|---|
| SY-1 | Scores (blobs), markings, metadata, lists and bookmarks sync across devices | SYNC-1 | partial: through a shared folder (`sync.backend.set`, `sync.now`): scores and their files, markings, details, pages, layers, bookmarks, folders, lists, Recently Deleted and settings; no background sync yet |
| SY-2 | Offline-first: every edit is local first; sync is background and resumable | SYNC-3 | partial: each device applies and logs its own changes, exchange by what each side has seen (`scorecraft-sync`); no background transport yet |
| SY-3 | Concurrent offline markings on two devices merge without loss | SYNC-6 | partial: markings and score details merge ([spec](spec/sync.md)); three-replica convergence property test |
| SY-13 | Google Drive backend: the whole library (score files and folders), app configuration and annotations sync to the user's own Drive, opt-in with their Google account | SYNC-1, SYNC-7 | missing |
| SY-7 | Optional end-to-end encryption | ours | missing |
| SY-8 | Download everything: a new or returning device fetches every score, annotation and setting in the background before it is needed; nothing is fetched when a score opens | SYNC-5 | missing |
| SY-9 | Backup to a single archive and restore, local or remote | SYNC-2 | missing |
| SY-10 | Removing an account or backend removes remote copies, keeps local ones | SYNC-4 | missing |
| SY-11 | Sync never interrupts performance mode | ours | missing |

## Between devices (part 2)

| Id | Target | Rows | Status |
|---|---|---|---|
| SY-14 | Devices on the same Wi-Fi find each other, pair once and sync library, configuration and annotations directly, across platforms | SYNC-1 | missing |
| SY-15 | Devices on different networks sync through Google Drive: one uploads, the others download | SYNC-1 | missing |
| SY-16 | Optional sign-in links a user's devices; sync works without it (pairing) | SYNC-1, SYNC-4 | missing |

## Later backends

| Id | Target | Rows | Status |
|---|---|---|---|
| SY-17 | OneDrive and Dropbox, behind the same backend trait as Google Drive | SYNC-7 | missing |
| SY-12 | Two-way mirror between a library folder and a folder on the computer or in OneDrive, free and account-free as in enScore | SYNC-8 | missing |
| SY-4 | Folder backend (iCloud Drive, Syncthing, any folder the OS syncs) | SYNC-7 | partial: each device writes only new files (change blocks, score files by content), so a cloud tool never sees a conflict; works today through Google Drive for desktop, OneDrive, Dropbox or iCloud Drive |
| SY-5 | WebDAV backend (Nextcloud and others) | SYNC-7 | missing |
| SY-6 | Self-hostable `scorecraft-sync-server` with accounts the operator controls | SYNC-1 | missing |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | patch | SY-1 and SY-4 partial: the shared-folder backend in the engine (M8 part 3) |
| 2026-10-31 | patch | SY-2 and SY-3 partial: the sync merge core for markings and score details (M8 part 1) |
| 2026-10-28 | minor | Owner's sync design: Google Drive first (SY-13), device-to-device over Wi-Fi and via Drive with optional sign-in (SY-14..16), download everything (SY-8 rewritten), OneDrive/Dropbox, folders, WebDAV and the server moved to later backends (SY-17, SY-4..6, SY-12) |
| 2026-10-28 | minor | SY-12 folder mirror (SYNC-8) from the Microsoft Store description |
| 2026-10-28 | major | First version |
