#!/usr/bin/env python3
"""Minimize, Maximize, Tile/Tabs/Float and Close for the focused window.

The buttons sit at the top-right of the focused app window, on its title bar
when it has one and over the corner when the border is only a pixel. Dialogs,
menus and shell surfaces (taskbar, Start, flyouts) are left alone. Turn them
off in Settings > Personalization > Window Layout.
"""
import fcntl
import os
import sys

import gi

gi.require_version("Gdk", "3.0")
gi.require_version("Gtk", "3.0")
gi.require_version("GtkLayerShell", "0.1")
from gi.repository import Gdk, GLib, Gtk, GtkLayerShell  # noqa: E402

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))
sys.path.insert(0, "/usr/share/mde/ws-settings")

import cairo  # noqa: E402

from common import HEIGHT, SWAY, is_dark, minimize, settings  # noqa: E402

RUNTIME = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
LOCK = os.path.join(RUNTIME, "ws-window-buttons.lock")
MAX_FILE = os.path.join(RUNTIME, "ws-window-buttons-max.json")
LOCK_READY = os.path.join(RUNTIME, "ws-lockscreen.ready")

BTN_W = 46
BUTTONS = ("cycle", "minimize", "maximize", "close")
CLOSE_RED = (196 / 255, 43 / 255, 28 / 255)
# A window thinner than this cannot hold the four buttons.
MIN_FRAME = BTN_W * 2

SHELL_IDS = {
    "ws-taskbar", "ws-start", "ws-watermark", "ws-winx", "ws-confirm",
    "ws-drawer", "ws-window-buttons", "ws-osd", "ws-power-menu",
    "swaync", "wofi", "nwg-bar", "swayosd", "nwg-drawer",
    "polkit-gnome-authentication-agent-1", "lxpolkit",
}
DIALOG_WORDS = ("dialog", "menu", "popup", "pop-up", "tooltip", "notification",
                "combo", "dnd", "splash", "utility", "dropdown", "bubble")


def hold_lock():
    handle = open(LOCK, "w")
    try:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        sys.exit(0)
    return handle


LOCK_FILE = None


def buttons_enabled():
    """Window Layout switch. Missing schema or key means on, which is the default."""
    try:
        import mde_gsettings as gs
        return bool(gs.window_layout_get("show-window-buttons"))
    except Exception:
        return True


def _words(value):
    return (value or "").replace("_", " ").lower()


def is_dialog(node):
    props = node.get("window_properties") or {}
    blob = _words(props.get("window_type")) + " " + _words(props.get("window_role"))
    return any(word in blob for word in DIALOG_WORDS)


def is_shell(node):
    props = node.get("window_properties") or {}
    app_id = node.get("app_id") or props.get("class") or props.get("instance") or ""
    return app_id.casefold() in SHELL_IDS


def is_floating(node, parent):
    if parent and parent.get("type") == "floating_con":
        return True
    if node.get("type") == "floating_con":
        return True
    mode = node.get("floating") or ""
    return isinstance(mode, str) and mode.endswith("_on")


def cycle_state(node, parent):
    """tiled, then tabbed, then floating."""
    if is_floating(node, parent):
        return "floating"
    layout = (parent or {}).get("layout") or ""
    if layout == "tabbed":
        return "tabbed"
    return "tiled"


def next_cycle(state):
    return {"tiled": "tabbed", "tabbed": "floating", "floating": "tiled"}[state]


def frame_of(node, parent):
    """The rectangle the buttons sit on: the shared frame for tabs, else the window."""
    if parent and parent.get("layout") in ("tabbed", "stacked") and parent.get("rect"):
        return parent["rect"], parent
    if parent and parent.get("type") == "floating_con" and parent.get("rect"):
        return parent["rect"], parent
    return node.get("rect") or {}, node


def title_height(node, parent):
    deco = node.get("deco_rect") or {}
    return int(deco.get("height") or 0)


def find_focused(tree):
    found = {}

    def walk(node, ws, output, parent):
        kind = node.get("type")
        if kind == "output":
            output = node.get("name")
        elif kind == "workspace":
            ws = node.get("name")
        if (node.get("pid") and kind in ("con", "floating_con") and node.get("focused")
                and node.get("visible", True)):
            found["node"] = node
            found["parent"] = parent
            found["ws"] = ws
            found["output"] = output
        for child in node.get("nodes", []):
            walk(child, ws, output, node)
        for child in node.get("floating_nodes", []):
            walk(child, ws, output, node)

    walk(tree, None, None, None)
    return found or None


def eligible(info):
    if not info:
        return False
    node = info["node"]
    if info.get("ws") in ("__i3_scratch", "desktop"):
        return False
    if node.get("scratchpad_state") not in (None, "none"):
        return False
    if is_shell(node) or is_dialog(node):
        return False
    rect, _owner = frame_of(node, info.get("parent"))
    if rect.get("width", 0) < MIN_FRAME or rect.get("height", 0) < 32:
        return False
    return True


def work_area(output_name):
    """Output rectangle above the taskbar. Auto-hide uses the full output."""
    outputs = [o for o in SWAY.outputs() if o.get("active")]
    target = next((o for o in outputs if o.get("name") == output_name), None)
    if not target:
        return None
    rect = target["rect"]
    x, y, w, h = rect["x"], rect["y"], rect["width"], rect["height"]
    conf = settings()
    if not conf.get("taskbar_autohide"):
        everywhere = conf.get("taskbar_all_displays", True)
        main = (conf.get("main_display") or "").strip()
        primary = main if main in {o["name"] for o in outputs} else (
            outputs[0]["name"] if outputs else None)
        if everywhere or output_name == primary:
            h -= HEIGHT
    if w < 64 or h < 64:
        return None
    return x, y, w, h


def near(rect, box, tol=10):
    if not rect or not box:
        return False
    x, y, w, h = box
    return (abs(rect.get("x", 0) - x) <= tol and abs(rect.get("y", 0) - y) <= tol
            and abs(rect.get("width", 0) - w) <= tol and abs(rect.get("height", 0) - h) <= tol)


def load_max():
    from common import load_json
    data = load_json(MAX_FILE, {})
    return data if isinstance(data, dict) else {}


def save_max(data):
    from common import save_json
    save_json(MAX_FILE, data)


def describe(info):
    """What the four buttons will do to this window, for glyphs and tooltips."""
    node, parent = info["node"], info.get("parent")
    state = cycle_state(node, parent)
    floating = state == "floating"
    fullscreen = int(node.get("fullscreen_mode") or 0) > 0
    rect, owner = frame_of(node, parent)
    area = work_area(info.get("output"))
    filled = floating and near(rect, area)
    if fullscreen or filled:
        max_tip, restore = "Restore", True
    elif floating:
        max_tip, restore = "Maximize", False
    else:
        max_tip, restore = "Full screen", False
    nxt = next_cycle(state)
    cycle_tip = {"tabbed": "Tabs", "floating": "Float", "tiled": "Tile"}[nxt]
    return {
        "id": node["id"],
        "frame_id": owner.get("id", node["id"]) if owner else node["id"],
        "state": state,
        "floating": floating,
        "fullscreen": fullscreen,
        "filled": filled,
        "restore": restore,
        "rect": rect,
        "area": area,
        "output": info.get("output"),
        "title_h": title_height(node, parent),
        "tips": (cycle_tip, "Minimize", max_tip, "Close"),
        "on_accent": _on_accent(node, parent, rect),
    }


def _on_accent(node, parent, frame):
    """The strip is on the focused accent title bar or tab, so the glyphs are white."""
    deco = node.get("deco_rect") or {}
    if int(deco.get("height") or 0) < 8 or not frame:
        return False
    if parent and parent.get("layout") in ("tabbed", "stacked"):
        base = parent.get("rect") or frame
    else:
        base = node.get("rect") or frame
    dx = base["x"] + deco.get("x", 0)
    dy = base["y"] + deco.get("y", 0)
    dw, dh = deco.get("width", 0), deco.get("height", 0)
    sx = frame["x"] + frame["width"] - BTN_W * len(BUTTONS)
    sy = frame["y"]
    return not (sx + BTN_W * len(BUTTONS) <= dx or dx + dw <= sx or sy + dh <= dy or dy + dh <= sy)


def run_cycle(target):
    cid = target["id"]
    state = target["state"]
    if state == "floating":
        # Back to a side-by-side tile. layout splith is only needed when the
        # window lands in a tab or stack; it would otherwise retile the neighbours.
        SWAY.command(f"[con_id={cid}] floating disable")
        _node, parent = _node_by_id(SWAY.tree(), cid)
        if parent and parent.get("layout") in ("tabbed", "stacked"):
            SWAY.command(f"[con_id={cid}] layout splith")
    elif state == "tabbed":
        SWAY.command(f"[con_id={cid}] floating enable")
    else:
        SWAY.command(f"[con_id={cid}] layout tabbed")


def _node_by_id(tree, cid):
    found = {}

    def walk(node, parent=None):
        if node.get("id") == cid:
            found["node"] = node
            found["parent"] = parent
        for child in node.get("nodes", []) + node.get("floating_nodes", []):
            walk(child, node)

    walk(tree)
    return found.get("node"), found.get("parent")


def place_floating(cid, x, y, w, h):
    """Size a floating container to a rectangle. resize set grows from the centre,
    so the position is applied after the size, then once more to cancel the gap
    left by the title bar."""
    x, y, w, h = int(x), int(y), int(w), int(h)

    def once(px, py, pw, ph):
        SWAY.command(
            f"[con_id={cid}] resize set {pw} px {ph} px, "
            f"move absolute position {px} px {py} px")

    once(x, y, w, h)
    node, _parent = _node_by_id(SWAY.tree(), cid)
    rect = (node or {}).get("rect") or {}
    if not rect:
        return
    dx = x - int(rect.get("x", x))
    dy = y - int(rect.get("y", y))
    dw = w - int(rect.get("width", w))
    dh = h - int(rect.get("height", h))
    if abs(dx) < 2 and abs(dy) < 2 and abs(dw) < 2 and abs(dh) < 2:
        return
    once(x + dx, y + dy, w + dw, h + dh)


def run_maximize(target):
    cid = target["id"]
    # Move and resize the floating frame. Kill, fullscreen and layout stay on the view.
    frame_id = target.get("frame_id") or cid
    if target["fullscreen"]:
        SWAY.command(f"[con_id={cid}] fullscreen disable")
        return
    if not target["floating"]:
        SWAY.command(f"[con_id={cid}] fullscreen enable")
        return
    area = target.get("area")
    rect = target.get("rect") or {}
    if not area:
        SWAY.command(f"[con_id={cid}] fullscreen toggle")
        return
    saved = load_max()
    key = str(cid)
    if target["filled"]:
        box = saved.get(key)
        if not box:
            x, y, w, h = area
            box = {"x": x + w // 6, "y": y + h // 6, "width": w * 2 // 3, "height": h * 2 // 3}
        place_floating(frame_id, box["x"], box["y"], box["width"], box["height"])
        saved.pop(key, None)
        save_max(saved)
        return
    saved[key] = {"x": rect.get("x", 0), "y": rect.get("y", 0),
                  "width": rect.get("width", 640), "height": rect.get("height", 480)}
    save_max(saved)
    x, y, w, h = area
    place_floating(frame_id, x, y, w, h)


def run_button(target, kind):
    cid = target["id"]
    if kind == "cycle":
        run_cycle(target)
    elif kind == "minimize":
        minimize({"id": cid, "floating": target["floating"], "workspace": target.get("workspace")})
    elif kind == "maximize":
        run_maximize(target)
    elif kind == "close":
        SWAY.command(f"[con_id={cid}] kill")


def _round_join(cr):
    cr.set_line_cap(cairo.LINE_CAP_ROUND)
    cr.set_line_join(cairo.LINE_JOIN_ROUND)


def _trace(cr, kind, cx, cy, restore):
    if kind == "minimize":
        cr.move_to(cx - 5, cy)
        cr.line_to(cx + 5, cy)
    elif kind == "maximize" and restore:
        cr.rectangle(cx - 2, cy - 5.5, 7.5, 7.5)
        cr.rectangle(cx - 5.5, cy - 2, 7.5, 7.5)
    elif kind == "maximize":
        cr.rectangle(cx - 5, cy - 5, 10, 10)
    elif kind == "close":
        cr.move_to(cx - 4.5, cy - 4.5)
        cr.line_to(cx + 4.5, cy + 4.5)
        cr.move_to(cx + 4.5, cy - 4.5)
        cr.line_to(cx - 4.5, cy + 4.5)
    else:
        # A window with a second window sliding out: tile, tabs or float.
        cr.rectangle(cx - 6, cy - 2.5, 8, 6.5)
        cr.rectangle(cx - 1.5, cy - 6, 6.5, 5)


def _stroke(cr, kind, cx, cy, color, halo, restore):
    _round_join(cr)
    cr.set_line_width(2.6)
    cr.set_source_rgba(halo[0], halo[1], halo[2], 0.55)
    _trace(cr, kind, cx, cy, restore)
    cr.stroke()
    cr.set_line_width(1.15)
    cr.set_source_rgb(*color)
    _trace(cr, kind, cx, cy, restore)
    cr.stroke()


class WindowButtons:
    def __init__(self):
        self.target = None
        self.hover = None
        self.pressed = None
        self.dark = is_dark()
        self.win = Gtk.Window()
        GtkLayerShell.init_for_window(self.win)
        GtkLayerShell.set_namespace(self.win, "ws-window-buttons")
        GtkLayerShell.set_layer(self.win, GtkLayerShell.Layer.TOP)
        GtkLayerShell.set_keyboard_mode(self.win, GtkLayerShell.KeyboardMode.NONE)
        GtkLayerShell.set_exclusive_zone(self.win, 0)
        GtkLayerShell.set_anchor(self.win, GtkLayerShell.Edge.TOP, True)
        GtkLayerShell.set_anchor(self.win, GtkLayerShell.Edge.LEFT, True)
        visual = self.win.get_screen().get_rgba_visual()
        if visual:
            self.win.set_visual(visual)
        self.win.set_app_paintable(True)
        self.win.set_decorated(False)
        self.win.set_accept_focus(False)
        self.win.set_resizable(False)
        css = Gtk.CssProvider()
        css.load_from_data(b"window, drawingarea { background-color: transparent; }")
        Gtk.StyleContext.add_provider_for_screen(
            self.win.get_screen(), css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        self.area = Gtk.DrawingArea()
        self.area.set_size_request(BTN_W * len(BUTTONS), 32)
        self.area.add_events(Gdk.EventMask.BUTTON_PRESS_MASK | Gdk.EventMask.BUTTON_RELEASE_MASK
                             | Gdk.EventMask.POINTER_MOTION_MASK | Gdk.EventMask.LEAVE_NOTIFY_MASK)
        self.area.connect("draw", self.on_draw)
        self.area.connect("button-press-event", self.on_press)
        self.area.connect("button-release-event", self.on_release)
        self.area.connect("motion-notify-event", self.on_motion)
        self.area.connect("leave-notify-event", self.on_leave)
        self.win.connect("draw", self.clear)
        self.win.add(self.area)
        self.win.show_all()
        self.win.hide()
        self.monitor = None
        SWAY.subscribe(["window", "workspace", "output"], lambda *_: self.refresh())
        GLib.timeout_add(400, self.refresh)
        self.refresh()

    def clear(self, _win, cr):
        cr.set_operator(cairo.OPERATOR_SOURCE)
        cr.set_source_rgba(0, 0, 0, 0)
        cr.paint()
        cr.set_operator(cairo.OPERATOR_OVER)
        return False

    def button_at(self, x):
        width = getattr(self, "btn_w", BTN_W) or BTN_W
        if self.target is None or x < 0:
            return None
        index = int(x // width)
        if index < 0 or index >= len(BUTTONS):
            return None
        return index

    def on_motion(self, _w, event):
        index = self.button_at(event.x)
        if index == self.hover:
            return False
        self.hover = index
        tip = ""
        if index is not None and self.target:
            tip = self.target["tips"][index]
        self.win.set_tooltip_text(tip or None)
        self.area.queue_draw()
        return False

    def on_leave(self, *_a):
        self.hover = None
        self.pressed = None
        self.area.queue_draw()
        return False

    def on_press(self, _w, event):
        if event.button != 1:
            return False
        self.pressed = self.button_at(event.x)
        self.area.queue_draw()
        return True

    def on_release(self, _w, event):
        index = self.button_at(event.x)
        pressed = self.pressed
        self.pressed = None
        self.area.queue_draw()
        if event.button != 1 or index is None or index != pressed or not self.target:
            return True
        kind = BUTTONS[index]
        target = dict(self.target)
        info = find_focused(SWAY.tree())
        if info:
            target["workspace"] = info.get("ws")
        run_button(target, kind)
        GLib.timeout_add(80, self.refresh)
        return True

    def on_draw(self, _w, cr):
        target = self.target
        if not target:
            return False
        height = self.area.get_allocated_height()
        width = getattr(self, "btn_w", BTN_W) or BTN_W
        dark = self.dark
        on_accent = target["on_accent"]
        if on_accent or dark:
            color, halo = (1, 1, 1), (0, 0, 0)
            wash = (1, 1, 1, 0.14)
        else:
            color, halo = (0.12, 0.12, 0.12), (1, 1, 1)
            wash = (0, 0, 0, 0.06)
        restore = target["restore"]
        for i, kind in enumerate(BUTTONS):
            x = i * width
            hot = self.hover == i
            down = hot and self.pressed == i
            if kind == "close" and hot:
                cr.set_source_rgb(*CLOSE_RED)
                cr.rectangle(x, 0, width, height)
                cr.fill()
                glyph, shade = (1, 1, 1), (0.4, 0, 0)
            else:
                glyph, shade = color, halo
                if hot:
                    cr.set_source_rgba(*wash)
                    if down:
                        cr.set_source_rgba(wash[0], wash[1], wash[2], min(0.28, wash[3] + 0.1))
                    cr.rectangle(x, 0, width, height)
                    cr.fill()
            _stroke(cr, kind, x + width / 2, height / 2, glyph, shade, restore)
        return False

    def place(self, info, target):
        rect = target["rect"]
        outputs = SWAY.outputs()
        display = Gdk.Display.get_default()
        monitor = None
        geo = None
        sway_rect = None
        for output in outputs:
            if output.get("name") != info.get("output"):
                continue
            sway_rect = output.get("rect") or {}
            for i in range(display.get_n_monitors()):
                mon = display.get_monitor(i)
                candidate = mon.get_geometry()
                if candidate.x == sway_rect.get("x") and candidate.y == sway_rect.get("y"):
                    monitor, geo = mon, candidate
                    break
        if monitor is None or geo is None:
            self.win.hide()
            return
        title_h = target["title_h"]
        btn_h = title_h if title_h >= 24 else 32
        room = int(rect["width"]) - (0 if title_h >= 24 else 8)
        btn_w = BTN_W if room >= BTN_W * len(BUTTONS) else room // len(BUTTONS)
        if btn_w < 28:
            self.win.hide()
            return
        self.btn_w = btn_w
        strip_w = btn_w * len(BUTTONS)
        if title_h >= 24:
            ox = rect["x"] + rect["width"] - strip_w
            oy = rect["y"] + max(0, (title_h - btn_h) // 2)
        else:
            ox = rect["x"] + rect["width"] - strip_w - 4
            oy = rect["y"] + 4
        # Keep the strip inside the window and the monitor.
        ox = min(max(rect["x"], ox), rect["x"] + rect["width"] - strip_w)
        oy = min(max(rect["y"], oy), rect["y"] + rect["height"] - btn_h)
        margin_l = int(ox - geo.x)
        margin_t = int(oy - geo.y)
        if margin_l < 0 or margin_t < 0:
            self.win.hide()
            return
        if monitor != self.monitor:
            GtkLayerShell.set_monitor(self.win, monitor)
            self.monitor = monitor
        layer = GtkLayerShell.Layer.OVERLAY if target["fullscreen"] else GtkLayerShell.Layer.TOP
        GtkLayerShell.set_layer(self.win, layer)
        GtkLayerShell.set_margin(self.win, GtkLayerShell.Edge.LEFT, margin_l)
        GtkLayerShell.set_margin(self.win, GtkLayerShell.Edge.TOP, margin_t)
        self.area.set_size_request(strip_w, btn_h)
        if not self.win.get_visible():
            self.win.show_all()

    def refresh(self):
        try:
            self._refresh()
        except Exception:
            self.win.hide()
        return True

    def _refresh(self):
        self.dark = is_dark()
        if not buttons_enabled() or os.path.exists(LOCK_READY):
            self.target = None
            self.win.hide()
            return
        info = find_focused(SWAY.tree())
        if not eligible(info):
            self.target = None
            self.win.hide()
            return
        target = describe(info)
        target["workspace"] = info.get("ws")
        self.target = target
        self.place(info, target)
        self.area.queue_draw()


def main():
    # labwc draws the Windows 11 caption (icon, title, minimize, maximize, close).
    # The old overlay sat on borderless Sway windows and has no window geometry
    # to place itself with.
    return


if __name__ == "__main__":
    main()
