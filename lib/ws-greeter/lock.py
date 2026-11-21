#!/usr/bin/env python3
"""ws-lockscreen: the Windows 11 lock screen on Sway's session-lock protocol.
"--demo" shows it in a normal window without locking (the password "demo" unlocks)."""

import ctypes
import json
import os
import pwd
import subprocess
import sys
import threading

ctypes.CDLL("libgtk4-layer-shell.so.0")
sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))

import gi  # noqa: E402

gi.require_version("Gtk4SessionLock", "1.0")
from gi.repository import Gtk4SessionLock  # noqa: E402

import pam  # noqa: E402
import sync  # noqa: E402
from signin import Gdk, GLib, Gtk, SignIn, battery_status, network_status  # noqa: E402

DEMO = "--demo" in sys.argv
RUNTIME = os.environ.get("XDG_RUNTIME_DIR") or "/tmp"
READY = os.path.join(RUNTIME, "ws-lockscreen.ready")
SETTINGS = os.path.expanduser("~/.config/ws-settings/settings.json")


def settings():
    try:
        with open(SETTINGS) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def account_icon(name):
    for path in (os.path.expanduser("~/.face"), f"/var/lib/AccountsService/icons/{name}"):
        if os.access(path, os.R_OK):
            return path
    return None


class Backend:
    def __init__(self, on_unlock):
        me = pwd.getpwuid(os.getuid())
        self.name = me.pw_name
        self.display = me.pw_gecos.split(",")[0] or me.pw_name
        self.on_unlock = on_unlock

    def users(self):
        return [{"name": self.name, "display": self.display, "image": account_icon(self.name), "logged_in": True}]

    def selected(self):
        return self.name

    def background(self, _name):
        s = settings()
        for path in (s.get("lock_image"), s.get("wallpaper")):
            if path and os.access(path, os.R_OK):
                return path
        return None

    def authenticate(self, name, password, done):
        def work():
            ok, _message = (password == "demo", "") if DEMO else pam.authenticate(name, password)
            GLib.idle_add(finish, ok)

        def finish(ok):
            if ok:
                self.on_unlock()
            else:
                done(False, "The password is incorrect. Try again.")
            return False

        threading.Thread(target=work, daemon=True).start()

    def power(self):
        def systemctl(*args):
            return lambda: subprocess.Popen(["systemctl", *args], start_new_session=True)

        def update_and(mode):
            cmd = ("gdbus call --system --dest org.freedesktop.PackageKit "
                   "--object-path /org/freedesktop/PackageKit "
                   "--method org.freedesktop.PackageKit.Offline.Trigger %s >/dev/null && "
                   "systemctl reboot" % mode)
            return lambda: subprocess.Popen(["sh", "-c", cmd], start_new_session=True)

        items = []
        if os.path.lexists("/system-update"):
            items += [("Update and restart", "system-reboot-symbolic", update_and("reboot")),
                      ("Update and shut down", "system-shutdown-symbolic", update_and("power-off"))]
        items += [("Sleep", "weather-clear-night-symbolic", systemctl("suspend")),
                  ("Shut down", "system-shutdown-symbolic", systemctl("poweroff")),
                  ("Restart", "system-reboot-symbolic", systemctl("reboot"))]
        return items

    def sessions(self):
        return []

    def session(self, _name):
        return None

    def set_session(self, _name, _key):
        pass

    def clock_24h(self, _name):
        return bool(settings().get("clock_24h"))

    def caps_hint(self):
        return settings().get("lock_caps_lock", True)

    network = staticmethod(network_status)
    battery = staticmethod(battery_status)


def scale_for(monitor):
    """GTK already applies the output scale; the lock page is laid out in logical pixels."""
    return 1.0


class Locker:
    def __init__(self):
        self.loop = GLib.MainLoop()
        self.backend = Backend(self.unlock)
        self.windows = []
        self.lock = None

    def window(self, monitor=None):
        geo = monitor.get_geometry() if monitor else None
        width, height = (geo.width, geo.height) if geo else (1280, 800)
        win = Gtk.Window(title="Lock screen")
        win.add_css_class("signin")
        win.set_default_size(width, height)
        win.set_child(SignIn(self.backend, width, height, scale_for(monitor), "lock"))
        self.windows.append(win)
        return win

    def on_monitor(self, lock, monitor):
        win = self.window(monitor)
        lock.assign_window_to_monitor(win, monitor)
        win.present()

    def on_locked(self, _lock):
        with open(READY, "w") as f:
            f.write(str(os.getpid()))

    def on_failed(self, _lock):
        print("ws-lockscreen: the compositor refused the lock", file=sys.stderr)
        self.quit(1)

    def unlock(self):
        if self.lock:
            self.lock.unlock()
            GLib.timeout_add(200, self.quit, 0)
        else:
            self.quit(0)

    def quit(self, code):
        self.code = code
        for win in self.windows:
            win.destroy()
        self.loop.quit()
        return False

    def run(self):
        self.code = 0
        if DEMO:
            win = self.window()
            win.connect("close-request", lambda *_: self.quit(0) or False)
            win.present()
            if "--signin" in sys.argv:
                win.get_child().show_signin()
        else:
            if not Gtk4SessionLock.is_supported():
                print("ws-lockscreen: session lock isn't supported here", file=sys.stderr)
                return 1
            self.lock = Gtk4SessionLock.Instance.new()
            self.lock.connect("monitor", self.on_monitor)
            self.lock.connect("locked", self.on_locked)
            self.lock.connect("failed", self.on_failed)
            if not self.lock.lock():
                return 1
        self.loop.run()
        try:
            os.unlink(READY)
        except OSError:
            pass
        return self.code


def main():
    if not DEMO:
        try:
            sync.sync()
        except Exception:
            pass
    Gtk.init()
    _ = Gdk.Display.get_default()
    sys.exit(Locker().run())


if __name__ == "__main__":
    main()
