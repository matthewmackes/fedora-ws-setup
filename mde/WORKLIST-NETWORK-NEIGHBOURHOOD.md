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
- [x] Live key setup verified on `mm@172.20.0.2`: generate, install, verify, then remount through systemd with the key only (no sshpass). Found and fixed on the real host: its `authorized_keys` was group-writable, so sshd's StrictModes ignored the key. Setup now runs `chmod go-w ~ ~/.ssh ~/.ssh/authorized_keys`, and a failed attempt removes its own public key from the host (it previously left it behind). Debug keys added during testing were removed from the host and the file diffed against a backup.

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
- [x] Jellyfin verified live (root login): 2,800 items, 22 genres, All / By Genre / Recently Added views, live FUSE mount, real MP4 bytes read. First successful login saves the access token (`<id>.jftoken`, root-only 0600, deleted with the place); later connections use only the token and fall back to the password if it is revoked. Journal showed "signed in; access token saved" then "connected with saved access token". Fixed: stream URLs now carry the token (file reads previously failed with EIO); helper logs now reach the journal. Test place removed.
- [x] Fixed on the live system: an autofs placeholder counted as "mounted"; helper-driven places (media, password SSH) no longer get systemd units; UpsertPlace with an empty id no longer discards the caller's fields. Systemd unit names now match `systemd-escape -p` (dotted hosts were rejected).

### M6: Redesign
- [x] Colours from the MDE `Palette`; 12 px card radius; status stripe, protocol chip, last-seen line.
- [x] Screenshot-reviewed. Found and fixed invisible accent buttons (theme gradient overriding background) and a clipped protocol chip.

## Verification status
43 unit tests pass (+4 live checks run with `--ignored`) (`cargo test -p mde-network-neighbourhood`). Two items remain, both needing credentials rather than code: the SSH password for `mm@172.20.0.2` and a Jellyfin login (`sudo bash mde/data/network-neighbourhood/install.sh`).

## Follow-up: guided "Add place" (done)
- [x] Step-by-step flow (`ui/guided.rs`, logic and tests in `guide.rs`): Type, Where, Sign in, What to open, Check and save. Steps adapt to the type (NFS skips sign-in, media skips the folder step).
- [x] Standard ports assumed (SMB 445, SSH 22, NFS 2049, WebDAV 443, Airsonic 4040, Jellyfin 8096); a different port is under "Use a different port", or typed as `host:port`.
- [x] Hosts found on the network are offered per type; SMB shares are listed for picking.
- [x] Real pre-save check for every type, including a Jellyfin/Airsonic sign-in. Walked through on screen against the live Airsonic server.
- [x] "Paste a URI instead…" keeps the old single-form editor; Browse network now starts the guided flow at the sign-in step.

## Housekeeping notes
- [x] Two stray places created by test automation (`172.20.0.21 (SSH)`, `adminadmin`) were removed via the daemon; only the user's `ssh:172.20.0.2` place remains.
- [x] Audit at close: 0 unchecked, 0 partial, 34 done; 51 tests pass; installed daemon and GUI byte-identical to the build of HEAD; daemon active and answering.
- Not removed (needs root, harmless): a stale `/etc/systemd/system/mnt-network-SSH-172\x2e20\x2e0\x2e2-ssh_172\x2e20\x2e0\x2e2.mount` left from before the unit-name fix. Remove with `sudo rm` plus `systemctl daemon-reload` when convenient.

## Fix: mounts "timed out" for any place name containing a space
- [x] Cause: `/proc/mounts` writes a space as `\040`, so the daemon never matched a successful mount to its path, then killed it and reported a timeout (SSH after 45 s, media after 60 s). The guided flow's default names ("host (SSH)", "host (Video)") all contain a space, so most new places hit it.
- [x] Fix: `/proc/mounts` octal escapes are decoded before comparing (`unescape_mount_field`), with a regression test. Reproduced live first (mount up at 8 s, reported as a 45 s timeout), then verified fixed live: SSH 2.8 s and Video 5.0 s with spaced names. Test places removed.

## Feature: media details exposed to file browsers (done)
- [x] Every detail the server gives is served as extended attributes on each Audio/Video file: `user.media.*` (title, artist, album, year, track, disc, duration, bitrate, sample rate, format, play count, server path, ids; for video also resolution, codecs, frame rate, channels, audio/subtitle languages, container, ratings, overview, tagline, genres, studios, IMDb/TMDb ids, series/season/episode) plus the freedesktop `user.xdg.comment|tags|rating|origin.url` that KDE and other file managers already read. Pure extraction in `media/meta.rs`, 8 unit tests.
- [x] Real file dates (server date-added) and sizes instead of "now".
- [x] Cast, director and writers are fetched on demand (full listing would be about 4x slower: 17 MB / 4.7 s vs 33 MB / 20 s) and cached per file.
- [x] Episodes are named `Series - S01E02 - Title` instead of colliding names like "Pilot".
- [x] Read-only. No token or password appears in any attribute (tested, and checked across 80 live files).
- [x] Verified live with `getfattr -d -m -`, `ls -l` and `stat` on Airsonic and Jellyfin (22 attributes per song, 40+ per movie). 60 unit tests pass.
- Not included: cover art / posters as image files (possible follow-up).

## Fix: Video (and any) place showed "Empty" in Thunar after connecting
- [x] Symptom: after Connect, opening the place in Thunar showed "0 items" although `ls`, `gio list` and the same folder via a symlink all listed it. Repeated after every daemon restart.
- [x] Cause (reproduced, then verified): the long-running `Thunar --daemon` had already shown the mount point as an empty directory while nothing was mounted. A mount that later covers that directory sends it no inotify event, so Thunar keeps the stale empty listing. A path Thunar has never viewed shows fine.
- [x] Fix: the daemon takes a handle on the mount-point directory before mounting and touches the original directory after the mount succeeds (`StaleViewNudge`), which makes file managers re-read it. Verified on the real Video place: Thunar open on the unmounted folder (empty) fills in by itself on Connect. Unit test added.
- [x] Also (hardening found along the way): folder timestamps are stable (mount time) instead of "now" on every lookup, real link counts and a normal `statfs`, and the media mount registers as `fuse.neighbourhood-mediafs`. These did not change the symptom.
- [x] Dev aid: `network-neighbourhoodd --mediafs-demo <dir> [options]` mounts a synthetic library without root or network, for testing file-manager behaviour.

## Feature: Audio and Video open in VLC's file browser (done)
- [x] Connect / Open on an Audio or Video place now opens VLC (falls back to the file manager if VLC is not installed). No playlist is used: VLC is started empty so its own browser is used.
- [x] The mounted library is published as `~/Videos/<name>` (Video) or `~/Music/<name>` (Audio), so it appears as a folder under VLC's **My Videos** / **My Music**. Verified on the live Jellyfin place: My Videos lists the place as a folder.
- [x] VLC keeps its browser sidebar in the playlist panel and remembers it as hidden, so the app turns that panel on in VLC's config (only while VLC is not running; VLC rewrites the file on exit). Tested with `ini_set` unit tests.
- Limit: VLC has no option to preselect a sidebar entry and does not remember one, so it opens with the sidebar visible and My Videos / My Music one click away.

## Fix: updates no longer drop helper-held mounts (done)
- [x] Cause: the daemon's systemd unit used the default `KillMode=control-group`, so restarting it (e.g. to install an update) killed the helper processes behind password-SSH and Audio/Video mounts, leaving those places looking connected but empty.
- [x] Fix: `KillMode=process` in `data/network-neighbourhood/network-neighbourhoodd.service`, installed and reloaded. Verified live: restarting the daemon left the Video mount up (helper alive, folder listing intact).
- Note: a helper that outlives the daemon is not supervised by the new daemon process; the mount stays usable and can be unmounted from the app as before.
