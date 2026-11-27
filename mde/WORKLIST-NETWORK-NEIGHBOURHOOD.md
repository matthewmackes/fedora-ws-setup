# Network Neighbourhood: worklist

Source: 30-question review. Nothing here is implemented yet; awaiting go-ahead.

## Diagnosed root causes (verified on this machine)

1. **The daemon panics on every Mount/Cancel/Unmount/Discover call (the real "hang").** `dbus/mod.rs` used `tokio::task::spawn_blocking` inside zbus handlers. zbus runs on its own executor, so it panicked ("no reactor running", 43 times in the journal) and the D-Bus call never replied. No progress file was ever written. **Fixed in M1** (`run_blocking`); needs install to take effect.
2. **The crate was not in the workspace and no longer compiled** (`mde_theme::Theme` was removed). The installed Nov 16 binaries predate that. **Fixed in M1** (added to `members`, `style.rs` uses `Palette`).
3. **Failures were invisible.** Errors were one redacted line; the journal held only discovery lines. **Fixed in M1** (`diagnose.rs`, tracing in connect steps and the sshfs command line).

Corrections to my first review: the password sshfs path (`sshpass`, installed) and the media FUSE (`--mediafs` in the daemon, flat library listing) already exist. M5 is therefore *restructure to the By Artist/Album/Genre/Recently Added views*, not write from scratch. Mount root is `/mnt/network`, not `/mnt/neighbourhood`; keep it and add `~/Network` symlinks.

## Decisions

| Area | Decision |
|---|---|
| Auth | First connect: username and password. Then offer (confirm prompt) to set up passwordless keys. |
| Keys | One key per place under `/etc/network-neighbourhood/`, root-owned. |
| Host keys | Auto-trust new (`accept-new`). Changed key fails with a clear error and a one-click "Trust new key". |
| Errors | Plain cause, suggested fix, expandable raw details, Copy log. Auto-fix actions where possible (install sshfs, trust key, re-enter password). Cover network, auth and missing-software/permission errors equally. |
| Logging | systemd journal only. Most verbose: every step plus raw command lines (passwords redacted). |
| Connecting dialog | Step checklist (ticks, spinner on current), progress bar, collapsed live log pane, always-visible Cancel. Success closes and opens the folder in Files. Failure stays open with Retry / Edit place / Copy log / auto-fix. |
| Media | Real read-only FUSE mount. Top-level views: By Artist, By Album, By Genre, Recently Added. Write the helper in Rust. |
| Paths | `/mnt/neighbourhood/<name>` plus a symlink/bookmark in `~/Network`. |
| Mounting | On-demand automount (idle unmount); per-place choice, default on demand. |
| Offline places | Greyed row, "Offline" badge, last-seen time, background retry, notify on return. |
| Wizard | Step-by-step with a built-in Test connection step reusing the same progress UI. |
| Privilege | Polkit, remembered for the session. |
| History | "Last connection" detail per place and an Activity page of recent attempts. |
| Look | MDE-theme restyle and card-based main window (status, protocol, last seen). |

## Milestones

### M1: Make SSH mounting work, and make failures visible
- [x] Fix daemon panic (`run_blocking` replaces `tokio::task::spawn_blocking` in all 7 handlers).
- [x] Crate builds again and is a workspace member.
- [x] Error classification (`diagnose.rs`, 6 tests): network / auth / software / host key, with cause, fix and auto-fix.
- [x] Journal logging of steps, failures and the redacted sshfs command line.
- [ ] Install and verify a real SSH mount end to end (needs `sudo`; see below).

### M2: Connecting dialog and error UX
- [ ] Dialog: checklist, progress bar, collapsed live log, Cancel.
- [ ] Success: open the folder in Files. Failure: Retry / Edit place / Copy log / auto-fix.
- [ ] Host-key-changed flow with "Trust new key".
- [ ] Polkit session caching.

### M3: Key setup wizard
- [ ] After first successful password connect, prompt "Set up passwordless access?".
- [ ] Generate a per-place key in `/etc/network-neighbourhood/`, install on host, switch place to key auth, verify.
- [ ] Failure and rollback handling.

### M4: Wizard, status and history
- [ ] Add Place wizard with Test connection step.
- [ ] Offline detection, last-seen, background retry, notification.
- [ ] Per-place "Last connection" and Activity page.
- [ ] Automount default and per-place choice; `~/Network` symlinks.

### M5: Media FUSE helper
- [ ] Build and install `neighbourhood-mediafs` (Rust, read-only).
- [ ] Jellyfin and Airsonic/Subsonic backends; By Artist / Album / Genre / Recently Added views.
- [ ] Packaging in `data/network-neighbourhood/install.sh`.

### M6: Redesign
- [ ] Restyle to MDE theme; card layout for places.

## Open items to confirm when starting
- Which SSH host and which Jellyfin/Airsonic server to test against.
- Whether M1 and M2 ship together (Q30 chose plan-first, so order is open).
