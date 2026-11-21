"""The Windows 11 lock and sign-in screen, shared by the LightDM greeter and the
in-session lock screen. GTK 4.

A backend object supplies the data and actions:
  users()                   -> [{"name", "display", "image", "logged_in"}]
  selected()                -> user name to show first
  background(name)          -> picture path or None
  authenticate(name, password, done)   done(ok, message) on the main loop
  power()                   -> [(label, icon, callback)]
  sessions()                -> [(key, label)] (empty in the lock screen)
  session(name) / set_session(name, key)
  network()                 -> (icon, tooltip) or None
  battery()                 -> (icon, tooltip) or None
Optional: clock_24h(name) -> bool, caps_hint() -> bool
"""

import math
import time

import gi

gi.require_version("Gdk", "4.0")
gi.require_version("Gtk", "4.0")
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import Gdk, GdkPixbuf, GLib, Gtk  # noqa: E402

FONT = "@FONT@"
IDLE_SECONDS = 60
FALLBACK_BACKGROUND = "/usr/share/backgrounds/mde/ws-wallpaper.jpg"


def css(scale):
    def s(px):
        return f"{round(px * scale, 1)}px"
    return f"""
window.signin {{ background-color: #0b1626; }}
.signin, .signin label, .signin button, .signin entry, .signin text {{ font-family: "{FONT}"; color: #ffffff; }}
.signin label {{ font-size: {s(14)}; }}
.dim {{ background-color: rgba(0, 0, 0, 0.32); }}
.signin label.lock-time {{ font-size: {s(120)}; font-weight: 600; text-shadow: 0 {s(2)} {s(12)} rgba(0, 0, 0, 0.35); }}
.signin label.lock-date {{ font-size: {s(26)}; font-weight: 400; text-shadow: 0 {s(1)} {s(8)} rgba(0, 0, 0, 0.35); }}
.signin label.name {{ font-size: {s(28)}; font-weight: 600; }}
.signin label.status {{ font-size: {s(13)}; color: rgba(255, 255, 255, 0.8); }}
.signin label.caps {{ font-size: {s(12)}; color: rgba(255, 255, 255, 0.9); }}
.pw-box {{
  background-color: rgba(255, 255, 255, 0.72); border-radius: {s(4)};
  border-bottom: {s(1)} solid rgba(0, 0, 0, 0.4); min-height: {s(36)};
}}
.pw-box:focus-within {{ background-color: #ffffff; border-bottom: {s(2)} solid @ACCENT@; }}
.pw-box entry, .pw-box text {{
  background: none; border: none; box-shadow: none; outline: none; color: #1a1a1a;
  font-size: {s(14)}; min-height: {s(34)}; padding: 0 {s(4)} 0 {s(10)};
}}
.pw-box text placeholder {{ color: rgba(0, 0, 0, 0.55); }}
.pw-box image {{ color: #1a1a1a; -gtk-icon-size: {s(16)}; }}
.pw-box button {{
  background: none; border: none; box-shadow: none; min-width: {s(30)}; min-height: {s(30)};
  margin: {s(3)}; padding: 0; border-radius: {s(3)};
}}
.pw-box button:hover {{ background-color: rgba(0, 0, 0, 0.06); }}
.pw-box button image {{ color: #1a1a1a; }}
.signin button.plain {{
  background-color: rgba(255, 255, 255, 0.18); border: {s(1)} solid rgba(255, 255, 255, 0.4);
  border-radius: {s(4)}; min-height: {s(32)}; min-width: {s(120)}; box-shadow: none; padding: 0 {s(16)};
}}
.signin button.plain:hover {{ background-color: rgba(255, 255, 255, 0.28); }}
.signin button.flat-white {{
  background: none; border: none; box-shadow: none; border-radius: {s(4)};
  min-width: {s(40)}; min-height: {s(40)}; padding: 0;
}}
.signin button.flat-white:hover {{ background-color: rgba(255, 255, 255, 0.12); }}
.signin button.flat-white image {{ -gtk-icon-size: {s(20)}; color: #ffffff; }}
.signin button.user {{
  background: none; border: none; box-shadow: none; border-radius: {s(4)}; padding: {s(6)} {s(12)} {s(6)} {s(8)};
  min-width: {s(220)};
}}
.signin button.user:hover {{ background-color: rgba(255, 255, 255, 0.12); }}
.signin button.user.selected {{ background-color: rgba(255, 255, 255, 0.2); }}
.signin .panel {{
  background-color: rgba(32, 32, 32, 0.92); border: {s(1)} solid rgba(255, 255, 255, 0.1);
  border-radius: {s(8)}; padding: {s(4)};
}}
.signin .panel button {{
  background: none; border: none; box-shadow: none; border-radius: {s(4)};
  padding: {s(8)} {s(12)}; min-width: {s(200)};
}}
.signin .panel button:hover {{ background-color: rgba(255, 255, 255, 0.08); }}
.signin .panel image {{ -gtk-icon-size: {s(16)}; color: #ffffff; }}
.signin .panel .checked {{ font-weight: 600; }}
.signin spinner {{ min-width: {s(28)}; min-height: {s(28)}; color: #ffffff; }}
"""


def load_pixbuf(path, width, height):
    try:
        pix = GdkPixbuf.Pixbuf.new_from_file(path)
    except (GLib.Error, TypeError):
        return None
    sw, sh = pix.get_width(), pix.get_height()
    factor = max(width / sw, height / sh)
    w, h = max(1, round(sw * factor)), max(1, round(sh * factor))
    pix = pix.scale_simple(w, h, GdkPixbuf.InterpType.BILINEAR)
    x, y = (w - width) // 2, (h - height) // 2
    return pix.new_subpixbuf(max(0, x), max(0, y), min(width, w), min(height, h)).copy()


def blurred(pix):
    """A soft blur: average down to a thumbnail, then enlarge in doubling steps so the
    interpolation leaves no blocks."""
    w, h = pix.get_width(), pix.get_height()
    sw, sh = max(1, w // 32), max(1, h // 32)
    small = pix.scale_simple(sw, sh, GdkPixbuf.InterpType.HYPER)
    while sw * 2 < w:
        sw, sh = sw * 2, sh * 2
        small = small.scale_simple(sw, sh, GdkPixbuf.InterpType.BILINEAR)
    return small.scale_simple(w, h, GdkPixbuf.InterpType.BILINEAR)


def texture(pix):
    return Gdk.Texture.new_for_pixbuf(pix) if pix else None


class Avatar(Gtk.DrawingArea):
    def __init__(self, size):
        super().__init__()
        self.size = size
        self.pix = None
        self.set_content_width(size)
        self.set_content_height(size)
        self.set_halign(Gtk.Align.CENTER)
        self.set_draw_func(self.draw)

    def set_image(self, path):
        self.pix = None
        if path:
            try:
                self.pix = GdkPixbuf.Pixbuf.new_from_file_at_scale(path, self.size * 2, self.size * 2, True)
            except (GLib.Error, TypeError):
                pass
        self.queue_draw()

    def draw(self, _area, cr, w, h):
        r = min(w, h) / 2
        cr.arc(w / 2, h / 2, r, 0, 2 * math.pi)
        cr.clip()
        if self.pix:
            pw, ph = self.pix.get_width(), self.pix.get_height()
            f = max(w / pw, h / ph)
            cr.translate((w - pw * f) / 2, (h - ph * f) / 2)
            cr.scale(f, f)
            Gdk.cairo_set_source_pixbuf(cr, self.pix, 0, 0)
            cr.paint()
            return
        cr.set_source_rgb(0.80, 0.80, 0.82)
        cr.paint()
        cr.set_source_rgb(1, 1, 1)
        cr.arc(w / 2, h * 0.39, r * 0.36, 0, 2 * math.pi)
        cr.fill()
        cr.arc(w / 2, h * 1.02, r * 0.68, math.pi, 2 * math.pi)
        cr.fill()


def system_prop(name, path, iface, prop):
    from gi.repository import Gio
    try:
        bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
        v = bus.call_sync(name, path, "org.freedesktop.DBus.Properties", "Get", GLib.Variant("(ss)", (iface, prop)),
                          None, Gio.DBusCallFlags.NONE, 1000, None)
        return v.unpack()[0]
    except GLib.Error:
        return None


def network_status():
    nm, path = "org.freedesktop.NetworkManager", "/org/freedesktop/NetworkManager"
    state = system_prop(nm, path, nm, "State")
    if state is None:
        return None
    primary = system_prop(nm, path, nm, "PrimaryConnection") or "/"
    kind = system_prop(nm, primary, nm + ".Connection.Active", "Type") if primary != "/" else ""
    name = system_prop(nm, primary, nm + ".Connection.Active", "Id") if primary != "/" else ""
    if state < 50 or primary == "/":
        return "network-offline-symbolic", "Not connected"
    tip = f"{name}\n" + ("Internet access" if state == 70 else "No internet access")
    if "wireless" in (kind or ""):
        return "network-wireless-signal-excellent-symbolic", tip
    return "network-wired-symbolic", tip


def battery_status():
    up, path = "org.freedesktop.UPower", "/org/freedesktop/UPower/devices/DisplayDevice"
    if not system_prop(up, path, up + ".Device", "IsPresent") or system_prop(up, path, up + ".Device", "Type") != 2:
        return None
    pct = system_prop(up, path, up + ".Device", "Percentage") or 0
    state = system_prop(up, path, up + ".Device", "State")
    level = min(100, int(pct // 10 * 10))
    charging = state in (1, 4, 5)
    name = "battery-level-100-charged-symbolic" if state == 4 else \
        f"battery-level-{level}{'-charging' if charging else ''}-symbolic"
    return name, f"Battery: {round(pct)}%" + (" (charging)" if charging and state != 4 else "")


def icon(name, size=None):
    image = Gtk.Image.new_from_icon_name(name)
    if size:
        image.set_pixel_size(size)
    return image


class SignIn(Gtk.Overlay):
    def __init__(self, backend, width, height, scale=1.0, mode="greeter"):
        super().__init__()
        self.backend, self.width, self.height, self.scale, self.mode = backend, width, height, scale, mode
        self.add_css_class("signin")
        self.provider = Gtk.CssProvider()
        self.set_scale(scale)
        Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), self.provider,
                                                  Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        settings = Gtk.Settings.get_default()
        settings.set_property("gtk-icon-theme-name", "Adwaita")
        settings.set_property("gtk-application-prefer-dark-theme", True)
        self.users = backend.users()
        self.user = backend.selected() or (self.users[0]["name"] if self.users else "")
        self.busy = False
        self.idle_id = 0

        self.bg = Gtk.Stack(transition_type=Gtk.StackTransitionType.CROSSFADE, transition_duration=400)
        self.sharp, self.soft = Gtk.Picture(), Gtk.Picture()
        for pic in (self.sharp, self.soft):
            pic.set_content_fit(Gtk.ContentFit.COVER)
            pic.set_can_shrink(True)
        self.bg.add_named(self.sharp, "sharp")
        self.bg.add_named(self.soft, "soft")
        self.set_child(self.bg)

        self.dim = Gtk.Box(visible=False)
        self.dim.add_css_class("dim")
        self.add_overlay(self.dim)

        self.pages = Gtk.Stack(transition_type=Gtk.StackTransitionType.SLIDE_UP, transition_duration=300)
        self.pages.add_named(self.build_lock(), "lock")
        self.pages.add_named(self.build_signin(), "signin")
        self.add_overlay(self.pages)

        self.users_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2, halign=Gtk.Align.START,
                                 valign=Gtk.Align.END, visible=False)
        self.users_box.set_margin_start(self.px(24))
        self.users_box.set_margin_bottom(self.px(24))
        self.add_overlay(self.users_box)

        self.corner = Gtk.Box(spacing=self.px(4), halign=Gtk.Align.END, valign=Gtk.Align.END)
        self.corner.set_margin_end(self.px(24))
        self.corner.set_margin_bottom(self.px(24))
        self.add_overlay(self.corner)
        self.panel = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, halign=Gtk.Align.END, valign=Gtk.Align.END,
                             visible=False)
        self.panel.add_css_class("panel")
        self.panel.set_margin_end(self.px(24))
        self.panel.set_margin_bottom(self.px(72))
        self.add_overlay(self.panel)

        keys = Gtk.EventControllerKey()
        keys.set_propagation_phase(Gtk.PropagationPhase.CAPTURE)
        keys.connect("key-pressed", self.on_key)
        self.add_controller(keys)
        click = Gtk.GestureClick()
        click.connect("pressed", self.on_click)
        self.add_controller(click)
        motion = Gtk.EventControllerMotion()
        motion.connect("motion", lambda *_: self.poke())
        self.add_controller(motion)

        self.select_user(self.user)
        self.build_corner()
        self.tick()
        GLib.timeout_add_seconds(1, self.tick)
        self.watch_caps()

    # Scale and helpers

    def px(self, value):
        return round(value * self.scale)

    def set_scale(self, scale):
        self.scale = scale
        self.provider.load_from_string(css(scale)) if hasattr(self.provider, "load_from_string") \
            else self.provider.load_from_data(css(scale).encode())

    # Lock page

    def build_lock(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, halign=Gtk.Align.CENTER, valign=Gtk.Align.START)
        box.set_margin_top(round(self.height * 0.12))
        self.time_label = Gtk.Label()
        self.time_label.add_css_class("lock-time")
        self.date_label = Gtk.Label()
        self.date_label.add_css_class("lock-date")
        box.append(self.time_label)
        box.append(self.date_label)
        return box

    def tick(self):
        h24 = hasattr(self.backend, "clock_24h") and self.backend.clock_24h(self.user)
        self.time_label.set_text(time.strftime("%H:%M" if h24 else "%-I:%M"))
        self.date_label.set_text(time.strftime("%A, %B %-d"))
        return True

    # Sign-in page

    def build_signin(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=self.px(8), halign=Gtk.Align.CENTER,
                      valign=Gtk.Align.CENTER)
        self.avatar = Avatar(self.px(192))
        box.append(self.avatar)
        self.name_label = Gtk.Label()
        self.name_label.add_css_class("name")
        self.name_label.set_margin_top(self.px(12))
        box.append(self.name_label)
        self.status_label = Gtk.Label(visible=False)
        self.status_label.add_css_class("status")
        box.append(self.status_label)

        self.action = Gtk.Stack(transition_type=Gtk.StackTransitionType.CROSSFADE, transition_duration=150,
                                vhomogeneous=False)
        self.action.set_margin_top(self.px(16))
        pw_box = Gtk.Box(halign=Gtk.Align.CENTER)
        pw_box.add_css_class("pw-box")
        pw_box.set_size_request(self.px(296), -1)
        self.entry = Gtk.PasswordEntry(show_peek_icon=True, hexpand=True)
        self.entry.set_property("placeholder-text", "Password")
        self.entry.connect("activate", lambda *_: self.submit())
        pw_box.append(self.entry)
        go = Gtk.Button(tooltip_text="Submit")
        go.set_child(icon("go-next-symbolic"))
        go.connect("clicked", lambda *_: self.submit())
        pw_box.append(go)
        self.action.add_named(pw_box, "password")

        error = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=self.px(16), halign=Gtk.Align.CENTER)
        self.error_label = Gtk.Label(wrap=True, justify=Gtk.Justification.CENTER, max_width_chars=40)
        error.append(self.error_label)
        ok = Gtk.Button(label="OK", halign=Gtk.Align.CENTER)
        ok.add_css_class("plain")
        ok.connect("clicked", lambda *_: self.show_password())
        error.append(ok)
        self.ok_button = ok
        self.action.add_named(error, "error")

        welcome = Gtk.Box(spacing=self.px(12), halign=Gtk.Align.CENTER)
        self.spinner = Gtk.Spinner()
        welcome.append(self.spinner)
        self.welcome_label = Gtk.Label(label="Welcome")
        welcome.append(self.welcome_label)
        self.action.add_named(welcome, "welcome")
        box.append(self.action)

        self.caps_label = Gtk.Label(label="Caps Lock is on", visible=False)
        self.caps_label.add_css_class("caps")
        box.append(self.caps_label)
        return box

    def select_user(self, name):
        self.user = name
        info = next((u for u in self.users if u["name"] == name), {"name": name, "display": name})
        self.name_label.set_text(info.get("display") or name)
        self.avatar.set_image(info.get("image"))
        if self.mode == "lock":
            self.status_label.set_text("Locked")
            self.status_label.set_visible(True)
        else:
            self.status_label.set_text("Signed in" if info.get("logged_in") else "")
            self.status_label.set_visible(bool(info.get("logged_in")))
        self.set_background(self.backend.background(name))
        self.build_users()
        self.show_password()

    def set_background(self, path):
        pix = None
        for candidate in (path, FALLBACK_BACKGROUND):
            if candidate:
                pix = load_pixbuf(candidate, self.width, self.height)
                if pix:
                    break
        if pix:
            self.sharp.set_paintable(texture(pix))
            self.soft.set_paintable(texture(blurred(pix)))

    def build_users(self):
        child = self.users_box.get_first_child()
        while child:
            nxt = child.get_next_sibling()
            self.users_box.remove(child)
            child = nxt
        if self.mode != "greeter" or len(self.users) < 2:
            self.users_box.set_visible(False)
            return
        for info in self.users:
            b = Gtk.Button()
            b.add_css_class("user")
            if info["name"] == self.user:
                b.add_css_class("selected")
            row = Gtk.Box(spacing=self.px(12))
            av = Avatar(self.px(40))
            av.set_image(info.get("image"))
            row.append(av)
            row.append(Gtk.Label(label=info.get("display") or info["name"], xalign=0))
            b.set_child(row)
            b.connect("clicked", lambda _b, n=info["name"]: (self.select_user(n), self.show_signin()))
            self.users_box.append(b)
        self.users_box.set_visible(self.pages.get_visible_child_name() == "signin")

    # Bottom-right corner

    def build_corner(self):
        child = self.corner.get_first_child()
        while child:
            nxt = child.get_next_sibling()
            self.corner.remove(child)
            child = nxt
        sessions = self.backend.sessions()
        if len(sessions) > 1:
            self.corner.append(self.corner_button("preferences-desktop-display-symbolic", "Session",
                                                  self.show_sessions))
        net = self.backend.network()
        if net:
            b = self.corner_button(net[0], net[1], None)
            self.corner.append(b)
        bat = self.backend.battery()
        if bat:
            self.corner.append(self.corner_button(bat[0], bat[1], None))
        self.corner.append(self.corner_button("preferences-desktop-accessibility-symbolic", "Accessibility",
                                              self.show_accessibility))
        if self.backend.power():
            self.corner.append(self.corner_button("system-shutdown-symbolic", "Power", self.show_power))

    def corner_button(self, icon_name, tip, callback):
        b = Gtk.Button(tooltip_text=tip)
        b.add_css_class("flat-white")
        b.set_child(icon(icon_name))
        if callback:
            b.connect("clicked", lambda *_: callback())
        else:
            b.set_can_target(False)
        return b

    def open_panel(self, items):
        """items: (label, icon or None, callback, checked)"""
        if self.panel.get_visible() and getattr(self, "panel_items", None) == [i[0] for i in items]:
            self.panel.set_visible(False)
            return
        child = self.panel.get_first_child()
        while child:
            nxt = child.get_next_sibling()
            self.panel.remove(child)
            child = nxt
        for text, icon_name, callback, checked in items:
            b = Gtk.Button()
            row = Gtk.Box(spacing=self.px(12))
            if icon_name:
                row.append(icon(icon_name))
            lab = Gtk.Label(label=text, xalign=0, hexpand=True)
            if checked:
                lab.add_css_class("checked")
            row.append(lab)
            if checked:
                row.append(icon("object-select-symbolic"))
            b.set_child(row)
            b.connect("clicked", lambda _b, c=callback: (self.panel.set_visible(False), c()))
            self.panel.append(b)
        self.panel_items = [i[0] for i in items]
        self.panel.set_visible(True)

    def show_power(self):
        self.open_panel([(text, icon_name, cb, False) for text, icon_name, cb in self.backend.power()])

    def show_accessibility(self):
        large = self.scale > self.base_scale if hasattr(self, "base_scale") else False
        self.open_panel([("Larger text", "zoom-in-symbolic", self.toggle_large, large)])

    def toggle_large(self):
        if not hasattr(self, "base_scale"):
            self.base_scale = self.scale
        self.set_scale(self.base_scale * 1.25 if self.scale == self.base_scale else self.base_scale)

    def show_sessions(self):
        current = self.backend.session(self.user)
        self.open_panel([(label, None, lambda k=key: self.backend.set_session(self.user, k), key == current)
                         for key, label in self.backend.sessions()])

    # Flow

    def show_signin(self):
        if self.pages.get_visible_child_name() != "signin":
            self.pages.set_visible_child_name("signin")
            self.bg.set_visible_child_name("soft")
            self.dim.set_visible(True)
            self.users_box.set_visible(self.mode == "greeter" and len(self.users) > 1)
        self.entry.grab_focus()
        self.poke()

    def show_lock(self):
        if self.busy:
            return
        self.panel.set_visible(False)
        self.pages.set_visible_child_name("lock")
        self.bg.set_visible_child_name("sharp")
        self.dim.set_visible(False)
        self.users_box.set_visible(False)
        self.entry.set_text("")

    def show_password(self):
        self.busy = False
        self.spinner.stop()
        self.action.set_visible_child_name("password")
        self.entry.set_text("")
        self.entry.set_sensitive(True)
        if self.pages.get_visible_child_name() == "signin":
            self.entry.grab_focus()

    def poke(self):
        if self.idle_id:
            GLib.source_remove(self.idle_id)
        self.idle_id = GLib.timeout_add_seconds(IDLE_SECONDS, self.on_idle)

    def on_idle(self):
        self.idle_id = 0
        if self.action.get_visible_child_name() != "welcome":
            self.show_lock()
        return False

    def on_click(self, *_args):
        if self.pages.get_visible_child_name() == "lock":
            self.show_signin()
        self.poke()

    def on_key(self, _ctrl, keyval, _code, state):
        self.poke()
        self.update_caps()
        if self.pages.get_visible_child_name() == "lock":
            self.show_signin()
            char = chr(Gdk.keyval_to_unicode(keyval)) if Gdk.keyval_to_unicode(keyval) else ""
            if char.isprintable() and char and not state & (Gdk.ModifierType.CONTROL_MASK | Gdk.ModifierType.ALT_MASK):
                self.entry.set_text(char)
                self.entry.set_position(-1)
            return True
        if keyval == Gdk.KEY_Escape:
            if self.panel.get_visible():
                self.panel.set_visible(False)
            elif self.action.get_visible_child_name() == "error":
                self.show_password()
            else:
                self.show_lock()
            return True
        if self.action.get_visible_child_name() == "error" and keyval in (Gdk.KEY_Return, Gdk.KEY_KP_Enter):
            self.show_password()
            return True
        return False

    def submit(self):
        if self.busy:
            return
        password = self.entry.get_text()
        self.busy = True
        self.entry.set_sensitive(False)
        self.action.set_visible_child_name("welcome")
        self.spinner.start()
        self.backend.authenticate(self.user, password, self.on_result)

    def on_result(self, ok, message=""):
        if ok:
            return
        self.busy = False
        self.spinner.stop()
        self.error_label.set_text(message or "The password is incorrect. Try again.")
        self.action.set_visible_child_name("error")
        self.ok_button.grab_focus()

    # Caps Lock

    def watch_caps(self):
        seat = Gdk.Display.get_default().get_default_seat()
        self.keyboard = seat.get_keyboard() if seat else None
        if self.keyboard:
            self.keyboard.connect("notify::caps-lock-state", lambda *_: self.update_caps())
        self.update_caps()

    def update_caps(self):
        on = bool(self.keyboard and self.keyboard.get_caps_lock_state())
        on = on and (not hasattr(self.backend, "caps_hint") or self.backend.caps_hint())
        self.caps_label.set_visible(on and self.pages.get_visible_child_name() == "signin")
