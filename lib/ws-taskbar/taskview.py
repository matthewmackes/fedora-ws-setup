"""Task View (Super+Tab): window cards for a desktop above a strip of desktops, and the
desktop name shown when switching desktops."""

from common import (Gdk, GLib, Gtk, GtkLayerShell, activate, app_icon, clear, close_window,
                    css_class, label, popup_menu, symbolic, windows, ws_motion)

WINDOW_TARGET = [Gtk.TargetEntry.new("ws-window", Gtk.TargetFlags.SAME_APP, 0)]


def layer_window(name, layer, keyboard):
    win = Gtk.Window()
    GtkLayerShell.init_for_window(win)
    GtkLayerShell.set_namespace(win, name)
    GtkLayerShell.set_layer(win, layer)
    GtkLayerShell.set_keyboard_mode(win, keyboard)
    visual = win.get_screen().get_rgba_visual()
    if visual:
        win.set_visual(visual)
    win.set_app_paintable(True)
    return win


class TaskView:
    def __init__(self, apps, desktops):
        self.apps, self.desktops = apps, desktops
        self.win = layer_window("ws-taskview", GtkLayerShell.Layer.OVERLAY, GtkLayerShell.KeyboardMode.EXCLUSIVE)
        for edge in (GtkLayerShell.Edge.LEFT, GtkLayerShell.Edge.RIGHT,
                     GtkLayerShell.Edge.TOP, GtkLayerShell.Edge.BOTTOM):
            GtkLayerShell.set_anchor(self.win, edge, True)
        self.win.set_name("taskview")
        self.win.connect("key-press-event", self.on_key)
        self.win.connect("button-press-event", self.on_background_click)
        # The backdrop and cards fade with CSS transitions (.tv-backdrop.shown): widget
        # opacity on a full-screen box renders through an offscreen buffer and drops frames.
        root = self.root = css_class(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "tv-backdrop")
        self.win.add(root)
        self.shown = False
        self.hide_id = 0
        self.grid_area = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, vexpand=True, valign=Gtk.Align.CENTER)
        root.pack_start(self.grid_area, True, True, 0)
        self.strip = css_class(Gtk.Box(spacing=12, halign=Gtk.Align.CENTER), "tv-strip")
        root.pack_end(self.strip, False, False, 0)
        self.shown_ws = None
        self.revert_id = 0

    # Opening and closing

    def toggle(self):
        if self.shown:
            self.close()
        else:
            self.open()

    def open(self):
        if self.hide_id:
            GLib.source_remove(self.hide_id)
            self.hide_id = 0
        self.shown = True
        self.shown_ws = self.desktops.current()
        self.build()
        self.win.show_all()
        self.win.present()

        def fade_in():
            if self.shown:
                self.root.get_style_context().add_class("shown")
            return False
        GLib.idle_add(fade_in)

    def close(self):
        if not self.shown:
            return
        self.shown = False
        self.root.get_style_context().remove_class("shown")

        def hide():
            self.hide_id = 0
            self.win.hide()
            return False
        if self.hide_id:
            GLib.source_remove(self.hide_id)
        self.hide_id = GLib.timeout_add(ws_motion.FAST if ws_motion.enabled() else 0, hide)

    def refresh(self):
        if self.win.get_visible():
            self.build()

    def on_key(self, _w, event):
        if event.keyval in (Gdk.KEY_Escape,) or (event.keyval == Gdk.KEY_Tab and event.state & Gdk.ModifierType.SUPER_MASK):
            self.close()
            return True
        return False

    def on_background_click(self, widget, event):
        if event.window == widget.get_window() and event.button == 1:
            self.close()
        return False

    # Content

    def build(self):
        self.all = windows()
        self.build_grid(self.shown_ws)
        self.build_strip()

    def windows_on(self, ws):
        cur = self.desktops.current()
        return [w for w in self.all if w["workspace"] == ws or (w["hidden"] and ws == cur)]

    def build_grid(self, ws):
        clear(self.grid_area)
        wins = self.windows_on(ws)
        if not wins:
            self.grid_area.pack_start(label("No windows open on " + self.desktops.name(ws), "tv-empty", xalign=0.5),
                                      False, False, 0)
            self.grid_area.show_all()
            return
        flow = Gtk.FlowBox(homogeneous=True, selection_mode=Gtk.SelectionMode.NONE, halign=Gtk.Align.CENTER,
                           max_children_per_line=min(4, len(wins)), min_children_per_line=min(4, len(wins)),
                           column_spacing=24, row_spacing=24)
        flow.connect("child-activated", lambda _f, child: self.focus(child.win))
        for win in wins:
            child = Gtk.FlowBoxChild()
            child.win = win
            child.add(self.card(win))
            flow.add(child)
        self.grid_area.pack_start(flow, False, False, 0)
        self.grid_area.show_all()
        first = flow.get_child_at_index(0)
        if first:
            first.grab_focus()

    def card(self, win):
        desktop_id = self.apps.match(win["app_id"], win["wm_class"])
        info = self.apps.info(desktop_id)
        gicon = info.get_icon() if info else win["app_id"]
        scale = self.win.get_scale_factor()
        box = css_class(Gtk.EventBox(), "tv-card")
        inner = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        head = css_class(Gtk.Box(spacing=8), "tv-card-head")
        head.pack_start(app_icon(gicon, 16, scale), False, False, 0)
        head.pack_start(label(win["title"] or (info.get_display_name() if info else win["app_id"]),
                              "tv-title", ellipsize=True), True, True, 0)
        x = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="Close"), "tv-close")
        x.add(symbolic("window-close-symbolic", 12))
        x.connect("clicked", lambda *_: (close_window(win), GLib.timeout_add(250, self.rebuild_later)))
        head.pack_end(x, False, False, 0)
        inner.pack_start(head, False, False, 0)
        body = css_class(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "tv-card-body")
        body.set_size_request(260, 150)
        body.pack_start(app_icon(gicon, 64, scale), True, False, 0)
        if win["hidden"]:
            body.pack_start(label("Minimised", "caption", xalign=0.5), False, False, 8)
        inner.pack_start(body, True, True, 0)
        box.add(inner)
        box.connect("button-release-event", lambda _w, e: self.on_card_click(win, e))
        box.drag_source_set(Gdk.ModifierType.BUTTON1_MASK, WINDOW_TARGET, Gdk.DragAction.MOVE)
        box.connect("drag-data-get", lambda _w, _c, data, _i, _t: data.set(data.get_target(), 8,
                                                                           str(win["id"]).encode()))
        box.connect("drag-begin", lambda w, ctx: Gtk.drag_set_icon_name(ctx, "window", 0, 0))
        return box

    def on_card_click(self, win, event):
        if event.button == 1:
            self.focus(win)
        elif event.button == 2:
            close_window(win)
            GLib.timeout_add(250, self.rebuild_later)
        elif event.button == 3:
            items = [("Move to " + self.desktops.name(ws), lambda w=ws: self.move(win, w))
                     for ws in self.desktops.list() if ws != win["workspace"]]
            items += [("Move to new desktop", lambda: self.move(win, self.desktops.new(switch=False))), None,
                      ("Close", lambda: (close_window(win), GLib.timeout_add(250, self.rebuild_later)))]
            popup_menu(event, items)
        return True

    def rebuild_later(self):
        self.refresh()
        return False

    def focus(self, win):
        self.close()
        activate(win)

    def move(self, win, ws):
        self.desktops.move_window(win["id"], ws)
        GLib.timeout_add(100, self.rebuild_later)

    # Desktop strip

    def build_strip(self):
        clear(self.strip)
        cur = self.desktops.current()
        scale = self.win.get_scale_factor()
        for ws in self.desktops.list():
            card = css_class(Gtk.EventBox(tooltip_text=self.desktops.name(ws)), "tv-desktop")
            if ws == cur:
                css_class(card, "current")
            col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
            head = Gtk.Box(spacing=4)
            head.pack_start(label(self.desktops.name(ws), "tv-desktop-name", ellipsize=True), True, True, 0)
            if len(self.desktops.list()) > 1:
                x = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="Close"), "tv-close")
                x.add(symbolic("window-close-symbolic", 10))
                x.connect("clicked", lambda _b, w=ws: (self.desktops.close(w), GLib.timeout_add(200, self.rebuild_later)))
                head.pack_end(x, False, False, 0)
            col.pack_start(head, False, False, 0)
            preview = css_class(Gtk.Box(spacing=4, halign=Gtk.Align.CENTER, valign=Gtk.Align.CENTER), "tv-preview")
            preview.set_size_request(176, 96)
            for win in [w for w in self.all if w["workspace"] == ws][:6]:
                info = self.apps.info(self.apps.match(win["app_id"], win["wm_class"]))
                preview.pack_start(app_icon(info.get_icon() if info else win["app_id"], 24, scale), False, False, 0)
            col.pack_start(preview, True, True, 0)
            card.add(col)
            card.connect("enter-notify-event", lambda *_a, w=ws: self.hover(w))
            card.connect("leave-notify-event", lambda *_a: self.unhover())
            card.connect("button-release-event", lambda _c, e, w=ws: self.on_desktop_click(w, e))
            card.drag_dest_set(Gtk.DestDefaults.ALL, WINDOW_TARGET, Gdk.DragAction.MOVE)
            card.connect("drag-data-received", lambda _c, _ctx, _x, _y, data, _i, _t, w=ws: self.on_drop(data, w))
            self.strip.pack_start(card, False, False, 0)
        new = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "tv-new")
        col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6, valign=Gtk.Align.CENTER)
        col.pack_start(symbolic("list-add-symbolic", 24), False, False, 0)
        col.pack_start(label("New desktop", xalign=0.5), False, False, 0)
        new.add(col)
        new.connect("clicked", lambda *_: (self.desktops.new(switch=False), self.build_strip()))
        new.drag_dest_set(Gtk.DestDefaults.ALL, WINDOW_TARGET, Gdk.DragAction.MOVE)
        new.connect("drag-data-received", lambda _c, _ctx, _x, _y, data, _i, _t:
                    self.on_drop(data, self.desktops.new(switch=False)))
        self.strip.pack_start(new, False, False, 0)
        self.strip.show_all()

    def hover(self, ws):
        if self.revert_id:
            GLib.source_remove(self.revert_id)
            self.revert_id = 0
        if ws != self.shown_ws:
            self.shown_ws = ws
            self.build_grid(ws)

    def unhover(self):
        def revert():
            self.revert_id = 0
            cur = self.desktops.current()
            if self.shown_ws != cur and self.win.get_visible():
                self.shown_ws = cur
                self.build_grid(cur)
            return False
        if not self.revert_id:
            self.revert_id = GLib.timeout_add(600, revert)

    def on_desktop_click(self, ws, event):
        if event.button == 1 and event.type == Gdk.EventType.BUTTON_RELEASE:
            self.close()
            self.desktops.switch(ws)
        elif event.button == 3:
            popup_menu(event, [("Rename", lambda: self.rename(ws)),
                               ("Close", (lambda: (self.desktops.close(ws), GLib.timeout_add(200, self.rebuild_later)))
                                if len(self.desktops.list()) > 1 else None)])
        return True

    def rename(self, ws):
        dialog = Gtk.Window(title="Rename desktop", modal=True)
        GtkLayerShell.init_for_window(dialog)
        GtkLayerShell.set_layer(dialog, GtkLayerShell.Layer.OVERLAY)
        GtkLayerShell.set_keyboard_mode(dialog, GtkLayerShell.KeyboardMode.EXCLUSIVE)
        box = css_class(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12), "tv-rename")
        box.pack_start(label("Rename desktop", "tv-rename-title"), False, False, 0)
        entry = Gtk.Entry(text=self.desktops.state.get("names", {}).get(ws, ""), width_chars=28,
                          placeholder_text=self.desktops.name(ws))
        box.pack_start(entry, False, False, 0)

        def done(*_):
            self.desktops.rename(ws, entry.get_text())
            dialog.destroy()
            self.win.present()
            self.build_strip()
        entry.connect("activate", done)
        dialog.connect("key-press-event", lambda _d, e: e.keyval == Gdk.KEY_Escape and dialog.destroy())
        dialog.add(box)
        dialog.show_all()

    def on_drop(self, data, ws):
        try:
            cid = int(data.get_data().decode())
        except (ValueError, AttributeError):
            return
        self.desktops.move_window(cid, ws)
        GLib.timeout_add(100, self.rebuild_later)


class DesktopToast:
    """The desktop name, briefly, in the middle of the screen after switching desktops."""

    def __init__(self):
        self.win = layer_window("ws-desktop-toast", GtkLayerShell.Layer.OVERLAY, GtkLayerShell.KeyboardMode.NONE)
        self.label = label("", "toast-text", xalign=0.5)
        box = css_class(Gtk.Box(), "toast")
        box.add(self.label)
        self.win.add(box)
        self.win.set_name("desktop-toast")
        self.fade = ws_motion.Fade(box)
        self.hide_id = 0
        self.win.connect("realize", lambda w: w.get_window().input_shape_combine_region(
            __import__("cairo").Region(), 0, 0))

    def show(self, text):
        self.label.set_text(text)
        self.win.show_all()
        self.fade.enter()
        if self.hide_id:
            GLib.source_remove(self.hide_id)
        self.hide_id = GLib.timeout_add(900, self.hide)

    def hide(self):
        self.hide_id = 0
        self.fade.leave(self.win.hide)
        return False
