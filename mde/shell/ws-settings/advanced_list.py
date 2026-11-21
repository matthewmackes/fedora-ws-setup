# Apps that belong in Settings > Advanced.
# Automatic tools are the settings and system apps Advanced already lists.
# "Moved from Start" is the tag file written by Start's "Move to Advanced".
# Start hides both sets from All apps, pins, Recommended, Recently added and
# Most used. Start search still finds them. Other menus are left alone.
import os
import shutil
import subprocess

from gi.repository import Gio, GLib

HOME = os.path.expanduser("~")
CONFIG = os.environ.get("XDG_CONFIG_HOME", os.path.join(HOME, ".config"))
MOVED_FILE = os.path.join(CONFIG, "ws-settings", "moved-to-advanced")
START_PINS = os.path.join(CONFIG, "ws-start", "pins")
TASKBAR_PINS = os.path.join(CONFIG, "ws-taskbar", "pins")

GROUPS = [("Hardware and sound", {"HardwareSettings", "Audio", "Printing"}),
          ("Network and internet", {"Network", "Dialup", "RemoteAccess", "X-GNOME-NetworkSettings"}),
          ("Security", {"Security"}),
          ("Appearance", {"DesktopSettings", "Accessibility"}),
          ("System and maintenance", {"System", "Monitor", "PackageManager", "Filesystem"})]
SKIP = {"ws-settings", "ws-shortcuts", "ws-display-save"}
# Hidden from app menus but useful in Advanced.
ALWAYS = {"nm-connection-editor", "blueman-adapters", "org.pulseaudio.pavucontrol", "pavucontrol",
          "system-config-printer", "firewall-config", "lightdm-gtk-greeter-settings"}


def short_id(desktop_id):
    if desktop_id.endswith(".desktop"):
        return desktop_id[:-8]
    return desktop_id


def load_moved():
    """Desktop ids the user sent to Advanced, in the order they were added."""
    try:
        with open(MOVED_FILE) as f:
            raw = [line.strip() for line in f if line.strip() and not line.startswith("#")]
    except OSError:
        return []
    seen = set()
    ids = []
    for desktop_id in raw:
        if desktop_id not in seen:
            seen.add(desktop_id)
            ids.append(desktop_id)
    return ids


def save_moved(ids):
    os.makedirs(os.path.dirname(MOVED_FILE), exist_ok=True)
    tmp = MOVED_FILE + ".tmp"
    with open(tmp, "w") as f:
        f.write("".join(desktop_id + "\n" for desktop_id in ids))
    os.replace(tmp, MOVED_FILE)


def add_moved(desktop_id):
    if not desktop_id:
        return
    ids = load_moved()
    if desktop_id not in ids:
        ids.append(desktop_id)
        save_moved(ids)


def remove_moved(desktop_id):
    key = short_id(desktop_id)
    save_moved([i for i in load_moved() if i != desktop_id and short_id(i) != key])


def desktop_app(desktop_id):
    name = desktop_id if desktop_id.endswith(".desktop") else desktop_id + ".desktop"
    try:
        return Gio.DesktopAppInfo.new(name)
    except (TypeError, GLib.Error):
        return None


def cockpit_entry():
    kf = GLib.KeyFile()
    data = ("[Desktop Entry]\nType=Application\nName=Cockpit\n"
            "Comment=Services, logs, storage and accounts in the browser\n"
            "Exec=xdg-open http://localhost:9090\nIcon=computer\n")
    kf.load_from_data(data, len(data.encode()), GLib.KeyFileFlags.NONE)
    return kf


def automatic_entries():
    """Settings and system tools Advanced lists on its own, as (app, categories)."""
    found = []
    for app in Gio.AppInfo.get_all():
        if not isinstance(app, Gio.DesktopAppInfo):
            continue
        cats = set((app.get_categories() or "").split(";")) - {""}
        app_id = short_id(app.get_id() or "")
        if app_id in SKIP or not ({"Settings", "System"} & cats or app_id in ALWAYS):
            continue
        if app.get_nodisplay() and app_id not in ALWAYS:
            continue
        if app.get_boolean("Terminal"):
            continue
        found.append((app, cats))
    if shutil.which("cockpit-bridge") or os.path.exists("/usr/lib/systemd/system/cockpit.socket"):
        found.append((Gio.DesktopAppInfo.new_from_keyfile(cockpit_entry()), {"System"}))
    return sorted(found, key=lambda item: item[0].get_display_name().casefold())


def id_sets():
    """Full desktop ids and ids without the .desktop suffix."""
    full, short = set(), set()
    for app, _cats in automatic_entries():
        desktop_id = app.get_id() or ""
        if desktop_id:
            full.add(desktop_id)
            short.add(short_id(desktop_id))
    for desktop_id in load_moved():
        full.add(desktop_id)
        short.add(short_id(desktop_id))
    return full, short


def is_advanced(app, ids=None):
    full, short = ids if ids is not None else id_sets()
    desktop_id = app.get_id() or ""
    return bool(desktop_id) and (desktop_id in full or short_id(desktop_id) in short)


def is_blocked_desktop_id(desktop_id, ids=None):
    if not desktop_id:
        return False
    full, short = ids if ids is not None else id_sets()
    return desktop_id in full or short_id(desktop_id) in short


def moved_apps():
    apps = []
    for desktop_id in load_moved():
        app = desktop_app(desktop_id)
        if app is not None:
            apps.append(app)
    apps.sort(key=lambda app: app.get_display_name().casefold())
    return apps


def filter_named_pins(pins, ids=None):
    full, short = ids if ids is not None else id_sets()
    return [pin for pin in pins if pin not in full and short_id(pin) not in short]


def _write_lines(path, lines):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        f.write("".join(line + "\n" for line in lines))
    os.replace(tmp, path)


def release_pins(start_pins):
    """Drop Advanced apps from Start pins and the taskbar pin file.

    Returns the Start pins to keep. Reloads a running taskbar when its pin
    file changes. Pins are not put back when an app leaves Advanced.
    """
    ids = id_sets()
    kept = filter_named_pins(start_pins, ids)
    if kept != list(start_pins):
        _write_lines(START_PINS, kept)
    try:
        with open(TASKBAR_PINS) as f:
            taskbar = [line.strip() for line in f if line.strip() and not line.startswith("#")]
    except OSError:
        taskbar = None
    if taskbar is not None:
        kept_taskbar = filter_named_pins(taskbar, ids)
        if kept_taskbar != taskbar:
            _write_lines(TASKBAR_PINS, kept_taskbar)
            subprocess.Popen(["ws-taskbar", "--reload"],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return kept
