# Settings > System: Display, Night light, Sound, Volume mixer, Notifications,
# Focus, Power & battery, Storage, Multitasking, Clipboard and About.
import json
import os
import platform
import re
import shutil
import threading

from gi.repository import Gdk, GLib, Gtk

import wsui as ui
import ws_theme
from p_display import build_display, build_nightlight  # noqa: F401

IDLE_TIMES = [(60, "1 minute"), (120, "2 minutes"), (180, "3 minutes"), (300, "5 minutes"),
              (600, "10 minutes"), (900, "15 minutes"), (1200, "20 minutes"),
              (1800, "30 minutes"), (2700, "45 minutes"), (3600, "1 hour"),
              (7200, "2 hours"), (0, "Never")]


def restart_helper(name):
    ui.ok(["pkill", "-u", str(os.getuid()), "-f", name])


# --- Sound -----------------------------------------------------------------------

def pactl_json(*args):
    try:
        return json.loads(ui.out(["pactl", "-f", "json", *args]) or "[]")
    except ValueError:
        return []


def volume_of(dev):
    try:
        return int(next(iter(dev["volume"].values()))["value_percent"].rstrip("%"))
    except (StopIteration, KeyError, ValueError, AttributeError):
        return 0


def device_list(app, body, kind, title, icon_name):
    devs = pactl_json("list", kind + "s")
    if kind == "source":
        devs = [d for d in devs if not d["name"].endswith(".monitor")]
    default = ui.out(["pactl", f"get-default-{kind}"]).strip()
    exp, inner = ui.expander(title, next((d["description"] for d in devs if d["name"] == default),
                                         "No devices found"), icon_name, expanded=True)
    group = None
    for d in devs:
        radio = Gtk.RadioButton.new_with_label_from_widget(group, d["description"])
        group = group or radio
        radio.set_active(d["name"] == default)
        radio.connect("toggled", lambda r, n=d["name"]: r.get_active() and (
            ui.run(["pactl", f"set-default-{kind}", n]), GLib.timeout_add(300, lambda: app.refresh() and False)))
        card = ui.css(Gtk.Box(), "card")
        card.pack_start(radio, True, True, 0)
        inner.pack_start(card, False, False, 0)
    body.pack_start(exp, False, False, 0)
    dev = next((d for d in devs if d["name"] == default), None)
    if not dev:
        return
    target = "@DEFAULT_SINK@" if kind == "sink" else "@DEFAULT_SOURCE@"
    mute = Gtk.ToggleButton(active=dev["mute"], tooltip_text="Mute")
    ui.css(mute, "btn")
    mute.add(Gtk.Image.new_from_icon_name(("audio-volume-muted" if kind == "sink" else
                                           "microphone-sensitivity-muted") + "-symbolic", Gtk.IconSize.MENU))
    mute.connect("toggled", lambda b: ui.run(["pactl", f"set-{kind}-mute", target, "1" if b.get_active() else "0"]))
    body.pack_start(ui.row("Volume", None, "audio-volume-high-symbolic" if kind == "sink" else "audio-input-microphone-symbolic",
                           [mute, ui.slider(volume_of(dev), 0, 100, lambda v: ui.run(
                               ["pactl", f"set-{kind}-volume", target, f"{v}%"]), live=True)]), False, False, 0)


def build_sound(app, body):
    if not ui.which("pactl"):
        body.pack_start(ui.infobar("Sound settings need PipeWire\u2019s pactl."), False, False, 0)
        return
    body.pack_start(ui.section("Output"), False, False, 0)
    device_list(app, body, "sink", "Choose where to play sound", "audio-speakers-symbolic")
    body.pack_start(ui.section("Input"), False, False, 0)
    device_list(app, body, "source", "Choose a device for speaking or recording",
                "audio-input-microphone-symbolic")
    body.pack_start(ui.section("Advanced"), False, False, 0)


def build_mixer(app, body):
    sinks = pactl_json("list", "sinks")
    default = ui.out(["pactl", "get-default-sink"]).strip()
    dev = next((d for d in sinks if d["name"] == default), None)
    if dev:
        body.pack_start(ui.row("System", dev["description"], "audio-speakers-symbolic",
                               ui.slider(volume_of(dev), 0, 100, lambda v: ui.run(
                                   ["pactl", "set-sink-volume", "@DEFAULT_SINK@", f"{v}%"]), live=True)),
                        False, False, 0)
    body.pack_start(ui.section("Apps"), False, False, 0)
    streams = pactl_json("list", "sink-inputs")
    if not streams:
        body.pack_start(ui.label("Apps playing sound will appear here.", "caption"), False, False, 0)
    for s in streams:
        props = s.get("properties", {})
        name = props.get("application.name") or props.get("media.name") or "App"
        icon_name = props.get("application.icon_name") or "audio-x-generic"
        sid = str(s["index"])
        out_combo = ui.combo([(d["name"], d["description"]) for d in sinks],
                             next((d["name"] for d in sinks if d["index"] == s.get("sink")), None),
                             lambda n, i=sid: ui.run(["pactl", "move-sink-input", i, n]), 200)
        body.pack_start(ui.row(name, props.get("media.name"), icon_name,
                               [out_combo, ui.slider(volume_of(s), 0, 100, lambda v, i=sid: ui.run(
                                   ["pactl", "set-sink-input-volume", i, f"{v}%"]), live=True, width=160)]),
                        False, False, 0)
    timer = GLib.timeout_add_seconds(3, lambda: len(pactl_json("list", "sink-inputs")) != len(streams)
                                     and app.refresh() or True)
    app.cleanups.append(lambda: GLib.source_remove(timer))


# --- Notifications and Focus ---------------------------------------------------------

def dnd():
    return ui.out(["swaync-client", "-D"]).strip() == "true"


def build_notifications(app, body):
    if not ui.which("swaync-client"):
        body.pack_start(ui.infobar("The notification centre (swaync) is not installed."), False, False, 0)
        return
    body.pack_start(ui.row("Notifications", "Get notifications from apps and other senders",
                           "preferences-system-notifications-symbolic",
                           ui.switch(not dnd(), lambda v: ui.run(["swaync-client", "-df" if v else "-dn"]))),
                    False, False, 0)
    body.pack_start(ui.row("Do not disturb", "Notifications go straight to the notification centre",
                           "notifications-disabled-symbolic",
                           ui.switch(dnd(), lambda v: ui.run(["swaync-client", "-dn" if v else "-df"]))),
                    False, False, 0)
    body.pack_start(ui.row("Notification centre", "Super+N opens it, or select the clock on the taskbar",
                           "view-list-symbolic",
                           [ui.button("Clear all", lambda: ui.run(["ws-taskbar", "--clear-notifications"])),
                            ui.button("Open", lambda: ui.run(["ws-taskbar", "--notifications"]))]), False, False, 0)


def build_focus(app, body):
    state_file = os.path.join(ui.STATE, "focus-until")
    until = 0
    try:
        until = float(ui.read_file(state_file, "0"))
    except ValueError:
        pass
    import time
    left = until - time.time()
    hero = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8), "hero")
    if left > 0:
        hero.pack_start(ui.label(f"{int(left // 60) + 1} min", "big", xalign=0.5), False, False, 0)
        hero.pack_start(ui.label("Focus session in progress. Do not disturb is on.", "caption", xalign=0.5),
                        False, False, 0)

        def stop():
            ui.run(["systemctl", "--user", "stop", "ws-focus.timer"])
            ui.run(["swaync-client", "-df"])
            ui.write_file(state_file, "0")
            app.refresh()
        hero.pack_start(ui.button("End session", stop), False, False, 8)
    else:
        minutes = {"v": 30}
        hero.pack_start(ui.label("Start a focus session", "hero-title", xalign=0.5), False, False, 0)
        hero.pack_start(ui.label("Do not disturb turns on, and turns off when the session ends.",
                                 "caption", xalign=0.5), False, False, 0)
        c = ui.combo([(m, f"{m} minutes") for m in (5, 10, 15, 20, 30, 45, 60, 90, 120)], 30,
                     lambda m: minutes.__setitem__("v", m), 160)
        c.set_halign(Gtk.Align.CENTER)
        hero.pack_start(c, False, False, 4)

        def start():
            m = minutes["v"]
            ui.run(["swaync-client", "-dn"])
            ui.run(["systemd-run", "--user", "--unit=ws-focus", f"--on-active={m}m",
                    "sh", "-c", "swaync-client -df; notify-send -a Settings -i alarm 'Focus session ended' "
                    "'Notifications are back on.'"])
            ui.write_file(state_file, str(time.time() + m * 60))
            app.refresh()
        b = ui.button("Start focus session", start, accent=True)
        b.set_halign(Gtk.Align.CENTER)
        hero.pack_start(b, False, False, 8)
    body.pack_start(hero, False, False, 0)


# --- Power & battery ---------------------------------------------------------------

PROFILE_NAMES = {"power-saver": "Best power efficiency", "balanced": "Balanced",
                 "performance": "Best performance"}


def power_profiles():
    for name, path in (("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles"),
                       ("org.freedesktop.UPower.PowerProfiles", "/org/freedesktop/UPower/PowerProfiles")):
        p = ui.proxy(name, path, name)
        if p and ui.prop(p, "ActiveProfile"):
            return p, name
    return None, None


def build_power(app, body):
    dev = ui.proxy("org.freedesktop.UPower", "/org/freedesktop/UPower/devices/DisplayDevice",
                   "org.freedesktop.UPower.Device")
    if ui.prop(dev, "IsPresent"):
        pct = ui.prop(dev, "Percentage", 0)
        state = ui.prop(dev, "State", 0)
        hero = ui.css(Gtk.Box(spacing=16), "hero")
        hero.pack_start(ui.icon(ui.prop(dev, "IconName") or "battery", 48), False, False, 0)
        texts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
        texts.pack_start(ui.label(f"{pct:.0f}%", "big"), False, False, 0)
        if state == 1:
            t = ui.prop(dev, "TimeToFull", 0)
            note = "Charging" + (f", {t // 3600} h {t % 3600 // 60} min until full" if t else "")
        elif state == 2:
            t = ui.prop(dev, "TimeToEmpty", 0)
            note = (f"{t // 3600} h {t % 3600 // 60} min remaining" if t else "On battery")
        elif state == 4:
            note = "Fully charged"
        else:
            note = "Plugged in"
        texts.pack_start(ui.label(note, "caption"), False, False, 0)
        hero.pack_start(texts, True, True, 0)
        body.pack_start(hero, False, False, 0)

    body.pack_start(ui.section("Power"), False, False, 0)
    p, iface = power_profiles()
    if p:
        profiles = [d["Profile"] for d in ui.prop(p, "Profiles", [])]

        def set_profile(v):
            ui.call(p, "org.freedesktop.DBus.Properties.Set", "(ssv)", iface, "ActiveProfile",
                    GLib.Variant("s", v), done=lambda ok, e: ok or ui.error(app.win, "Power mode", str(e)))
        body.pack_start(ui.row("Power mode", "Optimise your device based on power use and performance",
                               "power-profile-balanced-symbolic",
                               ui.combo([(x, PROFILE_NAMES.get(x, x)) for x in profiles],
                                        ui.prop(p, "ActiveProfile"), set_profile, 220)), False, False, 0)
    s = ui.load()

    def set_idle(**kw):
        ui.save(**kw)
        restart_helper("ws-idle")
        ui.ok(["pkill", "-u", str(os.getuid()), "-x", "swayidle"])
    exp, inner = ui.expander("Screen, sleep and lock", "Choose when to lock, turn off the screen, and sleep",
                             "preferences-desktop-screensaver-symbolic", expanded=True)
    for key, title in (("lock_after", "Lock the screen after"),
                       ("screen_off_after", "Turn off my screen after"),
                       ("sleep_after", "Make my device sleep after")):
        inner.pack_start(ui.row(title, None, None,
                                ui.combo(IDLE_TIMES, s[key], lambda v, k=key: set_idle(**{k: v}), 160)),
                         False, False, 0)
    inner.pack_start(ui.row("Lock when the device goes to sleep", None, None,
                            ui.switch(s["lock_on_sleep"], lambda v: set_idle(lock_on_sleep=v))),
                     False, False, 0)
    body.pack_start(exp, False, False, 0)


# --- Storage ----------------------------------------------------------------------

def du(path):
    total = 0
    for root, dirs, files in os.walk(path, onerror=lambda e: None):
        for f in files:
            try:
                total += os.lstat(os.path.join(root, f)).st_blocks * 512
            except OSError:
                pass
    return total


def build_storage(app, body):
    usage = shutil.disk_usage("/")
    hero = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8), "hero")
    top = Gtk.Box()
    top.pack_start(ui.label("Local Disk (/)", "hero-title"), True, True, 0)
    top.pack_end(ui.label(f"{ui.human_size(usage.used)} used of {ui.human_size(usage.total)}", "caption"),
                 False, False, 0)
    hero.pack_start(top, False, False, 0)
    bar = Gtk.ProgressBar(fraction=usage.used / usage.total)
    hero.pack_start(bar, False, False, 0)
    hero.pack_start(ui.label(f"{ui.human_size(usage.free)} free", "caption"), False, False, 0)
    body.pack_start(hero, False, False, 0)

    body.pack_start(ui.section("Your files"), False, False, 0)
    folders = [("Documents", "DOCUMENTS", "folder-documents"), ("Downloads", "DOWNLOAD", "folder-download"),
               ("Pictures", "PICTURES", "folder-pictures"), ("Music", "MUSIC", "folder-music"),
               ("Videos", "VIDEOS", "folder-videos"), ("Desktop", "DESKTOP", "user-desktop")]
    rows = {}
    for name, key, icon_name in folders:
        path = ui.out(["xdg-user-dir", key]).strip()
        if not path or path == ui.HOME or not os.path.isdir(path):
            continue
        size = ui.label("Calculating\u2026", "caption")
        r = ui.row(name, path.replace(ui.HOME, "~", 1), icon_name,
                   [size, ui.button("Open", lambda p=path: ui.spawn(["xdg-open", p]))])
        rows[path] = size
        body.pack_start(r, False, False, 0)

    body.pack_start(ui.section("Temporary files"), False, False, 0)
    trash = os.path.join(ui.HOME, ".local/share/Trash")
    thumbs = os.path.join(ui.HOME, ".cache/thumbnails")
    cleanups = [("Recycle Bin", "Files you deleted from the file manager", "user-trash", trash,
                 lambda: ui.run(["gio", "trash", "--empty"], lambda *_: app.refresh())),
                ("Thumbnails", "Picture previews; they are made again when needed", "image-x-generic", thumbs,
                 lambda: (shutil.rmtree(thumbs, ignore_errors=True), app.refresh()))]
    for name, caption, icon_name, path, action in cleanups:
        size = ui.label("Calculating\u2026", "caption")
        rows[path] = size
        body.pack_start(ui.row(name, caption, icon_name,
                               [size, ui.button("Clean up", action)]), False, False, 0)

    others = []
    for line in ui.read_file("/proc/mounts").splitlines():
        dev, mnt, fs = line.split()[:3]
        if dev.startswith("/dev/") and mnt not in ("/", "/boot", "/boot/efi") and not mnt.startswith(("/var", "/snap")):
            others.append(mnt.replace("\\040", " "))
    if others:
        body.pack_start(ui.section("Other drives"), False, False, 0)
        for mnt in sorted(set(others)):
            try:
                u = shutil.disk_usage(mnt)
            except OSError:
                continue
            body.pack_start(ui.row(mnt, f"{ui.human_size(u.free)} free of {ui.human_size(u.total)}",
                                   "drive-harddisk", ui.button("Open", lambda m=mnt: ui.spawn(["xdg-open", m]))),
                            False, False, 0)

    def measure():
        for path, lab in rows.items():
            n = du(path) if os.path.exists(path) else 0
            GLib.idle_add(lambda l=lab, n=n: l.set_text(ui.human_size(n)) and False)
    threading.Thread(target=measure, daemon=True).start()


# --- Clipboard ------------------------------------------------------------------------

def build_clipboard(app, body):
    s = ui.load()

    def toggle(v):
        ui.save(clipboard_history=v)
        ui.ok(["pkill", "-u", str(os.getuid()), "-f", "cliphist store|ws-clip-watch"])
    body.pack_start(ui.row("Clipboard history",
                           "Save multiple items to the clipboard to use later. Press Super+Shift+V to "
                           "see your history and paste from it.", "edit-paste-symbolic",
                           ui.switch(s["clipboard_history"], toggle)), False, False, 0)

    def clear():
        ui.run(["cliphist", "wipe"])
        ui.run(["wl-copy", "--clear"])
        ui.notify("Clipboard cleared", "Your clipboard history was deleted.", "edit-clear")
    body.pack_start(ui.row("Clear clipboard data", "Clear everything (except pinned items)",
                           "edit-clear-symbolic", ui.button("Clear", clear)), False, False, 0)


# --- About ----------------------------------------------------------------------------

def os_release():
    data = {}
    for line in ui.read_file("/etc/os-release").splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            data[k] = v.strip('"')
    return data


def specs():
    cpu = next((l.split(":", 1)[1].strip() for l in ui.read_file("/proc/cpuinfo").splitlines()
                if l.startswith("model name")), platform.processor() or "Unknown")
    mem = next((int(l.split()[1]) for l in ui.read_file("/proc/meminfo").splitlines()
                if l.startswith("MemTotal")), 0)
    gpus = [re.sub(r"^.*?: ", "", l) for l in ui.out(["lspci"]).splitlines()
            if re.search(r"VGA|3D|Display", l)]
    dmi = lambda f: ui.read_file(f"/sys/class/dmi/id/{f}").strip()
    total = shutil.disk_usage("/").total
    return [("Device name", ui.out(["hostname"]).strip()),
            ("Manufacturer", dmi("sys_vendor")),
            ("Model", " ".join(filter(None, (dmi("product_family"), dmi("product_name"))))),
            ("Processor", cpu),
            ("Installed RAM", f"{mem / 1048576:.1f} GB"),
            ("Graphics", "; ".join(gpus) or "Unknown"),
            ("Storage", f"{ui.human_size(total)} (system disk)"),
            ("System type", f"64-bit operating system, {platform.machine()} processor"
             if platform.architecture()[0] == "64bit" else platform.machine())]


BRAND = "Mackes Desktop Environment (MDE)"
SUBTITLE = "Rust Wayland desktop for Fedora"
TAGLINE = "Build to match commodity desktop concepts."
VERSION_FULL = "2026.0"
DESKTOP_STACK = "Wayland · mde-comp (Smithay) · mde-shell (iced)"
FONT_BRANDING = "UI · Selawik · Mono · Cascadia Mono"
LICENSES = [
    ("MDE platform (mde-comp, mde-shell, mde-control, …)", "MIT"),
    ("smithay-drm-extras", "MIT"),
    ("Mackes-Carbon / Mackes-Carbon-Light icons", "Apache-2.0"),
    ("Selawik UI font", "SIL OFL 1.1"),
    ("Cascadia Mono", "SIL OFL 1.1"),
    ("iced, Smithay, and other Rust crates", "per-crate (see Cargo)"),
    ("Base OS packages", "Fedora project licenses"),
]


def brand_logo(size):
    """The Mackes mark, shared with the Start button and the installer."""
    from gi.repository import GdkPixbuf, Gtk
    roots = ("/usr/share/mde/brand",
             os.path.normpath(os.path.join(os.path.dirname(__file__), "..", "..", "assets", "brand")))
    for root in roots:
        path = os.path.join(root, "mackes.svg")
        if not os.path.isfile(path):
            continue
        try:
            pix = GdkPixbuf.Pixbuf.new_from_file_at_scale(path, size, size, True)
        except GLib.Error:
            continue
        image = Gtk.Image.new_from_pixbuf(pix)
        image.set_pixel_size(size)
        return image
    return None


def os_specs():
    r = os_release()
    base = r.get("PRETTY_NAME") or " ".join(filter(None, (r.get("NAME"), r.get("VERSION")))) or "Fedora"
    installed = ui.out(["rpm", "-q", "--qf", "%{INSTALLTIME:day}", "filesystem"]).strip()
    return [("Edition", BRAND), ("Version", VERSION_FULL),
            ("Base OS", base),
            ("Installed on", installed), ("Kernel", platform.release()),
            ("Desktop", DESKTOP_STACK),
            ("Typography", FONT_BRANDING),
            ("Default accent", "#0078d4"),
            ("Subtitle", SUBTITLE),
            ("Tagline", TAGLINE)]


def build_about(app, body):
    host = ui.proxy("org.freedesktop.hostname1", "/org/freedesktop/hostname1", "org.freedesktop.hostname1")
    name = ui.prop(host, "PrettyHostname") or ui.prop(host, "StaticHostname") or ui.out(["hostname"]).strip()
    hero = ui.css(Gtk.Box(spacing=16), "hero")
    hero.pack_start(brand_logo(64) or ui.icon("computer", 48), False, False, 0)
    t = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
    t.pack_start(ui.label(BRAND, "hero-title"), False, False, 0)
    t.pack_start(ui.label(SUBTITLE, "caption"), False, False, 0)
    t.pack_start(ui.label(TAGLINE, "caption"), False, False, 0)
    t.pack_start(ui.label(name, "caption"), False, False, 0)
    dmi = ui.read_file("/sys/class/dmi/id/product_family").strip() or ui.read_file("/sys/class/dmi/id/product_name").strip()
    if dmi:
        t.pack_start(ui.label(dmi, "caption"), False, False, 0)
    hero.pack_start(t, True, True, 0)

    def rename():
        d = ui.Dialog(app.win, "Rename your PC", "Next")
        d.add_text("You can use letters, numbers and hyphens.")
        e = d.add_field("Current PC name: " + name, ui.entry("", "New name", 360))

        def go():
            new = e.get_text().strip()
            static = re.sub(r"[^A-Za-z0-9-]", "-", new).strip("-")[:63]
            if not static:
                return
            d.destroy()
            ui.call(host, "SetStaticHostname", "(sb)", static.lower(), True,
                    done=lambda ok, err: ok and ui.call(host, "SetPrettyHostname", "(sb)", new, True,
                                                         done=lambda *_: app.refresh())
                    or (not ok and ui.error(app.win, "Couldn\u2019t rename this PC", str(err))))
        d.on_primary(go)
        d.run_dialog()
    hero.pack_end(ui.button("Rename this PC", rename), False, False, 0)
    body.pack_start(hero, False, False, 0)

    for title, items, icon_name in (("Device specifications", specs(), "computer-symbolic"),
                                    ("MDE specifications", os_specs(), "distributor-logo-symbolic"),
                                    ("Open source licenses", LICENSES, "text-x-generic-symbolic")):
        exp, inner = ui.expander(title, None, icon_name, expanded=True)
        text = "\n".join(f"{k}\t{v}" for k, v in items)
        copy = ui.button("Copy", lambda t=text: Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).set_text(t, -1))
        exp.get_children()[0].get_child().pack_end(copy, False, False, 8)
        grid = Gtk.Grid(column_spacing=48, row_spacing=8)
        grid.set_margin_start(56)
        grid.set_margin_top(12)
        grid.set_margin_bottom(16)
        grid.set_margin_end(16)
        for i, (k, v) in enumerate(items):
            grid.attach(ui.label(k, "caption"), 0, i, 1, 1)
            grid.attach(ui.label(v or "Unknown", wrap=True, selectable=True), 1, i, 1, 1)
        inner.pack_start(grid, False, False, 0)
        body.pack_start(exp, False, False, 0)


def build_system(app, body):
    host = ui.out(["hostname"]).strip()
    hero = ui.css(Gtk.Box(spacing=16), "hero")
    hero.pack_start(ui.icon("computer", 48), False, False, 0)
    t = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
    t.pack_start(ui.label(host, "hero-title"), False, False, 0)
    t.pack_start(ui.label(BRAND, "caption"), False, False, 0)
    hero.pack_start(t, True, True, 0)
    hero.pack_end(ui.link("Rename", lambda: app.go("about")), False, False, 0)
    body.pack_start(hero, False, False, 8)


def build_home(app, body):
    import p_personal
    import p_update
    p_personal.build_personalization(app, body)
    pending = len(p_update.load_state().get("packages", [])) + len(p_update.load_state().get("flatpaks", []))
    body.pack_start(ui.section("Recommended settings"), False, False, 0)
    for key, caption in (("update", f"{pending} updates available" if pending else "You\u2019re up to date"),
                         ("display", "Brightness, scale and night light"),
                         ("sound", "Output and input devices"),
                         ("power", "Power mode, screen and sleep"),
                         ("colors", "Light and dark mode, accent colour")):
        page = app.by_key[key]
        body.pack_start(ui.nav_row(page.title, caption, page.icon, lambda k=key: app.go(k)), False, False, 0)
    usage = shutil.disk_usage("/")
    body.pack_start(ui.section("Storage"), False, False, 0)
    card = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8), "card", "tall")
    card.pack_start(ui.label(f"{ui.human_size(usage.used)} of {ui.human_size(usage.total)} used"), False, False, 0)
    card.pack_start(Gtk.ProgressBar(fraction=usage.used / usage.total), False, False, 0)
    card.pack_start(ui.link("Storage settings", lambda: app.go("storage")), False, False, 0)
    body.pack_start(card, False, False, 0)


CATEGORY = {"system": build_system, "home": build_home}

PAGES = [
    dict(key="display", title="Display", parent="system", icon="video-display",
         caption="Monitors, brightness, night light, display profile",
         keywords="screen monitor resolution scale refresh rate orientation rotate brightness",
         build=build_display, aliases=("screen", "displays")),
    dict(key="nightlight", title="Night light", parent="display", icon="night-light",
         caption="Warmer colours at night", keywords="blue light warm red evening sunset",
         build=build_nightlight, aliases=("night-light", "blue-light"), inline=True),
    dict(key="sound", title="Sound", parent="system", icon="audio-volume-high",
         caption="Volume levels, output, input, sound devices",
         keywords="audio speaker headphones microphone volume", build=build_sound),
    dict(key="mixer", title="Volume mixer", parent="sound", icon="multimedia-volume-control",
         caption="App volume mix, app input and output devices",
         keywords="app volume per-app", build=build_mixer, aliases=("apps-volume",)),
    dict(key="notifications", title="Notifications", parent="system", icon="preferences-system-notifications",
         caption="Alerts from apps and system, do not disturb",
         keywords="alerts toast do not disturb dnd banner", build=build_notifications),
    dict(key="focus", title="Focus", parent="system", icon="alarm",
         caption="Focus sessions, distraction settings", keywords="timer concentrate dnd",
         build=build_focus, aliases=("focusassist", "quiethours")),
    dict(key="power", title="Power & battery", parent="system", icon="battery",
         caption="Sleep, battery usage, power mode",
         keywords="battery sleep screen timeout power plan mode lock idle suspend",
         build=build_power, aliases=("powersleep", "batterysaver", "power-plan")),
    dict(key="storage", title="Storage", parent="system", icon="drive-harddisk",
         caption="Storage space, drives, temporary files",
         keywords="disk space free trash cleanup drive", build=build_storage, aliases=("storagesense",)),
    dict(key="clipboard", title="Clipboard", parent="system", icon="edit-paste",
         caption="Cut and copy history", keywords="copy paste history", build=build_clipboard),
    dict(key="about", title="About", parent="system", icon="help-about",
         caption="MDE branding, device specs, open source licenses, rename PC",
         keywords="pc name hostname specs processor ram version kernel license fedora wayland",
         build=build_about),
]
