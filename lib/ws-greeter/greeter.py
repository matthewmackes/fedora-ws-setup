#!/usr/bin/env python3
"""ws-greeter: the Windows 11 sign-in screen for LightDM. "--demo" shows it in a window
without LightDM (any password but "demo" is wrong)."""

import configparser
import json
import os
import pwd
import sys

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))

import gi  # noqa: E402

from signin import Gdk, GLib, Gtk, SignIn, battery_status, network_status  # noqa: E402

DEMO = "--demo" in sys.argv
if not DEMO:
    gi.require_version("LightDM", "1")
    from gi.repository import LightDM  # noqa: E402

CONF = "/etc/lightdm/ws-greeter.conf"
SHARED = "/var/lib/ws-greeter"
STATE = os.path.join(os.path.expanduser("~"), ".cache", "ws-greeter", "state.json")


def conf():
    parser = configparser.ConfigParser()
    parser.read(CONF)
    return parser["greeter"] if parser.has_section("greeter") else {}


def readable(path):
    return path if path and os.access(path, os.R_OK) else None


def owned(path, name):
    """The shared folder is world-writable: trust only files the named user owns."""
    try:
        return os.stat(path).st_uid == pwd.getpwnam(name).pw_uid and os.access(path, os.R_OK)
    except (OSError, KeyError):
        return False


def user_data(name):
    path = os.path.join(SHARED, name + ".json")
    if not owned(path, name):
        return {}
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def shared_picture(name):
    for ext in (".jpg", ".png", ".jpeg", ".webp"):
        path = os.path.join(SHARED, name + ext)
        if owned(path, name):
            return path
    return None


def load_state():
    try:
        with open(STATE) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def save_state(**kw):
    state = load_state()
    state.update(kw)
    try:
        os.makedirs(os.path.dirname(STATE), exist_ok=True)
        with open(STATE, "w") as f:
            json.dump(state, f)
    except OSError:
        pass


class Backend:
    def __init__(self):
        self.done = None
        self.password = None
        self.messages = []
        self.state = load_state()
        self.chosen = dict(self.state.get("sessions", {}))
        self.greeter = LightDM.Greeter()
        self.greeter.connect("show-prompt", self.on_prompt)
        self.greeter.connect("show-message", self.on_message)
        self.greeter.connect("authentication-complete", self.on_complete)
        self.greeter.connect_to_daemon_sync()
        self.list = LightDM.UserList.get_instance()

    def users(self):
        return [{"name": u.get_name(), "display": u.get_display_name() or u.get_name(),
                 "image": readable(u.get_image()), "logged_in": u.get_logged_in()}
                for u in self.list.get_users()]

    def selected(self):
        names = [u["name"] for u in self.users()]
        for candidate in (self.greeter.get_select_user_hint(), self.state.get("last_user")):
            if candidate in names:
                return candidate
        return names[0] if names else None

    def background(self, name):
        user = self.list.get_user_by_name(name) if name else None
        return (shared_picture(name) or readable(user.get_background() if user else None)
                or conf().get("background"))

    def authenticate(self, name, password, done):
        self.done, self.password, self.messages = done, password, []
        self.user = name
        if self.greeter.get_in_authentication():
            self.greeter.cancel_authentication()
        self.greeter.authenticate(name)

    def on_prompt(self, greeter, _text, _kind):
        if self.password is not None:
            greeter.respond(self.password)
            self.password = None

    def on_message(self, _greeter, text, kind):
        if kind == LightDM.MessageType.ERROR:
            self.messages.append(text)

    def on_complete(self, greeter):
        done = self.done
        if greeter.get_is_authenticated():
            save_state(last_user=self.user, sessions=self.chosen)
            try:
                greeter.start_session_sync(self.session(self.user))
                return
            except GLib.Error as e:
                done and done(False, f"Couldn't start the session. {e.message}")
                return
        done and done(False, " ".join(self.messages) or "The password is incorrect. Try again.")

    def power(self):
        actions = []
        if os.path.lexists("/system-update"):
            def update_and(mode):
                import subprocess
                cmd = ("gdbus call --system --dest org.freedesktop.PackageKit "
                       "--object-path /org/freedesktop/PackageKit "
                       "--method org.freedesktop.PackageKit.Offline.Trigger %s >/dev/null && "
                       "systemctl reboot" % mode)
                return lambda: subprocess.Popen(["sh", "-c", cmd], start_new_session=True)
            actions.append(("Update and restart", "system-reboot-symbolic", update_and("reboot")))
            actions.append(("Update and shut down", "system-shutdown-symbolic", update_and("power-off")))
        if LightDM.get_can_suspend():
            actions.append(("Sleep", "weather-clear-night-symbolic", LightDM.suspend))
        if LightDM.get_can_shutdown():
            actions.append(("Shut down", "system-shutdown-symbolic", LightDM.shutdown))
        if LightDM.get_can_restart():
            actions.append(("Restart", "system-reboot-symbolic", LightDM.restart))
        return actions

    def sessions(self):
        return [(s.get_key(), s.get_name()) for s in LightDM.get_sessions()]

    def session(self, name):
        user = self.list.get_user_by_name(name) if name else None
        keys = [k for k, _ in self.sessions()]
        for candidate in (self.chosen.get(name), user.get_session() if user else None,
                          self.greeter.get_default_session_hint(), "sway"):
            if candidate in keys:
                return candidate
        return keys[0] if keys else None

    def set_session(self, name, key):
        self.chosen[name] = key

    def clock_24h(self, name):
        return bool(user_data(name).get("clock_24h")) if name else False

    network = staticmethod(network_status)
    battery = staticmethod(battery_status)


class DemoBackend(Backend):
    def __init__(self):
        self.chosen = {}
        me = pwd.getpwuid(os.getuid())
        face = os.path.expanduser("~/.face")
        self.list_ = [{"name": me.pw_name, "display": me.pw_gecos.split(",")[0] or me.pw_name,
                       "image": face if os.path.exists(face) else None, "logged_in": True},
                      {"name": "guest", "display": "Guest", "image": None, "logged_in": False}]

    def users(self):
        return self.list_

    def selected(self):
        return self.list_[0]["name"]

    def background(self, name):
        return shared_picture(name) or conf().get("background")

    def authenticate(self, name, password, done):
        def finish():
            done(password == "demo", "")
            if password == "demo":
                GLib.timeout_add(1500, lambda: os._exit(0))
            return False
        GLib.timeout_add(900, finish)

    def power(self):
        actions = []
        if os.path.lexists("/system-update"):
            actions += [("Update and restart", "system-reboot-symbolic", lambda: None),
                        ("Update and shut down", "system-shutdown-symbolic", lambda: None)]
        actions += [("Sleep", "weather-clear-night-symbolic", lambda: None),
                    ("Shut down", "system-shutdown-symbolic", lambda: None),
                    ("Restart", "system-reboot-symbolic", lambda: None)]
        return actions

    def sessions(self):
        return [("sway", "Sway"), ("xfce", "Xfce Session")]

    def session(self, name):
        return self.chosen.get(name, "sway")


def x11_focus(window):
    """No window manager runs during sign-in: give the window the keyboard and set the
    root pointer to an arrow."""
    try:
        import ctypes
        gi.require_version("GdkX11", "4.0")
        from gi.repository import GdkX11
        surface = window.get_surface()
        if not isinstance(surface, GdkX11.X11Surface):
            return False
        x11 = ctypes.CDLL("libX11.so.6")
        x11.XOpenDisplay.restype = ctypes.c_void_p
        x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
        dpy = x11.XOpenDisplay(None)
        if not dpy:
            return False
        x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
        x11.XDefaultRootWindow.restype = ctypes.c_ulong
        x11.XCreateFontCursor.argtypes = [ctypes.c_void_p, ctypes.c_uint]
        x11.XCreateFontCursor.restype = ctypes.c_ulong
        x11.XDefineCursor.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong]
        x11.XSetInputFocus.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
        x11.XRaiseWindow.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
        x11.XFlush.argtypes = [ctypes.c_void_p]
        xid = surface.get_xid()
        x11.XDefineCursor(dpy, x11.XDefaultRootWindow(dpy), x11.XCreateFontCursor(dpy, 68))
        x11.XRaiseWindow(dpy, xid)
        x11.XSetInputFocus(dpy, xid, 1, 0)
        x11.XFlush(dpy)
        window._x11 = (x11, dpy)
    except Exception:
        pass
    return False


def auto_scale(monitor, user_scale):
    if user_scale:
        return float(user_scale)
    geo, mm = monitor.get_geometry(), monitor.get_width_mm()
    if not mm:
        return 1.0
    dpi = geo.width / (mm / 25.4)
    return max(1.0, min(2.5, round(dpi / 96 * 4) / 4))


def main():
    backend = DemoBackend() if DEMO else Backend()
    display = Gdk.Display.get_default()
    monitor = display.get_monitors().get_item(0)
    geo = monitor.get_geometry()
    first = backend.selected()
    scale = conf().get("scale") or (user_data(first).get("scale") if first else None)
    scale = auto_scale(monitor, scale)
    width, height = (1280, 800) if DEMO else (geo.width, geo.height)
    win = Gtk.Window(title="Sign in", decorated=DEMO)
    win.add_css_class("signin")
    win.set_default_size(width, height)
    win.set_child(SignIn(backend, width, height, scale, "greeter"))
    loop = GLib.MainLoop()
    win.connect("close-request", lambda *_: loop.quit() or False)
    win.present()
    if DEMO and "--signin" in sys.argv:
        win.get_child().show_signin()
    if not DEMO:
        win.fullscreen()
        GLib.timeout_add(300, x11_focus, win)
        GLib.timeout_add(1500, x11_focus, win)
    loop.run()


if __name__ == "__main__":
    main()
