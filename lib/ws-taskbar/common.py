"""Shared pieces of the Windows 11 taskbar: settings, theme, Sway IPC, app
matching, icons and the flyout window."""

import json
import os
import shlex
import socket
import struct
import subprocess
import sys

import gi

gi.require_version("Gdk", "3.0")
gi.require_version("Gtk", "3.0")
gi.require_version("GtkLayerShell", "0.1")
from gi.repository import Gdk, Gio, GLib, Gtk, GtkLayerShell  # noqa: E402

sys.path.insert(0, "/usr/local/share/ws-settings")
try:
    import ws_theme
except ImportError:
    ws_theme = None
import ws_motion  # noqa: E402

HOME = os.path.expanduser("~")
CONFIG = os.environ.get("XDG_CONFIG_HOME", os.path.join(HOME, ".config"))
CACHE = os.environ.get("XDG_CACHE_HOME", os.path.join(HOME, ".cache"))
STATE_HOME = os.environ.get("XDG_STATE_HOME", os.path.join(HOME, ".local", "state"))
RUNTIME = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
CONF = os.path.join(CONFIG, "ws-taskbar")
SETTINGS_FILE = os.path.join(CONFIG, "ws-settings", "settings.json")
PINS_FILE = os.path.join(CONF, "pins")
DESKTOPS_FILE = os.path.join(CONF, "desktops.json")
NOTES_FILE = os.path.join(CACHE, "ws-taskbar", "notifications.json")
TRAY_SEEN_FILE = os.path.join(CACHE, "ws-taskbar", "tray-seen.json")
UPDATES_FILE = os.path.join(STATE_HOME, "ws-settings", "updates.json")
FOCUS_FILE = os.path.join(STATE_HOME, "ws-settings", "focus-until")
SOCK = os.path.join(RUNTIME, "ws-taskbar.sock")
GEOMETRY_FILE = os.path.join(RUNTIME, "ws-taskbar.json")

HEIGHT = 48
DEFAULT_PINS = ["thunar.desktop", "org.mozilla.firefox.desktop", "terminator.desktop",
                "ws-settings.desktop"]
DEFAULTS = {
    "taskbar_align": "center", "taskbar_search": "hide", "taskbar_taskview": True,
    "taskbar_autohide": False, "taskbar_badges": True, "taskbar_flash": True,
    "taskbar_all_displays": True, "taskbar_corner": True, "taskbar_combine": "always",
    "taskbar_all_desktops": False, "tray_hidden": ["nm-applet", "blueman"], "tray_overflow": True,
    "clock_seconds": False, "clock_24h": False, "night_light": False,
}


def load_json(path, default):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return default


def save_json(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(data, f, indent=1)
    os.replace(tmp, path)


def settings():
    return {**DEFAULTS, **load_json(SETTINGS_FILE, {})}


def save_setting(**kw):
    data = load_json(SETTINGS_FILE, {})
    data.update(kw)
    save_json(SETTINGS_FILE, data)


def load_pins():
    try:
        with open(PINS_FILE) as f:
            pins = [line.strip() for line in f if line.strip() and not line.startswith("#")]
        existed = True
    except OSError:
        pins = [p for p in DEFAULT_PINS if Gio.DesktopAppInfo.new(p)]
        existed = False
    try:
        import advanced_list
        kept = advanced_list.filter_named_pins(pins)
    except (ImportError, GLib.Error, OSError):
        return pins
    if existed and kept != pins:
        save_pins(kept)
    return kept


def advanced_blocked(desktop_id):
    """True when the app is listed in Settings > Advanced and must not stay pinned."""
    try:
        import advanced_list
        return advanced_list.is_blocked_desktop_id(desktop_id)
    except (ImportError, GLib.Error, OSError):
        return False


def save_pins(pins):
    os.makedirs(CONF, exist_ok=True)
    with open(PINS_FILE, "w") as f:
        f.write("".join(p + "\n" for p in pins))


def theme_css(css):
    if ws_theme:
        try:
            return ws_theme.transform(css)
        except Exception:
            pass
    return css


def accent_rgba():
    """The accent fill as (r, g, b, a) floats."""
    fill = "#0078d4"
    if ws_theme:
        try:
            fill = ws_theme.accents(ws_theme.load(), "windows_mode")[0]
        except Exception:
            pass
    rgba = Gdk.RGBA()
    rgba.parse(fill)
    return rgba.red, rgba.green, rgba.blue, 1.0


def is_dark():
    if ws_theme:
        try:
            return ws_theme.is_dark(which="windows_mode")
        except Exception:
            pass
    return False


# --- Processes ------------------------------------------------------------------------

def sway_exec(cmd):
    """Run detached from the taskbar so apps survive a taskbar restart."""
    subprocess.Popen(["swaymsg", "exec", "--", cmd], stdout=subprocess.DEVNULL,
                     stderr=subprocess.DEVNULL)


def launch(app_id):
    sway_exec("gtk-launch " + shlex.quote(app_id[:-8] if app_id.endswith(".desktop") else app_id))


def run(argv, done=None):
    """Run argv without blocking; done(ok, stdout) gets called with the result."""
    try:
        proc = Gio.Subprocess.new(argv, Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE)
    except GLib.Error:
        if done:
            done(False, "")
        return

    def finish(p, res):
        try:
            _ok, out, _err = p.communicate_utf8_finish(res)
        except GLib.Error:
            out = ""
        if done:
            done(p.get_successful(), out or "")
    proc.communicate_utf8_async(None, None, finish)


def out(argv):
    try:
        return subprocess.run(argv, capture_output=True, text=True, timeout=5).stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def which(cmd):
    return GLib.find_program_in_path(cmd) is not None


# --- Sway IPC --------------------------------------------------------------------------

MAGIC = b"i3-ipc"
RUN_COMMAND, GET_WORKSPACES, SUBSCRIBE, GET_OUTPUTS, GET_TREE, GET_INPUTS = 0, 1, 2, 3, 4, 100


class Sway:
    def __init__(self):
        self.path = os.environ.get("SWAYSOCK") or out(["sway", "--get-socketpath"]).strip()
        self.sock = None

    def _connect(self):
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.connect(self.path)
        return s

    @staticmethod
    def _send(s, kind, payload=""):
        data = payload.encode()
        s.sendall(MAGIC + struct.pack("=II", len(data), kind) + data)

    @staticmethod
    def _recv(s):
        head = b""
        while len(head) < 14:
            chunk = s.recv(14 - len(head))
            if not chunk:
                raise ConnectionError("sway closed the IPC socket")
            head += chunk
        length, kind = struct.unpack("=II", head[6:])
        body = b""
        while len(body) < length:
            chunk = s.recv(length - len(body))
            if not chunk:
                raise ConnectionError("sway closed the IPC socket")
            body += chunk
        return kind, json.loads(body)

    def request(self, kind, payload=""):
        for _attempt in range(2):
            try:
                if not self.sock:
                    self.sock = self._connect()
                self._send(self.sock, kind, payload)
                return self._recv(self.sock)[1]
            except OSError:
                self.sock = None
        return [] if kind != GET_TREE else {}

    def command(self, cmd):
        return self.request(RUN_COMMAND, cmd)

    def tree(self):
        return self.request(GET_TREE)

    def workspaces(self):
        return self.request(GET_WORKSPACES)

    def outputs(self):
        return self.request(GET_OUTPUTS)

    def subscribe(self, events, callback):
        """Call callback(kind, event) from the main loop for each event."""
        s = self._connect()
        self._send(s, SUBSCRIBE, json.dumps(events))
        self._recv(s)

        def on_ready(_fd, cond):
            if cond & (GLib.IO_HUP | GLib.IO_ERR):
                return False
            try:
                kind, event = self._recv(s)
            except (OSError, ValueError):
                return False
            callback(kind & 0x7fffffff, event)
            return True
        GLib.io_add_watch(s.fileno(), GLib.PRIORITY_DEFAULT, GLib.IO_IN | GLib.IO_HUP | GLib.IO_ERR,
                          on_ready)
        self.event_sock = s


SWAY = Sway()


def windows(tree=None):
    """Every application window: dicts with id, app_id, title, workspace, output, focused,
    urgent, floating, hidden (scratchpad or show-desktop) and pid."""
    found = []

    def walk(node, ws, output, floating=False):
        kind = node.get("type")
        if kind == "output":
            output = node.get("name")
        elif kind == "workspace":
            ws = node.get("name")
        if node.get("pid") and kind in ("con", "floating_con"):
            props = node.get("window_properties") or {}
            found.append({
                "id": node["id"], "pid": node["pid"],
                "app_id": node.get("app_id") or props.get("class") or props.get("instance") or "",
                "wm_class": props.get("class") or "",
                "title": node.get("name") or "", "workspace": ws, "output": output,
                "focused": node.get("focused", False), "urgent": node.get("urgent", False),
                "floating": floating or kind == "floating_con",
                "hidden": ws in ("__i3_scratch", "desktop"),
            })
        for child in node.get("nodes", []):
            walk(child, ws, output, floating)
        for child in node.get("floating_nodes", []):
            walk(child, ws, output, True)
    walk(tree if tree is not None else SWAY.tree(), None, None)
    return found


MINIMIZED_FILE = os.path.join(RUNTIME, "ws-taskbar-minimized.json")


def current_workspace():
    for ws in SWAY.workspaces():
        if ws.get("focused"):
            return ws["name"]
    return "1"


def minimize(win):
    """Sway has no minimise: park the window in the scratchpad and remember whether it was
    tiled so restoring puts it back."""
    state = load_json(MINIMIZED_FILE, {})
    state[str(win["id"])] = {"floating": win["floating"], "workspace": win["workspace"]}
    save_json(MINIMIZED_FILE, state)
    SWAY.command(f'[con_id={win["id"]}] move scratchpad')


def activate(win):
    cid = win["id"]
    if win["workspace"] == "__i3_scratch":
        state = load_json(MINIMIZED_FILE, {})
        info = state.pop(str(cid), {})
        save_json(MINIMIZED_FILE, state)
        cmd = f"[con_id={cid}] scratchpad show"
        if not info.get("floating", False):
            cmd += ", floating disable"
        SWAY.command(cmd + ", focus")
    elif win["workspace"] == "desktop":
        SWAY.command(f'[con_id={cid}] move container to workspace "{current_workspace()}", focus')
    else:
        SWAY.command(f"[con_id={cid}] focus")


def close_window(win):
    SWAY.command(f"[con_id={win['id']}] kill")


class Desktops:
    """Windows virtual desktops on Sway workspaces. Empty desktops are remembered (Sway
    drops empty workspaces) and shown as "Desktop N" unless renamed."""

    HIDDEN = ("desktop", "__i3_scratch")

    def __init__(self):
        self.state = load_json(DESKTOPS_FILE, {"order": ["1"], "names": {}})

    def save(self):
        save_json(DESKTOPS_FILE, self.state)

    def list(self):
        order = [w for w in self.state.get("order", []) if w not in self.HIDDEN]
        for ws in SWAY.workspaces():
            if ws["name"] not in order and ws["name"] not in self.HIDDEN:
                order.append(ws["name"])
        if not order:
            order = ["1"]
        if order != self.state.get("order"):
            self.state["order"] = order
            self.save()
        return order

    def name(self, ws):
        order = self.list()
        custom = self.state.get("names", {}).get(ws)
        return custom or f"Desktop {order.index(ws) + 1 if ws in order else ws}"

    def current(self):
        cur = current_workspace()
        return cur if cur not in self.HIDDEN else self.list()[0]

    def switch(self, ws):
        SWAY.command(f'workspace "{ws}"')

    def new(self, switch=True):
        order = self.list()
        nums = [int(w) for w in order if w.isdigit()]
        ws = str(max(nums, default=0) + 1)
        self.state["order"] = order + [ws]
        self.save()
        if switch:
            self.switch(ws)
        return ws

    def step(self, delta):
        order = self.list()
        cur = self.current()
        i = order.index(cur) if cur in order else 0
        j = max(0, min(len(order) - 1, i + delta))
        if j != i:
            self.switch(order[j])
            return order[j]
        return None

    def close(self, ws):
        order = self.list()
        if len(order) < 2 or ws not in order:
            return
        i = order.index(ws)
        target = order[i - 1] if i > 0 else order[1]
        for win in windows():
            if win["workspace"] == ws:
                SWAY.command(f'[con_id={win["id"]}] move container to workspace "{target}"')
        if self.current() == ws:
            self.switch(target)
        order.remove(ws)
        self.state["order"] = order
        self.state.get("names", {}).pop(ws, None)
        self.save()

    def rename(self, ws, name):
        names = self.state.setdefault("names", {})
        if name.strip():
            names[ws] = name.strip()
        else:
            names.pop(ws, None)
        self.save()

    def move_window(self, cid, ws):
        SWAY.command(f'[con_id={cid}] move container to workspace "{ws}"')


# --- Apps and icons ----------------------------------------------------------------------

class Apps:
    """Matches Sway app_ids and X11 classes to desktop entries."""

    def __init__(self):
        self.monitor = Gio.AppInfoMonitor.get()
        self.monitor.connect("changed", lambda *_: self.reload())
        self.reload()

    def reload(self):
        self.by_wmclass = {}
        self.by_exec = {}
        self.cache = {}
        for app in Gio.AppInfo.get_all():
            if not isinstance(app, Gio.DesktopAppInfo):
                continue
            cls = app.get_startup_wm_class()
            if cls:
                self.by_wmclass.setdefault(cls.lower(), app.get_id())
            exe = os.path.basename(app.get_executable() or "")
            if exe:
                self.by_exec.setdefault(exe.lower(), app.get_id())

    def match(self, app_id, wm_class=""):
        key = (app_id, wm_class)
        if key in self.cache:
            return self.cache[key]
        found = None
        for name in filter(None, (app_id, wm_class)):
            low = name.lower()
            for candidate in (name, low, low.split(".")[-1], low.replace(" ", "-")):
                if Gio.DesktopAppInfo.new(candidate + ".desktop"):
                    found = candidate + ".desktop"
                    break
            found = found or self.by_wmclass.get(low) or self.by_exec.get(low)
            if not found:
                results = Gio.DesktopAppInfo.search(name)
                if results and results[0]:
                    found = results[0][0]
            if found:
                break
        self.cache[key] = found
        return found

    @staticmethod
    def info(desktop_id):
        try:
            return Gio.DesktopAppInfo.new(desktop_id) if desktop_id else None
        except TypeError:
            return None


_APP_THEME = None
COLOUR_THEMES = ("Papirus", "Papirus-Light", "Mint-Y", "breeze")


def app_theme():
    """Windows 11 shows full-colour app icons, so use a colour theme even when the desktop
    icon theme is monochrome."""
    global _APP_THEME
    if _APP_THEME is None:
        _APP_THEME = Gtk.IconTheme.get_default()
        for name in COLOUR_THEMES:
            for base in ("/usr/share/icons", os.path.join(HOME, ".local/share/icons"), os.path.join(HOME, ".icons")):
                if os.path.isfile(os.path.join(base, name, "index.theme")):
                    _APP_THEME = Gtk.IconTheme()
                    _APP_THEME.set_custom_theme(name)
                    return _APP_THEME
    return _APP_THEME


def app_icon(gicon_or_name, size, scale=1):
    """A Gtk.Image of a full-colour app icon at a crisp size for the scale."""
    theme = app_theme()
    if isinstance(gicon_or_name, str):
        name = gicon_or_name
        if os.path.isabs(name) and os.path.exists(name):
            gicon = Gio.FileIcon.new(Gio.File.new_for_path(name))
        else:
            gicon = Gio.ThemedIcon.new_with_default_fallbacks(name or "application-x-executable")
    else:
        gicon = gicon_or_name or Gio.ThemedIcon.new("application-x-executable")
    image = Gtk.Image()
    info = theme.lookup_by_gicon_for_scale(gicon, size, scale, Gtk.IconLookupFlags.FORCE_SIZE)
    if not info and theme is not Gtk.IconTheme.get_default():
        info = Gtk.IconTheme.get_default().lookup_by_gicon_for_scale(gicon, size, scale,
                                                                     Gtk.IconLookupFlags.FORCE_SIZE)
    if not info:
        info = theme.lookup_icon_for_scale("application-x-executable", size, scale,
                                           Gtk.IconLookupFlags.FORCE_SIZE)
    try:
        pix = info.load_icon() if info else None
    except GLib.Error:
        pix = None
    if pix:
        image.set_from_surface(Gdk.cairo_surface_create_from_pixbuf(pix, scale, None))
    else:
        image.set_from_icon_name("application-x-executable", Gtk.IconSize.LARGE_TOOLBAR)
        image.set_pixel_size(size)
    return image


_ADWAITA = None


class Symbolic(Gtk.Image):
    """An Adwaita symbolic icon drawn in the widget's text colour (the GTK icon theme
    may have no symbolic icons)."""

    def __init__(self, name, size=16):
        super().__init__()
        self.name, self.size = name, size
        self.connect("style-updated", lambda *_: self.render())
        self.connect("realize", lambda *_: self.render())
        self.set_size_request(size, size)

    def set_icon(self, name):
        if name != self.name:
            self.name = name
            self.render()

    def render(self):
        global _ADWAITA
        if _ADWAITA is None:
            _ADWAITA = Gtk.IconTheme()
            _ADWAITA.set_custom_theme("Adwaita")
        scale = self.get_scale_factor()
        info = (_ADWAITA.lookup_icon_for_scale(self.name, self.size, scale, Gtk.IconLookupFlags.FORCE_SIZE)
                or Gtk.IconTheme.get_default().lookup_icon_for_scale(self.name, self.size, scale,
                                                                     Gtk.IconLookupFlags.FORCE_SIZE))
        if not info:
            return
        try:
            pix, _was = info.load_symbolic_for_context(self.get_style_context())
        except GLib.Error:
            return
        self.set_from_surface(Gdk.cairo_surface_create_from_pixbuf(pix, scale, None))


def symbolic(name, size=16):
    return Symbolic(name, size)


def css_class(widget, *names):
    ctx = widget.get_style_context()
    for n in names:
        ctx.add_class(n)
    return widget


def label(text, *classes, xalign=0.0, ellipsize=False, wrap=False):
    lab = Gtk.Label(label=text, xalign=xalign)
    if ellipsize:
        from gi.repository import Pango
        lab.set_ellipsize(Pango.EllipsizeMode.END)
    if wrap:
        lab.set_line_wrap(True)
        lab.set_max_width_chars(36)
    return css_class(lab, *classes)


def clear(box):
    for child in box.get_children():
        child.destroy()


def popup_menu(event, items, widget=None):
    """items: (label, callback) or None for a separator. Returns the menu."""
    menu = Gtk.Menu()
    css_class(menu, "ws-menu")
    for item in items:
        if item is None:
            menu.append(Gtk.SeparatorMenuItem())
            continue
        text, callback = item[0], item[1]
        mi = Gtk.MenuItem(label=text)
        mi.set_sensitive(callback is not None)
        if callback:
            mi.connect("activate", lambda _m, c=callback: c())
        menu.append(mi)
    menu.show_all()
    menu.connect("deactivate", lambda m: GLib.idle_add(m.destroy))
    if widget is not None:
        menu.popup_at_widget(widget, Gdk.Gravity.NORTH, Gdk.Gravity.SOUTH, event)
    else:
        menu.popup_at_pointer(event)
    return menu


# --- Flyouts ----------------------------------------------------------------------------

class Flyout(Gtk.Window):
    """A Windows 11 flyout above the taskbar. The window covers the screen outside the
    taskbar (transparent) so a click anywhere else closes it."""

    def __init__(self, name, halign=Gtk.Align.END, monitor=None):
        super().__init__()
        GtkLayerShell.init_for_window(self)
        GtkLayerShell.set_namespace(self, name)
        GtkLayerShell.set_layer(self, GtkLayerShell.Layer.TOP)
        GtkLayerShell.set_keyboard_mode(self, GtkLayerShell.KeyboardMode.EXCLUSIVE)
        if monitor is not None:
            GtkLayerShell.set_monitor(self, monitor)
        for edge in (GtkLayerShell.Edge.LEFT, GtkLayerShell.Edge.RIGHT,
                     GtkLayerShell.Edge.TOP, GtkLayerShell.Edge.BOTTOM):
            GtkLayerShell.set_anchor(self, edge, True)
        visual = self.get_screen().get_rgba_visual()
        if visual:
            self.set_visual(visual)
        self.set_app_paintable(True)
        self.set_name("flyout-window")
        self.holder = Gtk.Box(halign=halign, valign=Gtk.Align.END)
        self.holder.set_margin_bottom(12)
        self.holder.set_margin_end(12)
        self.holder.set_margin_start(12)
        self.panel = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.panel.set_name(name)
        css_class(self.panel, "flyout")
        self.holder.add(self.panel)
        self.slide = ws_motion.Slide(self.holder, self.panel)
        self.add(self.slide)
        self.connect("button-press-event", self.on_click)
        self.connect("key-press-event", self.on_key)
        self.on_hide = None

    def on_click(self, _w, event):
        alloc = self.panel.get_allocation()
        x, y = self.panel.translate_coordinates(self, 0, 0)
        if not (x <= event.x <= x + alloc.width and y <= event.y <= y + alloc.height):
            self.close_flyout()
        return False

    def on_key(self, _w, event):
        if event.keyval == Gdk.KEY_Escape:
            self.close_flyout()
            return True
        return False

    def open(self):
        self.show_all()
        self.present()
        self.slide.enter()

    def close_flyout(self):
        if self.slide.shown:
            self.slide.leave(self.finish_close)

    def finish_close(self):
        self.hide()
        if self.on_hide:
            self.on_hide()

    def toggle(self):
        if self.slide.shown:
            self.close_flyout()
        else:
            self.open()
