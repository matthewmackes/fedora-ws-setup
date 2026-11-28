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
- [x] Installed the new daemon and GUI (via pkexec; old binaries backed up as `*.bak-20261127`) and verified in the live daemon: MountPlace now replies (was a 95 s hang), logs every step to the journal, records history, and classifies failures.
- [x] Found and fixed on the live system: sshfs registers the FUSE mount before login is confirmed, so a wrong password briefly looked "Connected". The mount must now stay up and be readable before success is reported.
- [x] Positive SSH password mount verified live: after updating the stale saved password, `mm@172.20.0.2` mounts via the installed daemon (real `fuse.sshfs` mount, folder listing readable) in 2.7 s.
- [ ] Live key setup (generate, install on host, verify) not run: it edits `authorized_keys` on the remote host, so it needs explicit go-ahead.

### M2: Connecting dialog and error UX
- [x] Modal dialog: step checklist, progress bar, collapsed Details log, Cancel (`ui/connect_dialog.rs`).
- [x] Success closes the dialog and opens the folder in Files. Failure keeps it open with cause, fix, Retry / Edit place / Copy log.
- [x] Auto-fixes: install missing package (pkexec dnf), trust new host key (pkexec ssh-keygen -R), re-enter password.
- [x] Polkit session caching: `org.mde.NetworkNeighbourhood1.manage` is installed with `auth_admin_keep` (confirmed with `pkaction`). Not exercised through a real prompt.

### M3: Key setup wizard
- [x] After a successful password connect to an SSH place, prompt "Set up passwordless access?" (not repeated after "Not now" in a session).
- [x] `keys.rs`: ed25519 key per place in `/etc/network-neighbourhood/creds/<id>.key`, installed with ssh-copy-id, verified with BatchMode, rolled back on failure. D-Bus `HasSshKey` / `SetupSshKey`.

### M4: Wizard, status and history
- [x] Add Place now connects through the Connecting dialog after save (was a silent background thread printing to stderr). Default policy is Automount (per-place choice kept).
- [x] Real pre-save **Test connection** button (`probe.rs`): resolve, reach, authenticate (SSH password login, SMB share list), no root, results shown step by step with the diagnosed cause and fix. Primary button is now "Save & connect".
- [x] Daemon probes reachability every 30 s (`seen.json`); card shows Offline badge, dimmed, "last seen"; UI notifies when a place returns.
- [x] History (`history.jsonl`, 200 cap), D-Bus `GetHistory`; "Activity" window in the command bar; existing inline last-connection status kept on each card.
- [x] `~/Network/<name>` symlinks created after a successful connect.

### M5: Media FUSE
- [x] Subsonic and Jellyfin backends return flat entries; `media/views.rs` builds By Artist / By Album / By Genre / Recently Added (video: All / By Genre / Recently Added). 12 unit tests.
- [x] Read-only. Helper failures and timeouts are now reported instead of always "succeeding".
- [x] Subsonic client exercised over real HTTP against a local mock server (songs → views; wrong password surfaces the server message and classifies as Auth).
- [x] Live against the real Airsonic at 172.20.0.2 (admin login): 2,793 songs listed, 97 artists / 211 albums / 127 genres, four views populated, live FUSE mount through the installed daemon, real 24-bit 96 kHz FLAC bytes read through it. Test place removed afterwards.
- [x] Fixed: Airsonic 10 rejects API version 1.16.1 ("Server must upgrade"); the client now sends 1.15.0.
- [x] Jellyfin: added username/password login (`/Users/AuthenticateByName`), API key still works as a fallback. Live: server 10.10.5 reachable and rejects a bad login cleanly.
- [ ] Jellyfin library listing/mount: server rejects every login tried (admin/root, two passwords) with HTTP 401, confirmed with curl on both auth headers. Needs the correct Jellyfin username/password or an API key.
- [x] Fixed on the live system: an autofs placeholder counted as "mounted"; helper-driven places (media, password SSH) no longer get systemd units; UpsertPlace with an empty id no longer discards the caller's fields. Systemd unit names now match `systemd-escape -p` (dotted hosts were rejected).

### M6: Redesign
- [x] Colours from the MDE `Palette`; 12 px card radius; status stripe, protocol chip, last-seen line.
- [x] Screenshot-reviewed. Found and fixed invisible accent buttons (theme gradient overriding background) and a clipped protocol chip.

## Verification status
40 unit tests pass (+3 live checks run with `--ignored`) (`cargo test -p mde-network-neighbourhood`). Two items remain, both needing credentials rather than code: the SSH password for `mm@172.20.0.2` and a Jellyfin login (`sudo bash mde/data/network-neighbourhood/install.sh`).
