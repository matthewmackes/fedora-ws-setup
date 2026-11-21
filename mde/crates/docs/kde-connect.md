# KDE Connect on MDE (W446)

MDE uses upstream **kdeconnect-kde** over D-Bus (`org.kde.kdeconnect`), with `kdeconnect-cli` as fallback. Do **not** fork the daemon.

## Fedora packages

```bash
sudo dnf install kde-connect
# provides kdeconnectd, kdeconnect-cli, and D-Bus services
```

Optional UI from KDE (not required for MDE):

```bash
sudo dnf install kde-connectd  # if split on your release
```

Verify:

```bash
rpm -q kde-connect
which kdeconnectd kdeconnect-cli
busctl --user status org.kde.kdeconnect 2>/dev/null || true
```

## Firewall

KDE Connect needs UDP/TCP **1716–1764** (typical) between devices on the LAN.

Firewalld example:

```bash
sudo firewall-cmd --permanent --add-service=kdeconnect
# or:
sudo firewall-cmd --permanent --add-port=1716-1764/udp
sudo firewall-cmd --permanent --add-port=1716-1764/tcp
sudo firewall-cmd --reload
```

If `kdeconnect` service definition is missing, use the port range above.

## Session policy (20-Q)

- **No auto-install** of packages from MDE.
- **On-demand start**: MDE starts `kdeconnectd` when Nearby / Share opens if the binary exists; leaves it running.
- If missing: Nearby shows an honest empty state — install `kde-connect` and check firewall (no fake devices).

## Manual start

```bash
kdeconnectd &
# or
systemctl --user start kdeconnectd.service   # when the unit is installed
```

## Pairing

Pair from **Settings → Network / Nearby** (full UI) or from the QS Nearby flyout when an unpaired device appears. Phone apps: KDE Connect (Android/iOS).

## Phone Link–class surface (A078)

Settings → Nearby Share and QS Nearby expose:

- Pair / unpair, clipboard share, file/URL share
- Per-device battery when the battery plugin is on
- Notification-mirror readiness (enable Notifications on the phone)
- SMS plugin probe (`requestAllConversations` via D-Bus)

## Gaps

- Seat without `kde-connect` RPM: MDE cannot share until the user installs it (first-run: `sudo dnf install kde-connect`).
- Corporate firewalls blocking 1716–1764 will prevent discovery.
- Full plugin surface (SMS compose UI, remote input, etc.) still depends on upstream plugins being enabled in kdeconnectd.
