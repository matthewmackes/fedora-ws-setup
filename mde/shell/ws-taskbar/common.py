"""Shared pieces of the Windows 11 taskbar: settings, theme, labwc windows,
app matching, icons and the flyout window."""

import json
import os
import shlex
import subprocess
import sys

import gi

gi.require_version("Gdk", "3.0")
gi.require_version("Gtk", "3.0")
gi.require_version("GtkLayerShell", "0.1")
from gi.repository import Gdk, Gio, GLib, Gtk, GtkLayerShell  # noqa: E402

sys.path.insert(0, "/usr/share/mde/ws-settings")
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
    "taskbar_align": "center", "taskbar_search": "hide", "taskbar_taskview": False,
    "taskbar_autohide": False, "taskbar_badges": True, "taskbar_flash": True,
    "taskbar_all_displays": True, "taskbar_corner": True, "taskbar_combine": "always",
    "taskbar_all_desktops": False, "tray_hidden": ["nm-applet", "blueman"], "tray_overflow": True,
    "clock_seconds": False, "clock_24h": False, "night_light": False,
    "main_display": "",
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
    subprocess.Popen(cmd, shell=True, start_new_session=True, stdin=subprocess.DEVNULL,
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


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


# --- labwc windows (wlr-foreign-toplevel) ---------------------------------------------

def _load_wm():
    root = os.path.abspath(os.path.join(os.path.dirname(os.path.realpath(__file__)), ".."))
    local = os.path.join(os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share")), "mde")
    for path in (local, root, "/usr/share/mde"):
        if path and path not in sys.path:
            sys.path.insert(0, path)
    import wm
    return wm


class Sway:
    """Window and output list for labwc. The name stays so the taskbar call sites
    keep working; the transport is Wayland, not Sway IPC."""

    def __init__(self):
        self.conn = None
        self._cb = None
        self.wm = None

    def _ensure(self):
        if self.conn is not None:
            return True
        try:
            self.wm = _load_wm()
            self.conn = self.wm.Connection()
        except Exception:
            self.conn = None
            return False
        return True

    def request(self, kind, payload=""):
        if kind == 100:
            try:
                return _load_wm().keyboard_devices()
            except Exception:
                return []
        return []

    def command(self, cmd):
        match = None
        if "xkb_switch_layout" in cmd:
            parts = cmd.split()
            if parts:
                try:
                    _load_wm().set_layout(parts[-1])
                except Exception:
                    pass
        return match

    def tree(self):
        return {}

    def workspaces(self):
        return [{"name": "1", "focused": True, "visible": True, "output": ""}]

    def outputs(self):
        if not self._ensure():
            return []
        try:
            return self.conn.output_list()
        except Exception:
            return []

    def subscribe(self, events, callback):
        """Call callback(kind, event) when the window list changes."""
        if not self._ensure():
            return
        self._cb = callback
        conn = self.conn
        conn.watched = True

        def on_ready(_fd, cond):
            if cond & (GLib.IO_HUP | GLib.IO_ERR):
                return False
            try:
                conn.dirty = False
                conn._pump()
            except (OSError, ValueError):
                return False
            if conn.dirty and self._cb:
                focused = next((w for w in conn.window_list() if w["focused"]), None)
                event = {"change": "focus", "container": {
                    "id": focused["id"] if focused else 0,
                    "name": focused["title"] if focused else "",
                }}
                self._cb(3, event)
            return True

        GLib.io_add_watch(conn.sock.fileno(), GLib.PRIORITY_DEFAULT,
                          GLib.IO_IN | GLib.IO_HUP | GLib.IO_ERR, on_ready)


SWAY = Sway()


def windows(tree=None):
    """Every application window. hidden means minimized."""
    if tree is not None:
        return []
    if not SWAY._ensure():
        return []
    try:
        return SWAY.conn.window_list()
    except Exception:
        return []


def current_workspace():
    return "1"


def _wm_call(method, ident):
    if not SWAY._ensure():
        return
    try:
        getattr(SWAY.conn, method)(ident)
    except Exception:
        pass


def minimize(win):
    _wm_call("minimize", win["id"])


def activate(win):
    _wm_call("unminimize" if win.get("hidden") else "activate", win["id"])


def close_window(win):
    _wm_call("close", win["id"])


class Desktops:
    """One desktop. Task View and extra desktops are not part of this session."""

    HIDDEN = ("desktop", "__i3_scratch")

    def __init__(self):
        self.state = {"order": ["1"], "names": {}}

    def save(self):
        return None

    def list(self):
        return ["1"]

    def name(self, ws):
        return "Desktop 1"

    def current(self):
        return "1"

    def switch(self, ws):
        return None

    def new(self, switch=True):
        return "1"

    def step(self, delta):
        return None

    def close(self, ws):
        return None

    def rename(self, ws, name):
        return None

    def move_window(self, cid, ws):
        return None


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
                try:
                    info = Gio.DesktopAppInfo.new(candidate + ".desktop")
                except TypeError:
                    info = None
                if info:
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
