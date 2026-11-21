# Settings > System > Display — Windows 11–shaped controls for labwc / Wayland.
import json
import os
import re
import subprocess

from gi.repository import Gdk, GLib, Gtk

import wsui as ui
import ws_theme

# 150% stays gated until ALLOW_FRACTIONAL_SCALE is true in labwc_cfg (GTK3 clicks).
SCALES = [100, 200]
ORIENTATIONS = [
    ("normal", "Landscape"),
    ("90", "Portrait"),
    ("180", "Landscape (flipped)"),
    ("270", "Portrait (flipped)"),
    ("flipped", "Landscape (mirrored)"),
    ("flipped-90", "Portrait (mirrored)"),
    ("flipped-180", "Landscape (mirrored, flipped)"),
    ("flipped-270", "Portrait (mirrored, flipped)"),
]
KANSHI = os.path.join(os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config")),
                      "kanshi", "config")
SETTINGS_DIR = os.path.join(os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config")),
                            "ws-settings")
EXTEND_POS_FILE = os.path.join(SETTINGS_DIR, "display-extend-positions.json")


def outputs():
    return [o for o in ui.sway_json("get_outputs") if o.get("name")]


def refresh_hz(mhz):
    try:
        return float(mhz) / 1000.0
    except (TypeError, ValueError):
        return 0.0


def output_cmd(o, **change):
    """Sway-style command that sets an output to its current state plus changes."""
    if "enabled" in change and not change["enabled"]:
        return f"output {o['name']} disable"
    if change.get("enable_only"):
        return f"output {o['name']} enable"
    mode = change.get("mode") or o.get("current_mode") or {"width": 0, "height": 0, "refresh": 0}
    scale = change.get("scale", o.get("scale", 1))
    transform = change.get("transform", o.get("transform", "normal"))
    x, y = change.get("position", (o.get("rect", {}).get("x", 0), o.get("rect", {}).get("y", 0)))
    adaptive = change.get("adaptive_sync", o.get("adaptive_sync_status", "disabled"))
    line = (f"output {o['name']} mode {mode['width']}x{mode['height']}@"
            f"{refresh_hz(mode.get('refresh', 0)):.3f}Hz scale {scale} transform {transform} "
            f"position {int(x)} {int(y)}")
    if adaptive is not None:
        line += f" adaptive_sync {adaptive}"
    return line


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


def restart_helper(name):
    ui.ok(["pkill", "-u", str(os.getuid()), "-f", name])


def notify_shell():
    """Ask the taskbar to rebuild after main-display / layout changes."""
    ui.ok(["pkill", "-USR1", "-u", str(os.getuid()), "-f", "ws-taskbar"])
    ui.ok(["pkill", "-HUP", "-u", str(os.getuid()), "-f", "ws-taskbar"])


# --- Arrangement canvas -------------------------------------------------------

class Arrangement(Gtk.DrawingArea):
    """Drag displays to arrange them, as in Windows 11 Display settings."""

    def __init__(self, outs, selected, on_change, on_select, on_menu):
        super().__init__()
        self.outs = list(outs)
        self.selected = selected
        self.on_change = on_change
        self.on_select = on_select
        self.on_menu = on_menu
        self.pos = {o["name"]: [o["rect"]["x"], o["rect"]["y"]] for o in self.outs
                    if o.get("active") and o.get("rect")}
        self.drag = None
        self.set_size_request(-1, 220)
        self.set_can_focus(True)
        self.add_events(Gdk.EventMask.BUTTON_PRESS_MASK | Gdk.EventMask.BUTTON_RELEASE_MASK
                        | Gdk.EventMask.POINTER_MOTION_MASK | Gdk.EventMask.KEY_PRESS_MASK)
        self.connect("draw", self.on_draw)
        self.connect("button-press-event", self.on_press)
        self.connect("motion-notify-event", self.on_motion)
        self.connect("button-release-event", self.on_release)
        self.connect("key-press-event", self.on_key)

    def size(self, o):
        r = o.get("rect") or {}
        w = max(r.get("width") or 100, 80)
        h = max(r.get("height") or 60, 50)
        return w, h

    def view(self):
        a = self.get_allocation()
        active = [o for o in self.outs if o["name"] in self.pos]
        if not active:
            return 1, 0, 0
        xs = [self.pos[o["name"]][0] for o in active] + [
            self.pos[o["name"]][0] + self.size(o)[0] for o in active]
        ys = [self.pos[o["name"]][1] for o in active] + [
            self.pos[o["name"]][1] + self.size(o)[1] for o in active]
        w, h = max(xs) - min(xs), max(ys) - min(ys)
        k = min((a.width - 40) / max(w, 1), (a.height - 40) / max(h, 1)) * 0.8
        ox = (a.width - w * k) / 2 - min(xs) * k
        oy = (a.height - h * k) / 2 - min(ys) * k
        return k, ox, oy

    def on_draw(self, _w, cr):
        k, ox, oy = self.view()
        fill = ws_theme.accents(which="app_mode")[0]
        rgb = [int(fill[i:i + 2], 16) / 255 for i in (1, 3, 5)]
        numbers = {o["name"]: i for i, o in enumerate(self.outs, 1)}
        for o in self.outs:
            if o["name"] not in self.pos:
                continue
            x, y = self.pos[o["name"]]
            w, h = self.size(o)
            selected = o["name"] == self.selected
            cr.rectangle(ox + x * k + 2, oy + y * k + 2, w * k - 4, h * k - 4)
            cr.set_source_rgba(*rgb, 1 if selected else 0.7)
            cr.fill()
            if selected:
                cr.rectangle(ox + x * k + 2, oy + y * k + 2, w * k - 4, h * k - 4)
                cr.set_source_rgb(1, 1, 1)
                cr.set_line_width(2)
                cr.stroke()
            cr.set_source_rgb(1, 1, 1)
            cr.select_font_face(ui.FONT, 0, 1)
            cr.set_font_size(28)
            label = str(numbers.get(o["name"], "?"))
            ext = cr.text_extents(label)
            cr.move_to(ox + (x + w / 2) * k - ext.width / 2, oy + (y + h / 2) * k + ext.height / 2)
            cr.show_text(label)

    def hit(self, ex, ey):
        k, ox, oy = self.view()
        for o in reversed([x for x in self.outs if x["name"] in self.pos]):
            x, y = self.pos[o["name"]]
            w, h = self.size(o)
            if ox + x * k <= ex <= ox + (x + w) * k and oy + y * k <= ey <= oy + (y + h) * k:
                return o, k
        return None, k

    def on_press(self, _w, e):
        self.grab_focus()
        o, k = self.hit(e.x, e.y)
        if not o:
            return
        self.selected = o["name"]
        if self.on_select:
            self.on_select(o["name"])
        if e.button == 3:
            if self.on_menu:
                self.on_menu(o, e)
            return
        if e.button == 1 and len(self.pos) > 1:
            self.drag = (o["name"], e.x, e.y, list(self.pos[o["name"]]), k)
        self.queue_draw()

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

    def on_key(self, _w, e):
        if self.selected not in self.pos:
            return False
        step = 10 if not (e.state & Gdk.ModifierType.SHIFT_MASK) else 1
        dx = dy = 0
        if e.keyval in (Gdk.KEY_Left, Gdk.KEY_KP_Left):
            dx = -step
        elif e.keyval in (Gdk.KEY_Right, Gdk.KEY_KP_Right):
            dx = step
        elif e.keyval in (Gdk.KEY_Up, Gdk.KEY_KP_Up):
            dy = -step
        elif e.keyval in (Gdk.KEY_Down, Gdk.KEY_KP_Down):
            dy = step
        else:
            return False
        self.pos[self.selected][0] += dx
        self.pos[self.selected][1] += dy
        self.snap(self.selected)
        self.queue_draw()
        self.on_change({n: tuple(int(v) for v in p) for n, p in self.pos.items()})
        return True

    def snap(self, name):
        me = next(o for o in self.outs if o["name"] == name)
        mw, mh = self.size(me)
        mx, my = self.pos[name]
        best = None
        for o in self.outs:
            if o["name"] == name or o["name"] not in self.pos:
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


def arrange(app, active, pos):
    before = [output_cmd(o) for o in active]
    after = [output_cmd(o, position=pos[o["name"]]) for o in active if o["name"] in pos]
    keep_changes(app, after, before)


def identify_displays(outs=None):
    if ui.which("ws-display-identify"):
        ui.run(["ws-display-identify"])
        return
    outs = outs or [o for o in outputs() if o.get("active")]
    for i, o in enumerate(outs, 1):
        GLib.idle_add(_flash_number, i, o)


def _flash_number(number, o):
    try:
        gi_ok = True
        import gi
        gi.require_version("GtkLayerShell", "0.1")
        from gi.repository import GtkLayerShell
    except (ValueError, ImportError):
        gi_ok = False
        GtkLayerShell = None
    win = Gtk.Window(type=Gtk.WindowType.TOPLEVEL)
    win.set_decorated(False)
    win.set_accept_focus(False)
    label = Gtk.Label(label=str(number))
    label.set_markup(f'<span size="102400" weight="bold">{number}</span>')
    box = Gtk.Box(halign=Gtk.Align.CENTER, valign=Gtk.Align.CENTER)
    box.pack_start(label, True, True, 0)
    win.add(box)
    r = o.get("rect") or {}
    if gi_ok and GtkLayerShell:
        GtkLayerShell.init_for_window(win)
        GtkLayerShell.set_layer(win, GtkLayerShell.Layer.OVERLAY)
        GtkLayerShell.set_namespace(win, "ws-display-identify")
        display = Gdk.Display.get_default()
        for i in range(display.get_n_monitors()):
            mon = display.get_monitor(i)
            geo = mon.get_geometry()
            if geo.x == r.get("x") and geo.y == r.get("y"):
                GtkLayerShell.set_monitor(win, mon)
                break
        for edge in (GtkLayerShell.Edge.TOP, GtkLayerShell.Edge.BOTTOM,
                     GtkLayerShell.Edge.LEFT, GtkLayerShell.Edge.RIGHT):
            GtkLayerShell.set_anchor(win, edge, True)
    else:
        win.move(int(r.get("x") or 0), int(r.get("y") or 0))
        win.resize(max(int(r.get("width") or 400), 200), max(int(r.get("height") or 300), 200))
    win.show_all()
    GLib.timeout_add(2000, win.destroy)
    return False


# --- Brightness / DDC ---------------------------------------------------------

def _is_laptop_panel(o):
    name = (o.get("name") or "").lower()
    return name.startswith("edp") or name.startswith("lvds") or name.startswith("dsi")


def brightness_for(o):
    """Return (percent, setter) or (None, None) when brightness cannot be controlled."""
    if _is_laptop_panel(o) and ui.which("brightnessctl"):
        line = ui.out(["brightnessctl", "-m", "info"]).strip().split(",")
        try:
            level = int(line[3].rstrip("%"))
        except (IndexError, ValueError):
            return None, None

        def set_panel(v):
            ui.run(["brightnessctl", "-q", "set", f"{int(v)}%"])

        return level, set_panel
    if not ui.which("ddcutil"):
        return None, None
    # Best-effort: detect first DDC bus (external monitors).
    detect = ui.out(["ddcutil", "detect", "--brief"])
    if "Display" not in detect and "I2C" not in detect:
        return None, None
    getv = ui.out(["ddcutil", "getvcp", "10", "--brief"])
    m = re.search(r"VCP\s+10\s+\w+\s+(\d+)\s+(\d+)", getv) or re.search(r"\b(\d+)\s*/\s*(\d+)", getv)
    if not m:
        # Still show a slider if a display was detected; start at 50.
        level = 50
    else:
        cur, mx = int(m.group(1)), max(int(m.group(2)), 1)
        level = int(round(cur * 100 / mx))

    def set_ddc(v):
        ui.run(["ddcutil", "setvcp", "10", str(int(v))])

    return level, set_ddc


# --- Projection modes ---------------------------------------------------------

def save_extend_positions(outs):
    data = {o["name"]: [o["rect"]["x"], o["rect"]["y"]] for o in outs if o.get("active")}
    try:
        os.makedirs(SETTINGS_DIR, exist_ok=True)
        with open(EXTEND_POS_FILE, "w") as f:
            json.dump(data, f)
    except OSError:
        pass


def load_extend_positions():
    try:
        with open(EXTEND_POS_FILE) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def projection_mode(outs):
    active = [o for o in outs if o.get("active")]
    if len(outs) < 2:
        return "extend"
    if len(active) == 1:
        return f"only:{active[0]['name']}"
    positions = {(o["rect"]["x"], o["rect"]["y"]) for o in active}
    if len(positions) == 1:
        return "duplicate"
    return "extend"


def apply_projection(app, outs, mode):
    before = []
    for o in outs:
        before.append(output_cmd(o) if o.get("active") else f"output {o['name']} disable")
    after = []
    if mode == "extend":
        save_extend_positions(outs)
        pos = load_extend_positions()
        x = 0
        for o in outs:
            if o["name"] in pos:
                px, py = pos[o["name"]]
            else:
                w = o.get("rect", {}).get("width") or o.get("current_mode", {}).get("width") or 1920
                px, py = x, 0
                x += w
            after.append(output_cmd({**o, "active": True}, position=(px, py)))
    elif mode == "duplicate":
        save_extend_positions(outs)
        main = next((o for o in outs if o.get("focused")), outs[0])
        for o in outs:
            after.append(output_cmd(o, mode=main.get("current_mode"), position=(0, 0),
                                    scale=main.get("scale", 1)))
    elif mode.startswith("only:"):
        save_extend_positions(outs)
        name = mode.split(":", 1)[1]
        for o in outs:
            if o["name"] == name:
                after.append(output_cmd(o, position=(0, 0)))
            else:
                after.append(f"output {o['name']} disable")
    keep_changes(app, after, before)


def set_main_display(app, outs, name):
    """Put the chosen display at (0,0), persist, and nudge the shell."""
    active = [o for o in outs if o.get("active")]
    target = next((o for o in active if o["name"] == name), None)
    if not target:
        return
    ox, oy = target["rect"]["x"], target["rect"]["y"]
    before = [output_cmd(o) for o in active]
    after = []
    for o in active:
        after.append(output_cmd(o, position=(o["rect"]["x"] - ox, o["rect"]["y"] - oy)))
    ui.save(main_display=name)
    keep_changes(app, after, before)
    notify_shell()


# --- Night light (inline) -----------------------------------------------------

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


def pack_nightlight(app, body):
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


def build_nightlight(app, body):
    """Kept for deep links; mirrors the inline Brightness & colour controls."""
    pack_nightlight(app, body)


# --- Color management (best-effort colord) ------------------------------------

def _colormgr(*args):
    if not ui.which("colormgr"):
        return ""
    return ui.out(["colormgr", *args])


def color_devices():
    text = _colormgr("get-devices")
    devices = []
    cur = {}
    for line in text.splitlines():
        if line.startswith("Device ID:"):
            if cur.get("id"):
                devices.append(cur)
            cur = {"id": line.split(":", 1)[1].strip()}
        elif ":" in line and cur:
            k, v = line.split(":", 1)
            cur[k.strip().lower().replace(" ", "_")] = v.strip()
    if cur.get("id"):
        devices.append(cur)
    return [d for d in devices if "display" in (d.get("device_kind") or d.get("kind") or "").lower()
            or "xrandr" in (d.get("id") or "").lower() or "drm" in (d.get("id") or "").lower()
            or d.get("id")]


def color_profiles():
    text = _colormgr("get-profiles")
    profiles = []
    cur = {}
    for line in text.splitlines():
        if line.startswith("Profile ID:"):
            if cur.get("id"):
                profiles.append(cur)
            cur = {"id": line.split(":", 1)[1].strip()}
        elif ":" in line and cur:
            k, v = line.split(":", 1)
            cur[k.strip().lower().replace(" ", "_")] = v.strip()
    if cur.get("id"):
        profiles.append(cur)
    return profiles


def show_color_management(app, o):
    d = ui.Dialog(app.win, "Colour management", None, "Close", 520)
    if not ui.which("colormgr"):
        d.body.pack_start(ui.infobar("Colour management needs the colord package."), False, False, 0)
        d.run_dialog()
        return
    devices = color_devices()
    profiles = color_profiles()
    d.body.pack_start(ui.label("Profiles are applied through colord when the compositor allows it.",
                               "caption", wrap=True), False, False, 0)
    if not devices:
        d.body.pack_start(ui.infobar("No display colour devices were reported by colord."), False, False, 0)
    device_id = None
    model = (o.get("model") or "").lower()
    name = (o.get("name") or "").lower()
    for dev in devices:
        blob = " ".join(str(v) for v in dev.values()).lower()
        if name and name in blob or model and model in blob:
            device_id = dev["id"]
            break
    if not device_id and devices:
        device_id = devices[0]["id"]
    options = [("", "(None)")] + [
        (p["id"], p.get("title") or p.get("filename") or p["id"]) for p in profiles]
    current = ""
    for dev in devices:
        if dev.get("id") == device_id:
            current = dev.get("profile_id") or dev.get("default_profile_id") or ""
            break

    def assign(pid):
        if not device_id:
            return
        if not pid:
            ui.run(["colormgr", "device-make-profile-default", device_id, ""])
            return
        # Best-effort; colord may reject when the session cannot apply ICC.
        ok = ui.ok(["colormgr", "device-add-profile", device_id, pid])
        if ok:
            ui.ok(["colormgr", "device-make-profile-default", device_id, pid])
        else:
            ui.error(app.win, "Colour profile",
                     "colord could not apply this profile on the current Wayland session.")

    d.body.pack_start(ui.row("Display device", device_id or "Unavailable"), False, False, 0)
    d.body.pack_start(ui.row("Colour profile", "ICC profiles from colord",
                             "preferences-color-symbolic",
                             ui.combo(options, current, assign, 280)), False, False, 0)
    # HDR: only when metadata says so (rare on labwc).
    hdr = o.get("hdr") or o.get("hdr_enabled")
    if hdr:
        d.body.pack_start(ui.row("Use HDR", "When available", "video-display-symbolic",
                                 ui.switch(bool(hdr), lambda v: None)), False, False, 0)
    else:
        d.body.pack_start(ui.row("Use HDR", "Not reported for this display on labwc",
                                 "video-display-symbolic",
                                 ui.label("Unavailable", "caption")), False, False, 0)
    d.run_dialog()


# --- Kanshi layouts -----------------------------------------------------------

def kanshi_profiles():
    text = ui.read_file(KANSHI)
    profiles = []
    name = None
    lines = []
    for line in text.splitlines():
        m = re.match(r"profile\s+(\S+)\s*\{", line)
        if m:
            if name and name.startswith("saved-"):
                profiles.append({"name": name, "lines": lines})
            name, lines = m.group(1), []
        elif name and line.strip() == "}":
            if name.startswith("saved-"):
                profiles.append({"name": name, "lines": lines})
            name, lines = None, []
        elif name:
            lines.append(line)
    return profiles


def delete_kanshi_profile(name):
    text = ui.read_file(KANSHI)
    out, skip, depth = [], False, 0
    for line in text.splitlines():
        m = re.match(r"profile\s+(\S+)\s*\{", line)
        if m and m.group(1) == name:
            skip, depth = True, 1
            continue
        if skip:
            if "{" in line:
                depth += line.count("{")
            if "}" in line:
                depth -= line.count("}")
            if depth <= 0:
                skip = False
            continue
        out.append(line)
    try:
        os.makedirs(os.path.dirname(KANSHI), exist_ok=True)
        with open(KANSHI, "w") as f:
            f.write("\n".join(out).rstrip() + "\n")
    except OSError:
        return False
    ui.ok(["pkill", "-HUP", "-xu", str(os.getuid()), "kanshi"])
    return True


def apply_kanshi_profile(name):
    # kanshi applies matching profiles on HUP; force by touching config order is enough.
    ui.ok(["pkill", "-HUP", "-xu", str(os.getuid()), "kanshi"])
    if ui.which("kanshictl"):
        ui.ok(["kanshictl", "switch", name])


# --- Advanced dialog ----------------------------------------------------------

def show_advanced_display(app, o, outs):
    d = ui.Dialog(app.win, "Advanced display", None, "Close", 560)
    mode = o.get("current_mode") or {}
    for k, v in (("Display", f"{o.get('make', '')} {o.get('model', '')}".strip() or o["name"]),
                 ("Connector", o["name"]),
                 ("Serial", o.get("serial") or "Unknown"),
                 ("Desktop resolution",
                  f"{o.get('rect', {}).get('width', 0)} \u00d7 {o.get('rect', {}).get('height', 0)}"),
                 ("Active signal resolution", f"{mode.get('width', 0)} \u00d7 {mode.get('height', 0)}"),
                 ("Refresh rate", f"{refresh_hz(mode.get('refresh', 0)):.3f} Hz"),
                 ("Scale", f"{o.get('scale', 1) * 100:.0f}%"),
                 ("Adaptive sync", str(o.get("adaptive_sync_status", "disabled")).capitalize())):
        d.body.pack_start(ui.texts(k, v), False, False, 0)

    d.body.pack_start(ui.section("Mode"), False, False, 0)
    current = [output_cmd(o)]
    pref_btn = ui.css(Gtk.Button(label="Set preferred"), "btn")
    pref_btn.connect("clicked", lambda *_: keep_changes(
        app, [f"output {o['name']} preferred"], current))
    d.body.pack_start(ui.row("Preferred mode", "Use the mode recommended by the display",
                             "video-display-symbolic", pref_btn), False, False, 0)

    sync_on = str(o.get("adaptive_sync_status", "disabled")).lower() == "enabled"
    d.body.pack_start(ui.row("Adaptive sync", "Variable refresh rate for this display",
                             "video-display-symbolic",
                             ui.switch(sync_on, lambda v: keep_changes(
                                 app, [f"output {o['name']} adaptive_sync {'enabled' if v else 'disabled'}"],
                                 [f"output {o['name']} adaptive_sync {'disabled' if v else 'enabled'}"]))),
                      False, False, 0)

    d.body.pack_start(ui.section("When a display is connected"), False, False, 0)
    s = ui.load()

    def set_auto(v):
        ui.save(auto_enable_outputs=v)
        ui.sway_conf("auto_enable_outputs", f"autoEnableOutputs {'yes' if v else 'no'}")

    def set_ask(v):
        ui.save(ask_on_display_connect=v)

    def set_reuse(v):
        ui.save(reuse_output_mode=v)
        ui.sway_conf("reuse_output_mode", f"reuseOutputMode {'yes' if v else 'no'}")

    d.body.pack_start(ui.row("Automatically enable new displays",
                             "Maps to labwc autoEnableOutputs",
                             "video-display-symbolic",
                             ui.switch(s.get("auto_enable_outputs", True), set_auto)), False, False, 0)
    d.body.pack_start(ui.row("Ask me what to do when a display is connected",
                             "Shows Extend, Duplicate and related choices",
                             "dialog-question-symbolic",
                             ui.switch(s.get("ask_on_display_connect", False), set_ask)), False, False, 0)
    d.body.pack_start(ui.row("Reuse output mode", "Flicker-free boot when possible",
                             "video-display-symbolic",
                             ui.switch(s.get("reuse_output_mode", False), set_reuse)), False, False, 0)

    d.body.pack_start(ui.section("Saved layouts"), False, False, 0)

    def save_layout():
        if ui.which("ws-display-save"):
            ui.run(["ws-display-save"])
            GLib.timeout_add(400, lambda: (d.destroy(), show_advanced_display(app, o, outs)) or False)

    save_btn = ui.css(Gtk.Button(label="Save current layout"), "btn", "accent")
    save_btn.connect("clicked", lambda *_: save_layout())
    d.body.pack_start(ui.row("Save", "Store this connector combination for kanshi",
                             "document-save-symbolic", save_btn), False, False, 0)
    for prof in kanshi_profiles():
        box = Gtk.Box(spacing=8)
        apply_btn = ui.css(Gtk.Button(label="Apply"), "btn")
        apply_btn.connect("clicked", lambda *_ , n=prof["name"]: apply_kanshi_profile(n))
        del_btn = ui.css(Gtk.Button(label="Delete"), "btn")
        del_btn.connect("clicked", lambda *_ , n=prof["name"]: (
            delete_kanshi_profile(n), d.destroy(), show_advanced_display(app, o, outs)))
        box.pack_start(apply_btn, False, False, 0)
        box.pack_start(del_btn, False, False, 0)
        d.body.pack_start(ui.row(prof["name"], "Saved connector combination",
                                 "video-display-symbolic", box), False, False, 0)

    d.body.pack_start(ui.section("Tools"), False, False, 0)
    if ui.which("wdisplays"):
        open_btn = ui.css(Gtk.Button(label="Open wdisplays"), "btn")
        open_btn.connect("clicked", lambda *_: ui.run(["wdisplays"]))
        d.body.pack_start(ui.row("wdisplays", "External layout tool for edge cases",
                                 "preferences-desktop-display", open_btn), False, False, 0)

    d.body.pack_start(ui.section("Scale"), False, False, 0)
    d.body.pack_start(ui.infobar(
        "150% scale is unavailable until pointer hit-testing is fixed for GTK3 under labwc."),
        False, False, 0)
    d.run_dialog()


# --- Main Display page --------------------------------------------------------

def build_display(app, body):
    outs = outputs()
    active = [o for o in outs if o.get("active")]
    s = ui.load()

    if len(outs) > 1:
        frame = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8), "hero")
        names = [o["name"] for o in outs]
        chosen = getattr(app, "display_choice", None)
        if chosen not in names:
            chosen = next((o["name"] for o in outs if o.get("focused")), names[0])

        def on_select(name):
            app.display_choice = name
            app.refresh()

        def on_menu(o, event):
            menu = Gtk.Menu()
            items = [
                ("Identify", lambda: identify_displays(active)),
                ("Make this my main display", lambda: set_main_display(app, outs, o["name"])),
                ("Disconnect", lambda: keep_changes(
                    app, [f"output {o['name']} disable"], [output_cmd(o)])
                 if len(active) > 1 else None),
            ]
            for label, cb in items:
                item = Gtk.MenuItem(label=label)
                if cb:
                    item.connect("activate", lambda *_ , c=cb: c())
                else:
                    item.set_sensitive(False)
                menu.append(item)
            rot = Gtk.MenuItem(label="Rotate")
            sub = Gtk.Menu()
            for key, title in ORIENTATIONS[:4]:
                it = Gtk.MenuItem(label=title)
                it.connect("activate", lambda *_ , t=key, out=o: keep_changes(
                    app, [output_cmd(out, transform=t)], [output_cmd(out)]))
                sub.append(it)
            rot.set_submenu(sub)
            menu.append(rot)
            menu.show_all()
            menu.popup_at_pointer(event)

        canvas = Arrangement(outs, chosen, lambda pos: arrange(app, active, pos),
                             on_select, on_menu)
        frame.pack_start(canvas, False, False, 0)
        row = Gtk.Box(spacing=8)
        row.pack_start(ui.label("Select and drag displays to rearrange them.", "caption"), True, True, 0)
        id_btn = ui.css(Gtk.Button(label="Identify"), "btn")
        id_btn.connect("clicked", lambda *_: identify_displays(active))
        row.pack_start(id_btn, False, False, 0)
        frame.pack_start(row, False, False, 0)

        # Precise X/Y for selected active display
        sel = next((o for o in active if o["name"] == chosen), active[0] if active else None)
        if sel:
            pos_box = Gtk.Box(spacing=8)

            def nudge(axis, value):
                pos = {o["name"]: (o["rect"]["x"], o["rect"]["y"]) for o in active}
                x, y = pos[sel["name"]]
                if axis == "x":
                    pos[sel["name"]] = (int(value), y)
                else:
                    pos[sel["name"]] = (x, int(value))
                arrange(app, active, pos)

            sx = Gtk.SpinButton.new_with_range(-10000, 10000, 1)
            sx.set_value(sel["rect"]["x"])
            sx.connect("value-changed", lambda w: nudge("x", w.get_value()))
            sy = Gtk.SpinButton.new_with_range(-10000, 10000, 1)
            sy.set_value(sel["rect"]["y"])
            sy.connect("value-changed", lambda w: nudge("y", w.get_value()))
            pos_box.pack_start(ui.label("X"), False, False, 0)
            pos_box.pack_start(sx, False, False, 0)
            pos_box.pack_start(ui.label("Y"), False, False, 0)
            pos_box.pack_start(sy, False, False, 0)
            frame.pack_start(ui.row("Position", "Arrow keys also nudge the selected display",
                                    None, pos_box), False, False, 0)
        body.pack_start(frame, False, False, 0)

        # Multiple displays / projection
        body.pack_start(ui.section("Multiple displays"), False, False, 0)
        proj_opts = [("extend", "Extend these displays"),
                     ("duplicate", "Duplicate these displays")]
        for i, o in enumerate(outs, 1):
            label = f"{i}. {o.get('make', '')} {o.get('model', '')}".strip() or o["name"]
            proj_opts.append((f"only:{o['name']}", f"Show only on {i} ({label})"))
        current_proj = projection_mode(outs)
        body.pack_start(ui.row("Multiple displays",
                               "Extend your desktop or show the same content on each display",
                               "video-joined-displays-symbolic",
                               ui.combo(proj_opts, current_proj,
                                        lambda m: apply_projection(app, outs, m), 280)),
                        False, False, 0)

    # Brightness & colour
    body.pack_start(ui.section("Brightness & colour"), False, False, 0)
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
                               ui.combo([(o["name"], f"{i}. {o.get('make', '')} {o.get('model', '')} ({o['name']})".strip())
                                         for i, o in enumerate(outs, 1)], chosen, pick, 300)),
                        False, False, 0)
    o = next(x for x in outs if x["name"] == chosen)
    if not o.get("current_mode"):
        o = {**o, "current_mode": {"width": 0, "height": 0, "refresh": 0}}
    if not o.get("rect"):
        o = {**o, "rect": {"x": 0, "y": 0, "width": 0, "height": 0}}
    if len(outs) > 1:
        body.pack_start(ui.row("Use this display", None, "video-display-symbolic",
                               ui.switch(o.get("active"), lambda v: keep_changes(
                                   app, [f"output {o['name']} {'enable' if v else 'disable'}"],
                                   [f"output {o['name']} {'disable' if v else 'enable'}"]),
                                   sensitive=len(active) > 1 or not o.get("active"))), False, False, 0)

    level, setter = brightness_for(o) if o.get("active") else (None, None)
    if level is not None and setter:
        body.pack_start(ui.row("Brightness", "Adjust the brightness of the selected display",
                               "display-brightness-symbolic",
                               ui.slider(level, 1, 100, setter, live=True)), False, False, 0)

    pack_nightlight(app, body)
    color_btn = ui.css(Gtk.Button(label="Colour management"), "btn")
    color_btn.connect("clicked", lambda *_: show_color_management(app, o))
    body.pack_start(ui.row("Colour profile", "ICC profiles and HDR when available",
                           "preferences-color-symbolic", color_btn), False, False, 0)

    if not o.get("active"):
        body.pack_start(ui.section("Related settings"), False, False, 0)
        body.pack_start(ui.nav_row("Advanced display", o["name"],
                                   "preferences-desktop-display",
                                   lambda: show_advanced_display(app, o, outs)), False, False, 0)
        return

    # Scale & layout
    body.pack_start(ui.section("Scale & layout"), False, False, 0)
    current = [output_cmd(o)]
    scale_pct = round(float(o.get("scale", 1)) * 100)
    if scale_pct not in SCALES:
        scale_pct = 200 if scale_pct >= 150 else 100
    body.pack_start(ui.row("Scale", "Change the size of text, apps and other items",
                           "zoom-in-symbolic",
                           ui.combo([(s, f"{s}%") for s in SCALES], scale_pct,
                                    lambda s: keep_changes(app, [output_cmd(o, scale=s / 100)], current))),
                    False, False, 0)

    modes = {}
    preferred_key = None
    for m in sorted(o.get("modes") or [], key=lambda m: (-m["width"] * m["height"], -m["refresh"])):
        key = (m["width"], m["height"])
        modes.setdefault(key, []).append(m)
        if m.get("preferred") and preferred_key is None:
            preferred_key = key
    biggest = next(iter(modes), None)
    cur = (o["current_mode"]["width"], o["current_mode"]["height"]) if o.get("current_mode") else biggest
    res_opts = []
    for k in modes:
        label = f"{k[0]} \u00d7 {k[1]}"
        if k == preferred_key or (preferred_key is None and k == biggest):
            label += " (Recommended)"
        res_opts.append((k, label))
    if modes:
        body.pack_start(ui.row("Display resolution",
                               "Adjust the resolution to fit your connected display",
                               "video-display-symbolic",
                               ui.combo(res_opts, cur,
                                        lambda k: keep_changes(app, [output_cmd(o, mode=modes[k][0])],
                                                               current), 220)),
                        False, False, 0)
        pref_btn = ui.css(Gtk.Button(label="Preferred"), "btn")
        pref_btn.connect("clicked", lambda *_: keep_changes(
            app, [f"output {o['name']} preferred"], current))
        body.pack_start(ui.row("Recommended mode", "Use the preferred mode from the display",
                               "emblem-ok-symbolic", pref_btn), False, False, 0)

    body.pack_start(ui.row("Display orientation", None, "object-rotate-right-symbolic",
                           ui.combo(ORIENTATIONS, o.get("transform", "normal"),
                                    lambda t: keep_changes(app, [output_cmd(o, transform=t)], current))),
                    False, False, 0)

    rates = modes.get(cur, []) if cur else []
    if len(rates) > 1:
        body.pack_start(ui.row("Choose a refresh rate",
                               "A higher rate gives smoother motion, but also uses more power",
                               "video-display-symbolic",
                               ui.combo([(m["refresh"], f"{refresh_hz(m['refresh']):.2f} Hz") for m in rates],
                                        o["current_mode"]["refresh"],
                                        lambda r: keep_changes(app, [output_cmd(o, mode=next(
                                            m for m in rates if m["refresh"] == r))], current))),
                        False, False, 0)

    is_main = s.get("main_display") == o["name"] or (
        not s.get("main_display") and o.get("focused"))
    main_sw = ui.switch(is_main, lambda v: set_main_display(app, outs, o["name"]) if v else None,
                        sensitive=not is_main)
    body.pack_start(ui.row("Make this my main display",
                           "Taskbar, Start and new windows use this display",
                           "go-home-symbolic", main_sw), False, False, 0)

    sync_on = str(o.get("adaptive_sync_status", "disabled")).lower() == "enabled"
    body.pack_start(ui.row("Adaptive sync", "Variable refresh rate when the display supports it",
                           "video-display-symbolic",
                           ui.switch(sync_on, lambda v: keep_changes(
                               app,
                               [f"output {o['name']} adaptive_sync {'enabled' if v else 'disabled'}"],
                               [f"output {o['name']} adaptive_sync {'disabled' if v else 'enabled'}"]))),
                    False, False, 0)

    body.pack_start(ui.row("Display scaling mode",
                           "Unavailable on this compositor",
                           "video-display-symbolic",
                           ui.label("Unavailable", "caption")), False, False, 0)
    # Disabled control look: keep the row but no interactive widget beyond caption.

    body.pack_start(ui.section("Related settings"), False, False, 0)
    body.pack_start(ui.nav_row("Advanced display",
                               f"{o.get('make', '')} {o.get('model', '')}, "
                               f"{o.get('current_mode', {}).get('width', 0)}\u00d7"
                               f"{o.get('current_mode', {}).get('height', 0)} at "
                               f"{refresh_hz(o.get('current_mode', {}).get('refresh', 0)):.0f} Hz",
                               "preferences-desktop-display",
                               lambda: show_advanced_display(app, o, outs)), False, False, 0)
    body.pack_start(ui.nav_row("Text size", "Make text larger throughout the system",
                               "format-text-larger", lambda: app.go("textsize")), False, False, 0)
    body.pack_start(ui.nav_row("Visual effects", "Animation and transparency",
                               "applications-graphics", lambda: app.go("visual")), False, False, 0)
    body.pack_start(ui.nav_row("Contrast themes", "High contrast for better visibility",
                               "preferences-desktop-accessibility",
                               lambda: app.go("contrast")), False, False, 0)


PAGES = [
    dict(key="display", title="Display", parent="system", icon="video-display",
         caption="Monitors, brightness, night light, display profile",
         keywords="screen monitor resolution scale refresh rate orientation rotate brightness hdr colour",
         build=build_display, aliases=("screen", "displays")),
    dict(key="nightlight", title="Night light", parent="display", icon="night-light",
         caption="Warmer colours at night", keywords="blue light warm red evening sunset",
         build=build_nightlight, aliases=("night-light", "blue-light"), inline=True),
]
