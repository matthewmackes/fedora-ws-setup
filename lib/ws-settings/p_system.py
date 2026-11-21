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

SCALES = [100, 125, 150, 175, 200, 225, 250, 300]
ORIENTATIONS = [("normal", "Landscape"), ("90", "Portrait"), ("180", "Landscape (flipped)"),
                ("270", "Portrait (flipped)")]
IDLE_TIMES = [(60, "1 minute"), (120, "2 minutes"), (180, "3 minutes"), (300, "5 minutes"),
              (600, "10 minutes"), (900, "15 minutes"), (1200, "20 minutes"),
              (1800, "30 minutes"), (2700, "45 minutes"), (3600, "1 hour"),
              (7200, "2 hours"), (0, "Never")]


# --- Display ---------------------------------------------------------------------

def outputs():
    return [o for o in ui.sway_json("get_outputs") if o.get("name")]


def refresh_hz(mhz):
    return mhz / 1000


def output_cmd(o, **change):
    """Sway command that sets an output to its current state plus changes."""
    mode = change.get("mode") or o["current_mode"]
    scale = change.get("scale", o.get("scale", 1))
    transform = change.get("transform", o.get("transform", "normal"))
    x, y = change.get("position", (o["rect"]["x"], o["rect"]["y"]))
    return (f"output {o['name']} mode {mode['width']}x{mode['height']}@{refresh_hz(mode['refresh']):.3f}Hz "
            f"scale {scale} transform {transform} position {x} {y}")


class Arrangement(Gtk.DrawingArea):
    """Drag displays to arrange them, as in Windows 11 Display settings."""

    def __init__(self, outs, on_change):
        super().__init__()
        self.outs = [o for o in outs if o.get("active")]
        self.on_change = on_change
        self.pos = {o["name"]: [o["rect"]["x"], o["rect"]["y"]] for o in self.outs}
        self.drag = None
        self.set_size_request(-1, 200)
        self.add_events(Gdk.EventMask.BUTTON_PRESS_MASK | Gdk.EventMask.BUTTON_RELEASE_MASK
                        | Gdk.EventMask.POINTER_MOTION_MASK)
        self.connect("draw", self.on_draw)
        self.connect("button-press-event", self.on_press)
        self.connect("motion-notify-event", self.on_motion)
        self.connect("button-release-event", self.on_release)

    def size(self, o):
        return o["rect"]["width"], o["rect"]["height"]

    def view(self):
        a = self.get_allocation()
        xs = [self.pos[o["name"]][0] for o in self.outs] + [self.pos[o["name"]][0] + self.size(o)[0] for o in self.outs]
        ys = [self.pos[o["name"]][1] for o in self.outs] + [self.pos[o["name"]][1] + self.size(o)[1] for o in self.outs]
        w, h = max(xs) - min(xs), max(ys) - min(ys)
        k = min((a.width - 40) / max(w, 1), (a.height - 40) / max(h, 1)) * 0.8
        ox = (a.width - w * k) / 2 - min(xs) * k
        oy = (a.height - h * k) / 2 - min(ys) * k
        return k, ox, oy

    def on_draw(self, _w, cr):
        k, ox, oy = self.view()
        fill = ui.ws_theme.accents(which="app_mode")[0]
        rgb = [int(fill[i:i + 2], 16) / 255 for i in (1, 3, 5)]
        for n, o in enumerate(self.outs, 1):
            x, y = self.pos[o["name"]]
            w, h = self.size(o)
            cr.rectangle(ox + x * k + 2, oy + y * k + 2, w * k - 4, h * k - 4)
            cr.set_source_rgba(*rgb, 1 if o.get("focused") else 0.75)
            cr.fill()
            cr.set_source_rgb(1, 1, 1)
            cr.select_font_face(ui.FONT, 0, 1)
            cr.set_font_size(28)
            ext = cr.text_extents(str(n))
            cr.move_to(ox + (x + w / 2) * k - ext.width / 2, oy + (y + h / 2) * k + ext.height / 2)
            cr.show_text(str(n))

    def hit(self, ex, ey):
        k, ox, oy = self.view()
        for o in self.outs:
            x, y = self.pos[o["name"]]
            w, h = self.size(o)
            if ox + x * k <= ex <= ox + (x + w) * k and oy + y * k <= ey <= oy + (y + h) * k:
                return o, k
        return None, k

    def on_press(self, _w, e):
        o, k = self.hit(e.x, e.y)
        if o and len(self.outs) > 1:
            self.drag = (o["name"], e.x, e.y, list(self.pos[o["name"]]), k)

    def on_motion(self, _w, e):
        if not self.drag:
            return
        name, sx, sy, start, k = self.drag
        self.pos[name] = [start[0] + (e.x - sx) / k, start[1] + (e.y - sy) / k]
        self.queue_draw()

    def on_release(self, _w, _e):
        if not self.drag:
            return
        name = self.drag[0]
        self.drag = None
        self.snap(name)
        self.queue_draw()
        self.on_change({n: tuple(int(v) for v in p) for n, p in self.pos.items()})

    def snap(self, name):
        """Put the moved display edge to edge with the nearest other display."""
        me = next(o for o in self.outs if o["name"] == name)
        mw, mh = self.size(me)
        mx, my = self.pos[name]
        best = None
        for o in self.outs:
            if o["name"] == name:
                continue
            x, y = self.pos[o["name"]]
            w, h = self.size(o)
            for cx, cy in ((x + w, my), (x - mw, my), (mx, y + h), (mx, y - mh)):
                if cx in (x + w, x - mw):
                    cy = max(y - mh + 1, min(cy, y + h - 1))
                else:
                    cx = max(x - mw + 1, min(cx, x + w - 1))
                d = (cx - mx) ** 2 + (cy - my) ** 2
                if best is None or d < best[0]:
                    best = (d, cx, cy)
        if best:
            self.pos[name] = [best[1], best[2]]
        minx = min(p[0] for p in self.pos.values())
        miny = min(p[1] for p in self.pos.values())
        for p in self.pos.values():
            p[0] -= minx
            p[1] -= miny


def keep_changes(app, apply_cmds, revert_cmds):
    """Apply display changes and offer the 15-second Keep/Revert countdown."""
    ui.sway(*apply_cmds)
    d = ui.Dialog(app.win, "Keep these display settings?", "Keep changes", "Revert")
    note = ui.label("", wrap=True)
    d.body.pack_start(note, False, False, 0)
    state = {"left": 15, "done": False}

    def revert():
        if not state["done"]:
            state["done"] = True
            ui.sway(*revert_cmds)
            GLib.timeout_add(600, lambda: app.refresh() and False)

    def tick():
        if state["done"]:
            return False
        note.set_text(f"Reverting to previous display settings in {state['left']} seconds.")
        state["left"] -= 1
        if state["left"] < 0:
            d.destroy()
            return False
        return True

    def keep():
        state["done"] = True
        d.destroy()
        if ui.which("ws-display-save"):
            ui.run(["ws-display-save"])
        GLib.timeout_add(600, lambda: app.refresh() and False)
    d.on_primary(keep)
    d.connect("destroy", lambda *_: revert())
    tick()
    GLib.timeout_add_seconds(1, tick)
    d.run_dialog()


def brightness():
    line = ui.out(["brightnessctl", "-m", "info"]).strip().split(",")
    try:
        return int(line[3].rstrip("%"))
    except (IndexError, ValueError):
        return None


def build_display(app, body):
    outs = outputs()
    active = [o for o in outs if o.get("active")]
    if len(outs) > 1:
        frame = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8), "hero")
        canvas = Arrangement(outs, lambda pos: arrange(app, active, pos))
        frame.pack_start(canvas, False, False, 0)
        frame.pack_start(ui.label("Select and drag displays to rearrange them.", "caption"),
                         False, False, 0)
        body.pack_start(frame, False, False, 0)
    level = brightness() if ui.which("brightnessctl") else None
    body.pack_start(ui.section("Brightness & colour"), False, False, 0)
    if level is not None:
        body.pack_start(ui.row("Brightness", "Adjust the brightness of the built-in display",
                               "display-brightness-symbolic",
                               ui.slider(level, 1, 100, lambda v: ui.run(
                                   ["brightnessctl", "-q", "set", f"{v}%"]), live=True)), False, False, 0)
    on = ui.get("night_light")
    body.pack_start(ui.nav_row("Night light", "Use warmer colours to help block blue light",
                               "night-light-symbolic", lambda: app.go("nightlight"),
                               ui.label("On" if on else "Off", "caption")), False, False, 0)

    body.pack_start(ui.section("Scale & layout"), False, False, 0)
    names = [o["name"] for o in outs]
    chosen = getattr(app, "display_choice", None)
    if chosen not in names:
        chosen = next((o["name"] for o in outs if o.get("focused")), names[0] if names else None)
    if not chosen:
        body.pack_start(ui.infobar("No displays were found."), False, False, 0)
        return
    if len(outs) > 1:
        def pick(name):
            app.display_choice = name
            app.refresh()
        body.pack_start(ui.row("Display", "Choose which display to change", "video-display-symbolic",
                               ui.combo([(o["name"], f"{i}. {o.get('make', '')} {o.get('model', '')} ({o['name']})")
                                         for i, o in enumerate(outs, 1)], chosen, pick, 300)),
                        False, False, 0)
    o = next(x for x in outs if x["name"] == chosen)
    if len(outs) > 1:
        body.pack_start(ui.row("Use this display", None, "video-display-symbolic",
                               ui.switch(o.get("active"), lambda v: keep_changes(
                                   app, [f"output {o['name']} {'enable' if v else 'disable'}"],
                                   [f"output {o['name']} {'disable' if v else 'enable'}"]),
                                   sensitive=len(active) > 1 or not o.get("active"))), False, False, 0)
    if not o.get("active"):
        return
    current = [output_cmd(o)]
    scale = round(o.get("scale", 1) * 100)
    scales = sorted(set(SCALES + [scale]))
    body.pack_start(ui.row("Scale", "Change the size of text, apps and other items",
                           "zoom-in-symbolic",
                           ui.combo([(s, f"{s}%" + (" (Recommended)" if s == 150 and o["rect"]["width"] * o["scale"] >= 1920 else ""))
                                     for s in scales], scale,
                                    lambda s: keep_changes(app, [output_cmd(o, scale=s / 100)], current))),
                    False, False, 0)
    modes = {}
    for m in sorted(o.get("modes", []), key=lambda m: (-m["width"] * m["height"], -m["refresh"])):
        modes.setdefault((m["width"], m["height"]), []).append(m)
    biggest = next(iter(modes), None)
    cur = (o["current_mode"]["width"], o["current_mode"]["height"])
    body.pack_start(ui.row("Display resolution", "Adjust the resolution to fit your connected display",
                           "video-display-symbolic",
                           ui.combo([(k, f"{k[0]} \u00d7 {k[1]}" + (" (Recommended)" if k == biggest else ""))
                                     for k in modes], cur,
                                    lambda k: keep_changes(app, [output_cmd(o, mode=modes[k][0])], current), 220)),
                    False, False, 0)
    body.pack_start(ui.row("Display orientation", None, "object-rotate-right-symbolic",
                           ui.combo(ORIENTATIONS, o.get("transform", "normal"),
                                    lambda t: keep_changes(app, [output_cmd(o, transform=t)], current))),
                    False, False, 0)
    rates = modes.get(cur, [])
    if len(rates) > 1:
        body.pack_start(ui.row("Choose a refresh rate",
                               "A higher rate gives smoother motion, but also uses more power",
                               "video-display-symbolic",
                               ui.combo([(m["refresh"], f"{refresh_hz(m['refresh']):.2f} Hz") for m in rates],
                                        o["current_mode"]["refresh"],
                                        lambda r: keep_changes(app, [output_cmd(o, mode=next(
                                            m for m in rates if m["refresh"] == r))], current))),
                        False, False, 0)
    body.pack_start(ui.section("Related settings"), False, False, 0)
    body.pack_start(ui.nav_row("Advanced display", f"{o.get('make', '')} {o.get('model', '')}, "
                               f"{o['current_mode']['width']}\u00d7{o['current_mode']['height']} at "
                               f"{refresh_hz(o['current_mode']['refresh']):.0f} Hz",
                               "preferences-desktop-display", lambda: show_advanced_display(app, o)),
                    False, False, 0)


def arrange(app, active, pos):
    before = [output_cmd(o) for o in active]
    after = [output_cmd(o, position=pos[o["name"]]) for o in active if o["name"] in pos]
    keep_changes(app, after, before)


def show_advanced_display(app, o):
    d = ui.Dialog(app.win, "Advanced display", None, "Close", 480)
    mode = o["current_mode"]
    for k, v in (("Display", f"{o.get('make', '')} {o.get('model', '')}"),
                 ("Connector", o["name"]), ("Serial", o.get("serial") or "Unknown"),
                 ("Desktop resolution", f"{o['rect']['width']} \u00d7 {o['rect']['height']}"),
                 ("Active signal resolution", f"{mode['width']} \u00d7 {mode['height']}"),
                 ("Refresh rate", f"{refresh_hz(mode['refresh']):.3f} Hz"),
                 ("Scale", f"{o.get('scale', 1) * 100:.0f}%"),
                 ("Adaptive sync", o.get("adaptive_sync_status", "disabled").capitalize())):
        d.body.pack_start(ui.texts(k, v), False, False, 0)
    d.run_dialog()


# --- Night light -----------------------------------------------------------------

def restart_helper(name):
    ui.ok(["pkill", "-u", str(os.getuid()), "-f", name])


def build_nightlight(app, body):
    s = ui.load()
    if not ui.which("wlsunset"):
        body.pack_start(ui.infobar("Night light needs the wlsunset package.",
                                   action=("Install", lambda: ui.install(app, "wlsunset"))),
                        False, False, 0)

    def set_(**kw):
        ui.save(**kw)
        restart_helper("ws-nightlight")
    body.pack_start(ui.row("Night light", "Use warmer colours to help block blue light",
                           "night-light-symbolic",
                           ui.switch(s["night_light"], lambda v: set_(night_light=v))), False, False, 0)
    strength = round((6500 - s["night_light_temp"]) / 35)
    body.pack_start(ui.row("Strength", None, None,
                           ui.slider(strength, 0, 100, lambda v: set_(night_light_temp=6500 - v * 35))),
                    False, False, 0)
    exp, inner = ui.expander("Schedule night light", "Turn on automatically at set times", None,
                             ui.combo([("always", "Always on"), ("sunset", "Sunset to sunrise"),
                                       ("hours", "Set hours")], s["night_light_schedule"],
                                      lambda v: (set_(night_light_schedule=v), app.refresh())),
                             expanded=s["night_light_schedule"] == "hours")
    if s["night_light_schedule"] == "sunset":
        loc = zone_location()
        inner.pack_start(ui.row("Location", (f"Sunset and sunrise for {loc[2]}" if loc else
                                             "Your time zone has no location; set hours instead")),
                         False, False, 0)
    elif s["night_light_schedule"] == "hours":
        for key, title in (("night_light_from", "Turn on"), ("night_light_to", "Turn off")):
            e = ui.entry(s[key], "HH:MM", 100)
            e.connect("changed", lambda w, k=key: re.fullmatch(r"\d{1,2}:\d{2}", w.get_text())
                      and set_(**{k: w.get_text()}))
            inner.pack_start(ui.row(title, None, None, e), False, False, 0)
    body.pack_start(exp, False, False, 0)


def zone_location():
    """Latitude and longitude of the time zone's main city, from zone1970.tab."""
    tz = ui.out(["timedatectl", "show", "-p", "Timezone", "--value"]).strip()
    for line in ui.read_file("/usr/share/zoneinfo/zone1970.tab").splitlines():
        parts = line.split("\t")
        if len(parts) >= 3 and parts[2] == tz:
            m = re.fullmatch(r"([+-]\d{2})(\d{2})(\d{2})?([+-]\d{3})(\d{2})(\d{2})?", parts[1])
            if m:
                lat = int(m[1]) + (1 if m[1][0] == "+" else -1) * int(m[2]) / 60
                lon = int(m[4]) + (1 if m[4][0] == "+" else -1) * int(m[5]) / 60
                return lat, lon, tz.split("/")[-1].replace("_", " ")
    return None


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


# --- Multitasking ---------------------------------------------------------------------

def build_multitasking(app, body):
    s = ui.load()

    def set_(key, value, line):
        ui.save(**{key: value})
        ui.sway_conf(key, line)
    body.pack_start(ui.row("Focus follows the mouse", "Windows get focus when you point at them",
                           "input-mouse-symbolic",
                           ui.switch(s["focus_follows_mouse"] == "yes", lambda v: set_(
                               "focus_follows_mouse", "yes" if v else "no",
                               f"focus_follows_mouse {'yes' if v else 'no'}"))), False, False, 0)
    body.pack_start(ui.row("Move the pointer with the focus",
                           "When you switch windows with the keyboard, the pointer follows",
                           "input-mouse-symbolic",
                           ui.switch(s["mouse_warping"] != "none", lambda v: set_(
                               "mouse_warping", "output" if v else "none",
                               f"mouse_warping {'output' if v else 'none'}"))), False, False, 0)
    body.pack_start(ui.section("Desktops"), False, False, 0)
    body.pack_start(ui.row("On the taskbar, show all the open windows", None, "view-dual-symbolic",
                           ui.combo([(False, "Only on the desktop I'm using"), (True, "On all desktops")],
                                    s["taskbar_all_desktops"], lambda v: (ui.save(taskbar_all_desktops=v),
                                                                          ws_theme.apply_taskbar()), 240)),
                    False, False, 0)
    body.pack_start(ui.row("Go back to the previous desktop",
                           "Pressing the shortcut for the current desktop switches to the last one",
                           "view-dual-symbolic",
                           ui.switch(s["workspace_back_and_forth"], lambda v: set_(
                               "workspace_back_and_forth", v,
                               f"workspace_auto_back_and_forth {'yes' if v else 'no'}"))), False, False, 0)
    body.pack_start(ui.row("Arrange new windows", "How windows share a desktop",
                           "view-grid-symbolic",
                           ui.combo([("default", "Side by side (tiled)"), ("stacking", "Stacked"),
                                     ("tabbed", "Tabs")], s["default_layout"],
                                    lambda v: set_("default_layout", v, f"workspace_layout {v}"))),
                    False, False, 0)
    body.pack_start(ui.row("Space between windows", None, "view-paged-symbolic",
                           ui.slider(s["gaps"], 0, 24, lambda v: (
                               set_("gaps", v, f"gaps inner {v}"), ui.sway(f"gaps inner all set {v}")),
                               fmt=lambda v: f"{v} px")), False, False, 0)
    body.pack_start(ui.row("Hide space with one window", "Remove gaps when a window is alone",
                           "view-fullscreen-symbolic",
                           ui.switch(s["smart_gaps"], lambda v: set_(
                               "smart_gaps", v, f"smart_gaps {'on' if v else 'off'}"))), False, False, 0)


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


BRAND = "Mackes Fedora Linux 2026"


def brand_logo(size):
    """The Mackes mark, shared with the Start button and the installer."""
    from gi.repository import GdkPixbuf, Gtk
    roots = ("/usr/local/share/fedora-ws-setup/brand",
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
    sway = ui.out(["sway", "--version"]).strip().replace("sway version ", "")
    installed = ui.out(["rpm", "-q", "--qf", "%{INSTALLTIME:day}", "filesystem"]).strip()
    return [("Edition", BRAND), ("Version", r.get("VERSION", "")),
            ("Installed on", installed), ("Kernel", platform.release()),
            ("Desktop", f"Sway {sway}" if sway else "Sway")]


def build_about(app, body):
    host = ui.proxy("org.freedesktop.hostname1", "/org/freedesktop/hostname1", "org.freedesktop.hostname1")
    name = ui.prop(host, "PrettyHostname") or ui.prop(host, "StaticHostname") or ui.out(["hostname"]).strip()
    hero = ui.css(Gtk.Box(spacing=16), "hero")
    hero.pack_start(brand_logo(64) or ui.icon("computer", 48), False, False, 0)
    t = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
    t.pack_start(ui.label(name, "hero-title"), False, False, 0)
    dmi = ui.read_file("/sys/class/dmi/id/product_family").strip() or ui.read_file("/sys/class/dmi/id/product_name").strip()
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
                                    ("Mackes specifications", os_specs(), "distributor-logo-symbolic")):
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
    dict(key="multitasking", title="Multitasking", parent="system", icon="view-grid",
         caption="Window layout, desktops, focus behaviour",
         keywords="snap windows tiling gaps workspaces desktops focus", build=build_multitasking),
    dict(key="clipboard", title="Clipboard", parent="system", icon="edit-paste",
         caption="Cut and copy history", keywords="copy paste history", build=build_clipboard),
    dict(key="about", title="About", parent="system", icon="help-about",
         caption="Device specifications, rename PC, Mackes specifications",
         keywords="pc name hostname specs processor ram version kernel", build=build_about),
]
