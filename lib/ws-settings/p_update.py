# Settings > Windows Update, through PackageKit (polkit asks before
# installing) and Flatpak. "ws-settings --check-updates" runs background_check
# from a user timer: it refreshes the list, notifies, and updates the panel.
import json
import os
import re
import subprocess
import time
from datetime import datetime, timedelta

STATE = os.path.join(os.environ.get("XDG_STATE_HOME", os.path.expanduser("~/.local/state")), "ws-settings")
STATE_FILE = os.path.join(STATE, "updates.json")
UPDATE_PENDING = "/system-update"
CORE = re.compile(r"^(kernel|kernel-core|kernel-modules.*|glibc|systemd|systemd-libs|dbus|dbus-broker|"
                  r"mesa-.*|sway|wlroots.*|linux-firmware|grub2-.*|shim-.*|labwc|mde-.*)$")
PAUSES = [(0, "Not paused"), (7, "Pause for 1 week"), (14, "Pause for 2 weeks"), (21, "Pause for 3 weeks"),
          (35, "Pause for 5 weeks")]
TIMER = """[Unit]
Description=Check for updates (Settings > Windows Update)

[Timer]
OnBootSec=10min
OnUnitActiveSec=6h
Persistent=true

[Install]
WantedBy=timers.target
"""
SERVICE = """[Unit]
Description=Check for updates (Settings > Windows Update)

[Service]
Type=oneshot
ExecStart=/usr/bin/ws-settings --check-updates
"""


def load_state():
    try:
        with open(STATE_FILE) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def save_state(state):
    os.makedirs(STATE, exist_ok=True)
    tmp = STATE_FILE + ".tmp"
    with open(tmp, "w") as f:
        json.dump(state, f, indent=2)
    os.replace(tmp, STATE_FILE)


def set_busy(status):
    st = load_state()
    st["busy"] = True
    st["busy_status"] = status
    save_state(st)


def clear_busy(**extra):
    st = load_state()
    st["busy"] = False
    st["busy_status"] = ""
    st.update(extra)
    save_state(st)


def is_busy(state=None):
    state = state if state is not None else load_state()
    return bool(state.get("busy"))


def updates_pending_restart():
    return os.path.lexists(UPDATE_PENDING)


def offline_trigger_cmd(mode):
    """Prepare PackageKit offline update; always reboot into the offline install.
    mode is 'reboot' or 'power-off' (action after updates finish)."""
    return ("gdbus call --system --dest org.freedesktop.PackageKit "
            "--object-path /org/freedesktop/PackageKit "
            "--method org.freedesktop.PackageKit.Offline.Trigger %s >/dev/null && "
            "systemctl reboot" % mode)


def run_update_and(mode):
    """mode: 'reboot' or 'power-off'."""
    subprocess.Popen(["sh", "-c", offline_trigger_cmd(mode)], start_new_session=True,
                     stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def power_update_items(busy_blocked=True):
    """Update and restart / Update and shut down when offline updates are prepared.
    Returns [(label, callback), ...] for Start / lock / greeter / Win+X."""
    if busy_blocked and is_busy():
        return [("Updates in progress\u2026", lambda: None)]
    if not updates_pending_restart():
        return []
    return [
        ("Update and restart", lambda: run_update_and("reboot")),
        ("Update and shut down", lambda: run_update_and("power-off")),
    ]


def parse_updates(text):
    packages, results = [], False
    for line in text.splitlines():
        if line.startswith("Results:"):
            results = True
            continue
        if not results or "\t" not in line:
            continue
        kind, _t, rest = line.partition("\t")
        nevra = rest.strip().split(" ")[0]
        m = re.match(r"(.+)-([^-]+)-([^-]+)\.([^.]+)$", nevra)
        if m:
            packages.append({"name": m[1], "version": f"{m[2]}-{m[3]}", "arch": m[4],
                             "kind": kind.strip() or "Update", "id": nevra})
    return packages


def flatpak_updates():
    try:
        out = subprocess.run(["flatpak", "remote-ls", "--updates", "--columns=application,version,name"],
                             capture_output=True, text=True, timeout=300).stdout
    except (OSError, subprocess.TimeoutExpired):
        return []
    rows = []
    for line in out.splitlines():
        parts = line.split("\t")
        if parts and parts[0]:
            rows.append({"name": parts[2] if len(parts) > 2 and parts[2] else parts[0], "id": parts[0],
                         "version": parts[1] if len(parts) > 1 else "", "kind": "Flatpak"})
    return rows


def check(refresh=True):
    """Refresh and list updates (blocking). Returns the new state."""
    if refresh:
        subprocess.run(["pkcon", "refresh", "--plain"], capture_output=True, timeout=900)
    out = subprocess.run(["pkcon", "get-updates", "--plain"], capture_output=True, text=True, timeout=900)
    text = out.stdout or ""
    if "Results:" not in text and out.returncode != 0:
        err = (out.stderr or text or "PackageKit reported an error.").strip()
        raise RuntimeError(err)
    state = load_state()
    state.update(checked=time.time(), packages=parse_updates(text), flatpaks=flatpak_updates())
    save_state(state)
    return state


def settings():
    path = os.path.join(os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config")),
                        "ws-settings", "settings.json")
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def background_check():
    s = settings()
    if s.get("update_paused_until", 0) > time.time() or s.get("update_auto_check") is False:
        return 0
    before = {p["id"] for p in load_state().get("packages", [])}
    try:
        state = check()
    except (OSError, subprocess.TimeoutExpired, RuntimeError):
        return 1
    new = [p for p in state["packages"] if p["id"] not in before]
    total = len(state["packages"]) + len(state["flatpaks"])
    if new and s.get("update_notify", True):
        security = sum(1 for p in state["packages"] if "ecurity" in p["kind"])
        body = f"{total} updates are ready to install" + (f", including {security} security updates" if security else "")
        subprocess.run(["notify-send", "-a", "Windows Update", "-i", "system-software-update",
                        "Updates are available", body], capture_output=True)
    return 0


def schedule_hint():
    """Active-hours stand-in from Power & battery sleep / screen timings."""
    s = settings()
    sleep_after = int(s.get("sleep_after") or 0)
    screen_off = int(s.get("screen_off_after") or 0)
    if sleep_after:
        mins = sleep_after // 60
        return f"Power & battery sleeps after {mins} min idle — schedule outside busy use."
    if screen_off:
        mins = screen_off // 60
        return f"Screen turns off after {mins} min idle (Power & battery)."
    return "Tip: set sleep timing in System › Power & battery for quieter restarts."


def next_slot(day_label, hour, minute):
    """day_label: 'today' or 'tomorrow'. Returns unix timestamp."""
    now = datetime.now()
    base = now if day_label == "today" else now + timedelta(days=1)
    when = base.replace(hour=hour, minute=minute, second=0, microsecond=0)
    if when <= now:
        when += timedelta(days=1)
    return when.timestamp()


# --- Page ---------------------------------------------------------------------------------------------

def build_update(app, body):
    from gi.repository import GLib, Gtk
    import wsui as ui

    state = load_state()
    s = ui.load()
    paused = s.get("update_paused_until", 0) > time.time()
    prepared = updates_pending_restart()
    packages, flats = state.get("packages", []), state.get("flatpaks", [])
    total = len(packages) + len(flats)
    busy = is_busy(state)
    scheduled = state.get("scheduled_restart")

    hero = ui.css(Gtk.Box(spacing=16), "hero")
    icon_name = "system-software-update" if total or prepared or busy else "emblem-default"
    hero.pack_start(ui.icon(icon_name, 48), False, False, 0)
    col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER, spacing=2)
    if busy and state.get("busy_status"):
        title, caption = "Working on updates", state["busy_status"]
    elif prepared:
        title, caption = "Restart required", "Updates will be installed when your PC restarts"
        if scheduled and scheduled > time.time():
            caption += "\nScheduled: " + time.strftime("%A, %-I:%M %p", time.localtime(scheduled))
    elif paused:
        until = time.strftime("%A, %B %-d", time.localtime(s["update_paused_until"]))
        title, caption = "Updates are paused", f"Updates will resume on {until}"
    elif total:
        title, caption = "Updates available", f"{total} updates ready to download and install"
    else:
        title, caption = "You\u2019re up to date", ""
    checked = state.get("checked")
    if checked and not busy:
        caption = (caption + "\n" if caption else "") + "Last checked: " + \
            time.strftime("%A, %-I:%M %p", time.localtime(checked))
    col.pack_start(ui.label(title, "hero-title"), False, False, 0)
    status = ui.label(caption, "caption", wrap=True)
    col.pack_start(status, False, False, 0)
    bar = Gtk.ProgressBar(no_show_all=True)
    if busy:
        bar.show()
        bar.pulse()
    col.pack_start(bar, False, False, 4)
    hero.pack_start(col, True, True, 0)
    buttons = Gtk.Box(spacing=8, valign=Gtk.Align.CENTER)
    hero.pack_end(buttons, False, False, 0)
    body.pack_start(hero, False, False, 0)

    def busy_ui(text):
        for b in buttons.get_children():
            b.set_sensitive(False)
        status.set_text(text)
        set_busy(text)
        bar.show()
        bar.set_fraction(0)

    def progress(line):
        m = re.match(r"Percentage:\s*(\d+)", line)
        if m:
            bar.set_fraction(int(m[1]) / 100)
        m = re.match(r"Status:\s*(.+)", line)
        if m:
            text = m[1].strip().capitalize() + "\u2026"
            status.set_text(text)
            set_busy(text)

    def restart_now():
        ui.run(["systemctl", "reboot"])

    def update_and_shut_down():
        busy_ui("Preparing to install when you shut down\u2026")
        ui.run(["sh", "-c", offline_trigger_cmd("power-off")],
               lambda ok, _o, err: (clear_busy(),
                                    ok or ui.error(app.win, "Couldn\u2019t prepare updates",
                                                   err.strip() or "PackageKit reported an error."),
                                    app.refresh()))

    def cancel_update():
        status.set_text("Cancelling update\u2026")
        set_busy("Cancelling update\u2026")
        for b in buttons.get_children():
            b.set_sensitive(False)

        def done(ok, _o, err):
            clear_busy(scheduled_restart=0)
            if not ok:
                ui.error(app.win, "Couldn\u2019t cancel the update",
                         err.strip() or "PackageKit reported an error.")
            app.refresh()
        ui.run(["pkcon", "offline-cancel"], done)

    def schedule_restart():
        d = ui.Dialog(app.win, "Schedule restart", "Schedule", "Cancel", width=420)
        d.add_text("Pick a time today or tomorrow. " + schedule_hint())
        day = {"v": "today"}
        hm = {"h": 3, "m": 0}
        d.add_field("Day", ui.combo([("today", "Today"), ("tomorrow", "Tomorrow")], "today",
                                    lambda v: day.__setitem__("v", v), 200))
        times = [((h, m), f"{h % 12 or 12}:{m:02d} {'AM' if h < 12 else 'PM'}")
                 for h in range(0, 24) for m in (0, 30)]
        d.add_field("Time", ui.combo(times, (3, 0),
                                     lambda v: (hm.__setitem__("h", v[0]), hm.__setitem__("m", v[1])), 200))

        def go():
            when = next_slot(day["v"], hm["h"], hm["m"])
            d.destroy()
            st = load_state()
            st["scheduled_restart"] = when
            save_state(st)
            delay = max(1, int(when - time.time()))
            ui.run(["systemd-run", "--user", "--unit=ws-update-scheduled-reboot",
                    f"--on-active={delay}s", "systemctl", "reboot"],
                   lambda ok, _o, err: (
                       ok or ui.error(app.win, "Couldn\u2019t schedule restart",
                                      err.strip() or "systemd-run failed."),
                       app.refresh()))
            ui.notify("Restart scheduled",
                      time.strftime("%A, %-I:%M %p", time.localtime(when)),
                      "system-software-update")
        d.on_primary(go)
        d.run_dialog()

    def offer_restart_dialog():
        d = ui.Dialog(app.win, "Restart required", "Restart now", "Later",
                      width=460, tertiary="Update and shut down")
        d.add_text("Updates are ready. Restart or shut down to finish installing them.")
        d.on_primary(lambda: (d.destroy(), restart_now()))
        d.on_tertiary(lambda: (d.destroy(), update_and_shut_down()))
        d.run_dialog()

    def check_now():
        busy_ui("Checking for updates\u2026")
        bar.pulse()
        pulse = GLib.timeout_add(150, lambda: bar.pulse() or True)

        def work():
            err = None
            try:
                check()
            except (OSError, subprocess.TimeoutExpired, RuntimeError) as e:
                err = str(e)
            finally:
                def done():
                    GLib.source_remove(pulse)
                    clear_busy()
                    if err:
                        status.set_text("We couldn\u2019t check for updates")
                        ui.error(app.win, "We couldn\u2019t check for updates", err)
                        for b in buttons.get_children():
                            b.set_sensitive(True)
                        bar.hide()
                    else:
                        app.refresh()
                    return False
                GLib.idle_add(done)
        import threading
        threading.Thread(target=work, daemon=True).start()

    def install_all():
        names = {p["name"] for p in packages}
        needs_restart = any(CORE.match(n) for n in names)
        busy_ui("Downloading updates\u2026")

        def flatpaks_then_done(*_):
            if flats and ui.which("flatpak"):
                text = "Updating apps from Flathub\u2026"
                status.set_text(text)
                set_busy(text)
                bar.pulse()
                ui.run(["flatpak", "update", "-y", "--noninteractive"], finish, lines=progress)
            else:
                finish(True, "", "")

        def finish(ok, _o, err):
            if ok:
                clear_busy(packages=[], flatpaks=[], installed=time.time())
            else:
                clear_busy()
                ui.error(app.win, "Some updates couldn\u2019t be installed",
                         err.strip() or "PackageKit reported an error.")
                app.refresh()
                return
            app.refresh()
            if needs_restart or updates_pending_restart():
                offer_restart_dialog()

        def downloaded(ok, _o, err):
            if not ok:
                clear_busy()
                ui.error(app.win, "Couldn\u2019t download updates",
                         err.strip() or "PackageKit reported an error.")
                app.refresh()
                return
            if needs_restart:
                text = "Preparing to install when you restart\u2026"
                status.set_text(text)
                set_busy(text)
                ui.run(["pkcon", "offline-trigger"], flatpaks_then_done)
            else:
                busy_ui("Installing updates\u2026")
                ui.run(["pkcon", "update", "-y", "--plain"], flatpaks_then_done, lines=progress)
        if packages:
            ui.run(["pkcon", "update", "--only-download", "-y", "--plain"], downloaded, lines=progress)
        else:
            flatpaks_then_done()

    if prepared:
        buttons.pack_start(ui.button("Restart now", restart_now, accent=True), False, False, 0)
        buttons.pack_start(ui.button("Update and shut down", update_and_shut_down), False, False, 0)
        buttons.pack_start(ui.button("Schedule restart", schedule_restart), False, False, 0)
        buttons.pack_start(ui.button("Cancel update", cancel_update), False, False, 0)
    elif total and not paused:
        buttons.pack_start(ui.button("Download & install all", install_all, accent=True), False, False, 0)
        buttons.pack_start(ui.button("Check for updates", check_now), False, False, 0)
    else:
        # Up to date, or paused: Check remains available while paused (install stays blocked).
        buttons.pack_start(ui.button("Check for updates", check_now, accent=True), False, False, 0)
    if busy:
        for b in buttons.get_children():
            b.set_sensitive(False)

    if total and not prepared:
        exp, inner = ui.expander(f"{total} updates", ", ".join(sorted({p['kind'] for p in packages + flats})),
                                 "view-list-symbolic")
        for p in sorted(packages, key=lambda p: (("ecurity" not in p["kind"]), p["name"]))[:200] + flats:
            core = " \u00b7 restart required" if CORE.match(p["name"]) else ""
            inner.pack_start(ui.row(p["name"], f"{p['kind']} \u00b7 {p['version']}{core}", None), False, False, 0)
        body.pack_start(exp, False, False, 0)

    body.pack_start(ui.section("More options"), False, False, 0)

    def pause(days):
        ui.save(update_paused_until=time.time() + days * 86400 if days else 0)
        app.refresh()
    body.pack_start(ui.row("Pause updates", None, "media-playback-pause-symbolic",
                           ui.combo(PAUSES, 0 if not paused else None, pause, 200)), False, False, 0)
    exp, inner = ui.expander("Advanced options", "Automatic checks, notifications", "preferences-other-symbolic")

    def auto(v):
        ui.save(update_auto_check=v)
        set_timer(v)
    inner.pack_start(ui.row("Check for updates automatically", "Every 6 hours, in the background", None,
                            ui.switch(s["update_auto_check"], auto)), False, False, 0)
    inner.pack_start(ui.row("Notify me when updates are ready", None, None,
                            ui.switch(s["update_notify"], lambda v: ui.save(update_notify=v))), False, False, 0)
    inner.pack_start(ui.label(schedule_hint(), "caption", wrap=True), False, False, 4)
    body.pack_start(exp, False, False, 0)


def set_timer(on):
    import wsui as ui
    units = os.path.join(ui.CONFIG, "systemd", "user")
    ui.write_file(os.path.join(units, "ws-update-check.timer"), TIMER)
    ui.write_file(os.path.join(units, "ws-update-check.service"), SERVICE)
    ui.run(["sh", "-c", "systemctl --user daemon-reload && systemctl --user "
            + ("enable --now" if on else "disable --now") + " ws-update-check.timer"])


def build_history(app, body):
    import wsui as ui
    out = ui.out(["dnf", "history", "list"], timeout=20)
    rows = []
    for line in out.splitlines()[1:]:
        m = re.match(r"\s*(\d+)\s+(.*?)\s+(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d)\s+(\S+)\s+(\d+)\s*$", line)
        if m:
            rows.append(m.groups())
    flat_hist = []
    try:
        fo = subprocess.run(["flatpak", "history", "--columns=time,change,application"],
                            capture_output=True, text=True, timeout=30).stdout
        for line in fo.splitlines()[1:]:
            parts = line.split("\t")
            if len(parts) >= 3 and parts[1].lower() in ("update", "upgrade", "install"):
                flat_hist.append(parts)
    except (OSError, subprocess.TimeoutExpired):
        pass

    if not rows and not flat_hist:
        body.pack_start(ui.label("No update history yet.", "caption"), False, False, 0)
        return
    names = {"I": "Installed", "U": "Updated", "E": "Removed", "D": "Downgraded", "R": "Reinstalled",
             "O": "Replaced", "C": "Changed"}
    groups = [("Quality updates", lambda c, a: "upgrade" in c or "update" in c or "pkcon" in c or a == "U"),
              ("App and package changes", lambda c, a: True)]
    used = set()
    for title, test in groups:
        picked = [r for r in rows if r[0] not in used and test(r[1], r[3])]
        if not picked:
            continue
        exp, inner = ui.expander(title, f"{len(picked)} entries", "view-list-symbolic", expanded=title == groups[0][0])
        for tid, cmd, when, action, count in picked[:60]:
            used.add(tid)
            verbs = ", ".join(names.get(a, a) for a in action.split(",")) if "," in action else \
                names.get(action[:1], action)
            inner.pack_start(ui.row(f"{verbs} {count} package{'s' if count != '1' else ''}",
                                    f"{when} \u00b7 {cmd}", None), False, False, 0)
        body.pack_start(exp, False, False, 0)

    if flat_hist:
        exp, inner = ui.expander("Flatpak / app updates", f"{len(flat_hist)} entries",
                                 "application-x-addon-symbolic", expanded=False)
        for parts in flat_hist[:60]:
            when, change, app_id = parts[0], parts[1], parts[2]
            inner.pack_start(ui.row(f"{change.capitalize()} {app_id}", when, None), False, False, 0)
        body.pack_start(exp, False, False, 0)


CATEGORY = {"update": build_update}

PAGES = [
    dict(key="updatehistory", title="Update history", parent="update", icon="document-open-recent",
         caption="Updates, installs and removals on this PC", keywords="history log installed updates dnf flatpak",
         build=build_history, aliases=("windowsupdate-history",)),
]
