#!/usr/bin/env python3
# Windows 11 Settings for the Sway session. "ws-settings" opens it (or raises
# the open window); "ws-settings <page>" opens a page directly, for example
# "ws-settings display" or "ws-settings ms-settings:colors".
# "ws-settings --check-updates" is the background update check.
# "ws-settings --bing-wallpaper" downloads today's Bing picture when
# Windows spotlight is the background.
import fcntl
import os
import socket
import sys

SHARE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, SHARE)
RUNTIME = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
SOCK = os.path.join(RUNTIME, "ws-settings.sock")
LOCK = os.path.join(RUNTIME, "ws-settings.lock")


def requested_page():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    page = args[0] if args else ""
    return page.split(":", 1)[1] if page.startswith("ms-settings:") else page


def client():
    if "--check-updates" in sys.argv:
        import p_update
        sys.exit(p_update.background_check())
    if "--bing-wallpaper" in sys.argv:
        import bing
        sys.exit(0 if bing.refresh() else 1)
    if "--index" in sys.argv:
        return None
    page = requested_page() or "@raise"
    s = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
    try:
        s.sendto(page.encode(), SOCK)
        sys.exit(0)
    except OSError:
        pass
    finally:
        s.close()
    lock = open(LOCK, "w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        sys.exit(0)
    return lock


LOCK_FILE = client()

import wsui as ui  # noqa: E402
from gi.repository import Gdk, GLib, Gtk  # noqa: E402
import p_system, p_personal, p_apps, p_accounts, p_time, p_more, p_update, p_advanced  # noqa: E402,E401


class Page:
    def __init__(self, key, title, parent, icon, caption="", keywords="", build=None,
                 aliases=(), inline=False):
        self.key, self.title, self.parent, self.icon = key, title, parent, icon
        self.caption, self.keywords, self.build, self.aliases = caption, keywords, build, aliases
        self.inline = inline


CATEGORIES = [
    Page("home", "Home", None, "user-home", "Your most used settings"),
    Page("system", "System", None, "preferences-system",
         "Display, sound, notifications, power", aliases=("system",)),
    Page("personalization", "Personalization", None, "preferences-desktop-wallpaper",
         "Background, colours, themes, Start, taskbar"),
    Page("apps", "Apps", None, "view-app-grid", "Installed apps, default apps, startup",
         aliases=("appsfeatures",)),
    Page("accounts", "Accounts", None, "system-users", "Your info, sign-in options, other users",
         aliases=("yourinfo",)),
    Page("time", "Time & language", None, "preferences-system-time",
         "Date and time, language and region, typing", aliases=("time-language",)),
    Page("gaming", "Gaming", None, "applications-games", "Game Mode, captures"),
    Page("accessibility", "Accessibility", None, "preferences-desktop-accessibility",
         "Text size, visual effects, mouse pointer, contrast, keyboard", aliases=("easeofaccess",)),
    Page("privacy", "Privacy & security", None, "security-high",
         "Windows Security, camera, microphone, location, activity history"),
    Page("update", "Windows Update", None, "system-software-update",
         "Check for updates, update history", aliases=("windowsupdate",)),
    Page("advanced", "Advanced", None, "applications-system",
         "Every settings tool installed on this PC"),
]


def all_pages():
    pages = list(CATEGORIES)
    for module in (p_system, p_personal, p_apps, p_accounts, p_time, p_more, p_update, p_advanced):
        for cat in CATEGORIES:
            cat.build = getattr(module, "CATEGORY", {}).get(cat.key, cat.build)
        for spec in module.PAGES:
            pages.append(Page(**spec))
    return pages


class Settings:
    def __init__(self):
        self.pages = all_pages()
        self.by_key = {p.key: p for p in self.pages}
        self.history = []
        self.current = None
        self.cleanups = []
        self.build()
        self.listen()

    # Window

    def build(self):
        win = self.win = Gtk.Window(title="Settings")
        win.set_name("settings")
        area = Gdk.Display.get_default().get_monitor(0).get_workarea()
        win.set_default_size(min(1040, area.width - 64), min(700, area.height - 96))
        win.set_icon_name("preferences-system")
        win.connect("destroy", lambda *_: Gtk.main_quit())
        win.connect("key-press-event", self.on_key)
        root = Gtk.Box()
        win.add(root)

        nav = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        nav.set_size_request(300, -1)
        top = Gtk.Box()
        self.back = ui.css(Gtk.Button(tooltip_text="Back", relief=Gtk.ReliefStyle.NONE), "link")
        self.back.add(Gtk.Image.new_from_icon_name("go-previous-symbolic", Gtk.IconSize.MENU))
        self.back.connect("clicked", lambda *_: self.go_back())
        self.back.set_margin_start(12)
        self.back.set_margin_top(8)
        top.pack_start(self.back, False, False, 0)
        nav.pack_start(top, False, False, 0)

        account = ui.css(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "account")
        box = Gtk.Box(spacing=16)
        self.avatar = ui.Avatar(64)
        box.pack_start(self.avatar, False, False, 0)
        names = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
        self.account_name = ui.label(ui.user_name(), "account-name")
        names.pack_start(self.account_name, False, False, 0)
        names.pack_start(ui.label("Local account", "caption"), False, False, 0)
        box.pack_start(names, True, True, 0)
        account.add(box)
        account.connect("clicked", lambda *_: self.go("yourinfo"))
        nav.pack_start(account, False, False, 0)

        self.search = ui.css(Gtk.SearchEntry(placeholder_text="Find a setting"), "nav-search")
        self.search.connect("search-changed", self.on_search)
        self.search.connect("activate", self.on_search_activate)
        nav.pack_start(self.search, False, False, 0)

        scroll = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER, vexpand=True)
        self.nav = ui.css(Gtk.ListBox(selection_mode=Gtk.SelectionMode.SINGLE), "nav")
        for cat in CATEGORIES:
            r = Gtk.ListBoxRow()
            r.key = cat.key
            b = Gtk.Box(spacing=16)
            b.pack_start(ui.icon(cat.icon, 18), False, False, 0)
            b.pack_start(ui.label(cat.title), True, True, 0)
            if cat.key == "update":
                self.update_dot = ui.css(ui.label("\u25cf", "link"), "caption")
                b.pack_end(self.update_dot, False, False, 0)
            r.add(b)
            self.nav.add(r)
        self.nav.connect("row-activated", lambda _l, r: self.go(r.key, push=True))
        scroll.add(self.nav)
        nav.pack_start(scroll, True, True, 0)
        root.pack_start(nav, False, False, 0)

        state = p_update.load_state()
        self.update_dot.set_no_show_all(not (state.get("packages") or state.get("flatpaks")
                                             or os.path.exists("/system-update")))
        self.update_dot.set_tooltip_text("Updates are ready")

        right = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "content")
        self.title_row = ui.css(Gtk.Box(spacing=12), "title-row")
        right.pack_start(self.title_row, False, False, 0)
        self.scroll = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER, vexpand=True)
        self.holder = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.scroll.add(self.holder)
        right.pack_start(self.scroll, True, True, 0)
        root.pack_start(right, True, True, 0)
        win.show_all()

    # Navigation

    def resolve(self, key):
        key = (key or "").strip().lower()
        if key in self.by_key:
            return key
        for p in self.pages:
            if key in p.aliases or key == p.title.lower():
                return p.key
        return None

    def go(self, key, push=True, keep_scroll=False):
        key = self.resolve(key) or "home"
        page = self.by_key[key]
        if push and self.current and self.current != key:
            self.history.append(self.current)
        for cleanup in self.cleanups:
            try:
                cleanup()
            except Exception:
                pass
        self.cleanups = []
        self.current = key
        scroll = self.scroll.get_vadjustment().get_value() if keep_scroll else 0
        top = page
        while top.parent:
            top = self.by_key[top.parent]
        for r in self.nav.get_children():
            if r.key == top.key:
                self.nav.select_row(r)
        self.set_title(page)
        ui.clear(self.holder)
        body = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "page")
        body.set_size_request(560, -1)
        try:
            if page.build:
                page.build(self, body)
            self.add_children(page, body)
        except Exception as e:  # a broken page must not take the window down
            import traceback
            traceback.print_exc()
            body.pack_start(ui.infobar(f"This page could not be shown: {e}"), False, False, 0)
        self.holder.pack_start(body, False, False, 0)
        self.holder.show_all()
        self.back.set_sensitive(bool(self.history))
        GLib.idle_add(lambda: self.scroll.get_vadjustment().set_value(scroll) and False)

    def add_children(self, page, body):
        children = [p for p in self.pages if p.parent == page.key and not p.inline]
        if not children:
            return
        for child in children:
            body.pack_start(ui.nav_row(child.title, child.caption, child.icon,
                                       lambda k=child.key: self.go(k)), False, False, 0)

    def refresh(self):
        self.go(self.current, push=False, keep_scroll=True)

    def go_back(self):
        if self.history:
            self.go(self.history.pop(), push=False)

    def set_title(self, page):
        ui.clear(self.title_row)
        trail = []
        p = page
        while p:
            trail.insert(0, p)
            p = self.by_key.get(p.parent) if p.parent else None
        for i, p in enumerate(trail):
            last = i == len(trail) - 1
            if last:
                self.title_row.pack_start(ui.label(p.title, "title"), False, False, 0)
            else:
                b = ui.css(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "crumb")
                b.add(ui.label(p.title, "title-parent"))
                b.connect("clicked", lambda _b, k=p.key: self.go(k))
                self.title_row.pack_start(b, False, False, 0)
                self.title_row.pack_start(ui.label("\u203a", "title-parent"), False, False, 0)
        self.title_row.show_all()

    # Search

    def matches(self, text):
        q = text.casefold().strip()
        results = []
        for p in self.pages:
            hay = f"{p.title} {p.caption} {p.keywords}".casefold()
            if p.title.casefold().startswith(q):
                results.append((0, p))
            elif q in p.title.casefold():
                results.append((1, p))
            elif all(w in hay for w in q.split()):
                results.append((2, p))
        return [p for _s, p in sorted(results, key=lambda x: (x[0], x[1].title))]

    def on_search(self, entry):
        text = entry.get_text().strip()
        if not text:
            self.go(self.current or "home", push=False)
            return
        ui.clear(self.title_row)
        self.title_row.pack_start(ui.label(f"Search results for \u201c{text}\u201d", "title"),
                                  False, False, 0)
        self.title_row.show_all()
        ui.clear(self.holder)
        body = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "page")
        found = self.matches(text)
        if not found:
            body.pack_start(ui.label("No results. Try a different word, like \u201cdisplay\u201d "
                                     "or \u201cdark mode\u201d.", "caption"), False, False, 0)
        for p in found[:30]:
            parent = self.by_key.get(p.parent)
            caption = f"{parent.title} \u203a {p.title}" if parent else p.caption
            body.pack_start(ui.nav_row(p.title, caption, p.icon,
                                       lambda k=p.key: self.open_result(k)), False, False, 0)
        self.holder.pack_start(body, False, False, 0)
        self.holder.show_all()

    def on_search_activate(self, entry):
        found = self.matches(entry.get_text())
        if found:
            self.open_result(found[0].key)

    def open_result(self, key):
        self.search.handler_block_by_func(self.on_search)
        self.search.set_text("")
        self.search.handler_unblock_by_func(self.on_search)
        self.go(key)

    def on_key(self, _w, event):
        mods = event.state & Gtk.accelerator_get_default_mod_mask()
        if event.keyval == Gdk.KEY_Left and mods == Gdk.ModifierType.MOD1_MASK:
            self.go_back()
            return True
        if event.keyval == Gdk.KEY_f and mods == Gdk.ModifierType.CONTROL_MASK:
            self.search.grab_focus()
            return True
        if event.keyval == Gdk.KEY_Escape and self.search.get_text():
            self.search.set_text("")
            return True
        return False

    # Theme changes and other instances

    def restyle(self):
        ui.load_css()
        self.avatar.queue_draw()

    def listen(self):
        try:
            os.unlink(SOCK)
        except FileNotFoundError:
            pass
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
        self.sock.bind(SOCK)
        GLib.io_add_watch(self.sock.fileno(), GLib.PRIORITY_DEFAULT, GLib.IO_IN, self.on_command)

    def on_command(self, *_):
        page = self.sock.recv(256).decode(errors="ignore")
        if page != "@raise":
            self.go(page)
        self.win.present()
        return True


def main():
    GLib.set_prgname("ws-settings")
    GLib.set_application_name("Settings")
    ui.load_css()
    app = Settings()
    app.go(requested_page() or "home", push=False)
    try:
        Gtk.main()
    finally:
        try:
            os.unlink(SOCK)
        except OSError:
            pass


def index():
    """Settings pages for Start search, as JSON."""
    import json
    by_key = {p.key: p for p in all_pages()}
    rows = []
    for p in by_key.values():
        parent = by_key.get(p.parent)
        rows.append({"key": p.key, "title": p.title, "icon": p.icon,
                     "caption": p.caption or (parent.title if parent else "Settings"),
                     "keywords": " ".join([p.keywords, p.caption, parent.title if parent else ""]).strip()})
    print(json.dumps(rows, indent=1))


if __name__ == "__main__":
    index() if "--index" in sys.argv else main()
