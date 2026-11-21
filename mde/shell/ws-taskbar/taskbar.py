#!/usr/bin/env python3
"""ws-taskbar: the Windows 11 taskbar for labwc.

Start, Search and Task View with pinned and running apps (grouped by app, with jump
lists), the system tray, Quick Settings, the clock with the notification centre, and the
show-desktop corner. Start is a flyout in this process; "ws-start" / "ws-drawer" forward
to the taskbar socket. "ws-taskbar --quick-settings", "--notifications", "--task-view",
"--activate N", "--launch N", "--desktop next|prev|new|close", "--pin ID", "--unpin ID"
and "--reload" talk to the running taskbar.
"""

import fcntl
import os
import signal
import socket
import sys

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))

RUNTIME = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
SOCK = os.path.join(RUNTIME, "ws-taskbar.sock")
LOCK = os.path.join(RUNTIME, "ws-taskbar.lock")
COMMANDS = {"--quick-settings": "quick", "--notifications": "notifications", "--task-view": "taskview",
            "--reload": "reload", "--clear-notifications": "clear-notifications"}


def client():
    args = sys.argv[1:]
    cmd = None
    if args and args[0] in COMMANDS:
        cmd = COMMANDS[args[0]]
    elif len(args) >= 2 and args[0] in ("--activate", "--launch", "--desktop", "--pin", "--unpin"):
        cmd = args[0][2:] + " " + args[1]
    elif args and args[0] not in ("--daemon",):
        print(__doc__.strip())
        sys.exit(0 if args[0] in ("-h", "--help") else 2)
    if cmd:
        s = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
        try:
            s.sendto(cmd.encode(), SOCK)
        except OSError:
            sys.exit("ws-taskbar is not running")
        sys.exit(0)
    lock = open(LOCK, "w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        sys.exit(0)
    return lock


LOCK_FILE = client()

import time  # noqa: E402

from common import (GEOMETRY_FILE, HEIGHT, UPDATES_FILE, SWAY, Apps, Desktops, Flyout, Gdk,  # noqa: E402
                    Gio, GLib, Gtk, GtkLayerShell, activate, app_icon, clear, close_window, css_class,
                    accent_rgba, advanced_blocked, is_dark, label, launch, load_json, load_pins, minimize, popup_menu, save_json, save_pins,
                    settings, sway_exec, symbolic, theme_css, windows, ws_motion)
from notify import NotificationCentre, Store  # noqa: E402
from quick import QuickSettings, Status  # noqa: E402
from start import StartMenu  # noqa: E402
from taskview import DesktopToast, TaskView  # noqa: E402
from tray import Tray  # noqa: E402

APP_TARGET = [Gtk.TargetEntry.new("ws-taskbar-app", Gtk.TargetFlags.SAME_APP, 0)]


def brand_image(size):
    """The Mackes mark. White on a dark taskbar, blue on a light one."""
    from gi.repository import GdkPixbuf
    name = "mackes-light.svg" if is_dark() else "mackes.svg"
    roots = ("/usr/share/mde/brand",
             os.path.normpath(os.path.join(os.path.dirname(__file__), "..", "..", "assets", "brand")))
    for root in roots:
        path = os.path.join(root, name)
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

CSS = """
* { font-family: "@FONT@"; }
#taskbar-window, #flyout-window, #desktop-toast { background: transparent; }
#taskbar {
  background-color: alpha(#f3f3f3, 0.85);
  border-top: 1px solid alpha(#000000, 0.08);
  color: #1a1a1a;
  font-size: @BODY@pt;
}
#taskbar button {
  background: none; border: none; box-shadow: none; text-shadow: none; -gtk-icon-shadow: none;
  min-height: 0; min-width: 0; padding: 0; border-radius: 4px; color: #1a1a1a;
  transition: background-color 83ms linear, box-shadow 83ms linear;
}
#taskbar button:hover { background-color: alpha(#ffffff, 0.7); box-shadow: inset 0 0 0 1px alpha(#000000, 0.05); }
#taskbar button:active { background-color: alpha(#ffffff, 0.5); }
#taskbar button image { transition: -gtk-icon-transform 167ms cubic-bezier(0, 0, 0, 1); }
#taskbar button:active image { -gtk-icon-transform: scale(0.85); }
#taskbar .start, #taskbar .app, #taskbar .taskview-button, #taskbar .search {
  min-width: 40px; min-height: 40px; margin: 4px 2px;
}
#taskbar .start { font-size: 18pt; color: #0078d4; }
#taskbar .search.box {
  min-width: 200px; padding: 0 12px; margin: 6px 4px; min-height: 36px; border-radius: 18px;
  background-color: alpha(#ffffff, 0.7); box-shadow: inset 0 0 0 1px alpha(#000000, 0.08);
}
#taskbar .search.label { padding: 0 12px 0 10px; }
#taskbar .search-text { color: #5d5d5d; }
#taskbar .app {
  transition: background-color 83ms linear, box-shadow 83ms linear,
              background-size 250ms cubic-bezier(0, 0, 0, 1), background-image 167ms linear;
}
#taskbar .app.running {
  background-image: linear-gradient(alpha(#000000, 0.45), alpha(#000000, 0.45));
  background-size: 6px 3px; background-position: 50% 95%; background-repeat: no-repeat;
}
#taskbar .app.active {
  background-color: alpha(#ffffff, 0.7);
  box-shadow: inset 0 0 0 1px alpha(#000000, 0.06);
  background-image: linear-gradient(#0078d4, #0078d4);
  background-size: 16px 3px; background-position: 50% 95%; background-repeat: no-repeat;
}
#taskbar .app.urgent {
  background-color: alpha(#da3b01, 0.22);
  background-image: linear-gradient(#da3b01, #da3b01);
  background-size: 16px 3px; background-position: 50% 95%; background-repeat: no-repeat;
}
#taskbar .app.labelled { padding: 0 10px 0 8px; }
#taskbar .app-label { margin-left: 8px; }
#taskbar .badge {
  background-color: #0078d4; color: #ffffff; border-radius: 8px; min-width: 16px; min-height: 16px;
  font-size: 7pt; font-weight: 600; margin: 4px 2px 0 0; padding: 0 3px;
}
#taskbar .tray-icon, #taskbar .chevron, #taskbar .updates, #taskbar .kbd {
  min-width: 28px; min-height: 40px; margin: 4px 0;
}
#taskbar .qs-group, #taskbar .clock { min-height: 40px; margin: 4px 0; padding: 0 8px; }
#taskbar .qs-group box { min-width: 0; }
#taskbar .kbd label { font-size: @CAPTION@pt; }
#taskbar .clock label { font-size: @CAPTION@pt; }
#taskbar .bell-count {
  background-color: #0078d4; color: #ffffff; border-radius: 9px; min-width: 18px; min-height: 18px;
  font-size: 7.5pt; font-weight: 600; padding: 0 3px;
}
#taskbar .update-dot { color: #da3b01; font-size: 7pt; }
#taskbar .corner { min-width: 7px; margin: 0 0 0 4px; border-radius: 0; border-left: 1px solid alpha(#000000, 0.15); }
#taskbar .corner:hover { background-color: alpha(#ffffff, 0.7); }

.flyout {
  background-color: #f3f3f3; color: #1a1a1a; font-size: @BODY@pt;
  border: 1px solid alpha(#000000, 0.1); border-radius: 8px;
  box-shadow: 0 8px 16px alpha(#000000, 0.14);
}
.flyout button {
  background: none; border: none; box-shadow: none; text-shadow: none; -gtk-icon-shadow: none;
  color: #1a1a1a; border-radius: 4px; min-height: 0; min-width: 0;
  transition: background-color 83ms linear;
}
.flyout button:hover { background-color: alpha(#000000, 0.04); }
.flyout button:active { background-color: alpha(#000000, 0.02); }
.flyout .caption { color: #5d5d5d; font-size: @CAPTION@pt; }
.flyout entry {
  background-color: #ffffff; color: #1a1a1a; border: 1px solid alpha(#000000, 0.1);
  border-bottom: 1px solid alpha(#000000, 0.45); border-radius: 4px; min-height: 30px; padding: 0 10px;
  box-shadow: none;
}
.flyout entry:focus { border-bottom: 2px solid #0078d4; }
.flyout scrolledwindow undershoot, .flyout scrolledwindow overshoot { background: none; box-shadow: none; }
#quick-settings { min-width: 360px; }
.qs-body { padding: 20px 20px 16px; }
.tile {
  background-color: #fbfbfb; border: 1px solid alpha(#000000, 0.06); border-radius: 4px; min-height: 48px;
  transition: background-color 167ms linear, border-color 167ms linear;
}
.tile button { min-height: 48px; color: #1a1a1a; }
.tile.on { background-color: #0078d4; border-color: #0078d4; }
.tile.on button { color: #ffffff; }
.tile.on button:hover { background-color: alpha(#ffffff, 0.1); }
.tile.split .tile-main { border-radius: 4px 0 0 4px; }
.tile .tile-more { min-width: 32px; border-radius: 0 4px 4px 0; border-left: 1px solid alpha(#000000, 0.08); }
.tile.on .tile-more { border-left: 1px solid alpha(#ffffff, 0.3); }
.tile-label { font-size: @CAPTION@pt; }
.qs-footer {
  background-color: #f0f0f2; border-top: 1px solid alpha(#000000, 0.08);
  border-radius: 0 0 8px 8px; padding: 6px 12px; min-height: 36px;
}
.qs-icon { min-width: 32px; min-height: 32px; }
.qs-icon.on { background-color: alpha(#0078d4, 0.15); }
.qs-icon.flat-icon:hover { background: none; }
.qs-link { min-height: 32px; padding: 0 8px; }
.qs-head { padding: 12px 16px 8px 8px; }
.qs-title { font-weight: 600; font-size: 12pt; }
.qs-list { padding: 0 8px 8px; }
.qs-row { min-height: 48px; padding: 4px 12px; }
.qs-row.selected, .qs-row-box.selected { background-color: alpha(#000000, 0.04); border-radius: 4px; }
.qs-details { padding: 4px 12px 12px 40px; }
.qs-button {
  background-color: #fbfbfb; border: 1px solid alpha(#000000, 0.1); border-radius: 4px; padding: 4px 16px;
  min-height: 24px;
}
.qs-button.accent { background-color: #0078d4; border-color: #0078d4; color: #ffffff; }
.qs-button.accent:hover { background-color: alpha(#0078d4, 0.9); }
.qs-empty { color: #5d5d5d; }
.flyout scale { padding: 8px 0; }
.flyout scale trough { min-height: 4px; border-radius: 2px; background-color: alpha(#000000, 0.45); border: none; }
.flyout scale highlight { background-color: #0078d4; border-radius: 2px; border: none; }
.flyout scale slider {
  min-width: 12px; min-height: 12px; margin: -10px; border-radius: 12px; background-color: #0078d4;
  border: 5px solid #ffffff; box-shadow: 0 0 0 1px alpha(#000000, 0.1);
}

#notification-centre { min-width: 364px; }
.nc-card { padding-bottom: 8px; }
.nc-head { padding: 10px 12px 6px 16px; }
.nc-title { font-weight: 600; }
.nc-hint { padding: 0 16px 8px; }
.nc-empty { color: #5d5d5d; padding: 36px 0 44px; }
.nc-list { padding: 0 8px; }
.nc-group { padding: 8px 4px 2px 8px; }
.nc-app { font-size: @CAPTION@pt; }
.flyout .nc-note {
  background-color: #fbfbfb; border: 1px solid alpha(#000000, 0.06); border-radius: 6px; padding: 12px;
}
.flyout .nc-note:hover { background-color: #f9f9f9; }
.flyout .nc-note.critical { border-left: 3px solid #da3b01; }
.nc-summary { font-weight: 600; }
.nc-close { min-width: 24px; min-height: 24px; }
.cal-head { padding: 12px 16px; border-radius: 8px 8px 0 0; }
.cal-title { font-weight: 600; }
.flyout calendar {
  background: none; border: none; color: #1a1a1a; padding: 4px 16px 8px; font-size: @BODY@pt;
}
.flyout calendar.header { border: none; background: none; box-shadow: none; }
.flyout calendar.button { color: #1a1a1a; }
.flyout calendar:selected { background-color: #0078d4; color: #ffffff; border-radius: 16px; }
.flyout calendar:indeterminate { color: alpha(#1a1a1a, 0.35); }
.focus-row { padding: 8px 16px 12px; border-top: 1px solid alpha(#000000, 0.06); }

#tray-overflow { padding: 8px; }
#tray-overflow button { min-width: 36px; min-height: 36px; }

menu, .ws-menu {
  background-color: #f9f9f9; color: #1a1a1a; border: 1px solid alpha(#000000, 0.1); padding: 4px;
  font-family: "@FONT@"; font-size: @BODY@pt;
}
menu menuitem {
  padding: 7px 12px; border-radius: 4px; min-width: 180px; color: #1a1a1a;
  transition: background-color 83ms linear;
}
menu menuitem:hover { background-color: alpha(#000000, 0.05); }
menu menuitem:disabled label { color: alpha(#1a1a1a, 0.4); }
menu separator { background-color: alpha(#000000, 0.08); min-height: 1px; margin: 4px 0; }
menu .menu-title { font-weight: 600; }

#taskview { background: transparent; color: #1a1a1a; font-size: @BODY@pt; }
.tv-backdrop {
  background-color: alpha(#f3f3f3, 0);
  transition: background-color 167ms linear;
}
.tv-backdrop.shown {
  background-color: alpha(#f3f3f3, 0.8);
  transition: background-color 167ms cubic-bezier(0, 0, 0, 1);
}
.tv-backdrop .tv-card, .tv-backdrop .tv-strip, .tv-backdrop .tv-empty {
  opacity: 0; transition: opacity 167ms linear;
}
.tv-backdrop.shown .tv-card, .tv-backdrop.shown .tv-strip, .tv-backdrop.shown .tv-empty {
  opacity: 1; transition: opacity 250ms cubic-bezier(0, 0, 0, 1);
}
#taskview button {
  background: none; border: none; box-shadow: none; color: #1a1a1a; border-radius: 4px; min-height: 0; min-width: 0;
}
#taskview button:hover { background-color: alpha(#000000, 0.06); }
#taskview flowboxchild { padding: 4px; border-radius: 10px; background: none; }
#taskview flowboxchild:focus { box-shadow: inset 0 0 0 2px #0078d4; }
.tv-card-head { padding: 4px 4px 6px 6px; }
.tv-close { min-width: 24px; min-height: 24px; }
.tv-card-body {
  background-color: #fbfbfb; border: 1px solid alpha(#000000, 0.08); border-radius: 8px;
  box-shadow: 0 4px 12px alpha(#000000, 0.12);
}
.tv-card:hover .tv-card-body { box-shadow: 0 0 0 3px #0078d4; }
.tv-empty { font-size: 14pt; color: #5d5d5d; }
.tv-strip { padding: 16px 24px 24px; }
.tv-desktop { padding: 8px; border-radius: 8px; background-color: alpha(#ffffff, 0.5); }
.tv-desktop:hover { background-color: alpha(#ffffff, 0.7); }
.tv-desktop.current { box-shadow: inset 0 0 0 2px #0078d4; }
.tv-desktop-name { font-size: @CAPTION@pt; padding-left: 2px; }
.tv-preview { background-color: alpha(#000000, 0.05); border-radius: 4px; }
#taskview .tv-new {
  min-width: 192px; min-height: 120px; border-radius: 8px; border: 1px dashed alpha(#000000, 0.2);
  background-color: alpha(#ffffff, 0.5);
}
.tv-rename { padding: 20px; background-color: #f3f3f3; color: #1a1a1a; }
.tv-rename-title { font-weight: 600; font-size: 12pt; }
.toast {
  background-color: #f3f3f3; color: #1a1a1a; border-radius: 8px; padding: 16px 32px;
  border: 1px solid alpha(#000000, 0.1); box-shadow: 0 8px 16px alpha(#000000, 0.14); margin: 16px;
}
.toast-text { font-size: 18pt; font-weight: 600; }
"""

# Optional translucent fills (used only when ws_motion.acrylic() is true).
# MDE's Fedora sway build keeps this off; opaque Fluent fills are the default.
ACRYLIC_CSS = """
.flyout, .toast { background-color: alpha(#f3f3f3, 0.85); }
.qs-footer { background-color: alpha(#f0f0f2, 0.6); }
"""


def on_signal(sig, callback):
    try:
        import gi
        gi.require_version("GLibUnix", "2.0")
        from gi.repository import GLibUnix
        GLibUnix.signal_add(GLib.PRIORITY_DEFAULT, sig, callback)
    except (ImportError, ValueError):
        GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, sig, callback)


class TaskViewIcon(Gtk.DrawingArea):
    """The Windows 11 Task View glyph: two overlapping rounded squares."""

    def __init__(self):
        super().__init__(halign=Gtk.Align.CENTER, valign=Gtk.Align.CENTER)
        self.set_size_request(20, 20)
        self.connect("draw", self.on_draw)

    def on_draw(self, widget, cr):
        color = widget.get_style_context().get_color(widget.get_state_flags())
        import math

        def rounded(x, y, w, h, r):
            cr.new_sub_path()
            cr.arc(x + w - r, y + r, r, -math.pi / 2, 0)
            cr.arc(x + w - r, y + h - r, r, 0, math.pi / 2)
            cr.arc(x + r, y + h - r, r, math.pi / 2, math.pi)
            cr.arc(x + r, y + r, r, math.pi, 3 * math.pi / 2)
            cr.close_path()
        cr.set_source_rgba(color.red, color.green, color.blue, 1)
        cr.set_line_width(1.4)
        rounded(2.5, 6.5, 11, 11, 2)
        cr.stroke()
        cr.set_source_rgba(*accent_rgba())
        rounded(7, 2, 11, 11, 2)
        cr.fill()
        return False


class AppButton(Gtk.Button):
    def __init__(self, bar, key):
        super().__init__(relief=Gtk.ReliefStyle.NONE)
        self.bar, self.key = bar, key
        css_class(self, "app")
        self.overlay = Gtk.Overlay()
        self.box = Gtk.Box()
        self.overlay.add(self.box)
        self.add(self.overlay)
        self.badge = css_class(Gtk.Label(halign=Gtk.Align.END, valign=Gtk.Align.START, no_show_all=True), "badge")
        self.overlay.add_overlay(self.badge)
        self.overlay.set_overlay_pass_through(self.badge, True)
        self.connect("button-release-event", self.on_release)
        self.drag_source_set(Gdk.ModifierType.BUTTON1_MASK, APP_TARGET, Gdk.DragAction.MOVE)
        self.drag_dest_set(Gtk.DestDefaults.ALL, APP_TARGET, Gdk.DragAction.MOVE)
        self.connect("drag-begin", self.on_drag_begin)
        self.connect("drag-data-get", lambda _w, _c, data, _i, _t: data.set(data.get_target(), 8, key.encode()))
        self.connect("drag-data-received", lambda _w, _c, _x, _y, data, _i, _t:
                     bar.owner.reorder(data.get_data().decode(), key))
        self.icon_key = None

    def on_drag_begin(self, _w, ctx):
        info = self.bar.owner.apps.info(self.key)
        if info and info.get_icon():
            Gtk.drag_set_icon_gicon(ctx, info.get_icon(), 12, 12)

    def update(self, info, wins, active, urgent, badge, labelled, title):
        icon_key = (id(info), labelled, title if labelled else None)
        if icon_key != self.icon_key:
            self.icon_key = icon_key
            clear(self.box)
            gicon = info.get_icon() if info else (wins[0]["app_id"] if wins else "application-x-executable")
            self.box.pack_start(app_icon(gicon, 24, self.get_scale_factor()), False, False, 0)
            if labelled:
                self.box.pack_start(label(title, "app-label", ellipsize=True), False, False, 0)
                self.box.get_children()[-1].set_max_width_chars(16)
            self.box.show_all()
        ctx = self.get_style_context()
        for cls, on in (("running", bool(wins)), ("active", active), ("urgent", urgent), ("labelled", labelled)):
            (ctx.add_class if on else ctx.remove_class)(cls)
        name = info.get_display_name() if info else (wins[0]["app_id"] if wins else self.key)
        if wins:
            tip = "\n".join(w["title"] for w in wins[:8]) if len(wins) > 1 else (wins[0]["title"] or name)
            if tip != name and len(wins) == 1:
                tip = f"{name} - {tip}" if name.lower() not in tip.lower() else tip
        else:
            tip = name
        self.set_tooltip_text(tip)
        self.badge.set_text(str(badge) if badge < 100 else "99+")
        self.badge.set_visible(badge > 0)
        self.info, self.wins = info, wins

    def on_release(self, _w, event):
        owner = self.bar.owner
        owner.close_flyouts()
        if event.button == 1 and not event.state & Gdk.ModifierType.SHIFT_MASK:
            owner.click_app(self.key, self.wins, self, event)
        elif event.button == 2 or event.button == 1:
            owner.new_instance(self.key, self.wins)
        elif event.button == 3:
            owner.jump_list(self.key, self.info, self.wins, self, event)
        return True


class Bar:
    """One taskbar window on one monitor."""

    def __init__(self, owner, monitor, output, primary):
        self.owner, self.monitor, self.output, self.primary = owner, monitor, output, primary
        win = self.win = Gtk.Window()
        GtkLayerShell.init_for_window(win)
        GtkLayerShell.set_namespace(win, "ws-taskbar")
        GtkLayerShell.set_layer(win, GtkLayerShell.Layer.TOP)
        GtkLayerShell.set_monitor(win, monitor)
        for edge in (GtkLayerShell.Edge.LEFT, GtkLayerShell.Edge.RIGHT, GtkLayerShell.Edge.BOTTOM):
            GtkLayerShell.set_anchor(win, edge, True)
        visual = win.get_screen().get_rgba_visual()
        if visual:
            win.set_visual(visual)
        win.set_app_paintable(True)
        win.set_name("taskbar-window")
        self.root = Gtk.EventBox()
        self.root.set_name("taskbar")
        self.root.connect("button-release-event", self.on_bar_click)
        self.root.connect("enter-notify-event", lambda *_: owner.reveal(self))
        self.root.connect("leave-notify-event", lambda _w, e: e.detail != Gdk.NotifyType.INFERIOR and owner.unreveal(self))
        win.add(self.root)
        self.buttons = {}
        self.build()
        win.show_all()
        self.set_autohide(owner.conf["taskbar_autohide"], hidden=True)

    def set_autohide(self, on, hidden=False):
        if on:
            GtkLayerShell.set_exclusive_zone(self.win, 0)
            self.collapse(hidden)
        else:
            GtkLayerShell.auto_exclusive_zone_enable(self.win)
            self.collapse(False)

    def collapse(self, hidden):
        self.hidden = hidden
        self.content.set_visible(not hidden)
        self.win.set_size_request(-1, 2 if hidden else HEIGHT)
        self.root.set_size_request(-1, 2 if hidden else HEIGHT)
        self.win.resize(1, 2 if hidden else HEIGHT)

    def build(self):
        conf = self.owner.conf
        clear(self.root)
        self.content = Gtk.Box()
        self.content.set_size_request(-1, HEIGHT)
        self.root.add(self.content)
        self.buttons = {}
        group = Gtk.Box()
        self.start = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="Start"), "start")
        mark = brand_image(18)
        self.start.add(mark if mark is not None else Gtk.Label(label="\uf30a"))
        self.start.connect("button-release-event", self.on_start)
        group.pack_start(self.start, False, False, 0)
        style = conf["taskbar_search"]
        if style != "hide":
            search = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="Search"), "search", style)
            row = Gtk.Box(spacing=8)
            row.pack_start(symbolic("system-search-symbolic", 16), False, False, 0)
            if style in ("label", "box"):
                row.pack_start(label("Search", "search-text"), False, False, 0)
            search.add(row)
            search.connect("clicked", lambda *_: self.owner.show_start())
            group.pack_start(search, False, False, 0)
        self.apps_box = Gtk.Box()
        group.pack_start(self.apps_box, False, False, 0)
        if conf["taskbar_align"] == "center":
            self.content.set_center_widget(group)
        else:
            self.content.pack_start(group, False, False, 4)

        right = Gtk.Box()
        self.content.pack_end(right, False, False, 0)
        if conf["taskbar_corner"]:
            corner = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="Show desktop"), "corner")
            corner.connect("clicked", lambda *_: (self.owner.close_flyouts(), sway_exec("ws-show-desktop")))
            right.pack_end(corner, False, False, 0)
        self.clock = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "clock")
        clock_row = Gtk.Box(spacing=10)
        times = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
        self.time_label = label("", xalign=1.0)
        self.date_label = label("", xalign=1.0)
        times.pack_start(self.time_label, False, False, 0)
        times.pack_start(self.date_label, False, False, 0)
        clock_row.pack_start(times, False, False, 0)
        self.bell = Gtk.Stack(valign=Gtk.Align.CENTER)
        self.bell_icon = symbolic("preferences-system-notifications-symbolic", 16)
        self.bell_count = css_class(Gtk.Label(), "bell-count")
        self.bell.add_named(self.bell_icon, "icon")
        self.bell.add_named(self.bell_count, "count")
        clock_row.pack_start(self.bell, False, False, 0)
        self.clock.add(clock_row)
        self.clock.connect("button-release-event", self.on_clock)
        right.pack_end(self.clock, False, False, 0)

        self.qs = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "qs-group")
        qs_row = Gtk.Box(spacing=12)
        self.net_icon = symbolic("network-offline-symbolic", 16)
        self.vol_icon = symbolic("audio-volume-high-symbolic", 16)
        self.bat_icon = symbolic("battery-level-100-symbolic", 16)
        for name, image in (("network", self.net_icon), ("volume", self.vol_icon), ("battery", self.bat_icon)):
            ev = Gtk.EventBox(above_child=False)
            ev.add(image)
            ev.kind = name
            ev.add_events(Gdk.EventMask.SCROLL_MASK)
            if name == "volume":
                ev.connect("scroll-event", self.on_volume_scroll)
            qs_row.pack_start(ev, False, False, 0)
            setattr(self, name + "_box", ev)
        self.qs.add(qs_row)
        self.qs.connect("button-release-event", self.on_qs)
        right.pack_end(self.qs, False, False, 0)

        self.kbd = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, no_show_all=True), "kbd")
        kbd_col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
        self.kbd_lang, self.kbd_region = label("", xalign=0.5), label("", xalign=0.5)
        kbd_col.pack_start(self.kbd_lang, False, False, 0)
        kbd_col.pack_start(self.kbd_region, False, False, 0)
        self.kbd.add(kbd_col)
        kbd_col.show_all()
        self.kbd.connect("button-release-event", self.on_kbd)
        right.pack_end(self.kbd, False, False, 0)

        self.updates = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, no_show_all=True), "updates")
        up_overlay = Gtk.Overlay()
        up_overlay.add(symbolic("software-update-available-symbolic", 16))
        self.update_dot = css_class(Gtk.Label(label="\u25cf", halign=Gtk.Align.END, valign=Gtk.Align.START,
                                              no_show_all=True), "update-dot")
        up_overlay.add_overlay(self.update_dot)
        self.updates.add(up_overlay)
        up_overlay.show_all()
        self.updates.connect("clicked", lambda *_: (self.owner.close_flyouts(), sway_exec("ws-settings update")))
        right.pack_end(self.updates, False, False, 0)

        self.tray_box = Gtk.Box()
        right.pack_end(self.tray_box, False, False, 0)
        self.chevron = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="Show hidden icons",
                                            no_show_all=True), "chevron")
        self.chevron.add(symbolic("go-up-symbolic", 12))
        self.chevron.get_child().show()
        self.chevron.connect("clicked", lambda b: self.owner.toggle_overflow(b))
        right.pack_end(self.chevron, False, False, 0)
        self.content.show_all()

    # Events

    def on_bar_click(self, _w, event):
        if event.button == 3:
            self.owner.close_flyouts()
            popup_menu(event, [("Task Manager", lambda: sway_exec("xfce4-taskmanager")), None,
                               ("Taskbar settings", lambda: sway_exec("ws-settings taskbar"))])
            return True
        if event.button == 1:
            self.owner.close_flyouts()
        return False

    def on_start(self, _w, event):
        if event.button == 1:
            self.owner.toggle(self.owner.start)
        elif event.button == 3:
            self.owner.close_flyouts()
            self.owner.write_geometry(self)
            sway_exec("ws-winx")
        return True

    def on_clock(self, _w, event):
        if event.button == 1:
            self.owner.toggle(self.owner.centre)
        elif event.button == 3:
            self.owner.close_flyouts()
            popup_menu(event, [("Adjust date and time", lambda: sway_exec("ws-settings datetime")),
                               ("Notification settings", lambda: sway_exec("ws-settings notifications")), None,
                               ("Task Manager", lambda: sway_exec("xfce4-taskmanager")),
                               ("Taskbar settings", lambda: sway_exec("ws-settings taskbar"))])
        return True

    def on_qs(self, widget, event):
        if event.button == 1:
            self.owner.toggle(self.owner.quick)
        elif event.button == 3:
            self.owner.close_flyouts()
            x = event.x
            kind = "network"
            for box in (self.network_box, self.volume_box, self.battery_box):
                if box.get_visible():
                    bx, _by = box.translate_coordinates(widget, 0, 0)
                    if x >= bx - 6:
                        kind = box.kind
            items = {
                "network": [("Network and Internet settings", lambda: sway_exec("nm-connection-editor"))],
                "volume": [("Open volume mixer", lambda: sway_exec("ws-settings mixer")),
                           ("Sound settings", lambda: sway_exec("ws-settings sound"))],
                "battery": [("Power and battery settings", lambda: sway_exec("ws-settings power"))],
            }[kind]
            popup_menu(event, items)
        return True

    def on_volume_scroll(self, _w, event):
        status = self.owner.status
        delta = {Gdk.ScrollDirection.UP: 2, Gdk.ScrollDirection.DOWN: -2}.get(event.direction, 0)
        if event.direction == Gdk.ScrollDirection.SMOOTH:
            delta = -2 if event.delta_y > 0 else 2
        if delta:
            status.set_volume(max(0, min(100, status.volume + delta)))
            self.owner.update_status()
        return True

    def on_kbd(self, widget, event):
        self.owner.close_flyouts()
        layouts, active = self.owner.layouts
        items = [((("\u2022 " if i == active else "    ") + name),
                  lambda i=i: SWAY.command(f"input type:keyboard xkb_switch_layout {i}"))
                 for i, name in enumerate(layouts)]
        items += [None, ("More keyboard settings", lambda: sway_exec("ws-settings language"))]
        popup_menu(event, items, widget)
        return True


class Taskbar:
    def __init__(self):
        self.conf = settings()
        self.dark = False
        self.apps = Apps()
        self.desktops = Desktops()
        self.pins = load_pins()
        self.order = list(self.pins)
        self.last_focus = None
        self.wins = []
        self.layouts = ([], 0)
        self.bars = []
        self.css = Gtk.CssProvider()
        Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), self.css,
                                                 Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        self.load_css()
        self.status = Status()
        self.status.listeners.append(self.update_status)
        self.store = Store(self.update_notifications)
        self.quick = QuickSettings(self.status)
        self.centre = NotificationCentre(self.store)
        self.centre.on_open_app = self.focus_app
        self.taskview = TaskView(self.apps, self.desktops)
        self.toast = DesktopToast()
        self.overflow = Flyout("tray-overflow", Gtk.Align.START)
        self.start = StartMenu()
        self.tray = Tray(self.update_tray, self.close_flyouts)
        self.flyouts = [self.quick, self.centre, self.overflow, self.start]
        for f in self.flyouts:
            f.on_hide = self.after_flyout
        self.build_bars()
        screen = Gdk.Display.get_default()
        screen.connect("monitor-added", lambda *_: GLib.timeout_add(500, self.rebuild))
        screen.connect("monitor-removed", lambda *_: GLib.timeout_add(500, self.rebuild))
        self.refresh_windows()
        self.read_layouts()
        SWAY.subscribe(["window", "workspace", "input", "output"], self.on_sway)
        self.tick()
        GLib.timeout_add_seconds(1, self.tick)
        self.watch_updates()
        self.listen()
        on_signal(signal.SIGUSR1, self.reload)
        on_signal(signal.SIGTERM, self.quit)
        on_signal(signal.SIGINT, self.quit)

    def quit(self):
        Gtk.main_quit()
        return False

    def load_css(self):
        import common
        self.dark = common.is_dark()
        self.css.load_from_data(theme_css(CSS + (ACRYLIC_CSS if ws_motion.acrylic() else "")).encode())

    # Bars

    def monitors(self):
        display = Gdk.Display.get_default()
        outputs = SWAY.outputs()
        main = (self.conf.get("main_display") or "").strip()
        result = []
        for i in range(display.get_n_monitors()):
            mon = display.get_monitor(i)
            geo = mon.get_geometry()
            name = None
            for o in outputs:
                r = o.get("rect", {})
                if o.get("active") and r.get("x") == geo.x and r.get("y") == geo.y:
                    name = o["name"]
                    break
            result.append((mon, name, False))
        # Honour Settings › Display › Make this my main display.
        if main:
            result.sort(key=lambda row: 0 if row[1] == main else 1)
        if result:
            result = [(mon, name, i == 0) for i, (mon, name, _) in enumerate(result)]
        return result

    def build_bars(self):
        for bar in self.bars:
            bar.win.destroy()
        self.bars = []
        mons = self.monitors()
        if not self.conf["taskbar_all_displays"]:
            mons = mons[:1]
        for mon, name, primary in mons:
            self.bars.append(Bar(self, mon, name, primary))
        self.refresh_buttons()
        self.update_status()
        self.update_tray()
        self.update_notifications()
        self.update_updates()
        self.update_layout_label()
        self.tick()
        GLib.timeout_add(300, lambda: self.write_geometry(self.bars[0]) if self.bars else False)

    def rebuild(self):
        self.build_bars()
        return False

    def reload(self):
        self.conf = settings()
        self.load_css()
        self.start.reload_css()
        self.pins = load_pins()
        self.order = self.pins + [k for k in self.order if k not in self.pins]
        self.build_bars()
        self.refresh_windows()
        return True

    def write_geometry(self, bar):
        try:
            x, _y = bar.start.translate_coordinates(bar.win, 0, 0)
        except TypeError:
            return False
        geo = bar.monitor.get_geometry()
        save_json(GEOMETRY_FILE, {"start_x": x, "start_width": bar.start.get_allocated_width(),
                                  "align": self.conf["taskbar_align"], "width": geo.width,
                                  "height": HEIGHT})
        return False

    # Auto-hide

    def reveal(self, bar):
        if self.conf["taskbar_autohide"] and bar.hidden:
            bar.collapse(False)
        if getattr(bar, "hide_id", 0):
            GLib.source_remove(bar.hide_id)
            bar.hide_id = 0
        return False

    def unreveal(self, bar):
        if not self.conf["taskbar_autohide"]:
            return False

        def hide():
            bar.hide_id = 0
            if any(f.get_visible() for f in self.flyouts):
                return True
            bar.collapse(True)
            return False
        if not getattr(bar, "hide_id", 0):
            bar.hide_id = GLib.timeout_add(700, hide)
        return False

    # Flyouts

    def toggle(self, flyout):
        others = [f for f in self.flyouts if f is not flyout and f.get_visible()]
        for f in others:
            f.close_flyout()
        # Prefer the main display (Settings › Display) for Quick Settings / notifications / Start.
        primary = next((b for b in self.bars if b.primary), self.bars[0] if self.bars else None)
        if primary is not None:
            GtkLayerShell.set_monitor(flyout, primary.monitor)
        flyout.toggle()

    def show_start(self):
        """Open Start (Search). Closes other flyouts; does not toggle closed."""
        if self.start.slide.shown:
            return
        others = [f for f in self.flyouts if f is not self.start and f.get_visible()]
        for f in others:
            f.close_flyout()
        primary = next((b for b in self.bars if b.primary), self.bars[0] if self.bars else None)
        if primary is not None:
            GtkLayerShell.set_monitor(self.start, primary.monitor)
        self.start.open()

    def close_flyouts(self):
        for f in self.flyouts:
            f.close_flyout()

    def after_flyout(self):
        for bar in self.bars:
            if self.conf["taskbar_autohide"]:
                self.unreveal(bar)

    def toggle_overflow(self, button):
        if self.overflow.slide.shown:
            self.overflow.close_flyout()
            return
        self.close_flyouts()
        clear(self.overflow.panel)
        grid = Gtk.FlowBox(max_children_per_line=4, min_children_per_line=1, selection_mode=Gtk.SelectionMode.NONE)
        hidden = set(self.conf["tray_hidden"])
        for item in self.tray.visible_items():
            if item.ident in hidden:
                grid.add(item.button(button.get_scale_factor()))
        self.overflow.panel.pack_start(grid, False, False, 0)
        self.overflow.panel.set_name("tray-overflow")
        x, _y = button.translate_coordinates(button.get_toplevel(), 0, 0)
        n = min(4, max(1, len(hidden)))
        self.overflow.holder.set_margin_start(max(12, x + button.get_allocated_width() // 2 - (n * 40 + 16) // 2))
        self.overflow.open()

    # Windows and app buttons

    def on_sway(self, kind, event):
        if kind == 3 and event.get("change") == "focus":  # window
            self.last_focus = event["container"]["id"]
            # Close Start when a real app window takes focus — not when the
            # exclusive keyboard grab merely clears the previous focus.
            if self.start.slide.shown and event.get("container", {}).get("id"):
                self.start.close_flyout()
        if kind == 3 and event.get("change") == "title":
            for w in self.wins:
                if w["id"] == event["container"]["id"]:
                    w["title"] = event["container"].get("name") or ""
            self.schedule_buttons()
            return
        if kind == 21:  # input
            self.read_layouts()
            return
        if kind == 0 and event.get("change") == "focus":  # workspace
            cur = (event.get("current") or {}).get("name")
            old = (event.get("old") or {}).get("name")
            if cur and old and cur != old and cur not in Desktops.HIDDEN and old not in Desktops.HIDDEN \
                    and len(self.desktops.list()) > 1 and not self.taskview.win.get_visible():
                self.toast.show(self.desktops.name(cur))
        if kind == 1:  # output
            GLib.timeout_add(500, self.rebuild)
            return
        self.schedule_windows()

    def schedule_windows(self):
        if not getattr(self, "win_pending", False):
            self.win_pending = True
            GLib.timeout_add(40, self.refresh_windows)

    def schedule_buttons(self):
        if not getattr(self, "button_pending", False):
            self.button_pending = True
            GLib.timeout_add(100, self.refresh_buttons_cb)

    def refresh_buttons_cb(self):
        self.button_pending = False
        self.refresh_buttons()
        return False

    def refresh_windows(self):
        self.win_pending = False
        self.wins = windows()
        for w in self.wins:
            if w["focused"]:
                self.last_focus = w["id"]
            w["key"] = self.apps.match(w["app_id"], w["wm_class"]) or ("window:" + w["app_id"])
        for w in self.wins:
            if w["key"] not in self.order:
                self.order.append(w["key"])
        self.refresh_buttons()
        self.taskview.refresh()
        return False

    def visible_windows(self, bar):
        if self.conf["taskbar_all_desktops"]:
            return [w for w in self.wins if not bar.output or w["output"] in (bar.output, None) or w["hidden"]]
        shown = {ws["name"] for ws in SWAY.workspaces() if ws.get("visible") and
                 (not bar.output or ws.get("output") == bar.output)}
        return [w for w in self.wins if w["workspace"] in shown or w["hidden"]]

    def groups(self, bar):
        groups = {}
        for w in self.visible_windows(bar):
            groups.setdefault(w["key"], []).append(w)
        keys = [k for k in self.order if k in self.pins or k in groups]
        return keys, groups

    def refresh_buttons(self):
        combine = self.conf["taskbar_combine"]
        for bar in self.bars:
            keys, groups = self.groups(bar)
            wanted = []
            n_windows = sum(len(v) for v in groups.values())
            labelled = combine == "never" or (combine == "full" and n_windows <= 6)
            for key in keys:
                wins = groups.get(key, [])
                if labelled and len(wins) > 1:
                    wanted += [(f"{key}#{w['id']}", key, [w]) for w in wins]
                else:
                    wanted.append((key, key, wins))
            existing = bar.buttons
            for bkey in list(existing):
                if bkey not in [w[0] for w in wanted]:
                    existing.pop(bkey).destroy()
            for pos, (bkey, key, wins) in enumerate(wanted):
                button = existing.get(bkey)
                if not button:
                    button = existing[bkey] = AppButton(bar, key)
                    bar.apps_box.pack_start(button, False, False, 0)
                    button.show_all()
                bar.apps_box.reorder_child(button, pos)
                info = self.apps.info(key) if not key.startswith("window:") else None
                active = any(w["id"] == self.last_focus and not w["hidden"] for w in wins)
                urgent = self.conf["taskbar_flash"] and any(w["urgent"] for w in wins)
                badge = self.store.count_for(key, info.get_display_name() if info else "") \
                    if self.conf["taskbar_badges"] else 0
                title = wins[0]["title"] if wins else (info.get_display_name() if info else key)
                button.update(info, wins, active, urgent, badge, labelled and bool(wins), title)
        return False

    def click_app(self, key, wins, button, event):
        if not wins:
            launch(key) if not key.startswith("window:") else None
            return
        if len(wins) == 1:
            w = wins[0]
            visible_ws = {ws["name"] for ws in SWAY.workspaces() if ws.get("visible")}
            if w["id"] == self.last_focus and not w["hidden"] and w["workspace"] in visible_ws:
                minimize(w)
            else:
                activate(w)
            return
        items = []
        for w in wins:
            prefix = "\u2022 " if w["id"] == self.last_focus else ""
            items.append((prefix + (w["title"] or key)[:60], lambda w=w: activate(w)))
        popup_menu(event, items, button)

    def new_instance(self, key, wins):
        if not key.startswith("window:"):
            launch(key)

    def focus_app(self, key):
        for w in self.wins:
            if w["key"] == key:
                activate(w)
                return True
        return False

    def jump_list(self, key, info, wins, button, event):
        items = []
        if info:
            items.append((info.get_display_name(), lambda: launch(key)))
            for action in info.list_actions()[:4]:
                items.append(("    " + info.get_action_name(action),
                              lambda a=action: info.launch_action(a, None)))
            items.append(None)
            if key in self.pins:
                items.append(("Unpin from taskbar", lambda: self.unpin(key)))
            elif not advanced_blocked(key):
                items.append(("Pin to taskbar", lambda: self.pin(key)))
        if wins:
            if len(wins) == 1:
                items.append(("Close window", lambda: close_window(wins[0])))
            else:
                items.append(("Close all windows", lambda: [close_window(w) for w in wins]))
        if items:
            popup_menu(event, items, button)

    def pin(self, key):
        if advanced_blocked(key):
            return
        if key not in self.pins and not key.startswith("window:"):
            self.pins.append(key)
            save_pins(self.pins)
            self.refresh_buttons()

    def unpin(self, key):
        if key in self.pins:
            self.pins.remove(key)
            save_pins(self.pins)
            self.refresh_buttons()

    def reorder(self, moved, target):
        if moved == target or moved not in self.order:
            return
        self.order.remove(moved)
        self.order.insert(self.order.index(target), moved)
        if moved in self.pins:
            self.pins = [k for k in self.order if k in self.pins]
            save_pins(self.pins)
        self.refresh_buttons()

    def nth_app(self, n):
        bar = self.bars[0] if self.bars else None
        if not bar:
            return None
        keys, groups = self.groups(bar)
        if 0 < n <= len(keys):
            return keys[n - 1], groups.get(keys[n - 1], [])
        return None

    # Status area

    def update_status(self):
        icon, tip = self.status.network()
        present, _pct, bat_icon, bat_tip = self.status.battery_state()
        vol_tip = f"Speakers: {'Muted' if self.status.muted else str(self.status.volume) + '%'}"
        for bar in self.bars:
            bar.net_icon.set_icon(icon)
            bar.network_box.set_tooltip_text(tip)
            bar.vol_icon.set_icon(self.status.volume_icon())
            bar.volume_box.set_tooltip_text(vol_tip)
            bar.battery_box.set_visible(present)
            if present:
                bar.bat_icon.set_icon(bat_icon)
                bar.battery_box.set_tooltip_text(bat_tip)

    def update_tray(self):
        hidden = set(self.conf["tray_hidden"]) if self.conf["tray_overflow"] else set()
        items = self.tray.visible_items()
        for bar in self.bars:
            clear(bar.tray_box)
            for item in items:
                if item.ident not in hidden:
                    b = item.button(bar.win.get_scale_factor())
                    bar.tray_box.pack_start(b, False, False, 0)
            bar.tray_box.show_all()
            bar.chevron.set_visible(any(i.ident in hidden for i in items))
        if self.overflow.get_visible():
            self.overflow.close_flyout()

    def update_notifications(self):
        unread = self.store.unread
        dnd = False
        try:
            from notify import dnd_on
            dnd = dnd_on()
        except Exception:
            pass
        for bar in self.bars:
            if unread and self.conf["taskbar_badges"] and not dnd:
                bar.bell_count.set_text(str(unread) if unread < 100 else "99+")
                bar.bell.set_visible_child_name("count")
            else:
                bar.bell_icon.set_icon("notifications-disabled-symbolic" if dnd
                                       else "preferences-system-notifications-symbolic")
                bar.bell.set_visible_child_name("icon")
            bar.clock.set_tooltip_text(time.strftime("%A, %B %-d, %Y") +
                                       (f"\n{unread} new notification{'s' if unread != 1 else ''}" if unread else ""))
        self.centre.refresh()
        self.schedule_buttons()

    def tick(self):
        conf = self.conf
        fmt = "%H:%M" if conf.get("clock_24h") else "%-I:%M"
        if conf.get("clock_seconds"):
            fmt += ":%S"
        if not conf.get("clock_24h"):
            fmt += " %p"
        now = time.strftime(fmt)
        date = time.strftime("%-m/%-d/%Y")
        for bar in self.bars:
            if bar.time_label.get_text() != now:
                bar.time_label.set_text(now)
                bar.date_label.set_text(date)
        return True

    def watch_updates(self):
        self.update_monitor = Gio.File.new_for_path(UPDATES_FILE).monitor_file(Gio.FileMonitorFlags.NONE, None)
        self.update_monitor.connect("changed", lambda *_: self.update_updates())
        GLib.timeout_add_seconds(60, lambda: self.update_updates() or True)

    def update_updates(self):
        state = load_json(UPDATES_FILE, {})
        count = len(state.get("packages", [])) + len(state.get("flatpaks", []))
        restart = os.path.lexists("/system-update")
        busy = bool(state.get("busy"))
        busy_status = state.get("busy_status") or "Installing updates\u2026"
        for bar in self.bars:
            bar.updates.set_visible(bool(count or restart or busy))
            bar.update_dot.set_visible(restart or busy)
            if busy:
                tip = busy_status
            elif restart:
                tip = ("Restart required \u2014 Update and restart or Update and shut down "
                       "from Start \u203a Power")
            else:
                tip = f"{count} update{'s' if count != 1 else ''} available"
            bar.updates.set_tooltip_text(tip)
        return False

    def read_layouts(self):
        for dev in SWAY.request(100) or []:
            if dev.get("type") == "keyboard" and dev.get("xkb_layout_names"):
                self.layouts = (dev["xkb_layout_names"], dev.get("xkb_active_layout_index") or 0)
                break
        self.update_layout_label()

    def update_layout_label(self):
        names, active = self.layouts
        show = len(names) > 1
        name = names[active] if show and active < len(names) else ""
        lang = (name.split(" ")[0][:3] or "").upper()
        region = name[name.find("(") + 1:name.find(")")][:3].upper() if "(" in name else ""
        for bar in self.bars:
            bar.kbd.set_visible(show)
            bar.kbd_lang.set_text(lang)
            bar.kbd_region.set_text(region)
            bar.kbd.set_tooltip_text(name)

    # Commands

    def listen(self):
        try:
            os.unlink(SOCK)
        except OSError:
            pass
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
        self.sock.bind(SOCK)
        GLib.io_add_watch(self.sock.fileno(), GLib.PRIORITY_DEFAULT, GLib.IO_IN, self.on_command)

    def on_command(self, *_args):
        try:
            cmd = self.sock.recv(1024).decode().strip()
        except OSError:
            return True
        word, _sp, arg = cmd.partition(" ")
        if word == "quick":
            self.toggle(self.quick)
        elif word == "notifications":
            self.toggle(self.centre)
        elif word in ("start", "start-toggle"):
            self.toggle(self.start)
        elif word == "start-show":
            self.show_start()
        elif word == "start-hide":
            self.start.close_flyout()
        elif word == "start-settings":
            self.start.open_settings()
        elif word == "taskview":
            return True
        elif word == "reload":
            self.reload()
        elif word == "clear-notifications":
            self.store.clear()
        elif word in ("activate", "launch") and arg.isdigit():
            found = self.nth_app(int(arg))
            if found:
                key, wins = found
                if word == "launch" or not wins:
                    self.new_instance(key, wins)
                elif len(wins) == 1:
                    self.click_app(key, wins, None, None)
                else:
                    ids = [w["id"] for w in wins]
                    nxt = (ids.index(self.last_focus) + 1) % len(ids) if self.last_focus in ids else 0
                    activate(wins[nxt])
        elif word == "desktop":
            return True
        elif word == "pin":
            self.pins = load_pins()
            self.pin(arg)
        elif word == "unpin":
            self.pins = load_pins()
            self.unpin(arg)
        return True


def main():
    GLib.set_prgname("ws-taskbar")
    Taskbar()
    try:
        Gtk.main()
    finally:
        try:
            os.unlink(SOCK)
        except OSError:
            pass


if __name__ == "__main__":
    main()
