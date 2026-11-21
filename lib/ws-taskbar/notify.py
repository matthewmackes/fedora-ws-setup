"""Notification centre: a record of every notification (swaync still shows the toasts) and
the Windows 11 flyout with notifications above a calendar and Focus controls."""

import os
import time

from common import (FOCUS_FILE, NOTES_FILE, Flyout, Gio, GLib, Gtk, app_icon,
                    clear, css_class, label, launch, load_json, out, run, save_json, symbolic, which)

NOTIFY_IFACE = "org.freedesktop.Notifications"
SKIP_HINTS = ("transient", "x-canonical-private-synchronous")


class Store:
    """Listens to Notify and CloseNotification calls on the session bus."""

    def __init__(self, on_change):
        self.on_change = on_change
        self.notes = load_json(NOTES_FILE, [])
        self.unread = 0
        self.pending = {}
        self.save_id = 0
        self.daemon = None
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        Gio.bus_watch_name_on_connection(self.bus, NOTIFY_IFACE, Gio.BusNameWatcherFlags.NONE,
                                         self.on_daemon, lambda *_: None)

    def on_daemon(self, _bus, _name, owner):
        """(Re)start monitoring whenever the notification daemon (re)appears."""
        if self.daemon == owner:
            return
        self.daemon = owner
        addr = Gio.dbus_address_get_for_bus_sync(Gio.BusType.SESSION, None)
        try:
            conn = Gio.DBusConnection.new_for_address_sync(
                addr, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT |
                Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
            rules = [f"type='method_call',interface='{NOTIFY_IFACE}',member='Notify'",
                     f"type='method_call',interface='{NOTIFY_IFACE}',member='CloseNotification'",
                     f"type='method_return',sender='{owner}'"]
            conn.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus.Monitoring",
                           "BecomeMonitor", GLib.Variant("(asu)", (rules, 0)), None, 0, -1, None)
        except GLib.Error:
            return
        conn.add_filter(self.on_message)
        self.monitor = conn

    def on_message(self, _conn, msg, _incoming):
        # Runs in the D-Bus worker thread: hand everything to the main loop.
        kind = msg.get_message_type()
        if kind == Gio.DBusMessageType.METHOD_CALL:
            member = msg.get_member()
            try:
                body = msg.get_body().unpack()
            except Exception:
                return None
            if member == "Notify":
                GLib.idle_add(self.on_notify, msg.get_serial(), msg.get_sender(), body)
            elif member == "CloseNotification":
                GLib.idle_add(self.remove_id, body[0])
        elif kind == Gio.DBusMessageType.METHOD_RETURN:
            serial = msg.get_reply_serial()
            if serial in self.pending:
                try:
                    nid = msg.get_body().unpack()[0]
                except Exception:
                    return None
                GLib.idle_add(self.on_reply, serial, nid)
        return None

    def on_notify(self, serial, _sender, body):
        app, replaces, icon, summary, text, _actions, hints, _timeout = body
        if any(hints.get(h) for h in SKIP_HINTS) or not (summary or text):
            return False
        note = {"id": replaces or 0, "app": app or "", "icon": icon or hints.get("image-path", "") or "",
                "desktop": hints.get("desktop-entry", "") or "", "summary": summary, "body": text,
                "time": time.time(), "urgency": int(hints.get("urgency", 1))}
        if replaces:
            for i, old in enumerate(self.notes):
                if old.get("id") == replaces:
                    self.notes[i] = note
                    self.changed(new=False)
                    return False
        self.pending[serial] = note
        self.notes.insert(0, note)
        del self.notes[100:]
        self.changed(new=True)
        return False

    def on_reply(self, serial, nid):
        note = self.pending.pop(serial, None)
        if note is not None:
            note["id"] = nid
            self.changed(new=False)
        return False

    def remove_id(self, nid):
        before = len(self.notes)
        self.notes = [n for n in self.notes if n.get("id") != nid]
        if len(self.notes) != before:
            self.changed()
        return False

    def remove(self, note):
        if note in self.notes:
            self.notes.remove(note)
            self.changed()

    def clear(self, app=None):
        self.notes = [n for n in self.notes if app is not None and n["app"] != app]
        self.unread = 0 if app is None else self.unread
        self.changed()

    def changed(self, new=False):
        if new:
            self.unread += 1
        self.unread = min(self.unread, len(self.notes))
        if self.save_id:
            GLib.source_remove(self.save_id)
        self.save_id = GLib.timeout_add(500, self.save)
        self.on_change()

    def save(self):
        self.save_id = 0
        save_json(NOTES_FILE, self.notes)
        return False

    def count_for(self, desktop_id, app_name):
        base = (desktop_id or "").removesuffix(".desktop").lower()
        return sum(1 for n in self.notes[:self.unread]
                   if (n["desktop"] and n["desktop"].lower() == base) or
                   (app_name and n["app"].lower() == app_name.lower()))


def ago(ts):
    secs = time.time() - ts
    if secs < 60:
        return "Now"
    if secs < 3600:
        return f"{int(secs // 60)}m ago"
    if time.localtime(ts)[:3] == time.localtime()[:3]:
        return time.strftime("%-I:%M %p", time.localtime(ts))
    return time.strftime("%-m/%-d/%Y", time.localtime(ts))


def dnd_on():
    return which("swaync-client") and out(["swaync-client", "-D"]).strip() == "true"


class NotificationCentre(Flyout):
    def __init__(self, store, monitor=None):
        super().__init__("notification-centre", Gtk.Align.END, monitor)
        self.store = store
        self.panel.get_style_context().remove_class("flyout")
        self.panel.set_spacing(12)
        self.notes_card = css_class(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "flyout", "nc-card")
        self.cal_card = css_class(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "flyout", "cal-card")
        self.panel.pack_start(self.notes_card, True, True, 0)
        self.panel.pack_start(self.cal_card, False, False, 0)
        self.focus_minutes = 30
        self.cal_open = True
        self.tick = 0

    def open(self):
        self.store.unread = 0
        self.store.on_change()
        self.build_notes()
        self.build_calendar()
        super().open()
        if not self.tick:
            self.tick = GLib.timeout_add_seconds(30, self.refresh_tick)

    def close_flyout(self):
        super().close_flyout()
        if self.tick:
            GLib.source_remove(self.tick)
            self.tick = 0

    def refresh_tick(self):
        if self.get_visible():
            self.build_notes()
            self.build_focus()
            return True
        self.tick = 0
        return False

    def refresh(self):
        if self.get_visible():
            self.store.unread = 0
            self.build_notes()

    # Notifications

    def build_notes(self):
        clear(self.notes_card)
        head = css_class(Gtk.Box(spacing=4), "nc-head")
        head.pack_start(label("Notifications", "nc-title"), False, False, 0)
        dnd = dnd_on()
        bell = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE,
                                    tooltip_text="Do not disturb is on" if dnd else "Do not disturb"), "qs-icon")
        if dnd:
            css_class(bell, "on")
        bell.add(symbolic("notifications-disabled-symbolic" if dnd else "preferences-system-notifications-symbolic", 16))
        bell.connect("clicked", lambda *_: run(["swaync-client", "-df" if dnd else "-dn"],
                                               lambda *_: self.build_notes()))
        head.pack_end(bell, False, False, 0)
        if self.store.notes:
            clear_all = css_class(Gtk.Button(label="Clear all", relief=Gtk.ReliefStyle.NONE), "qs-link")
            clear_all.connect("clicked", lambda *_: (self.store.clear(), self.build_notes()))
            head.pack_end(clear_all, False, False, 0)
        self.notes_card.pack_start(head, False, False, 0)
        if dnd:
            self.notes_card.pack_start(label("Do not disturb is on. You'll only see urgent notifications "
                                             "as banners.", "caption", "nc-hint", wrap=True), False, False, 0)
        if not self.store.notes:
            self.notes_card.pack_start(label("No new notifications", "nc-empty", xalign=0.5), False, False, 0)
            self.notes_card.show_all()
            return
        scroll = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER, propagate_natural_height=True)
        geo = self.get_screen().get_display().get_monitor(0).get_workarea()
        scroll.set_max_content_height(max(200, geo.height - (420 if self.cal_open else 160)))
        body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        css_class(body, "nc-list")
        scroll.add(body)
        groups = {}
        for note in self.store.notes:
            groups.setdefault(note["app"] or "Other", []).append(note)
        for app, notes in groups.items():
            body.pack_start(self.group_header(app, notes[0]), False, False, 0)
            for note in notes:
                body.pack_start(self.card(note), False, False, 0)
        self.notes_card.pack_start(scroll, True, True, 0)
        self.notes_card.show_all()

    def group_header(self, app, first):
        row = css_class(Gtk.Box(spacing=8), "nc-group")
        icon = first.get("desktop") and Gio.DesktopAppInfo.new(first["desktop"] + ".desktop")
        image = app_icon(icon.get_icon() if icon else (first.get("icon") or "dialog-information"), 16,
                         self.get_scale_factor())
        row.pack_start(image, False, False, 0)
        row.pack_start(label(app, "nc-app", ellipsize=True), True, True, 0)
        x = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="Clear notifications from " + app),
                      "nc-close")
        x.add(symbolic("window-close-symbolic", 12))
        x.connect("clicked", lambda *_: (self.store.clear(app), self.build_notes()))
        row.pack_end(x, False, False, 0)
        return row

    def card(self, note):
        button = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "nc-note")
        if note.get("urgency") == 2:
            css_class(button, "critical")
        box = Gtk.Box(spacing=12)
        icon = note.get("icon")
        if icon and (icon.startswith("/") or icon.startswith("file://") or Gtk.IconTheme.get_default().has_icon(icon)):
            box.pack_start(app_icon(icon.replace("file://", ""), 32, self.get_scale_factor()), False, False, 0)
        texts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        top = Gtk.Box(spacing=8)
        top.pack_start(label(note["summary"], "nc-summary", ellipsize=True), True, True, 0)
        top.pack_end(label(ago(note["time"]), "caption"), False, False, 0)
        texts.pack_start(top, False, False, 0)
        if note.get("body"):
            body = label("", "nc-body", wrap=True)
            text = note["body"]
            try:
                from gi.repository import Pango
                Pango.parse_markup(text, -1, "\0")
                body.set_markup(text)
            except Exception:
                body.set_text(text)
            body.set_lines(4)
            body.set_max_width_chars(40)
            texts.pack_start(body, False, False, 0)
        box.pack_start(texts, True, True, 0)
        x = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="Dismiss", valign=Gtk.Align.START),
                      "nc-close")
        x.add(symbolic("window-close-symbolic", 12))
        x.connect("clicked", lambda *_: (self.store.remove(note), self.build_notes()))
        box.pack_end(x, False, False, 0)
        button.add(box)
        button.connect("clicked", lambda *_: self.open_note(note))
        return button

    def open_note(self, note):
        self.store.remove(note)
        self.close_flyout()
        desktop = note.get("desktop")
        if desktop and Gio.DesktopAppInfo.new(desktop + ".desktop"):
            if self.on_open_app and self.on_open_app(desktop + ".desktop"):
                return
            launch(desktop + ".desktop")

    on_open_app = None

    # Calendar and Focus

    def build_calendar(self):
        clear(self.cal_card)
        head = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "cal-head")
        row = Gtk.Box(spacing=8)
        row.pack_start(label(time.strftime("%A, %B %-d"), "cal-title"), True, True, 0)
        row.pack_end(symbolic("go-down-symbolic" if self.cal_open else "go-up-symbolic", 12), False, False, 0)
        head.add(row)
        head.connect("clicked", lambda *_: (setattr(self, "cal_open", not self.cal_open),
                                            self.build_calendar(), self.build_notes()))
        self.cal_card.pack_start(head, False, False, 0)
        if self.cal_open:
            cal = Gtk.Calendar(show_heading=True, show_day_names=True)
            css_class(cal, "calendar")
            self.cal_card.pack_start(cal, False, False, 0)
        self.focus_box = css_class(Gtk.Box(spacing=8), "focus-row")
        self.cal_card.pack_start(self.focus_box, False, False, 0)
        self.build_focus()
        self.cal_card.show_all()

    def focus_left(self):
        try:
            with open(FOCUS_FILE) as f:
                return float(f.read().strip() or 0) - time.time()
        except (OSError, ValueError):
            return 0

    def build_focus(self):
        if not hasattr(self, "focus_box"):
            return
        clear(self.focus_box)
        left = self.focus_left()
        if left > 0:
            self.focus_box.pack_start(label(f"Focus session: {int(left // 60) + 1} min left", "focus-text"),
                                      True, True, 0)
            stop = css_class(Gtk.Button(label="End session"), "qs-button")
            stop.connect("clicked", lambda *_: self.stop_focus())
            self.focus_box.pack_end(stop, False, False, 0)
        else:
            minus = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="Decrease time"), "qs-icon")
            minus.add(symbolic("list-remove-symbolic", 12))
            plus = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="Increase time"), "qs-icon")
            plus.add(symbolic("list-add-symbolic", 12))
            minutes = label(f"{self.focus_minutes} mins", "focus-text", xalign=0.5)
            minutes.set_width_chars(8)

            def step(d):
                steps = [5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240]
                i = steps.index(self.focus_minutes) if self.focus_minutes in steps else 4
                self.focus_minutes = steps[max(0, min(len(steps) - 1, i + d))]
                minutes.set_text(f"{self.focus_minutes} mins")
            minus.connect("clicked", lambda *_: step(-1))
            plus.connect("clicked", lambda *_: step(1))
            self.focus_box.pack_start(minus, False, False, 0)
            self.focus_box.pack_start(minutes, False, False, 0)
            self.focus_box.pack_start(plus, False, False, 0)
            start = css_class(Gtk.Button(), "qs-button")
            inner = Gtk.Box(spacing=6)
            inner.pack_start(symbolic("media-playback-start-symbolic", 12), False, False, 0)
            inner.pack_start(label("Focus"), False, False, 0)
            start.add(inner)
            start.connect("clicked", lambda *_: self.start_focus())
            self.focus_box.pack_end(start, False, False, 0)
        self.focus_box.show_all()

    def start_focus(self):
        m = self.focus_minutes
        run(["systemctl", "--user", "stop", "ws-focus.timer", "ws-focus.service"], lambda *_: run(
            ["systemd-run", "--user", "--unit=ws-focus", f"--on-active={m}m", "sh", "-c",
             "swaync-client -df; notify-send -a Focus -i alarm 'Focus session ended' 'Notifications are back on.'"]))
        run(["swaync-client", "-dn"])
        os.makedirs(os.path.dirname(FOCUS_FILE), exist_ok=True)
        with open(FOCUS_FILE, "w") as f:
            f.write(str(time.time() + m * 60))
        self.build_focus()
        GLib.timeout_add(300, lambda: self.build_notes() and False)

    def stop_focus(self):
        run(["systemctl", "--user", "stop", "ws-focus.timer"])
        run(["swaync-client", "-df"])
        with open(FOCUS_FILE, "w") as f:
            f.write("0")
        self.build_focus()
        GLib.timeout_add(300, lambda: self.build_notes() and False)

