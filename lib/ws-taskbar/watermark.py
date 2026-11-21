#!/usr/bin/env python3
"""Desktop watermark, in the style of a Windows beta build.

Bottom right of every screen, just above the taskbar. It is drawn above the
wallpaper and under every window. Clicks pass through to the desktop.
"""
import fcntl
import os
import sys

import gi

gi.require_version("Gdk", "3.0")
gi.require_version("Gtk", "3.0")
gi.require_version("GtkLayerShell", "0.1")
gi.require_version("Pango", "1.0")
gi.require_version("PangoCairo", "1.0")
from gi.repository import Gdk, GLib, Gtk, GtkLayerShell, Pango, PangoCairo  # noqa: E402

import cairo  # noqa: E402

FONT = "@FONT@"
if FONT.startswith("@"):
    FONT = "Selawik"
# The taskbar already reserves its own strip. This is the gap between the
# letters and the top of that strip, plus the inset from the right edge.
MARGIN_BOTTOM = 8
MARGIN_RIGHT = 20
PAD = 3
LINES = ("Mackes Fedora Linux 2026", "NOT FOR RELEASE")
LOCK = os.path.join(os.environ.get("XDG_RUNTIME_DIR", "/tmp"), "ws-watermark.lock")


def hold_lock():
    handle = open(LOCK, "w")
    try:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        sys.exit(0)
    return handle


LOCK_FILE = hold_lock()


def make_layout(widget):
    layout = widget.create_pango_layout("\n".join(LINES))
    layout.set_font_description(Pango.FontDescription.from_string(f"{FONT} 9"))
    layout.set_alignment(Pango.Alignment.RIGHT)
    widest = 0
    for line in LINES:
        one = widget.create_pango_layout(line)
        one.set_font_description(layout.get_font_description())
        widest = max(widest, one.get_pixel_size()[0])
    layout.set_width(widest * Pango.SCALE)
    return layout


class Mark(Gtk.DrawingArea):
    def __init__(self):
        super().__init__()
        self.connect("draw", self.on_draw)
        self.connect("realize", lambda *_: self.remeasure())
        self.connect("style-updated", lambda *_: self.remeasure())

    def remeasure(self):
        width, height = make_layout(self).get_pixel_size()
        self.set_size_request(width + PAD * 2, height + PAD * 2)

    def on_draw(self, _widget, cr):
        layout = make_layout(self)
        # Soft dark shadow, then the white letters. A few offset copies read
        # as the blur Windows uses, so the text stays clear on any wallpaper.
        cr.set_source_rgba(0, 0, 0, 0.45)
        for dx, dy in ((0, 1), (1, 1), (-1, 1), (0, 2), (1, 2), (2, 2)):
            cr.move_to(PAD + dx, PAD + dy)
            PangoCairo.show_layout(cr, layout)
        cr.set_source_rgb(1, 1, 1)
        cr.move_to(PAD, PAD)
        PangoCairo.show_layout(cr, layout)
        return True


def pass_through(win):
    win.get_window().input_shape_combine_region(cairo.Region(), 0, 0)


def on_monitor(monitor):
    win = Gtk.Window()
    GtkLayerShell.init_for_window(win)
    GtkLayerShell.set_namespace(win, "ws-watermark")
    GtkLayerShell.set_layer(win, GtkLayerShell.Layer.BOTTOM)
    GtkLayerShell.set_monitor(win, monitor)
    GtkLayerShell.set_keyboard_mode(win, GtkLayerShell.KeyboardMode.NONE)
    GtkLayerShell.set_exclusive_zone(win, 0)
    GtkLayerShell.set_anchor(win, GtkLayerShell.Edge.BOTTOM, True)
    GtkLayerShell.set_anchor(win, GtkLayerShell.Edge.RIGHT, True)
    GtkLayerShell.set_margin(win, GtkLayerShell.Edge.BOTTOM, MARGIN_BOTTOM)
    GtkLayerShell.set_margin(win, GtkLayerShell.Edge.RIGHT, MARGIN_RIGHT)
    visual = win.get_screen().get_rgba_visual()
    if visual:
        win.set_visual(visual)
    win.set_app_paintable(True)
    win.set_decorated(False)
    win.set_accept_focus(False)
    screen = win.get_screen()
    css = Gtk.CssProvider()
    css.load_from_data(b"window, drawingarea { background-color: transparent; }")
    Gtk.StyleContext.add_provider_for_screen(screen, css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
    win.add(Mark())
    win.connect("realize", pass_through)
    win.connect("draw", clear_window)
    win.show_all()
    return win


def clear_window(_win, cr):
    cr.set_operator(cairo.OPERATOR_SOURCE)
    cr.set_source_rgba(0, 0, 0, 0)
    cr.paint()
    cr.set_operator(cairo.OPERATOR_OVER)
    return False


def main():
    GLib.set_prgname("ws-watermark")
    windows = {}
    display = Gdk.Display.get_default()
    if display is None:
        sys.exit("ws-watermark: no display")

    def add(_display, monitor):
        if monitor not in windows:
            windows[monitor] = on_monitor(monitor)

    def remove(_display, monitor):
        win = windows.pop(monitor, None)
        if win:
            win.destroy()

    for i in range(display.get_n_monitors()):
        add(display, display.get_monitor(i))
    display.connect("monitor-added", add)
    display.connect("monitor-removed", remove)
    if not windows:
        sys.exit("ws-watermark: no monitors")
    Gtk.main()


if __name__ == "__main__":
    main()
