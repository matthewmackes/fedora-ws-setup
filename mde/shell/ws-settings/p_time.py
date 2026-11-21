# Settings > Time & language: Date & time, Language & region, and Typing.
# System-wide changes go through timedate1 and locale1 (polkit asks first).
import os
import re
from datetime import datetime
from zoneinfo import ZoneInfo

from gi.repository import GLib, Gtk

import wsui as ui
import ws_theme

XKB_APPLY = "/usr/libexec/sway-systemd/locale1-xkb-config"
REGION_FILE = os.path.join(ui.CONFIG, "environment.d", "60-ws-region.conf")
REGION_VARS = ["LC_TIME", "LC_NUMERIC", "LC_MONETARY", "LC_MEASUREMENT", "LC_PAPER"]
LANGPACKS = [("ar", "Arabic"), ("zh_CN", "Chinese (Simplified)"), ("zh_TW", "Chinese (Traditional)"),
             ("cs", "Czech"), ("da", "Danish"), ("nl", "Dutch"), ("fi", "Finnish"), ("fr", "French"),
             ("de", "German"), ("el", "Greek"), ("he", "Hebrew"), ("hi", "Hindi"), ("hu", "Hungarian"),
             ("id", "Indonesian"), ("it", "Italian"), ("ja", "Japanese"), ("ko", "Korean"),
             ("nb", "Norwegian"), ("pl", "Polish"), ("pt", "Portuguese"), ("pt_BR", "Portuguese (Brazil)"),
             ("ro", "Romanian"), ("ru", "Russian"), ("es", "Spanish"), ("sv", "Swedish"), ("th", "Thai"),
             ("tr", "Turkish"), ("uk", "Ukrainian"), ("vi", "Vietnamese")]
CAPS = [("", "Caps Lock"), ("ctrl:nocaps", "Ctrl"), ("caps:escape", "Esc"), ("caps:none", "Off")]
SWITCH = [("grp:win_space_toggle", "Super+Space"), ("grp:alt_shift_toggle", "Alt+Shift"),
          ("grp:ctrl_shift_toggle", "Ctrl+Shift"), ("", "No shortcut")]


def timedate():
    return ui.proxy("org.freedesktop.timedate1", "/org/freedesktop/timedate1", "org.freedesktop.timedate1")


def locale1():
    return ui.proxy("org.freedesktop.locale1", "/org/freedesktop/locale1", "org.freedesktop.locale1")


def utc_label(tz):
    try:
        off = datetime.now(ZoneInfo(tz)).utcoffset()
    except Exception:
        return tz, 0
    mins = int(off.total_seconds() // 60)
    sign = "+" if mins >= 0 else "-"
    return f"(UTC{sign}{abs(mins) // 60:02d}:{abs(mins) % 60:02d}) {tz.replace('_', ' ')}", mins


def clock_format():
    return "%H:%M" if ui.load().get("clock_24h") else "%-I:%M %p"


def build_datetime(app, body):
    td = timedate()
    hero = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2), "hero")
    now = ui.label("", "big")
    day = ui.label("", "caption")
    hero.pack_start(now, False, False, 0)
    hero.pack_start(day, False, False, 0)
    body.pack_start(hero, False, False, 0)

    def tick():
        t = datetime.now()
        now.set_text(t.strftime(clock_format()))
        day.set_text(t.strftime("%A, %B %-d, %Y"))
        return True
    tick()
    timer = GLib.timeout_add_seconds(1, tick)
    app.cleanups.append(lambda: GLib.source_remove(timer))

    ntp = ui.prop(td, "NTP", False)
    body.pack_start(ui.row("Set time automatically", None, "preferences-system-time-symbolic",
                           ui.switch(ntp, lambda v: ui.call(td, "SetNTP", "(bb)", v, True,
                                                            done=lambda *_: app.refresh()),
                                     sensitive=ui.prop(td, "CanNTP", True))), False, False, 0)

    def manual():
        d = ui.Dialog(app.win, "Change date and time", "Change")
        cal = Gtk.Calendar()
        d.body.pack_start(cal, False, False, 0)
        t = datetime.now()
        hour = Gtk.SpinButton.new_with_range(0, 23, 1)
        minute = Gtk.SpinButton.new_with_range(0, 59, 1)
        hour.set_value(t.hour)
        minute.set_value(t.minute)
        box = Gtk.Box(spacing=8)
        box.pack_start(hour, False, False, 0)
        box.pack_start(ui.label(":"), False, False, 0)
        box.pack_start(minute, False, False, 0)
        d.add_field("Time (24-hour)", box)

        def go():
            y, m, dd = cal.get_date()
            new = datetime(y, m + 1, dd, hour.get_value_as_int(), minute.get_value_as_int())
            usec = int(new.timestamp() * 1_000_000)
            d.destroy()
            ui.call(td, "SetTime", "(xbb)", usec, False, True,
                    done=lambda ok, e: ok or ui.error(app.win, "Couldn\u2019t change the time", str(e)))
        d.on_primary(go)
        d.run_dialog()
    body.pack_start(ui.row("Set the date and time manually", None, "x-office-calendar-symbolic",
                           ui.button("Change", manual, sensitive=not ntp)), False, False, 0)

    zones = ui.out(["timedatectl", "list-timezones"]).split()
    labelled = sorted(((z, *utc_label(z)) for z in zones if "/" in z), key=lambda x: (x[2], x[1]))
    current = ui.prop(td, "Timezone", "UTC")
    options = [(z, lab) for z, lab, _m in labelled]
    body.pack_start(ui.row("Time zone", None, "preferences-system-time-symbolic",
                           ui.combo(options, current, lambda z: ui.call(td, "SetTimezone", "(sb)", z, True,
                                                                        done=lambda *_: app.refresh()), 320)),
                    False, False, 0)
    body.pack_start(ui.section("Additional settings"), False, False, 0)
    tracking = ui.out(["chronyc", "-n", "tracking"]) if ui.which("chronyc") else ""
    server = re.search(r"Reference ID\s*:\s*\S+ \(([^)]+)\)", tracking)
    ref = re.search(r"Ref time \(UTC\)\s*:\s*(.+)", tracking)
    body.pack_start(ui.row("Time server", f"{server[1] if server else 'Not synchronised'}"
                           + (f"  \u00b7  Last synced {ref[1].strip()} UTC" if ref else ""),
                           "network-server-symbolic"), False, False, 0)
    body.pack_start(ui.row("Show the time with a 24-hour clock", "On the taskbar and in Settings", "clock-symbolic",
                           ui.switch(ui.load().get("clock_24h", False), lambda v: (
                               ui.save(clock_24h=v), ws_theme.apply_taskbar(), ws_theme.apply_greeter()))),
                    False, False, 0)


def locales():
    return [l for l in ui.out(["localectl", "list-locales"]).split() if l != "C.UTF-8"]


def locale_name(code):
    names = {"en_US": "English (United States)", "en_GB": "English (United Kingdom)",
             "en_AU": "English (Australia)", "en_CA": "English (Canada)", "en_IE": "English (Ireland)",
             "en_NZ": "English (New Zealand)", "en_IN": "English (India)", "en_ZA": "English (South Africa)",
             "de_DE": "Deutsch (Deutschland)", "fr_FR": "Fran\u00e7ais (France)", "es_ES": "Espa\u00f1ol (Espa\u00f1a)"}
    base = code.split(".")[0]
    return names.get(base, base.replace("_", " (") + (")" if "_" in base else ""))


def region():
    values = {}
    for line in ui.read_file(REGION_FILE).splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            values[k] = v
    return values.get("LC_TIME")


def xkb_layouts():
    l1 = locale1()
    layouts = [x for x in (ui.prop(l1, "X11Layout", "us") or "us").split(",") if x]
    variants = (ui.prop(l1, "X11Variant", "") or "").split(",")
    variants += [""] * (len(layouts) - len(variants))
    options = [o for o in (ui.prop(l1, "X11Options", "") or "").split(",") if o]
    return l1, list(zip(layouts, variants)), options


def xkb_names():
    names, section = {}, None
    for line in ui.read_file("/usr/share/X11/xkb/rules/evdev.lst").splitlines():
        if line.startswith("!"):
            section = line[1:].strip()
        elif line.strip() and section == "layout":
            k, _s, v = line.strip().partition(" ")
            names[k] = v.strip()
    return names


def set_keyboard(app, pairs, options):
    l1 = locale1()
    layout = ",".join(p[0] for p in pairs)
    variant = ",".join(p[1] for p in pairs) if any(p[1] for p in pairs) else ""

    def done(ok, err):
        if ok and os.path.exists(XKB_APPLY):
            ui.run([XKB_APPLY, "--oneshot"])
        if not ok:
            ui.error(app.win, "Couldn\u2019t change the keyboard", str(err))
        GLib.timeout_add(500, lambda: app.refresh() and False)
    ui.call(l1, "SetX11Keyboard", "(ssssbb)", layout, ui.prop(l1, "X11Model", "") or "pc105", variant,
            ",".join(options), True, True, done=done)


def build_language(app, body):
    l1 = locale1()
    lang = next((x.split("=", 1)[1] for x in ui.prop(l1, "Locale", []) if x.startswith("LANG=")), "en_US.UTF-8")
    body.pack_start(ui.section("Language"), False, False, 0)
    installed = locales()

    def set_lang(code):
        def done(ok, err):
            if ok:
                ui.notify("Sign out to change the display language", "", "preferences-desktop-locale")
            else:
                ui.error(app.win, "Couldn\u2019t change the language", str(err))
        ui.call(l1, "SetLocale", "(asb)", [f"LANG={code}"], True, done=done)
    body.pack_start(ui.row("Windows display language", "Settings, Start and apps appear in this language "
                           "after you sign out", "preferences-desktop-locale-symbolic",
                           ui.combo([(c, locale_name(c)) for c in installed], lang, set_lang, 260)), False, False, 0)

    def add_language():
        d = ui.Dialog(app.win, "Choose a language to install", "Install")
        choice = {"v": None}
        d.add_field("Language", ui.combo(LANGPACKS, None, lambda v: choice.__setitem__("v", v), 360))

        def go():
            if choice["v"]:
                d.destroy()
                ui.install(app, f"langpacks-{choice['v']}")
        d.on_primary(go)
        d.run_dialog()
    body.pack_start(ui.row("Preferred languages", f"{len(installed)} languages installed",
                           "preferences-desktop-locale-symbolic",
                           ui.button("Add a language", add_language)), False, False, 0)

    body.pack_start(ui.section("Region"), False, False, 0)

    def set_region(code):
        text = "# Regional format from Settings > Time & language. Applies after signing out.\n"
        text += "".join(f"{v}={code}\n" for v in REGION_VARS)
        ui.write_file(REGION_FILE, text)
        ui.notify("Sign out to change the regional format", "", "preferences-desktop-locale")
    body.pack_start(ui.row("Regional format", "Dates, numbers, currency and measurements",
                           "x-office-calendar-symbolic",
                           ui.combo([(c, locale_name(c)) for c in installed], region() or lang, set_region, 260)),
                    False, False, 0)
    sample = datetime.now()
    body.pack_start(ui.row("Format preview", f"{sample.strftime('%x')}  \u00b7  {sample.strftime('%X')}  \u00b7  "
                           f"{sample.strftime('%A, %B %-d, %Y')}", None), False, False, 0)

    body.pack_start(ui.section("Keyboard"), False, False, 0)
    _l1, pairs, options = xkb_layouts()
    names = xkb_names()
    for i, (layout, variant) in enumerate(pairs):
        title = names.get(layout, layout) + (f" ({variant})" if variant else "")
        controls = []
        if i > 0:
            controls.append(ui.button("Move up", lambda i=i: set_keyboard(
                app, pairs[:i - 1] + [pairs[i], pairs[i - 1]] + pairs[i + 1:], options)))
        if len(pairs) > 1:
            controls.append(ui.button("Remove", lambda i=i: set_keyboard(app, pairs[:i] + pairs[i + 1:], options)))
        body.pack_start(ui.row(title, "Default" if i == 0 else None, "input-keyboard-symbolic", controls or None),
                        False, False, 0)

    def add_layout():
        d = ui.Dialog(app.win, "Add a keyboard", "Add")
        choice = {"v": None}
        d.add_field("Keyboard layout", ui.combo(sorted(names.items(), key=lambda x: x[1]), None,
                                                lambda v: choice.__setitem__("v", v), 360))

        def go():
            if choice["v"]:
                d.destroy()
                opts = options if any(o.startswith("grp:") for o in options) else options + ["grp:win_space_toggle"]
                set_keyboard(app, pairs + [(choice["v"], "")], opts)
        d.on_primary(go)
        d.run_dialog()
    body.pack_start(ui.row("Add a keyboard", "The sign-in screen uses these too", "list-add-symbolic",
                           ui.button("Add a keyboard", add_layout)), False, False, 0)
    if len(pairs) > 1:
        current = next((o for o in options if o.startswith("grp:")), "")
        body.pack_start(ui.row("Switch keyboards with", None, "input-keyboard-symbolic",
                               ui.combo(SWITCH, current, lambda v: set_keyboard(
                                   app, pairs, [o for o in options if not o.startswith("grp:")] + ([v] if v else [])),
                                   180)), False, False, 0)


def build_typing(app, body):
    s = ui.load()

    def set_(key, value, line):
        ui.save(**{key: value})
        ui.sway_conf(key, line)
    body.pack_start(ui.section("Keyboard"), False, False, 0)
    body.pack_start(ui.row("Repeat delay", "How long to hold a key before it repeats", "input-keyboard-symbolic",
                           ui.slider(s.get("repeat_delay", 500), 150, 1000, lambda v: set_(
                               "repeat_delay", v, f"input type:keyboard repeat_delay {v}"), step=50,
                               fmt=lambda v: f"{v} ms")), False, False, 0)
    body.pack_start(ui.row("Repeat rate", "How fast a held key repeats", "input-keyboard-symbolic",
                           ui.slider(s.get("repeat_rate", 30), 5, 60, lambda v: set_(
                               "repeat_rate", v, f"input type:keyboard repeat_rate {v}"),
                               fmt=lambda v: f"{v}/s")), False, False, 0)
    body.pack_start(ui.row("Turn on Num Lock when I sign in", None, "input-keyboard-symbolic",
                           ui.switch(s.get("numlock", False), lambda v: set_(
                               "numlock", v, f"input type:keyboard xkb_numlock {'enabled' if v else 'disabled'}"))),
                    False, False, 0)
    _l1, pairs, options = xkb_layouts()
    caps = next((o for o in options if o.startswith(("ctrl:nocaps", "caps:"))), "")
    body.pack_start(ui.row("Caps Lock key", "What the Caps Lock key does", "input-keyboard-symbolic",
                           ui.combo(CAPS, caps, lambda v: set_keyboard(
                               app, pairs, [o for o in options if not o.startswith(("ctrl:nocaps", "caps:"))]
                               + ([v] if v else [])), 160)), False, False, 0)
    test = ui.entry("", "Type here to test your settings", 320)
    body.pack_start(ui.row("Try it", None, "edit-symbolic", test), False, False, 0)
    body.pack_start(ui.section("Related settings"), False, False, 0)
    body.pack_start(ui.nav_row("Keyboard layouts", "Add and remove keyboards", "input-keyboard",
                               lambda: app.go("language")), False, False, 0)
    body.pack_start(ui.nav_row("Keyboard shortcuts", "Super key shortcuts for this desktop",
                               "preferences-desktop-keyboard-shortcuts", lambda: app.go("shortcuts")), False, False, 0)


def build_time(app, body):
    hero = ui.css(Gtk.Box(spacing=16), "hero")
    t = datetime.now()
    hero.pack_start(ui.icon("preferences-system-time", 48), False, False, 0)
    col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
    col.pack_start(ui.label(t.strftime(clock_format()), "hero-title"), False, False, 0)
    col.pack_start(ui.label(f"{t.strftime('%A, %B %-d, %Y')}  \u00b7  "
                            f"{ui.prop(timedate(), 'Timezone', '')}", "caption"), False, False, 0)
    hero.pack_start(col, True, True, 0)
    body.pack_start(hero, False, False, 8)


CATEGORY = {"time": build_time}

PAGES = [
    dict(key="datetime", title="Date & time", parent="time", icon="preferences-system-time",
         caption="Time zones, automatic clock settings, calendar display",
         keywords="clock date time zone timezone ntp 24-hour sync", build=build_datetime,
         aliases=("dateandtime", "date-time", "clock")),
    dict(key="language", title="Language & region", parent="time", icon="preferences-desktop-locale",
         caption="Windows and some apps format dates and time based on your region",
         keywords="language locale region format keyboard layout input", build=build_language,
         aliases=("regionlanguage", "region", "keyboard-layout")),
    dict(key="typing", title="Typing", parent="time", icon="input-keyboard",
         caption="Key repeat, Num Lock, Caps Lock", keywords="keyboard repeat delay rate caps numlock",
         build=build_typing),
]
