# Shared pieces for the ws-settings pages: the Windows 11 Settings styles,
# setting cards and controls, the settings store, and async command and
# D-Bus helpers (D-Bus calls let polkit ask for a password when needed).
import json
import math
import os
import pwd
import shlex
import shutil
import subprocess
import sys

import gi

gi.require_version("Gdk", "3.0")
gi.require_version("Gtk", "3.0")
gi.require_version("PangoCairo", "1.0")
from gi.repository import Gdk, GdkPixbuf, Gio, GLib, Gtk, Pango, PangoCairo  # noqa: E402

import ws_theme  # noqa: E402

FONT = "@FONT@"
BODY = "@BODY@"
CAPTION = "@CAPTION@"
HOME = os.path.expanduser("~")
CONFIG = os.environ.get("XDG_CONFIG_HOME", os.path.join(HOME, ".config"))
STATE = os.path.join(os.environ.get("XDG_STATE_HOME", os.path.join(HOME, ".local", "state")),
                     "ws-settings")
SETTINGS_FILE = ws_theme.settings_path()

CSS = """
#settings, #settings .content { background-color: #f3f3f3; }
#settings label, #settings entry, #settings button, #settings combobox, menu label,
#settings-dialog label, #settings-dialog entry, #settings-dialog button {
  font-family: "@FONT@";
  font-size: @BODY@pt;
  color: #1a1a1a;
}
#settings .caption, #settings-dialog .caption { font-size: @CAPTION@pt; color: #5d5d5d; }
#settings .nav { margin: 0 4px 0 16px; }
#settings .nav-search { margin: 0 4px 12px 16px; }
#settings .account { margin: 16px 16px 16px 20px; }
#settings .account-name { font-weight: 600; }
#settings button.account { background: none; border: none; box-shadow: none; padding: 4px; border-radius: 4px; }
#settings button.account:hover { background-color: alpha(#000000, 0.04); }
#settings list, #settings row { background: none; }
#settings .nav row {
  min-height: 36px;
  border-radius: 4px;
  padding: 0 12px;
  margin: 1px 0;
  border-left: 3px solid transparent;
}
#settings .nav row:hover { background-color: alpha(#000000, 0.04); }
#settings .nav row:selected {
  background-color: alpha(#000000, 0.06);
  border-left-color: @ACCENT@;
}
#settings .nav row:selected label { color: #1a1a1a; }
#settings .page { margin: 0 40px 40px 28px; }
#settings .title-row { margin: 20px 24px 24px 28px; }
#settings .title { font-size: 21pt; font-weight: 600; }
#settings .title-parent { font-size: 21pt; font-weight: 600; color: #5d5d5d; }
#settings button.window-close {
  padding: 6px; min-width: 36px; min-height: 36px;
  background: none; border: none; box-shadow: none; border-radius: 4px;
}
#settings button.window-close:hover { background-color: #c42b1c; color: #ffffff; }
#settings button.window-close:hover image { color: #ffffff; -gtk-icon-style: symbolic; }
#settings button.crumb { padding: 0; min-height: 0; background: none; border: none; box-shadow: none; }
#settings button.crumb:hover label { color: #1a1a1a; }
#settings .section { font-weight: 600; margin: 24px 0 8px 2px; }
#settings .card, #settings .expander-head {
  min-height: 68px;
  padding: 0 16px;
  background-color: #fbfbfb;
  border: 1px solid alpha(#000000, 0.06);
  border-radius: 4px;
}
#settings .card { margin-bottom: 4px; }
#settings .card.tall { padding: 16px; }
#settings button.card { box-shadow: none; }
#settings button.card:hover { background-color: alpha(#f9f9f9, 0.5); }
#settings button.card:active { background-color: alpha(#f9f9f9, 0.3); }
#settings .expander { margin-bottom: 4px; }
#settings .expander.open .expander-head { border-radius: 4px 4px 0 0; }
#settings .expander-body {
  background-color: #fbfbfb;
  border: 1px solid alpha(#000000, 0.06);
  border-top: none;
  border-radius: 0 0 4px 4px;
}
#settings .expander-body .card {
  margin: 0;
  border: none;
  border-top: 1px solid alpha(#000000, 0.06);
  border-radius: 0;
  background: none;
  padding-left: 56px;
}
#settings .hero {
  padding: 16px;
  margin-bottom: 4px;
  background-color: #fbfbfb;
  border: 1px solid alpha(#000000, 0.06);
  border-radius: 8px;
}
#settings .hero-title { font-size: 15pt; font-weight: 600; }
#settings .big { font-size: 21pt; font-weight: 600; }
#settings .link label, #settings-dialog .link label { color: @ACCENT_TEXT@; }
#settings button.link, #settings-dialog button.link {
  background: none; border: none; box-shadow: none; padding: 0 4px; min-height: 24px;
}
#settings button.link:hover label { text-decoration: underline; }
#settings button.btn, #settings combobox button, #settings-dialog button.btn,
#settings button.swatch-more {
  min-height: 30px;
  padding: 0 12px;
  background-color: #fbfbfb;
  background-image: none;
  border: 1px solid alpha(#000000, 0.06);
  border-bottom-color: alpha(#000000, 0.16);
  border-radius: 4px;
  box-shadow: none;
  text-shadow: none;
}
#settings button.btn:hover, #settings combobox button:hover, #settings-dialog button.btn:hover {
  background-color: #f6f6f6;
}
#settings button.btn:disabled label { color: alpha(#000000, 0.36); }
#settings button.accent, #settings-dialog button.accent {
  background-color: @ACCENT@;
  border-color: alpha(#000000, 0.1);
}
#settings button.accent label, #settings-dialog button.accent label { color: #ffffff; }
#settings button.accent:hover, #settings-dialog button.accent:hover { background-color: @ACCENT_TEXT@; }
#settings button.danger label { color: #c42b1c; }
#settings entry, #settings-dialog entry {
  min-height: 30px;
  padding: 0 10px;
  background-color: alpha(#ffffff, 0.7);
  background-image: none;
  border: 1px solid alpha(#000000, 0.06);
  border-bottom-color: alpha(#000000, 0.45);
  border-radius: 4px;
  box-shadow: none;
}
#settings entry:focus, #settings-dialog entry:focus {
  background-color: #ffffff;
  border-bottom: 2px solid @ACCENT@;
}
#settings switch {
  min-width: 38px;
  min-height: 18px;
  border-radius: 999px;
  border: 1px solid alpha(#000000, 0.45);
  background-color: transparent;
  background-image: none;
  box-shadow: none;
}
#settings switch:checked { background-color: @ACCENT@; border-color: @ACCENT@; }
#settings switch slider {
  min-width: 12px;
  min-height: 12px;
  margin: 3px;
  border-radius: 999px;
  border: none;
  background-color: alpha(#000000, 0.6);
  background-image: none;
  box-shadow: none;
}
#settings switch:checked slider { background-color: #ffffff; }
#settings switch:disabled { opacity: 0.4; }
#settings scale trough {
  min-height: 4px;
  border: none;
  border-radius: 999px;
  background-color: alpha(#000000, 0.45);
  background-image: none;
}
#settings scale highlight { min-height: 4px; border-radius: 999px; background-color: @ACCENT@; background-image: none; border: none; }
#settings scale slider {
  min-width: 20px;
  min-height: 20px;
  margin: -8px;
  border-radius: 999px;
  border: 1px solid alpha(#000000, 0.06);
  background-color: #ffffff;
  background-image: none;
  box-shadow: inset 0 0 0 5px #ffffff, inset 0 0 0 10px @ACCENT@;
}
#settings radio, #settings check { border-color: alpha(#000000, 0.45); background-image: none; }
#settings radio:checked, #settings check:checked {
  background-color: @ACCENT@; border-color: @ACCENT@; color: #ffffff;
}
#settings button.swatch {
  min-width: 40px; min-height: 40px; padding: 0; border-radius: 6px; border: 2px solid transparent;
  background: none; box-shadow: none;
}
#settings button.swatch.active { border-color: #1a1a1a; }
#settings button.thumb { padding: 2px; border-radius: 6px; border: 2px solid transparent; background: none; }
#settings button.thumb.active { border-color: @ACCENT@; }
#settings button.tool { padding: 12px 4px; border-radius: 4px; background: none; border: none; box-shadow: none; }
#settings button.tool:hover { background-color: alpha(#000000, 0.04); }
#settings .infobar {
  padding: 12px 16px;
  margin-bottom: 8px;
  border-radius: 4px;
  background-color: #fff4ce;
  border: 1px solid alpha(#000000, 0.06);
}
#settings .infobar label { color: #1a1a1a; }
#settings .ok-badge { color: #0f7b0f; }
#settings .preview-box { border-radius: 8px; border: 1px solid alpha(#000000, 0.06); }
#settings separator { background-color: alpha(#000000, 0.08); min-height: 1px; }
#settings scrolledwindow undershoot, #settings scrolledwindow overshoot { background: none; }
#settings progressbar trough { min-height: 4px; border-radius: 999px; background-color: alpha(#000000, 0.1); border: none; }
#settings progressbar progress { min-height: 4px; border-radius: 999px; background-color: @ACCENT@; border: none; }
#settings-dialog { background-color: #f3f3f3; }
#settings-dialog .dialog-title { font-size: 15pt; font-weight: 600; }
#settings-dialog .dialog-foot {
  padding: 20px 24px;
  background-color: #f3f3f3;
  border-top: 1px solid alpha(#000000, 0.06);
}
#settings-dialog .dialog-body { padding: 24px; background-color: #fbfbfb; }
menu { background-color: #f9f9f9; padding: 4px; border: 1px solid alpha(#000000, 0.1); border-radius: 8px; }
menuitem { min-height: 32px; padding: 0 12px; border-radius: 4px; }
menuitem:hover { background-color: alpha(#000000, 0.05); }
tooltip, tooltip.background { background-color: #f9f9f9; border: 1px solid alpha(#000000, 0.1); border-radius: 4px; }
tooltip label { color: #1a1a1a; font-family: "@FONT@"; font-size: @CAPTION@pt; }
"""

_provider = None


def load_css():
    global _provider
    screen = Gdk.Screen.get_default()
    if _provider:
        Gtk.StyleContext.remove_provider_for_screen(screen, _provider)
    _provider = Gtk.CssProvider()
    _provider.load_from_data(ws_theme.transform(CSS, which="app_mode").encode())
    Gtk.StyleContext.add_provider_for_screen(screen, _provider,
                                             Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 1)
    settings = Gtk.Settings.get_default()
    settings.set_property("gtk-application-prefer-dark-theme", ws_theme.is_dark(which="app_mode"))


# --- Settings store ------------------------------------------------------------

DEFAULTS = {
    **ws_theme.DEFAULTS,
    "clipboard_history": True,
    "night_light": False, "night_light_temp": 4000, "night_light_schedule": "sunset",
    "night_light_from": "21:00", "night_light_to": "07:00",
    "main_display": "",
    "auto_enable_outputs": True,
    "ask_on_display_connect": False,
    "reuse_output_mode": False,
    "color_profiles": {},
    "lock_after": 300, "screen_off_after": 330, "sleep_after": 0,
    "lock_on_sleep": True,
    "taskbar_align": "center", "taskbar_search": "hide", "taskbar_taskview": False,
    "taskbar_autohide": False, "taskbar_badges": True, "taskbar_flash": True,
    "taskbar_all_displays": True, "taskbar_corner": True, "taskbar_combine": "always",
    "taskbar_all_desktops": False, "tray_hidden": ["nm-applet", "blueman"], "tray_overflow": True,
    "clock_seconds": False,
    "focus_follows_mouse": "no", "mouse_warping": "none", "workspace_back_and_forth": False,
    "default_layout": "default", "smart_gaps": False, "gaps": 8,
    "screenshot_dir": "", "update_auto_check": True, "update_notify": True,
    "update_paused_until": 0,
}


def load():
    try:
        with open(SETTINGS_FILE) as f:
            data = json.load(f)
    except (OSError, ValueError):
        data = {}
    return {**DEFAULTS, **(data if isinstance(data, dict) else {})}


def save(**changes):
    data = {}
    try:
        with open(SETTINGS_FILE) as f:
            data = json.load(f)
    except (OSError, ValueError):
        pass
    data.update(changes)
    write_file(SETTINGS_FILE, json.dumps(data, indent=2) + "\n")


def get(key):
    return load()[key]


def write_file(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        f.write(text)
    os.replace(tmp, path)


def read_file(path, default=""):
    try:
        with open(path) as f:
            return f.read()
    except OSError:
        return default


def json_file(path, default):
    try:
        with open(path) as f:
            data = json.load(f)
        return data if isinstance(data, type(default)) else default
    except (OSError, ValueError):
        return default


# --- Commands, D-Bus and Sway ------------------------------------------------------

def which(cmd):
    return shutil.which(cmd) is not None


def out(argv, timeout=10):
    try:
        return subprocess.run(argv, capture_output=True, text=True, timeout=timeout).stdout
    except (OSError, subprocess.TimeoutExpired):
        return ""


def ok(argv, timeout=20):
    try:
        return subprocess.run(argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                              timeout=timeout).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def spawn(cmd):
    """Start a program outside this process (it survives Settings closing)."""
    if isinstance(cmd, list):
        cmd = " ".join(shlex.quote(c) for c in cmd)
    subprocess.Popen(cmd, shell=True, start_new_session=True, stdin=subprocess.DEVNULL,
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def run(argv, done=None, stdin=None, lines=None):
    """Run argv without blocking. done(success, stdout, stderr); lines(line) streams stdout."""
    flags = Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE
    if stdin is not None:
        flags |= Gio.SubprocessFlags.STDIN_PIPE
    try:
        proc = Gio.Subprocess.new(argv, flags)
    except GLib.Error as e:
        message = e.message
        if done:
            GLib.idle_add(lambda: done(False, "", message) and False)
        return None
    if lines:
        stream = Gio.DataInputStream.new(proc.get_stdout_pipe())
        collected = []

        def on_line(s, result):
            try:
                line, _n = s.read_line_finish_utf8(result)
            except GLib.Error:
                line = None
            if line is None:
                proc.wait_async(None, lambda p, r: done and done(
                    p.wait_finish(r) and p.get_successful(), "\n".join(collected), ""))
                return
            collected.append(line)
            lines(line)
            s.read_line_async(GLib.PRIORITY_DEFAULT, None, on_line)
        stream.read_line_async(GLib.PRIORITY_DEFAULT, None, on_line)
        return proc

    def finish(p, result):
        try:
            _ok, o, e = p.communicate_utf8_finish(result)
        except GLib.Error as err:
            o, e = "", err.message
        if done:
            done(p.get_successful(), o or "", e or "")
    proc.communicate_utf8_async(stdin, None, finish)
    return proc


def _labwc():
    root = os.path.abspath(os.path.join(os.path.dirname(os.path.realpath(__file__)), ".."))
    local = os.path.join(os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share")), "mde")
    for path in (local, root, "/usr/share/mde"):
        if path and path not in sys.path:
            sys.path.insert(0, path)
    import labwc_cfg
    return labwc_cfg


def sway(*commands):
    """Apply compositor commands. Output lines go to wlr-randr."""
    try:
        cfg = _labwc()
    except Exception:
        return
    for c in commands:
        if c.startswith("output "):
            cfg.apply_output_line(c)
        elif c.startswith("gaps "):
            cfg.apply_legacy("gaps", c)


def sway_json(kind):
    if kind != "get_outputs":
        return []
    try:
        return _labwc().outputs()
    except Exception:
        return []


def proxy(name, path, iface, bus=Gio.BusType.SYSTEM):
    try:
        return Gio.DBusProxy.new_for_bus_sync(bus, Gio.DBusProxyFlags.NONE, None,
                                              name, path, iface, None)
    except GLib.Error:
        return None


def prop(p, name, default=None):
    if not p:
        return default
    v = p.get_cached_property(name)
    return v.unpack() if v is not None else default


def call(p, method, signature=None, *args, done=None):
    """Call a D-Bus method asynchronously; polkit may ask for a password."""
    params = GLib.Variant(signature, args) if signature else None

    def finish(px, result):
        try:
            value = px.call_finish(result)
            if done:
                done(True, value.unpack() if value else None)
        except GLib.Error as e:
            Gio.DBusError.strip_remote_error(e)
            if done:
                done(False, e.message)
    if not p:
        if done:
            done(False, "Service not available")
        return
    p.call(method, params, Gio.DBusCallFlags.ALLOW_INTERACTIVE_AUTHORIZATION, 120000,
           None, finish)


SWAY_SETTINGS = os.path.join(CONFIG, "mde", "sway", "config.d", "60-ws-settings.conf")


def sway_conf(key, line):
    """Apply a settings change on labwc. The old Sway config.d file is no longer written."""
    if not line:
        return
    try:
        _labwc().apply_legacy(key, line)
    except Exception:
        pass


def notify(title, body="", icon="preferences-system"):
    subprocess.Popen(["notify-send", "-a", "Settings", "-i", icon, title, body],
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def human_size(n):
    for unit in ("bytes", "KB", "MB", "GB", "TB"):
        if n < 1024 or unit == "TB":
            return f"{n:.0f} {unit}" if unit == "bytes" else f"{n:.1f} {unit}"
        n /= 1024


# --- Widgets ---------------------------------------------------------------------

def css(widget, *names):
    for name in names:
        widget.get_style_context().add_class(name)
    return widget


def clear(box):
    for child in box.get_children():
        child.destroy()


def label(text, *classes, wrap=False, xalign=0.0, markup=False, selectable=False):
    lab = Gtk.Label(xalign=xalign)
    lab.set_markup(text) if markup else lab.set_text(text)
    if wrap:
        lab.set_line_wrap(True)
        lab.set_line_wrap_mode(Pango.WrapMode.WORD_CHAR)
        lab.set_max_width_chars(60)
    else:
        lab.set_ellipsize(Pango.EllipsizeMode.END)
    lab.set_selectable(selectable)
    return css(lab, *classes)


def icon(name, size=20):
    image = Gtk.Image()
    theme = Gtk.IconTheme.get_default()
    gicon = name if isinstance(name, Gio.Icon) else None
    if gicon is None:
        for candidate in (name, (name or "").replace("-symbolic", ""), "application-x-executable"):
            if candidate and theme.has_icon(candidate):
                gicon = Gio.ThemedIcon.new(candidate)
                break
    if gicon is None or not theme.lookup_by_gicon(gicon, size, 0):
        gicon = Gio.ThemedIcon.new("application-x-executable")
    image.set_from_gicon(gicon, Gtk.IconSize.DIALOG)
    image.set_pixel_size(size)
    if ws_theme.is_dark(which="app_mode"):
        image.connect("realize", lambda w: lighten(w, gicon, size))
    return image


def lighten(image, gicon, size):
    """Dark mode: redraw icons drawn in plain black as light grey, keeping colour icons."""
    scale = image.get_scale_factor()
    info = Gtk.IconTheme.get_default().lookup_by_gicon_for_scale(gicon, size, scale,
                                                                 Gtk.IconLookupFlags.FORCE_SIZE)
    try:
        pix = info.load_icon() if info else None
    except GLib.Error:
        pix = None
    if not pix or not pix.get_has_alpha() or pix.get_n_channels() != 4:
        return
    data = bytearray(pix.get_pixels())
    stride = pix.get_rowstride()
    w, h = pix.get_width(), pix.get_height()
    seen = 0
    for y in range(h):
        for x in range(w):
            i = y * stride + x * 4
            if data[i + 3] < 40:
                continue
            r, g, b = data[i], data[i + 1], data[i + 2]
            if max(r, g, b) - min(r, g, b) > 40 or max(r, g, b) > 110:
                return
            seen += 1
    if not seen:
        return
    for y in range(h):
        for x in range(w):
            i = y * stride + x * 4
            data[i] = data[i + 1] = data[i + 2] = 255 - data[i]
    out = GdkPixbuf.Pixbuf.new_from_bytes(GLib.Bytes.new(bytes(data)), GdkPixbuf.Colorspace.RGB,
                                          True, 8, w, h, stride)
    image.set_from_surface(Gdk.cairo_surface_create_from_pixbuf(out, scale, None))


def texts(title, caption=None, wrap=True):
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
    box.pack_start(label(title, wrap=wrap), False, False, 0)
    if caption:
        box.pack_start(label(caption, "caption", wrap=wrap), False, False, 0)
    return box


def row(title, caption=None, icon_name=None, control=None, tall=False):
    """A setting card: icon, title and caption on the left, a control on the right."""
    card = css(Gtk.Box(spacing=16), "card")
    if tall:
        css(card, "tall")
    if icon_name:
        card.pack_start(icon(icon_name), False, False, 4)
    card.pack_start(texts(title, caption), True, True, 0)
    if control is not None:
        if isinstance(control, (list, tuple)):
            box = Gtk.Box(spacing=8, valign=Gtk.Align.CENTER)
            for c in control:
                box.pack_start(c, False, False, 0)
            control = box
        control.set_valign(Gtk.Align.CENTER)
        card.pack_end(control, False, False, 0)
    return card


def nav_row(title, caption, icon_name, callback, extra=None):
    """A clickable card with a chevron, leading to another page."""
    b = css(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "card")
    box = Gtk.Box(spacing=16)
    if icon_name:
        box.pack_start(icon(icon_name), False, False, 4)
    box.pack_start(texts(title, caption), True, True, 0)
    if extra:
        box.pack_end(chevron(), False, False, 0)
        box.pack_end(extra, False, False, 0)
    else:
        box.pack_end(chevron(), False, False, 0)
    b.add(box)
    b.connect("clicked", lambda *_: callback())
    return b


def chevron(name="go-next-symbolic"):
    image = Gtk.Image.new_from_icon_name(name, Gtk.IconSize.MENU)
    image.set_pixel_size(12)
    return image


def expander(title, caption=None, icon_name=None, control=None, expanded=False):
    """A Windows 11 settings expander. Returns (widget, body box)."""
    outer = css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "expander")
    head = css(Gtk.EventBox(), "expander-head")
    inner = Gtk.Box(spacing=16)
    if icon_name:
        inner.pack_start(icon(icon_name), False, False, 4)
    inner.pack_start(texts(title, caption), True, True, 0)
    arrow = chevron("pan-down-symbolic")
    inner.pack_end(arrow, False, False, 0)
    if control is not None:
        control.set_valign(Gtk.Align.CENTER)
        inner.pack_end(control, False, False, 8)
    head.add(inner)
    head.set_size_request(-1, 68)
    inner.set_margin_start(16)
    inner.set_margin_end(16)
    body = css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "expander-body")
    rev = Gtk.Revealer(transition_type=Gtk.RevealerTransitionType.SLIDE_DOWN,
                       transition_duration=150)
    rev.add(body)
    outer.pack_start(head, False, False, 0)
    outer.pack_start(rev, False, False, 0)

    def toggle(open_):
        rev.set_reveal_child(open_)
        arrow.set_from_icon_name("pan-up-symbolic" if open_ else "pan-down-symbolic",
                                 Gtk.IconSize.MENU)
        (outer.get_style_context().add_class if open_ else
         outer.get_style_context().remove_class)("open")
    head.connect("button-press-event", lambda *_: toggle(not rev.get_reveal_child()) or True)
    GLib.idle_add(lambda: toggle(expanded) and False)
    return outer, body


def section(title):
    return label(title, "section")


def switch(active, callback, sensitive=True):
    s = Gtk.Switch(active=bool(active), valign=Gtk.Align.CENTER, sensitive=sensitive)
    state = Gtk.Label(label="On" if active else "Off")
    box = Gtk.Box(spacing=12, valign=Gtk.Align.CENTER)
    box.pack_start(state, False, False, 0)
    box.pack_start(s, False, False, 0)

    def changed(sw, _p):
        state.set_text("On" if sw.get_active() else "Off")
        callback(sw.get_active())
    s.connect("notify::active", changed)
    box.switch = s
    return box


def combo(options, active, callback, width=200):
    """options: list of (value, label)."""
    c = Gtk.ComboBoxText(valign=Gtk.Align.CENTER)
    c.set_size_request(width, -1)
    values = []
    for value, text in options:
        c.append_text(text)
        values.append(value)
    if active is not None and active not in values and isinstance(active, int):
        c.append_text(f"{active // 60} min {active % 60} s" if active % 60 else f"{active // 60} minutes")
        values.append(active)
    if active in values:
        c.set_active(values.index(active))
    c.connect("changed", lambda w: w.get_active() >= 0 and callback(values[w.get_active()]))
    return c


def slider(value, lo, hi, callback, step=1, width=220, live=False, fmt=None):
    s = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, lo, hi, step)
    s.set_value(value)
    s.set_draw_value(False)
    s.set_size_request(width, -1)
    s.set_valign(Gtk.Align.CENTER)
    lab = Gtk.Label(label=(fmt or str)(int(value)))
    lab.set_width_chars(5)
    box = Gtk.Box(spacing=8, valign=Gtk.Align.CENTER)
    box.pack_start(lab, False, False, 0)
    box.pack_start(s, False, False, 0)
    pending = {"id": 0}

    def fire():
        pending["id"] = 0
        callback(int(round(s.get_value())))
        return False

    def changed(w):
        lab.set_text((fmt or str)(int(round(w.get_value()))))
        if live:
            callback(int(round(w.get_value())))
            return
        if pending["id"]:
            GLib.source_remove(pending["id"])
        pending["id"] = GLib.timeout_add(250, fire)
    s.connect("value-changed", changed)
    box.scale = s
    return box


def button(text, callback, accent=False, danger=False, sensitive=True):
    b = css(Gtk.Button(label=text, valign=Gtk.Align.CENTER, sensitive=sensitive), "btn")
    if accent:
        css(b, "accent")
    if danger:
        css(b, "danger")
    b.connect("clicked", lambda *_: callback())
    return b


def link(text, callback):
    b = css(Gtk.Button(label=text, halign=Gtk.Align.START, valign=Gtk.Align.CENTER), "link")
    b.connect("clicked", lambda *_: callback())
    return b


def entry(text="", placeholder="", width=240, password=False):
    e = Gtk.Entry(text=text, placeholder_text=placeholder, valign=Gtk.Align.CENTER)
    e.set_size_request(width, -1)
    if password:
        e.set_visibility(False)
        e.set_input_purpose(Gtk.InputPurpose.PASSWORD)
    return e


def infobar(text, kind="warning", action=None):
    box = css(Gtk.Box(spacing=12), "infobar")
    box.pack_start(icon("dialog-information-symbolic" if kind == "info" else
                        "dialog-warning-symbolic", 16), False, False, 0)
    box.pack_start(label(text, wrap=True), True, True, 0)
    if action:
        box.pack_end(button(action[0], action[1]), False, False, 0)
    return box


def vbox(spacing=0, *children):
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=spacing)
    for c in children:
        box.pack_start(c, False, False, 0)
    return box


def popup_menu(anchor, items):
    menu = Gtk.Menu()
    for text, callback in items:
        item = Gtk.MenuItem(label=text)
        item.connect("activate", lambda _i, c=callback: GLib.idle_add(lambda: c() and False))
        menu.append(item)
    menu.show_all()
    menu.attach_to_widget(anchor, None)
    menu.popup_at_widget(anchor, Gdk.Gravity.SOUTH_EAST, Gdk.Gravity.NORTH_EAST, None)
    return menu


class Avatar(Gtk.DrawingArea):
    def __init__(self, size):
        super().__init__()
        self.size = size
        self.set_size_request(size, size)
        self.connect("draw", self.on_draw)
        self.reload()

    def reload(self):
        self.picture = None
        name = pwd.getpwuid(os.getuid()).pw_name
        for path in (os.path.join(HOME, ".face"), os.path.join(HOME, ".face.icon"),
                     f"/var/lib/AccountsService/icons/{name}"):
            try:
                pix = GdkPixbuf.Pixbuf.new_from_file(path)
                side = min(pix.get_width(), pix.get_height())
                pix = pix.new_subpixbuf((pix.get_width() - side) // 2,
                                        (pix.get_height() - side) // 2, side, side)
                self.picture = pix.scale_simple(self.size, self.size, GdkPixbuf.InterpType.BILINEAR)
                break
            except GLib.Error:
                continue
        self.queue_draw()

    def on_draw(self, _w, cr):
        s = self.size
        cr.arc(s / 2, s / 2, s / 2, 0, 2 * math.pi)
        cr.clip()
        if self.picture:
            Gdk.cairo_set_source_pixbuf(cr, self.picture, 0, 0)
            cr.paint()
            return
        fill = ws_theme.accents(which="app_mode")[0]
        cr.set_source_rgb(*(int(fill[i:i + 2], 16) / 255 for i in (1, 3, 5)))
        cr.paint()
        layout = PangoCairo.create_layout(cr)
        layout.set_text(user_name()[:1].upper(), -1)
        layout.set_font_description(Pango.FontDescription(f"{FONT} Semibold {s * 0.3:.0f}"))
        w, h = layout.get_pixel_size()
        cr.move_to((s - w) / 2, (s - h) / 2)
        on = ws_theme.readable_on(fill)
        cr.set_source_rgb(*(int(on[i:i + 2], 16) / 255 for i in (1, 3, 5)))
        PangoCairo.show_layout(cr, layout)


def user_name():
    p = pwd.getpwuid(os.getuid())
    return (p.pw_gecos.split(",")[0] or p.pw_name).strip()


class Dialog(Gtk.Window):
    """A Windows 11 content dialog: title, body, and buttons in a footer."""

    def __init__(self, parent, title, primary=None, secondary="Cancel", width=440,
                 tertiary=None):
        super().__init__(title=title, transient_for=parent, modal=True)
        self.set_name("settings-dialog")
        self.set_default_size(width, -1)
        self.set_resizable(False)
        self.set_type_hint(Gdk.WindowTypeHint.DIALOG)
        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.body = css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12), "dialog-body")
        self.body.pack_start(label(title, "dialog-title", wrap=True), False, False, 0)
        outer.pack_start(self.body, True, True, 0)
        self.foot = css(Gtk.Box(spacing=8, homogeneous=True), "dialog-foot")
        self.primary = None
        self.tertiary = None
        if primary:
            self.primary = css(Gtk.Button(label=primary), "btn", "accent")
            self.foot.pack_start(self.primary, True, True, 0)
        if tertiary:
            self.tertiary = css(Gtk.Button(label=tertiary), "btn")
            self.foot.pack_start(self.tertiary, True, True, 0)
        if secondary:
            b = css(Gtk.Button(label=secondary), "btn")
            b.connect("clicked", lambda *_: self.destroy())
            self.foot.pack_start(b, True, True, 0)
        outer.pack_start(self.foot, False, False, 0)
        self.add(outer)
        self.connect("key-press-event", lambda _w, e: e.keyval == Gdk.KEY_Escape
                     and (self.destroy() or True))

    def add_text(self, text):
        self.body.pack_start(label(text, wrap=True), False, False, 0)

    def add_field(self, title, widget):
        self.body.pack_start(label(title, "caption"), False, False, 0)
        self.body.pack_start(widget, False, False, 0)
        return widget

    def on_primary(self, callback):
        if self.primary:
            self.primary.connect("clicked", lambda *_: callback())

    def on_tertiary(self, callback):
        if self.tertiary:
            self.tertiary.connect("clicked", lambda *_: callback())

    def run_dialog(self):
        self.show_all()
        self.present()


def confirm(parent, title, text, action, callback, danger=False):
    d = Dialog(parent, title, action)
    d.add_text(text)
    d.on_primary(lambda: (d.destroy(), callback()))
    d.run_dialog()


def error(parent, title, text):
    d = Dialog(parent, title, None, "Close")
    d.add_text(text)
    d.run_dialog()


def install(app, package):
    """Install a package through PackageKit (polkit asks for a password)."""
    ui_note = Dialog(app.win, f"Installing {package}\u2026", None, None)
    ui_note.add_text("This can take a minute.")
    ui_note.run_dialog()

    def done(success, _o, err):
        ui_note.destroy()
        if success:
            app.refresh()
        else:
            error(app.win, f"Couldn\u2019t install {package}", err.strip() or "PackageKit reported an error.")
    run(["pkcon", "install", "-y", "--plain", package], done)
