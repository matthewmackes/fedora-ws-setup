#!/usr/bin/env python3
"""Flash large numbers on each active display (Settings › Display › Identify)."""
import os
import sys

import gi

gi.require_version("Gdk", "3.0")
gi.require_version("Gtk", "3.0")
gi.require_version("GtkLayerShell", "0.1")
from gi.repository import Gdk, GLib, Gtk, GtkLayerShell  # noqa: E402

SHARE = os.path.dirname(os.path.abspath(__file__))
for path in (SHARE, "/usr/share/mde", os.path.join(SHARE, "ws-settings"),
             "/usr/share/mde/ws-settings"):
    if path not in sys.path and os.path.isdir(path):
        sys.path.insert(0, path)

try:
    import labwc_cfg
except ImportError:
    labwc_cfg = None


def outputs():
    if labwc_cfg:
        try:
            return [o for o in labwc_cfg.outputs() if o.get("active")]
        except Exception:
            pass
    return []


def flash(number, monitor):
    win = Gtk.Window(type=Gtk.WindowType.TOPLEVEL)
    win.set_decorated(False)
    win.set_accept_focus(False)
    label = Gtk.Label()
    label.set_markup(f'<span size="102400" weight="bold" foreground="white">{number}</span>')
    box = Gtk.Box(halign=Gtk.Align.CENTER, valign=Gtk.Align.CENTER)
    ui = Gtk.EventBox()
    ui.override_background_color(Gtk.StateFlags.NORMAL, Gdk.RGBA(0, 0, 0, 0.55))
    ui.add(box)
    box.pack_start(label, True, True, 0)
    win.add(ui)
    GtkLayerShell.init_for_window(win)
    GtkLayerShell.set_namespace(win, "ws-display-identify")
    GtkLayerShell.set_layer(win, GtkLayerShell.Layer.OVERLAY)
    GtkLayerShell.set_monitor(win, monitor)
    for edge in (GtkLayerShell.Edge.TOP, GtkLayerShell.Edge.BOTTOM,
                 GtkLayerShell.Edge.LEFT, GtkLayerShell.Edge.RIGHT):
        GtkLayerShell.set_anchor(win, edge, True)
    win.show_all()
    GLib.timeout_add(2000, win.destroy)
    return win


def main():
    outs = outputs()
    display = Gdk.Display.get_default()
    if not display:
        return 1
    wins = []
    for i in range(display.get_n_monitors()):
        mon = display.get_monitor(i)
        geo = mon.get_geometry()
        number = i + 1
        for n, o in enumerate(outs, 1):
            r = o.get("rect") or {}
            if r.get("x") == geo.x and r.get("y") == geo.y:
                number = n
                break
        wins.append(flash(number, mon))
    if not wins:
        return 1
    GLib.timeout_add(2200, Gtk.main_quit)
    Gtk.main()
    return 0


if __name__ == "__main__":
    sys.exit(main())
