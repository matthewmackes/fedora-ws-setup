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
- [x] Modal dialog: step checklist, progress bar, collapsed Details log, Cancel (`ui/connect_dialog.rs`).
- [x] Success closes the dialog and opens the folder in Files. Failure keeps it open with cause, fix, Retry / Edit place / Copy log.
- [x] Auto-fixes: install missing package (pkexec dnf), trust new host key (pkexec ssh-keygen -R), re-enter password.
- [~] Polkit session caching: relies on the existing `auth_admin_keep` action; not re-verified.

### M3: Key setup wizard
- [x] After a successful password connect to an SSH place, prompt "Set up passwordless access?" (not repeated after "Not now" in a session).
- [x] `keys.rs`: ed25519 key per place in `/etc/network-neighbourhood/creds/<id>.key`, installed with ssh-copy-id, verified with BatchMode, rolled back on failure. D-Bus `HasSshKey` / `SetupSshKey`.

### M4: Wizard, status and history
- [x] Add Place now connects through the Connecting dialog after save (was a silent background thread printing to stderr). Default policy is Automount (per-place choice kept).
- [~] "Test connection" is the Save & connect step, not a separate pre-save probe.
- [x] Daemon probes reachability every 30 s (`seen.json`); card shows Offline badge, dimmed, "last seen"; UI notifies when a place returns.
- [x] History (`history.jsonl`, 200 cap), D-Bus `GetHistory`; "Activity" window in the command bar; existing inline last-connection status kept on each card.
- [x] `~/Network/<name>` symlinks created after a successful connect.

### M5: Media FUSE
- [x] Subsonic and Jellyfin backends return flat entries; `media/views.rs` builds By Artist / By Album / By Genre / Recently Added (video: All / By Genre / Recently Added). 12 unit tests.
- [x] Read-only. Helper failures and timeouts are now reported instead of always "succeeding".
- [ ] Not exercised against a real Jellyfin/Airsonic server.

### M6: Redesign
- [x] Colours from the MDE `Palette`; 12 px card radius; status stripe, protocol chip, last-seen line.
- [ ] Not visually reviewed (launches clean; no screenshot taken).

## Verification status
30 unit tests pass (`cargo test -p mde-network-neighbourhood`). Nothing has run against the installed system daemon: install needs root (`sudo bash mde/data/network-neighbourhood/install.sh`).
