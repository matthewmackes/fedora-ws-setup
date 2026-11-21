"""Windows 11 Start menu as a taskbar flyout.

Hosted by ws-taskbar so Start shares open/close, exclusive keyboard, and
monitor placement with Quick Settings. External callers use the thin
ws-start / ws-drawer clients that talk to ws-taskbar.sock.
"""
import json
import math
import os
import pwd
import re
import shlex
import shutil
import subprocess
import sys
import time
from datetime import datetime, timedelta

import gi

gi.require_version("Gdk", "3.0")
gi.require_version("Gtk", "3.0")
gi.require_version("GtkLayerShell", "0.1")
gi.require_version("PangoCairo", "1.0")
from gi.repository import (Gdk, GdkPixbuf, Gio, GLib, Gtk,  # noqa: E402
                           GtkLayerShell, Pango, PangoCairo)

from common import (HEIGHT, HOME, SWAY, Flyout, accent_rgba, clear, css_class,  # noqa: E402
                    settings as taskbar_settings, sway_exec, theme_css, ws_motion)

sys.path.insert(0, "/usr/share/mde/ws-settings")
_settings_dir = os.path.join(os.path.dirname(os.path.realpath(__file__)), "..", "ws-settings")
if os.path.isdir(_settings_dir):
    sys.path.insert(0, _settings_dir)
import advanced_list  # noqa: E402

GAP = 12
WIDTH = 640
MAX_HEIGHT = 726
COLS = 6
TILE_HEIGHT = 76
RECENT_DAYS = 7

CONFIG_HOME = os.environ.get("XDG_CONFIG_HOME", os.path.join(HOME, ".config"))
CONF = os.path.join(CONFIG_HOME, "ws-start")
STATE = os.path.join(os.environ.get("XDG_STATE_HOME", os.path.join(HOME, ".local", "state")), "ws-start")
SETTINGS_FILE = os.path.join(CONF, "settings.json")
PINS_FILE = os.path.join(CONF, "pins")
USAGE_FILE = os.path.join(STATE, "usage.json")
OLD_PINS = os.path.join(HOME, ".cache", "nwg-pin-cache")

# Windows 11 layouts as (pinned rows, recommended rows).
LAYOUTS = {"more_pins": (4, 1), "default": (3, 3), "more_recommendations": (2, 4)}
DEFAULTS = {"layout": "default", "recently_added": True, "most_used": False,
            "recommended_files": True, "folders": []}

# Folders that can sit next to the power button: (key, label, icon, target).
# Upper-case targets are xdg-user-dir names.
FOLDERS = [
    ("settings", "Settings", "preferences-system", "@settings"),
    ("explorer", "File Explorer", "system-file-manager", "@explorer"),
    ("documents", "Documents", "folder-documents", "DOCUMENTS"),
    ("downloads", "Downloads", "folder-download", "DOWNLOAD"),
    ("music", "Music", "folder-music", "MUSIC"),
    ("pictures", "Pictures", "folder-pictures", "PICTURES"),
    ("videos", "Videos", "folder-videos", "VIDEOS"),
    ("network", "Network", "network-workgroup", "network:///"),
    ("personal", "Personal folder", "user-home", "~"),
]

SETTINGS_INDEX = "/usr/share/mde/ws-settings/index.json"
TASKBAR_PINS = os.path.join(CONFIG_HOME, "ws-taskbar", "pins")
DEFAULT_TASKBAR_PINS = ("thunar.desktop", "org.mozilla.firefox.desktop", "terminator.desktop",
                        "ws-settings.desktop")
UPDATE_PENDING = "/system-update"

# Light-mode tokens; ws_theme.transform / DARK_MAP restyle at runtime.
CSS = """
#start {
  background-color: #f3f3f3;
  border: 1px solid alpha(#000000, 0.1);
  border-radius: 8px;
  box-shadow: 0 8px 16px alpha(#000000, 0.14);
}
#start label, menu label {
  color: #1a1a1a;
  font-family: "Selawik", sans-serif;
  font-size: 10.5pt;
}
#start button {
  background: none;
  border: none;
  box-shadow: none;
  outline: none;
  border-radius: 4px;
  transition: background-color 83ms linear;
}
#start button:hover, #start button:focus { background-color: alpha(#000000, 0.04); }
#start button:active { background-color: alpha(#000000, 0.02); }
#start button image { transition: -gtk-icon-transform 167ms cubic-bezier(0, 0, 0, 1); }
#start button.tile:active image, #start button.rec:active image { -gtk-icon-transform: scale(0.85); }
#start-search {
  margin: 32px 32px 0;
  min-height: 30px;
  padding: 0 16px;
  border-radius: 999px;
  border: 1px solid alpha(#000000, 0.06);
  border-bottom-color: alpha(#000000, 0.45);
  background-color: alpha(#ffffff, 0.7);
  box-shadow: none;
  color: #1a1a1a;
  font-family: "Selawik", sans-serif;
  font-size: 10.5pt;
}
#start-search:focus { background-color: #ffffff; border-bottom: 2px solid @ACCENT@; }
#start .section-row { margin: 24px 32px 8px; min-height: 28px; }
#start .section { font-weight: 600; }
#start button.pill {
  min-height: 24px;
  padding: 0 8px;
  background-color: alpha(#ffffff, 0.7);
  border: 1px solid alpha(#000000, 0.06);
}
#start button.pill:hover { background-color: alpha(#f9f9f9, 0.5); }
#start button.pill label, #start .caption, #start button.tile label { font-size: 9pt; }
#start .caption { color: #5d5d5d; }
#start .pinned { margin: 0 24px; }
#start button.tile { padding: 8px 2px; }
#start .dots { margin-left: 4px; }
#start button.dot {
  min-width: 6px;
  min-height: 6px;
  padding: 0;
  margin: 3px 0;
  border-radius: 999px;
  background-color: alpha(#000000, 0.25);
}
#start button.dot.active { background-color: #5d5d5d; min-height: 14px; }
#start .recommended { margin: 0 24px; }
#start button.rec { padding: 8px; }
#start .hint { color: #5d5d5d; margin: 4px 32px; }
#start button.link { padding: 0 32px; }
#start button.link label { color: @ACCENT_TEXT@; }
#start .list { margin: 0 24px 8px; }
#start button.row, #start button.letter { min-height: 36px; padding: 0 8px; }
#start button.letter label, #start .group { font-weight: 600; }
#start .group { margin: 12px 8px 4px; }
#start .jump { margin: 8px 32px; }
#start .jump button { min-width: 56px; min-height: 56px; }
#start .jump button label { font-size: 15pt; font-weight: 600; }
#start .jump button:disabled label { color: alpha(#000000, 0.3); }
#start .results { margin: 16px 0 8px 24px; }
#start .preview {
  margin: 16px 32px 16px 8px;
  padding: 24px 16px;
  background-color: #fbfbfb;
  border: 1px solid alpha(#000000, 0.06);
  border-radius: 8px;
}
#start .preview-title { font-size: 15pt; font-weight: 600; }
#start .preview button { min-height: 36px; padding: 0 8px; }
#start separator { background-color: alpha(#000000, 0.08); min-height: 1px; margin: 12px 0; }
#start scrolledwindow undershoot, #start scrolledwindow overshoot { background: none; box-shadow: none; }
#start-footer {
  min-height: 64px;
  padding: 0 24px;
  background-color: alpha(#000000, 0.03);
  border-top: 1px solid alpha(#000000, 0.06);
  border-radius: 0 0 8px 8px;
}
#start-footer button.account { min-height: 40px; padding: 0 12px 0 8px; }
#start-footer button.footer-icon { min-width: 40px; min-height: 40px; padding: 0; }
menu {
  background-color: #f9f9f9;
  padding: 4px;
  border: 1px solid alpha(#000000, 0.1);
  border-radius: 8px;
}
.csd.popup decoration { border-radius: 8px; box-shadow: 0 8px 16px alpha(#000000, 0.14); }
menuitem { min-height: 32px; padding: 0 12px; border-radius: 4px; transition: background-color 83ms linear; }
menuitem:hover { background-color: alpha(#000000, 0.05); }
#update-dot {
  min-width: 8px; min-height: 8px; margin: 4px;
  border-radius: 4px; background-color: #f7630c;
}
"""

ACRYLIC_CSS = "#start { background-color: alpha(#f3f3f3, 0.85); }\n"


# --- Data -------------------------------------------------------------------

def load_json(path, default):
    try:
        with open(path) as f:
            data = json.load(f)
        return data if isinstance(data, type(default)) else default
    except (OSError, ValueError):
        return default


def save_json(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(data, f, indent=2)
    os.replace(tmp, path)


def load_settings():
    return {**DEFAULTS, **load_json(SETTINGS_FILE, {})}


def load_pins():
    for path in PINS_FILE, OLD_PINS:
        try:
            with open(path) as f:
                pins = [line.strip() for line in f if line.strip()]
            if path == OLD_PINS:
                save_pins(pins)
            return pins
        except OSError:
            continue
    return []


def save_pins(pins):
    os.makedirs(CONF, exist_ok=True)
    with open(PINS_FILE, "w") as f:
        f.write("".join(p + "\n" for p in pins))


def app_id(app):
    i = app.get_id() or ""
    return i[:-8] if i.endswith(".desktop") else i


def app_by_id(i):
    try:
        return Gio.DesktopAppInfo.new(i + ".desktop")
    except TypeError:
        return None


def app_list():
    apps = [a for a in Gio.AppInfo.get_all()
            if isinstance(a, Gio.DesktopAppInfo) and a.should_show()]
    apps.sort(key=lambda a: a.get_display_name().casefold())
    return apps


def installed_at(app):
    try:
        return os.stat(app.get_filename()).st_ctime
    except (OSError, TypeError):
        return 0


def user_dir(key):
    try:
        out = subprocess.run(["xdg-user-dir", key], capture_output=True, text=True).stdout.strip()
    except OSError:
        out = ""
    return out or HOME


def user_name():
    p = pwd.getpwuid(os.getuid())
    return (p.pw_gecos.split(",")[0] or p.pw_name).strip()


def when(ts):
    now = datetime.now()
    then = datetime.fromtimestamp(ts)
    secs = (now - then).total_seconds()
    if secs < 60:
        return "Just now"
    if secs < 3600:
        return f"{int(secs // 60)}m ago"
    if then.date() == now.date():
        return f"{int(secs // 3600)}h ago"
    clock = then.strftime("%-I:%M %p")
    if then.date() == (now - timedelta(days=1)).date():
        return f"Yesterday at {clock}"
    if then.year == now.year:
        return then.strftime("%b %-d")
    return then.strftime("%b %-d, %Y")


def score(query, name, extra=""):
    n = name.casefold()
    if n.startswith(query):
        return 0
    if any(w.startswith(query) for w in re.split(r"[\s\-_.()]+", n)):
        return 1
    if query in n:
        return 2
    if query in extra.casefold():
        return 3
    return None


def available(cmd):
    return cmd.startswith("@") or shutil.which(cmd.split()[0])


def taskbar_centred():
    return taskbar_settings().get("taskbar_align", "center") == "center"


def output_height():
    try:
        return next(o["rect"]["height"] for o in SWAY.outputs() if o.get("focused"))
    except (StopIteration, KeyError, TypeError):
        try:
            outs = SWAY.outputs()
            if outs:
                return outs[0].get("rect", {}).get("height") or 720
        except Exception:
            pass
        return 720


def settings_pages():
    """Settings search results: (label, icon, command, extra search words, caption)."""
    try:
        with open(SETTINGS_INDEX) as f:
            pages = json.load(f)
    except (OSError, ValueError):
        return [("Settings", "preferences-system", "ws-settings", "control panel", "System")]
    return [(p["title"], p["icon"], "ws-settings " + p["key"], p["keywords"], p["caption"])
            for p in pages if p["key"] != "home"]


# --- Widgets ----------------------------------------------------------------

def icon(gicon, size, fallback="application-x-executable"):
    image = Gtk.Image()
    theme = Gtk.IconTheme.get_default()
    if isinstance(gicon, str):
        gicon = Gio.ThemedIcon.new(gicon)
    if gicon is None or not theme.lookup_by_gicon(gicon, size, 0):
        gicon = Gio.ThemedIcon.new(fallback)
    image.set_from_gicon(gicon, Gtk.IconSize.DIALOG)
    image.set_pixel_size(size)
    return image


def start_label(text, *classes, xalign=0.0, ellipsize=True):
    lab = Gtk.Label(label=text, xalign=xalign)
    if ellipsize:
        lab.set_ellipsize(Pango.EllipsizeMode.END)
        lab.set_max_width_chars(1)
    return css_class(lab, *classes)


def on_right_click(widget, callback):
    def handler(_w, event):
        if event.button == 3:
            callback(event)
            return True
        return False
    widget.connect("button-press-event", handler)


class Avatar(Gtk.DrawingArea):
    def __init__(self, size=32):
        super().__init__()
        self.size = size
        self.set_size_request(size, size)
        self.picture = None
        name = pwd.getpwuid(os.getuid()).pw_name
        for path in (os.path.join(HOME, ".face"), os.path.join(HOME, ".face.icon"),
                     f"/var/lib/AccountsService/icons/{name}"):
            try:
                self.picture = GdkPixbuf.Pixbuf.new_from_file_at_scale(path, size, size, True)
                break
            except GLib.Error:
                continue
        self.connect("draw", self.on_draw)

    def on_draw(self, _w, cr):
        s = self.size
        cr.arc(s / 2, s / 2, s / 2, 0, 2 * math.pi)
        cr.clip()
        if self.picture:
            Gdk.cairo_set_source_pixbuf(cr, self.picture, (s - self.picture.get_width()) / 2,
                                        (s - self.picture.get_height()) / 2)
            cr.paint()
            return
        r, g, b, _a = accent_rgba()
        cr.set_source_rgb(r, g, b)
        cr.paint()
        layout = PangoCairo.create_layout(cr)
        layout.set_text(user_name()[:1].upper(), -1)
        layout.set_font_description(Pango.FontDescription("Sans Semibold 11"))
        w, h = layout.get_pixel_size()
        cr.move_to((s - w) / 2, (s - h) / 2)
        cr.set_source_rgb(1, 1, 1)
        PangoCairo.show_layout(cr, layout)


# --- Start menu -------------------------------------------------------------

class StartMenu(Flyout):
    def __init__(self, monitor=None):
        super().__init__("start", Gtk.Align.CENTER if taskbar_centred() else Gtk.Align.START, monitor)
        GtkLayerShell.set_namespace(self, "ws-start")
        self.menu = self.panel
        self.settings = load_settings()
        self.pins = advanced_list.release_pins(load_pins())
        self.usage = load_json(USAGE_FILE, {})
        self.apps = []
        self.apps_dirty = True
        self.page = 0
        self.pages = 1
        self.base_page = "home"
        self.results = None
        self.best = None
        self.file_proc = None
        self.popup_menu = None
        self.settings_pages = settings_pages()
        self.letters = {}
        self.move_confirm = None
        self.css = Gtk.CssProvider()
        Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), self.css,
                                                 Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        self.reload_css()
        self.monitor_apps = Gio.AppInfoMonitor.get()
        self.monitor_apps.connect("changed", self.on_apps_changed)
        self.add_events(Gdk.EventMask.SCROLL_MASK)
        self.connect("scroll-event", self.on_scroll)
        self.build_content()

    def reload_css(self):
        style = CSS + (ACRYLIC_CSS if ws_motion.acrylic() else "")
        self.css.load_from_data(theme_css(style).encode())

    def build_content(self):
        search = self.search = Gtk.SearchEntry()
        search.set_name("start-search")
        search.set_placeholder_text("Search for apps, settings, and documents")
        search.connect("search-changed", self.on_search_changed)
        search.connect("activate", self.on_search_activate)
        search.connect("key-press-event", self.on_search_key)
        self.panel.pack_start(search, False, False, 0)

        stack = self.stack = Gtk.Stack(vexpand=True)
        stack.set_transition_duration(ws_motion.NORMAL)
        self.panel.pack_start(stack, True, True, 0)

        self.home = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        stack.add_named(self.home, "home")

        self.all_page, self.all_scroll, self.all_list = self.list_page(
            "All apps", "\u2039  Back", self.show_home)
        stack.add_named(self.all_page, "all")

        self.more_page, _scroll, self.more_list = self.list_page(
            "Recommended", "\u2039  Back", self.show_home)
        stack.add_named(self.more_page, "more")

        self.jump = css_class(Gtk.Grid(column_homogeneous=True, row_spacing=4, column_spacing=4,
                                       halign=Gtk.Align.CENTER, valign=Gtk.Align.CENTER), "jump")
        stack.add_named(self.jump, "jump")

        self.search_page = Gtk.Box()
        scroll = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER)
        scroll.set_size_request(300, -1)
        self.result_list = css_class(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "results")
        scroll.add(self.result_list)
        self.search_page.pack_start(scroll, False, False, 0)
        self.preview = css_class(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4), "preview")
        self.search_page.pack_start(self.preview, True, True, 0)
        stack.add_named(self.search_page, "search")

        self.panel.pack_start(self.build_footer(), False, False, 0)
        self.slide.show_all()

    def header(self, title, button=None, callback=None):
        row = css_class(Gtk.Box(), "section-row")
        row.pack_start(start_label(title, "section", ellipsize=False), True, True, 0)
        if button:
            b = css_class(Gtk.Button(label=button), "pill")
            b.connect("clicked", lambda *_: callback())
            row.pack_end(b, False, False, 0)
        return row

    def list_page(self, title, button, callback):
        page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        page.pack_start(self.header(title, button, callback), False, False, 0)
        scroll = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER, vexpand=True)
        box = css_class(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "list")
        scroll.add(box)
        page.pack_start(scroll, True, True, 0)
        return page, scroll, box

    def build_footer(self):
        footer = Gtk.Box()
        footer.set_name("start-footer")
        account = css_class(Gtk.Button(valign=Gtk.Align.CENTER), "account")
        box = Gtk.Box(spacing=12)
        box.pack_start(Avatar(), False, False, 0)
        box.pack_start(start_label(user_name(), ellipsize=False), False, False, 0)
        account.add(box)
        account.connect("clicked", lambda b: self.popup(b, [
            ("Lock", lambda: self.run("ws-lock")),
            ("Sign out", lambda: self.run("labwc --exit")),
        ], above=True))
        footer.pack_start(account, False, False, 0)

        power = self.footer_button("system-shutdown-symbolic", "Power")
        power.connect("clicked", lambda b: self.popup(b, self.power_items(), above=True, right=True))
        overlay = Gtk.Overlay()
        overlay.add(power)
        self.update_dot = Gtk.Box(halign=Gtk.Align.END, valign=Gtk.Align.START, no_show_all=True)
        self.update_dot.set_name("update-dot")
        overlay.add_overlay(self.update_dot)
        overlay.set_overlay_pass_through(self.update_dot, True)
        footer.pack_end(overlay, False, False, 0)
        self.folder_box = Gtk.Box(spacing=4, valign=Gtk.Align.CENTER)
        footer.pack_end(self.folder_box, False, False, 4)
        return footer

    def power_items(self):
        update_items = []
        try:
            import p_update
            if p_update.is_busy():
                return [("Updates in progress\u2026", lambda: None)]
            update_items = list(p_update.power_update_items(busy_blocked=False))
        except ImportError:
            if os.path.lexists(UPDATE_PENDING):
                trigger = ("gdbus call --system --dest org.freedesktop.PackageKit "
                           "--object-path /org/freedesktop/PackageKit "
                           "--method org.freedesktop.PackageKit.Offline.Trigger %s >/dev/null && "
                           "systemctl reboot")
                update_items = [
                    ("Update and restart",
                     lambda: self.run("sh -c " + shlex.quote(trigger % "reboot"))),
                    ("Update and shut down",
                     lambda: self.run("sh -c " + shlex.quote(trigger % "power-off"))),
                ]
        # Win11-style: Update actions first while pending, then Sleep / Shut down / Restart.
        return update_items + [("Sleep", lambda: self.run("systemctl suspend")),
                               ("Shut down", lambda: self.run("systemctl poweroff")),
                               ("Restart", lambda: self.run("systemctl reboot"))]

    def footer_button(self, icon_name, tooltip):
        b = css_class(Gtk.Button(valign=Gtk.Align.CENTER, tooltip_text=tooltip), "footer-icon")
        image = Gtk.Image.new_from_icon_name(icon_name, Gtk.IconSize.MENU)
        image.set_pixel_size(16)
        b.add(image)
        return b

    # Home page: Pinned and Recommended

    def build_home(self):
        clear(self.home)
        pin_rows, rec_rows = LAYOUTS.get(self.settings["layout"], LAYOUTS["default"])
        self.home.pack_start(self.header("Pinned", "All apps  \u203a", self.show_all_apps),
                             False, False, 0)
        ids = advanced_list.id_sets()
        pins = [a for a in map(app_by_id, self.pins) if a and not advanced_list.is_advanced(a, ids)]
        per_page = pin_rows * COLS
        self.pages = max(1, math.ceil(len(pins) / per_page))
        self.page = min(self.page, self.pages - 1)
        area = css_class(Gtk.Box(), "pinned")
        grid = Gtk.Grid(column_homogeneous=True, hexpand=True)
        grid.set_size_request(-1, pin_rows * TILE_HEIGHT)
        for i, app in enumerate(pins[self.page * per_page:(self.page + 1) * per_page]):
            index = self.page * per_page + i
            grid.attach(self.tile(app, index), i % COLS, i // COLS, 1, 1)
        if not pins:
            grid.attach(start_label("Right-click an app in All apps and choose Pin to Start.",
                                    "hint", ellipsize=False), 0, 0, COLS, 1)
        area.pack_start(grid, True, True, 0)
        if self.pages > 1:
            dots = css_class(Gtk.Box(orientation=Gtk.Orientation.VERTICAL,
                                     valign=Gtk.Align.CENTER), "dots")
            for p in range(self.pages):
                dot = css_class(Gtk.Button(tooltip_text=f"Page {p + 1}"), "dot")
                if p == self.page:
                    css_class(dot, "active")
                dot.connect("clicked", lambda _b, p=p: self.set_page(p))
                dots.pack_start(dot, False, False, 0)
            area.pack_end(dots, False, False, 0)
        self.home.pack_start(area, False, False, 0)

        items = self.recommended()
        shown = rec_rows * 2
        self.home.pack_start(self.header("Recommended", "More  \u203a" if len(items) > shown else None,
                                         self.show_more), False, False, 0)
        if not (self.settings["recently_added"] or self.settings["recommended_files"]):
            self.home.pack_start(start_label("To show your recent files and new apps, turn them on "
                                             "in Start settings.", "hint", ellipsize=False),
                                 False, False, 0)
            link = css_class(Gtk.Button(label="Start settings", halign=Gtk.Align.START), "link")
            link.connect("clicked", lambda *_: self.open_settings())
            self.home.pack_start(link, False, False, 0)
        elif not items:
            self.home.pack_start(start_label("Your recent files and new apps will show up here.",
                                             "hint", ellipsize=False), False, False, 0)
        else:
            grid = css_class(Gtk.Grid(column_homogeneous=True), "recommended")
            for i, item in enumerate(items[:shown]):
                grid.attach(self.rec_button(item), i % 2, i // 2, 1, 1)
            self.home.pack_start(grid, False, False, 0)
        self.build_folders()
        self.home.show_all()

    def tile(self, app, index):
        b = css_class(Gtk.Button(tooltip_text=app.get_display_name()), "tile")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6, valign=Gtk.Align.CENTER)
        box.pack_start(icon(app.get_icon(), 32), False, False, 0)
        box.pack_start(start_label(app.get_display_name(), xalign=0.5), False, False, 0)
        b.add(box)
        b.connect("clicked", lambda *_: self.launch(app))
        items = []
        if index > 0:
            items.append(("Move to front", lambda: self.move_pin_to_front(app)))
        items.append(("Unpin from Start", lambda: self.unpin(app)))
        on_right_click(b, lambda e: self.popup(e, items + [self.taskbar_action(app)]))
        return b

    def set_page(self, page):
        self.page = max(0, min(page, self.pages - 1))
        self.build_home()

    def build_folders(self):
        clear(self.folder_box)
        for key, name, icon_name, target in FOLDERS:
            if key in self.settings["folders"]:
                b = self.footer_button(icon_name + "-symbolic", name)
                b.connect("clicked", lambda _b, t=target: self.open_folder(t))
                self.folder_box.pack_start(b, False, False, 0)
        self.folder_box.show_all()

    def open_folder(self, target):
        if target == "@settings":
            self.run("ws-settings")
        elif target == "@explorer":
            self.run("thunar")
        elif target == "~":
            self.run("thunar " + shlex.quote(HOME))
        elif target.isupper():
            self.run("thunar " + shlex.quote(user_dir(target)))
        else:
            self.run("thunar " + shlex.quote(target))

    # Recommended, most used and recently added

    def recent_apps(self, ids=None):
        ids = ids or advanced_list.id_sets()
        cutoff = time.time() - RECENT_DAYS * 86400
        apps = [(installed_at(a), a) for a in self.apps if not advanced_list.is_advanced(a, ids)]
        return [(t, a) for t, a in sorted(apps, key=lambda x: -x[0]) if t > cutoff]

    def most_used(self, ids=None):
        ids = ids or advanced_list.id_sets()
        found = []
        for app_key in sorted(self.usage, key=lambda i: -self.usage[i]):
            app = app_by_id(app_key)
            if app and app.should_show() and not advanced_list.is_advanced(app, ids):
                found.append(app)
                if len(found) == 6:
                    break
        return found

    def recommended(self):
        items = []
        if self.settings["recently_added"]:
            for t, app in self.recent_apps()[:3]:
                items.append({"kind": "app", "name": app.get_display_name(), "icon": app.get_icon(),
                              "time": t, "caption": "Recently added", "app": app})
        if self.settings["recommended_files"]:
            for info in Gtk.RecentManager.get_default().get_items():
                uri = info.get_uri()
                if (not uri.startswith("file://") or info.get_mime_type() == "inode/directory"
                        or not info.exists()):
                    continue
                t = info.get_modified()
                items.append({"kind": "file", "name": info.get_display_name(),
                              "icon": info.get_gicon(), "time": t, "caption": when(t),
                              "path": Gio.File.new_for_uri(uri).get_path(), "uri": uri})
        items.sort(key=lambda i: -i["time"])
        return items

    def rec_button(self, item):
        b = css_class(Gtk.Button(tooltip_text=item.get("path") or item["name"]), "rec")
        box = Gtk.Box(spacing=12)
        box.pack_start(icon(item["icon"], 32, "text-x-generic"), False, False, 0)
        texts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
        texts.pack_start(start_label(item["name"]), False, False, 0)
        texts.pack_start(start_label(item["caption"], "caption"), False, False, 0)
        box.pack_start(texts, True, True, 0)
        b.add(box)
        b.connect("clicked", lambda *_: self.activate(item))
        on_right_click(b, lambda e: self.popup(e, self.item_actions(item)))
        return b

    def item_actions(self, item):
        if item["kind"] == "app":
            if advanced_list.is_advanced(item["app"]):
                return [("Open", lambda: self.activate(item))]
            return [self.pin_action(item["app"]), self.taskbar_action(item["app"])]
        if item["kind"] == "file":
            actions = [("Open", lambda: self.activate(item)),
                       ("Open file location", lambda: self.open_location(item["path"]))]
            if item.get("uri"):
                actions.append(("Remove from list", lambda: self.forget(item["uri"])))
            return actions
        return [("Open", lambda: self.activate(item))]

    def forget(self, uri):
        try:
            Gtk.RecentManager.get_default().remove_item(uri)
        except GLib.Error:
            pass
        GLib.idle_add(lambda: self.build_home() or self.fill_more() or False)

    # All apps and Recommended lists

    def show_all_apps(self):
        clear(self.all_list)
        self.letters = {}
        ids = advanced_list.id_sets()
        if self.settings["recently_added"]:
            recent = self.recent_apps(ids)[:3]
            if recent:
                self.all_list.pack_start(start_label("Recently added", "group"), False, False, 0)
                for _t, app in recent:
                    self.all_list.pack_start(self.app_row(app), False, False, 0)
        if self.settings["most_used"]:
            used = self.most_used(ids)
            if used:
                self.all_list.pack_start(start_label("Most used", "group"), False, False, 0)
                for app in used:
                    self.all_list.pack_start(self.app_row(app), False, False, 0)
        last = None
        for app in self.apps:
            if advanced_list.is_advanced(app, ids):
                continue
            first = app.get_display_name()[:1].upper()
            key = first if "A" <= first <= "Z" else "#"
            if key != last:
                head = css_class(Gtk.Button(label=key), "letter")
                head.get_child().set_xalign(0)
                head.connect("clicked", lambda *_: self.show_jump())
                self.letters[key] = head
                self.all_list.pack_start(head, False, False, 0)
                last = key
            self.all_list.pack_start(self.app_row(app), False, False, 0)
        self.all_list.show_all()
        staying = self.stack.get_visible_child_name() == "all"
        if not staying:
            self.all_scroll.get_vadjustment().set_value(0)
        self.base_page = "all"
        self.stack.set_visible_child_full(
            "all", Gtk.StackTransitionType.NONE if staying else Gtk.StackTransitionType.SLIDE_LEFT)

    def app_row(self, app):
        b = css_class(Gtk.Button(), "row")
        box = Gtk.Box(spacing=12)
        box.pack_start(icon(app.get_icon(), 24), False, False, 0)
        box.pack_start(start_label(app.get_display_name()), True, True, 0)
        b.add(box)
        b.connect("clicked", lambda *_: self.launch(app))
        actions = [self.pin_action(app), self.taskbar_action(app)]
        if app.get_id():
            actions.append(("Move to Advanced", lambda target=app, anchor=b: self.ask_move(target, anchor)))
        on_right_click(b, lambda e, row_actions=actions: self.popup(e, row_actions))
        return b

    def ask_move(self, app, _anchor):
        # Kept inside the Start window. A separate dialog would take focus and
        # the menu would close before Move could be clicked.
        old = getattr(self, "move_confirm", None)
        if old is not None:
            if old.get_parent() is not None:
                old.destroy()
            self.move_confirm = None
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.set_margin_top(8)
        box.set_margin_bottom(8)
        box.set_margin_start(8)
        box.set_margin_end(8)
        title = start_label("Move this app to Advanced?", "section", ellipsize=False)
        body = start_label('It will appear at the top of Settings › Advanced, under "Moved from Start".',
                           "caption", ellipsize=False)
        body.set_line_wrap(True)
        body.set_max_width_chars(42)
        box.pack_start(title, False, False, 0)
        box.pack_start(body, False, False, 0)
        buttons = Gtk.Box(spacing=8, homogeneous=True)
        move = css_class(Gtk.Button(label="Move"), "pill")
        cancel = css_class(Gtk.Button(label="Cancel"), "pill")

        def dismiss(*_):
            box.destroy()
            self.move_confirm = None

        cancel.connect("clicked", dismiss)
        move.connect("clicked", lambda *_: GLib.idle_add(lambda: self.move_to_advanced(app) or False))
        buttons.pack_start(move, True, True, 0)
        buttons.pack_start(cancel, True, True, 0)
        box.pack_start(buttons, False, False, 0)
        self.move_confirm = box
        self.all_list.pack_start(box, False, False, 0)
        self.all_list.reorder_child(box, 0)
        box.show_all()
        self.all_scroll.get_vadjustment().set_value(0)

    def move_to_advanced(self, app):
        self.move_confirm = None
        advanced_list.add_moved(app.get_id() or "")
        self.pins = advanced_list.release_pins(self.pins)
        self.show_all_apps()

    def show_jump(self):
        clear(self.jump)
        for i, key in enumerate(["#"] + [chr(c) for c in range(ord("A"), ord("Z") + 1)]):
            b = Gtk.Button(label=key)
            b.set_sensitive(key in self.letters)
            b.connect("clicked", lambda _b, k=key: self.jump_to(k))
            self.jump.attach(b, i % 6, i // 6, 1, 1)
        self.jump.show_all()
        self.stack.set_visible_child_full("jump", Gtk.StackTransitionType.CROSSFADE)

    def jump_to(self, key):
        self.stack.set_visible_child_full("all", Gtk.StackTransitionType.CROSSFADE)
        y = self.letters[key].get_allocation().y
        self.all_scroll.get_vadjustment().set_value(y)
        self.letters[key].grab_focus()

    def fill_more(self):
        clear(self.more_list)
        for item in self.recommended():
            self.more_list.pack_start(self.rec_button(item), False, False, 0)
        self.more_list.show_all()

    def show_more(self):
        self.fill_more()
        self.base_page = "more"
        self.stack.set_visible_child_full("more", Gtk.StackTransitionType.SLIDE_LEFT)

    def show_home(self):
        self.base_page = "home"
        self.stack.set_visible_child_full("home", Gtk.StackTransitionType.SLIDE_RIGHT)

    # Search

    def on_search_changed(self, entry):
        text = entry.get_text().strip()
        if not text:
            self.results = None
            self.stack.set_visible_child_full(self.base_page, Gtk.StackTransitionType.NONE)
            return
        q = text.casefold()
        apps = []
        for app in self.apps:
            extra = " ".join(filter(None, [app.get_generic_name(), app.get_executable(),
                                           " ".join(app.get_keywords() or [])]))
            s = score(q, app.get_display_name(), extra)
            if s is not None:
                apps.append((s, {"kind": "app", "name": app.get_display_name(),
                                 "icon": app.get_icon(), "caption": "App", "app": app}))
        settings = []
        for name, icon_name, cmd, words, caption in self.settings_pages:
            s = score(q, name, words)
            if s is not None:
                settings.append((s, {"kind": "setting", "name": name, "icon": icon_name,
                                     "caption": caption, "cmd": cmd}))
        apps.sort(key=lambda x: x[0])
        settings.sort(key=lambda x: x[0])
        self.results = {"text": text, "apps": apps[:8], "settings": settings[:5], "files": []}
        self.render_results()
        self.search_files(text)

    def search_files(self, text):
        if self.file_proc:
            self.file_proc.force_exit()
            self.file_proc = None
        if len(text) < 2:
            return
        if shutil.which("fd"):
            argv = ["fd", "-i", "-F", "--max-results", "40", "--max-depth", "8", "--", text, HOME]
        elif shutil.which("plocate"):
            argv = ["plocate", "-i", "-l", "200", "--", text]
        else:
            return
        try:
            proc = Gio.Subprocess.new(argv, Gio.SubprocessFlags.STDOUT_PIPE
                                      | Gio.SubprocessFlags.STDERR_SILENCE)
        except GLib.Error:
            return
        self.file_proc = proc
        proc.communicate_utf8_async(None, None, self.on_files, text)

    def on_files(self, proc, result, text):
        try:
            _ok, out, _err = proc.communicate_utf8_finish(result)
        except GLib.Error:
            return
        if proc is self.file_proc:
            self.file_proc = None
        if not self.results or self.results["text"] != text:
            return
        files = []
        for path in (out or "").splitlines():
            path = path.rstrip("/")
            rel = path[len(HOME):]
            if not path.startswith(HOME + "/") or "/." in rel:
                continue
            is_dir = os.path.isdir(path)
            mime = "inode/directory" if is_dir else Gio.content_type_guess(path, None)[0]
            folder = os.path.dirname(path).replace(HOME, "~", 1)
            files.append((score(text.casefold(), os.path.basename(path)) or 2,
                          {"kind": "file", "name": os.path.basename(path),
                           "icon": Gio.content_type_get_icon(mime),
                           "caption": folder, "path": path}))
            if len(files) >= 12:
                break
        files.sort(key=lambda x: x[0])
        self.results["files"] = files
        self.render_results()

    def best_match(self):
        r = self.results
        candidates = [x for group in ("apps", "settings", "files") for x in r[group][:1]]
        return min(candidates, key=lambda x: x[0])[1] if candidates else None

    def render_results(self):
        r = self.results
        clear(self.result_list)
        best = self.best_match()
        self.best = best
        if not best:
            self.result_list.pack_start(start_label(f"No results for \u201c{r['text']}\u201d",
                                                    "hint", ellipsize=False), False, False, 0)
        else:
            self.result_list.pack_start(start_label("Best match", "group"), False, False, 0)
            self.result_list.pack_start(self.result_row(best, True), False, False, 0)
            for group, title in (("apps", "Apps"), ("settings", "Settings"), ("files", "Documents")):
                rows = [item for _s, item in r[group] if item is not best]
                if rows:
                    self.result_list.pack_start(start_label(title, "group"), False, False, 0)
                    for item in rows:
                        self.result_list.pack_start(self.result_row(item), False, False, 0)
        self.result_list.show_all()
        self.render_preview(best)
        if self.stack.get_visible_child_name() != "search":
            self.stack.set_visible_child_full("search", Gtk.StackTransitionType.NONE)

    def result_row(self, item, best=False):
        b = css_class(Gtk.Button(tooltip_text=item.get("path") or item["name"]), "row")
        box = Gtk.Box(spacing=12)
        box.pack_start(icon(item["icon"], 32 if best else 24, "text-x-generic"), False, False, 0)
        texts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
        texts.pack_start(start_label(item["name"]), False, False, 0)
        if best or item["kind"] == "file":
            texts.pack_start(start_label(item["caption"], "caption"), False, False, 0)
        box.pack_start(texts, True, True, 0)
        b.add(box)
        b.connect("clicked", lambda *_: self.activate(item))
        b.connect("focus-in-event", lambda *_: self.render_preview(item) or False)
        on_right_click(b, lambda e: self.popup(e, self.item_actions(item)))
        return b

    def render_preview(self, item):
        clear(self.preview)
        if not item:
            return
        self.preview.pack_start(icon(item["icon"], 64, "text-x-generic"), False, False, 8)
        title = start_label(item["name"], "preview-title", xalign=0.5, ellipsize=False)
        title.set_line_wrap(True)
        title.set_justify(Gtk.Justification.CENTER)
        self.preview.pack_start(title, False, False, 0)
        self.preview.pack_start(start_label(item["caption"], "caption", xalign=0.5), False, False, 0)
        self.preview.pack_start(Gtk.Separator(), False, False, 0)
        actions = [("Open", "document-open", lambda: self.activate(item))]
        if item["kind"] == "app":
            for text, callback in (self.pin_action(item["app"]), self.taskbar_action(item["app"])):
                actions.append((text, "view-pin", callback))
        elif item["kind"] == "file":
            actions.append(("Open file location", "folder-open",
                            lambda: self.open_location(item["path"])))
        for text, icon_name, callback in actions:
            b = Gtk.Button()
            box = Gtk.Box(spacing=12)
            image = Gtk.Image.new_from_icon_name(icon_name + "-symbolic", Gtk.IconSize.MENU)
            image.set_pixel_size(16)
            box.pack_start(image, False, False, 0)
            box.pack_start(start_label(text), True, True, 0)
            b.add(box)
            b.connect("clicked", lambda _b, c=callback: c())
            self.preview.pack_start(b, False, False, 0)
        self.preview.show_all()

    def on_search_activate(self, _entry):
        if self.results and self.best:
            self.activate(self.best)

    def on_search_key(self, _entry, event):
        if event.keyval == Gdk.KEY_Down:
            self.stack.get_visible_child().child_focus(Gtk.DirectionType.TAB_FORWARD)
            return True
        return False

    # Actions

    def pin_action(self, app):
        if app_id(app) in self.pins:
            return ("Unpin from Start", lambda: self.unpin(app))
        return ("Pin to Start", lambda: self.pin(app))

    def taskbar_action(self, app):
        desktop_id = app.get_id()
        try:
            with open(TASKBAR_PINS) as f:
                pinned = desktop_id in {line.strip() for line in f}
        except OSError:
            pinned = desktop_id in DEFAULT_TASKBAR_PINS
        if pinned:
            return ("Unpin from taskbar", lambda: sway_exec("ws-taskbar --unpin " + shlex.quote(desktop_id)))
        return ("Pin to taskbar", lambda: sway_exec("ws-taskbar --pin " + shlex.quote(desktop_id)))

    def pin(self, app):
        if advanced_list.is_advanced(app):
            return
        if app_id(app) not in self.pins:
            self.pins.append(app_id(app))
            save_pins(self.pins)
        self.refresh_after_pin()

    def unpin(self, app):
        if app_id(app) in self.pins:
            self.pins.remove(app_id(app))
            save_pins(self.pins)
        self.refresh_after_pin()

    def move_pin_to_front(self, app):
        self.pins.remove(app_id(app))
        self.pins.insert(0, app_id(app))
        save_pins(self.pins)
        self.page = 0
        self.refresh_after_pin()

    def refresh_after_pin(self):
        self.build_home()
        if self.results:
            self.render_results()

    def activate(self, item):
        if item["kind"] == "app":
            self.launch(item["app"])
        elif item["kind"] == "setting":
            self.run(item["cmd"])
        else:
            self.run("xdg-open " + shlex.quote(item["path"]))

    def launch(self, app):
        i = app_id(app)
        self.usage[i] = self.usage.get(i, 0) + 1
        save_json(USAGE_FILE, self.usage)
        self.run("gtk-launch " + shlex.quote(i))

    def open_location(self, path):
        self.run("thunar " + shlex.quote(os.path.dirname(path)))

    def run(self, cmd):
        self.close_flyout()
        sway_exec(cmd)

    def popup(self, anchor, items, above=False, right=False):
        menu = Gtk.Menu()
        for text, callback in items:
            item = Gtk.MenuItem(label=text)
            item.connect("activate", lambda _i, c=callback: GLib.idle_add(lambda: c() and False))
            menu.append(item)
        menu.show_all()
        menu.attach_to_widget(self.menu, None)
        self.popup_menu = menu
        if isinstance(anchor, Gtk.Widget):
            if right:
                menu.popup_at_widget(anchor, Gdk.Gravity.NORTH_EAST, Gdk.Gravity.SOUTH_EAST, None)
            else:
                menu.popup_at_widget(anchor, Gdk.Gravity.NORTH_WEST, Gdk.Gravity.SOUTH_WEST, None)
        else:
            menu.popup_at_pointer(anchor)

    # Showing and hiding

    def prepare(self):
        self.settings = load_settings()
        self.holder.set_halign(Gtk.Align.CENTER if taskbar_centred() else Gtk.Align.START)
        self.settings_pages = settings_pages()
        self.update_dot.set_visible(os.path.lexists(UPDATE_PENDING))
        self.pins = advanced_list.release_pins(load_pins())
        if self.apps_dirty:
            self.apps = app_list()
            self.apps_dirty = False
        self.page = 0
        self.results = None
        self.base_page = "home"
        self.search.set_text("")
        self.build_home()
        self.stack.set_visible_child_full("home", Gtk.StackTransitionType.NONE)
        height = min(MAX_HEIGHT, output_height() - HEIGHT - 2 * GAP)
        self.menu.set_size_request(WIDTH, height)

    def open(self):
        if self.slide.shown:
            return
        self.prepare()
        super().open()
        self.search.grab_focus()

    def close_flyout(self):
        if self.popup_menu:
            self.popup_menu.popdown()
            self.popup_menu = None
        if self.file_proc:
            self.file_proc.force_exit()
            self.file_proc = None
        super().close_flyout()

    def on_click(self, _w, event):
        if event.window != self.get_window():
            return False
        a = self.menu.get_allocation()
        x, y = self.menu.translate_coordinates(self, 0, 0)
        if x is None:
            x, y = a.x, a.y
        if x <= event.x < x + a.width and y <= event.y < y + a.height:
            if event.button == 3:
                self.popup(event, [("Start settings", self.open_settings)])
            return True
        self.close_flyout()
        return True

    def on_key(self, _w, event):
        if event.keyval == Gdk.KEY_Escape:
            if self.search.get_text():
                self.search.set_text("")
            elif self.stack.get_visible_child_name() == "jump":
                self.stack.set_visible_child_full("all", Gtk.StackTransitionType.CROSSFADE)
            elif self.stack.get_visible_child_name() in ("all", "more"):
                self.show_home()
            else:
                self.close_flyout()
            return True
        if not self.search.has_focus() and self.search.handle_event(event):
            self.search.grab_focus_without_selecting()
            self.search.set_position(-1)
            return True
        return False

    def on_scroll(self, _w, event):
        if self.stack.get_visible_child_name() != "home" or self.pages < 2:
            return False
        if event.direction == Gdk.ScrollDirection.DOWN:
            self.set_page(self.page + 1)
        elif event.direction == Gdk.ScrollDirection.UP:
            self.set_page(self.page - 1)
        return True

    def on_apps_changed(self, *_):
        self.apps_dirty = True

    def open_settings(self):
        self.close_flyout()
        sway_exec("ws-settings start")
