#!/usr/bin/env python3
"""Fullscreen Fedora workstation installer using Anaconda's graphical widgets."""

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
CSS_PATH = HERE / "anaconda.css"
ANACONDA_CSS = Path("/usr/share/anaconda/anaconda-gtk.css")
BRAND = "MACKES"
PRODUCT = "Mackes Fedora Linux 2026"
BRAND_DIRS = (
    Path("/usr/local/share/fedora-ws-setup/brand"),
    HERE.parent / "assets" / "brand",
)
LOCALIZATION_NAME = "org.fedoraproject.Anaconda.Modules.Localization"
LOCALIZATION_PATH = "/org/fedoraproject/Anaconda/Modules/Localization"

DESKTOPS = (
    ("xfce", "_XFCE", "Classic desktop with a panel and the mm look. LightDM starts the session."),
    ("mde", "_MDE", "Mackes Desktop Environment 2026 on labwc — stacking windows, Win11-proportioned shell. Coexists with other desktops."),
    ("pipedal", "_PiPedal", "Console appliance with no display server. PiPedal is built from source and controlled from a phone or browser."),
)

DE_LABELS = {"xfce": "XFCE", "mde": "MDE", "pipedal": "PiPedal"}

MDE_PROFILES = (
    ("workstation", "_Workstation MDE", "Shell plus workstation baseline (codecs, firewalld, updates, plymouth)."),
    ("minimal", "_Minimal shell", "Compositor and MDE shell only."),
)

CAT_ICONS = {
    "recommended": "emblem-default-symbolic",
    "desktop polish": "preferences-desktop-wallpaper-symbolic",
    "devops": "applications-engineering-symbolic",
    "browsers": "web-browser-symbolic",
    "communication": "mail-unread-symbolic",
    "media": "multimedia-volume-control-symbolic",
    "creative": "applications-graphics-symbolic",
    "data science": "applications-science-symbolic",
    "gaming": "applications-games-symbolic",
    "security": "security-high-symbolic",
    "hardware": "computer-symbolic",
    "audio production": "audio-x-generic-symbolic",
}


def _brand_file(light=False):
    name = "mackes-light.svg" if light else "mackes.svg"
    for root in BRAND_DIRS:
        path = root / name
        if path.is_file():
            return path
    return None


def _brand_pixbuf(size, light=False):
    path = _brand_file(light)
    if path is None:
        return None
    from gi.repository import GdkPixbuf, GLib
    try:
        return GdkPixbuf.Pixbuf.new_from_file_at_scale(str(path), size, size, True)
    except GLib.Error:
        return None


def _read(path):
    return Path(path).read_text(encoding="utf-8")


def _strip(path):
    return _read(path).strip()


def compile_model(src, dest):
    src = Path(src)
    users = [ln for ln in _read(src / "users.txt").splitlines() if ln]
    desktops = {}
    categories = {}
    for de in ("xfce", "mde", "pipedal"):
        flag = _strip(src / f"switch-{de}.flag") == "1"
        desktops[de] = {
            "base": _read(src / f"base-{de}.txt").rstrip(),
            "switch": flag,
            "switch_text": _read(src / f"switch-{de}.txt").rstrip(),
        }
        cats = []
        folder = src / f"cats-{de}"
        if folder.is_dir():
            indexes = sorted(
                {p.name.split(".", 1)[0] for p in folder.iterdir() if p.suffix in {".title", ".args"}},
                key=lambda n: int(n) if n.isdigit() else n,
            )
            for idx in indexes:
                title = _strip(folder / f"{idx}.title")
                lines = _read(folder / f"{idx}.args").splitlines()
                items = []
                for i in range(0, len(lines) - 2, 3):
                    item_id, label, status = lines[i], lines[i + 1], lines[i + 2]
                    if not item_id:
                        continue
                    items.append({"id": item_id, "label": label, "on": status == "ON"})
                if items:
                    cats.append({"title": title, "key": f"cat:{de}:{idx}", "items": items})
        categories[de] = cats
    model = {
        "dry_run": _strip(src / "dry.txt") == "1",
        "log": _strip(src / "log.txt"),
        "fedora": _strip(src / "fedora.txt"),
        "default_de": _strip(src / "default_de.txt") or "xfce",
        "users": users,
        "preflight": _read(src / "preflight.txt").rstrip(),
        "force_surface": _strip(src / "force_surface.txt"),
        "no_surface": _strip(src / "no_surface.txt") == "1",
        "simulate_surface": _strip(src / "simulate_surface.txt"),
        "desktops": desktops,
        "categories": categories,
    }
    if model["default_de"] not in ("xfce", "mde", "pipedal"):
        model["default_de"] = "xfce"
    model["mde_profile"] = "workstation"
    text = json.dumps(model, indent=2)
    if str(dest) == "-":
        sys.stdout.write(text)
    else:
        Path(dest).write_text(text, encoding="utf-8")


def serve_bus(address):
    from dbus.mainloop.glib import DBusGMainLoop
    from gi.repository import GLib
    import dbus
    import dbus.service

    DBusGMainLoop(set_as_default=True)
    bus = dbus.bus.BusConnection(address)
    # Keep the well-known name alive for the process lifetime. Dropping the
    # BusName object releases it and Anaconda's layout widget cannot connect.
    owner = dbus.service.BusName(LOCALIZATION_NAME, bus)

    class Localization(dbus.service.Object):
        def __init__(self):
            dbus.service.Object.__init__(self, bus, LOCALIZATION_PATH)
            self.owner = owner

        @dbus.service.method(LOCALIZATION_NAME, out_signature="s")
        def GetCompositorSelectedLayout(self):
            return "us"

        @dbus.service.method(LOCALIZATION_NAME, out_signature="as")
        def GetCompositorLayouts(self):
            return ["us"]

    Localization()
    sys.stdout.write("ready\n")
    sys.stdout.flush()
    GLib.MainLoop().run()


def _mnemonic(title):
    return f"_{title}" if title else title


def _find_child(widget, name):
    if widget.get_name() == name:
        return widget
    getter = getattr(widget, "get_children", None)
    if getter is None:
        return None
    for child in getter():
        found = _find_child(child, name)
        if found is not None:
            return found
    return None


class Installer:
    def __init__(self, model, script, screenshot=None):
        self.model = model
        self.script = script
        self.screenshot = screenshot
        self.auto_accept = screenshot is not None
        self.de = model["default_de"]
        if self.de not in ("xfce", "mde", "pipedal"):
            self.de = "xfce"
        self.mde_profile = model.get("mde_profile", "workstation")
        self.users = set(model["users"])
        self.picks = {}
        for de, cats in model["categories"].items():
            chosen = set()
            for cat in cats:
                for item in cat["items"]:
                    if item["on"]:
                        chosen.add(item["id"])
            self.picks[de] = chosen
        self.switch_ok = False
        self.selectors = {}
        self.proc = None
        self.result = None
        self._finished = False
        self._log_pos = 0
        self._progress_pos = 0
        self._phases_done = 0
        self._phase_total = 9
        self._bus = None
        self._radios = {}
        self._checks = []

    def run(self):
        self._start_bus()
        import warnings

        warnings.filterwarnings("ignore", category=DeprecationWarning)
        import gi

        gi.require_version("Gdk", "3.0")
        gi.require_version("Gtk", "3.0")
        gi.require_version("AnacondaWidgets", "3.4")
        from gi.repository import Gdk, Gtk, AnacondaWidgets

        self.Gtk = Gtk
        self.Gdk = Gdk
        self.AW = AnacondaWidgets
        Gtk.init([])
        self._load_css()
        self.win = Gtk.Window(decorated=False)
        self.win.set_title("Mackes Workstation Setup")
        self.win.set_icon_name("org.fedoraproject.AnacondaInstaller")
        self.win.set_default_size(1600, 900)
        self.stack = Gtk.Stack()
        self.stack.set_transition_type(Gtk.StackTransitionType.SLIDE_LEFT)
        self.stack.set_transition_duration(0 if self.screenshot else 250)
        self.stack.set_hexpand(True)
        self.stack.set_vexpand(True)
        self.win.add(self.stack)
        self.win.connect("delete-event", self._on_delete)
        self._build_welcome()
        self._build_hub()
        self._build_progress()
        self._show("welcome")
        if self.screenshot:
            self._exercise()
            return 0
        if not self.model["dry_run"]:
            self.win.fullscreen()
        else:
            self.win.fullscreen()
        self.win.show_all()
        Gtk.main()
        return 0

    def _start_bus(self):
        addr = subprocess.check_output(
            ["dbus-daemon", "--session", "--print-address", "--fork", "--nopidfile"],
            text=True,
        ).strip()
        proc = subprocess.Popen(
            [sys.executable, str(Path(__file__).resolve()), "--serve-bus", addr],
            stdout=subprocess.PIPE,
            text=True,
        )
        line = proc.stdout.readline().strip() if proc.stdout else ""
        if line != "ready":
            proc.kill()
            raise RuntimeError("Anaconda localization bus did not start")
        os.environ["DBUS_ANACONDA_SESSION_BUS_ADDRESS"] = addr
        self._bus = proc

    def _load_css(self):
        provider = self.Gtk.CssProvider()
        css = ""
        if ANACONDA_CSS.is_file():
            css += ANACONDA_CSS.read_text(encoding="utf-8") + "\n"
        if CSS_PATH.is_file():
            css += CSS_PATH.read_text(encoding="utf-8")
        provider.load_from_data(css.encode("utf-8"))
        self.Gtk.StyleContext.add_provider_for_screen(
            self.Gdk.Screen.get_default(),
            provider,
            self.Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION,
        )

    def _chrome(self, kind, title):
        if kind == "spoke":
            page = self.AW.SpokeWindow()
        else:
            page = self.AW.StandaloneWindow()
        page.set_property("distribution", BRAND)
        page.set_property("window-name", title)
        page.set_hexpand(True)
        page.set_vexpand(True)
        return page

    def _paint_nav(self, page):
        nav = page.get_nav_area()
        box = nav.get_parent() if nav is not None else None
        if box is None:
            return
        color = self.Gdk.RGBA()
        color.parse("#2f4265")
        box.override_background_color(self.Gtk.StateFlags.NORMAL, color)
        for name in ("anaconda-name-label", "anaconda-distro-label"):
            label = _find_child(page, name)
            if label is not None and hasattr(label, "override_color"):
                white = self.Gdk.RGBA()
                white.parse("white")
                label.override_color(self.Gtk.StateFlags.NORMAL, white)
        indicator = _find_child(page, "layout-indicator")
        if indicator is not None:
            indicator.set_no_show_all(True)
            indicator.hide()
        beta = _find_child(page, "anaconda-beta-label")
        if beta is not None:
            beta.set_text("NOT FOR RELEASE")
            beta.set_no_show_all(False)
            beta.show()
        nav = page.get_nav_area()
        if nav is not None and not getattr(page, "_mackes_logo", False):
            pix = _brand_pixbuf(28, light=True)
            if pix is not None and hasattr(nav, "insert_column"):
                nav.insert_column(0)
                img = self.Gtk.Image.new_from_pixbuf(pix)
                img.set_margin_start(16)
                img.set_margin_end(8)
                img.set_halign(self.Gtk.Align.CENTER)
                img.set_valign(self.Gtk.Align.CENTER)
                nav.attach(img, 0, 0, 1, 3)
                img.show()
                page._mackes_logo = True

    def _use_standalone_buttons(self, page, continue_label, continue_cb, quit_cb):
        quit_btn = page.get_property("quit-button")
        cont = page.get_property("continue-button")
        quit_btn.connect("clicked", lambda *_: quit_cb())
        cont.connect("clicked", lambda *_: continue_cb())
        cont.set_label(continue_label)
        cont.set_use_underline(True)
        return quit_btn, cont

    def _show(self, name):
        page = self.stack.get_child_by_name(name)
        if page is not None:
            page.show_all()
            self._paint_nav(page)
        self.stack.set_visible_child_name(name)

    def _build_welcome(self):
        page = self._chrome("standalone", "WELCOME")
        action = page.get_action_area()
        pix = _brand_pixbuf(72)
        if pix is not None:
            logo = self.Gtk.Image.new_from_pixbuf(pix)
            logo.set_halign(self.Gtk.Align.CENTER)
            logo.set_margin_bottom(12)
            action.pack_start(logo, False, False, 0)
        body = self.Gtk.Label()
        body.set_markup(
            f"<span size='xx-large' weight='bold'>{PRODUCT}</span>\n"
            "<span weight='bold'>NOT FOR RELEASE</span>\n\n"
            "This installer turns a base Fedora system into one workstation desktop,\n"
            "or into a headless PiPedal appliance.\n"
            "Pick XFCE, Sway, or PiPedal, then choose optional upgrades.\n\n"
            "The look, codecs, and LightDM apply to the graphical desktops.\n"
            "PiPedal removes the display server and is reached from a browser.\n"
            "Nothing is installed until you click <b>Begin Installation</b>."
        )
        body.set_justify(self.Gtk.Justification.CENTER)
        body.set_halign(self.Gtk.Align.CENTER)
        body.set_valign(self.Gtk.Align.CENTER)
        body.set_line_wrap(True)
        body.set_max_width_chars(64)
        body.get_style_context().add_class("ws-body")
        action.pack_start(body, True, True, 0)
        self._use_standalone_buttons(page, "_Continue", self._go_hub, self._quit)
        self.stack.add_named(page, "welcome")

    def _button_row(self, page, left_label, left_cb, right_label, right_cb, begin=True):
        row = self.Gtk.Box(orientation=self.Gtk.Orientation.HORIZONTAL, spacing=12)
        row.set_margin_start(12)
        row.set_margin_end(12)
        row.set_margin_top(6)
        row.set_margin_bottom(12)
        left = self.Gtk.Button.new_with_mnemonic(left_label)
        right = self.Gtk.Button.new_with_mnemonic(right_label)
        left.connect("clicked", lambda *_: left_cb())
        right.connect("clicked", lambda *_: right_cb())
        right.get_style_context().add_class("suggested-action")
        left.set_size_request(140, 36)
        right.set_size_request(220 if begin else 160, 40)
        row.pack_start(left, False, False, 0)
        row.pack_end(right, False, False, 0)
        return row

    def _go_hub(self):
        self._rebuild_grid()
        self._refresh_statuses()
        self._show("hub")

    def _build_hub(self):
        hub = self.AW.HubWindow()
        hub.set_property("distribution", BRAND)
        hub.set_property("window-name", "INSTALLATION SUMMARY")
        hub.set_hexpand(True)
        hub.set_vexpand(True)
        self.hub = hub
        if self.model["dry_run"]:
            hub.set_warning("Dry run. Begin Installation will not change this system.")
        note = self.Gtk.Label(label="Nothing is installed until you click Begin Installation.")
        note.set_halign(self.Gtk.Align.END)
        note.set_margin_end(18)
        note.get_style_context().add_class("ws-note")
        buttons = self._button_row(hub, "_Quit", self._quit, "_Begin Installation", self._begin)
        main = hub.get_main_box()
        main.pack_end(buttons, False, False, 0)
        main.pack_end(note, False, False, 0)
        self.stack.add_named(hub, "hub")
        self._rebuild_grid()
        self._refresh_statuses()

    def _rebuild_grid(self):
        area = self.hub.get_spoke_area()
        child = area.get_child()
        if child is not None:
            area.remove(child)
        grid = self.Gtk.Grid(
            row_spacing=18,
            column_spacing=28,
            column_homogeneous=True,
            row_homogeneous=False,
            margin_top=12,
            margin_bottom=12,
            margin_start=18,
            margin_end=18,
            halign=self.Gtk.Align.CENTER,
            valign=self.Gtk.Align.CENTER,
        )
        columns = [
            ("SYSTEM", self._system_specs()),
            ("SOFTWARE", self._software_specs()[0::2]),
            ("SOFTWARE", self._software_specs()[1::2]),
        ]
        # Drop an empty third column when there is only one software spoke.
        if not columns[2][1]:
            columns = columns[:2]
        for col, (heading, specs) in enumerate(columns):
            if not specs:
                continue
            show_heading = col == 0 or heading != columns[col - 1][0]
            label = self.Gtk.Label()
            if show_heading:
                label.set_markup(f"<span size='larger' weight='bold'>{heading}</span>")
            label.set_halign(self.Gtk.Align.START)
            label.set_margin_bottom(6)
            grid.attach(label, col, 0, 1, 1)
            for row, spec in enumerate(specs, start=1):
                selector = self._selector(spec["key"], spec["title"], spec["icon"])
                grid.attach(selector, col, row, 1, 1)
        viewport = self.Gtk.Viewport()
        viewport.add(grid)
        viewport.set_shadow_type(self.Gtk.ShadowType.NONE)
        area.add(viewport)
        self._refresh_statuses()

    def _system_specs(self):
        return (
            {"key": "preflight", "title": _mnemonic("Preflight"), "icon": "dialog-information-symbolic"},
            {"key": "desktop", "title": _mnemonic("Desktop"), "icon": "preferences-desktop-display-symbolic"},
            {"key": "users", "title": _mnemonic("Users"), "icon": "system-users-symbolic"},
            {"key": "base", "title": _mnemonic("Base plan"), "icon": "package-x-generic-symbolic"},
        )

    def _software_specs(self):
        specs = []
        for cat in self.model["categories"].get(self.de, []):
            icon = CAT_ICONS.get(cat["title"].lower(), "package-x-generic-symbolic")
            specs.append({"key": cat["key"], "title": _mnemonic(cat["title"]), "icon": icon})
        return specs

    def _selector(self, key, title, icon):
        selector = self.AW.SpokeSelector(title=title, icon=icon, status="…")
        selector.connect("button-press-event", self._on_selector_click, key)
        selector.connect("key-release-event", self._on_selector_key, key)
        self.selectors[key] = selector
        return selector

    def _on_selector_click(self, _selector, _event, key):
        self._open_spoke(key)
        return True

    def _on_selector_key(self, _selector, event, key):
        if event.keyval in (self.Gdk.KEY_Return, self.Gdk.KEY_space, self.Gdk.KEY_KP_Enter):
            self._open_spoke(key)
            return True
        return False

    def _refresh_statuses(self):
        de_name = DE_LABELS.get(self.de, self.de)
        if self.de == "pipedal":
            base_status = "Removes the display server" if self._switching() else "Console appliance"
        else:
            base_status = "Removes the other desktop" if self._switching() else "Codecs, look, hardening"
        mapping = {
            "preflight": f"Fedora {self.model['fedora']}",
            "desktop": de_name,
            "users": self._user_status(),
            "base": base_status,
        }
        for key, status in mapping.items():
            widget = self.selectors.get(key)
            if widget is not None:
                widget.set_property("status", status)
        for cat in self.model["categories"].get(self.de, []):
            widget = self.selectors.get(cat["key"])
            if widget is None:
                continue
            picked = sum(1 for item in cat["items"] if item["id"] in self.picks[self.de])
            widget.set_property("status", f"{picked} of {len(cat['items'])} selected")

    def _user_status(self):
        names = [user for user in self.model["users"] if user in self.users]
        if not names:
            return "Look skipped"
        if len(names) == 1:
            return names[0]
        return f"{len(names)} users"

    def _switching(self):
        return bool(self.model["desktops"][self.de]["switch"])

    def _open_spoke(self, key):
        old = self.stack.get_child_by_name("spoke")
        if old is not None:
            self.stack.remove(old)
        page = self._make_spoke(key)
        self.stack.add_named(page, "spoke")
        done = _find_child(page, "anaconda-spoke-window-done")
        if done is not None:
            done.connect("clicked", lambda *_: self._close_spoke(key))
        else:
            page.connect("button-clicked", lambda *_: self._close_spoke(key))
        self._show("spoke")

    def _make_spoke(self, key):
        if key == "preflight":
            return self._text_spoke("PREFLIGHT", self.model["preflight"])
        if key == "base":
            text = self.model["desktops"][self.de]["base"]
            if self._switching():
                text += "\n\n" + self.model["desktops"][self.de]["switch_text"]
            return self._text_spoke("BASE PLAN", text)
        if key == "desktop":
            return self._desktop_spoke()
        if key == "users":
            return self._users_spoke()
        for cat in self.model["categories"].get(self.de, []):
            if cat["key"] == key:
                return self._check_spoke(cat)
        return self._text_spoke("SPOKE", "This spoke is not available.")

    def _text_spoke(self, title, text):
        page = self._chrome("spoke", title)
        scroll = self.Gtk.ScrolledWindow()
        scroll.set_policy(self.Gtk.PolicyType.NEVER, self.Gtk.PolicyType.AUTOMATIC)
        label = self.Gtk.Label(label=text)
        label.set_line_wrap(True)
        label.set_max_width_chars(88)
        label.set_halign(self.Gtk.Align.START)
        label.set_valign(self.Gtk.Align.START)
        label.set_margin_start(24)
        label.set_margin_end(24)
        label.set_margin_top(12)
        label.set_selectable(True)
        label.set_xalign(0)
        label.get_style_context().add_class("ws-body")
        scroll.add(label)
        page.get_action_area().pack_start(scroll, True, True, 0)
        return page

    def _desktop_spoke(self):
        page = self._chrome("spoke", "DESKTOP")
        box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=16)
        box.set_margin_start(28)
        box.set_margin_end(28)
        box.set_margin_top(16)
        box.set_halign(self.Gtk.Align.START)
        box.set_valign(self.Gtk.Align.START)
        hint = self.Gtk.Label(
            label="MDE coexists with other desktops. PiPedal removes the display server. XFCE may remove MDE packages when switching."
        )
        hint.set_halign(self.Gtk.Align.START)
        hint.get_style_context().add_class("ws-hint")
        box.pack_start(hint, False, False, 0)
        group = None
        self._radios = {}
        for de, title, blurb in DESKTOPS:
            radio = self.Gtk.RadioButton.new_with_mnemonic_from_widget(group, title)
            group = radio
            radio.set_active(de == self.de)
            self._radios[de] = radio
            desc = self.Gtk.Label(label=blurb)
            desc.set_halign(self.Gtk.Align.START)
            desc.set_margin_start(28)
            desc.set_line_wrap(True)
            desc.set_max_width_chars(72)
            desc.set_xalign(0)
            box.pack_start(radio, False, False, 0)
            box.pack_start(desc, False, False, 0)
        # MDE profile: Minimal shell vs Workstation
        self._mde_profile_box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=8)
        self._mde_profile_box.set_margin_top(12)
        ph = self.Gtk.Label(label="MDE profile")
        ph.set_halign(self.Gtk.Align.START)
        ph.get_style_context().add_class("ws-hint")
        self._mde_profile_box.pack_start(ph, False, False, 0)
        self._profile_radios = {}
        pgroup = None
        cur_profile = getattr(self, "mde_profile", None) or self.model.get("mde_profile", "workstation")
        for key, title, blurb in MDE_PROFILES:
            radio = self.Gtk.RadioButton.new_with_mnemonic_from_widget(pgroup, title)
            pgroup = radio
            radio.set_active(key == cur_profile)
            self._profile_radios[key] = radio
            desc = self.Gtk.Label(label=blurb)
            desc.set_halign(self.Gtk.Align.START)
            desc.set_margin_start(28)
            desc.set_line_wrap(True)
            desc.set_xalign(0)
            self._mde_profile_box.pack_start(radio, False, False, 0)
            self._mde_profile_box.pack_start(desc, False, False, 0)
        box.pack_start(self._mde_profile_box, False, False, 0)
        for de, radio in self._radios.items():
            radio.connect("toggled", self._on_de_toggled)
        self._on_de_toggled(None)
        page.get_action_area().pack_start(box, True, True, 0)
        return page

    def _on_de_toggled(self, _widget):
        show = False
        for de, radio in getattr(self, "_radios", {}).items():
            if radio.get_active() and de == "mde":
                show = True
                break
        box = getattr(self, "_mde_profile_box", None)
        if box is not None:
            box.set_visible(show)

    def _users_spoke(self):
        page = self._chrome("spoke", "USERS")
        box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=8)
        box.set_margin_start(28)
        box.set_margin_top(16)
        box.set_halign(self.Gtk.Align.START)
        box.set_valign(self.Gtk.Align.START)
        if self.de == "pipedal":
            hint_label = "These users can sign in on the console and over SSH. PiPedal runs as its own service account."
        else:
            look = "MDE" if self.de == "mde" else "XFCE"
            hint_label = f"Apply the {look} look for these users."
        hint = self.Gtk.Label(label=hint_label)
        hint.set_halign(self.Gtk.Align.START)
        hint.get_style_context().add_class("ws-hint")
        box.pack_start(hint, False, False, 0)
        if not self.model["users"]:
            empty = self.Gtk.Label(label="No local users were found. Theme settings will be skipped.")
            empty.set_halign(self.Gtk.Align.START)
            box.pack_start(empty, False, False, 0)
        for user in self.model["users"]:
            check = self.Gtk.CheckButton.new_with_label(user)
            check.set_active(user in self.users)
            check.connect("toggled", self._toggle_user, user)
            box.pack_start(check, False, False, 0)
        page.get_action_area().pack_start(box, True, True, 0)
        return page

    def _check_spoke(self, cat):
        page = self._chrome("spoke", cat["title"].upper())
        outer = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=8)
        hint_text = "Recommended items are selected. Uncheck anything you do not want."
        if cat["title"].lower() != "recommended":
            hint_text = "Optional. Selected items are installed after Begin Installation."
        hint = self.Gtk.Label(label=hint_text)
        hint.set_halign(self.Gtk.Align.START)
        hint.set_margin_start(24)
        hint.set_margin_top(12)
        hint.get_style_context().add_class("ws-hint")
        outer.pack_start(hint, False, False, 0)
        scroll = self.Gtk.ScrolledWindow()
        scroll.set_policy(self.Gtk.PolicyType.NEVER, self.Gtk.PolicyType.AUTOMATIC)
        box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=4)
        box.set_margin_start(24)
        box.set_margin_end(24)
        box.set_margin_bottom(12)
        for item in cat["items"]:
            check = self.Gtk.CheckButton.new_with_label(item["label"])
            check.set_active(item["id"] in self.picks[self.de])
            check.connect("toggled", self._toggle_item, item["id"])
            box.pack_start(check, False, False, 0)
        scroll.add(box)
        outer.pack_start(scroll, True, True, 0)
        page.get_action_area().pack_start(outer, True, True, 0)
        return page

    def _toggle_user(self, check, user):
        if check.get_active():
            self.users.add(user)
        else:
            self.users.discard(user)

    def _toggle_item(self, check, item_id):
        if check.get_active():
            self.picks[self.de].add(item_id)
        else:
            self.picks[self.de].discard(item_id)

    def _selected_radio(self):
        for de, radio in self._radios.items():
            if radio.get_active():
                return de
        return self.de

    def _close_spoke(self, key):
        if key == "desktop":
            new = self._selected_radio()
            previous = self.de
            if new != previous:
                self.switch_ok = False
                self.de = new
            if self._switching() and not self.switch_ok:
                if not self._confirm_switch():
                    self.de = previous
                    radio = self._radios.get(previous)
                    if radio is not None:
                        radio.set_active(True)
                    return
                self.switch_ok = True
            if new != previous:
                self._rebuild_grid()
        self._refresh_statuses()
        self._show("hub")

    def _confirm_switch(self):
        if self.auto_accept:
            return True
        text = self.model["desktops"][self.de]["switch_text"] or "The other desktop will be removed."
        dialog = self.Gtk.MessageDialog(
            transient_for=self.win,
            modal=True,
            message_type=self.Gtk.MessageType.WARNING,
            buttons=self.Gtk.ButtonsType.NONE,
            text="Remove the graphical desktop?" if self.de == "pipedal" else "Remove the other desktop?",
        )
        dialog.format_secondary_text(text)
        dialog.add_button("_Back", self.Gtk.ResponseType.CANCEL)
        dialog.add_button("_Remove and continue", self.Gtk.ResponseType.OK)
        response = dialog.run()
        dialog.destroy()
        return response == self.Gtk.ResponseType.OK

    def _begin(self):
        if self._switching() and not self.switch_ok:
            if not self._confirm_switch():
                return
            self.switch_ok = True
        self._start_apply()

    def _start_apply(self):
        work = Path(self.model["log"]).parent / f".ui-{os.getpid()}"
        work.mkdir(parents=True, exist_ok=True)
        choices = work / "choices.json"
        self.progress_path = work / "progress"
        self.result_path = work / "result.json"
        self.progress_path.write_text("", encoding="utf-8")
        names = [user for user in self.model["users"] if user in self.users]
        profile = self.mde_profile
        for key, radio in getattr(self, "_profile_radios", {}).items():
            if radio.get_active():
                profile = key
                break
        self.mde_profile = profile
        payload = {
            "de": self.de,
            "mde_profile": profile,
            "users": names,
            "items": sorted(self.picks.get(self.de, [])),
        }
        choices.write_text(json.dumps(payload), encoding="utf-8")
        cmd = [self.script, f"--apply-choices={choices}"]
        if self.model["dry_run"]:
            cmd.append("--dry-run")
        if self.model["no_surface"]:
            cmd.append("--no-surface")
        if self.model["force_surface"]:
            cmd.append(f"--force-surface={self.model['force_surface']}")
        if self.model["simulate_surface"]:
            cmd.append(f"--simulate-surface={self.model['simulate_surface']}")
        env = os.environ.copy()
        env["WS_LOG_FILE"] = self.model["log"]
        env["WS_LOG_QUIET"] = "1"
        env["WS_PROGRESS_FILE"] = str(self.progress_path)
        env["WS_RESULT_FILE"] = str(self.result_path)
        logf = open(self.model["log"], "a", encoding="utf-8")
        self.proc = subprocess.Popen(
            cmd,
            env=env,
            stdout=logf,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
        )
        self._log_pos = 0
        self._progress_pos = 0
        self._phases_done = 0
        self.phase_label.set_text("Starting installation")
        self.bar.set_fraction(0)
        self.spinner.start()
        self._show("progress")
        from gi.repository import GLib

        self._poll_id = GLib.timeout_add(200, self._poll)

    def _build_progress(self):
        page = self._chrome("standalone", "INSTALLATION PROGRESS")
        self.progress_page = page
        action = page.get_action_area()
        box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=10)
        box.set_margin_start(48)
        box.set_margin_end(48)
        box.set_margin_top(24)
        box.set_valign(self.Gtk.Align.FILL)
        row = self.Gtk.Box(spacing=12)
        self.spinner = self.Gtk.Spinner()
        self.phase_label = self.Gtk.Label(label="Preparing", halign=self.Gtk.Align.START, hexpand=True)
        self.phase_label.set_xalign(0)
        row.pack_start(self.spinner, False, False, 0)
        row.pack_start(self.phase_label, True, True, 0)
        self.bar = self.Gtk.ProgressBar(show_text=True)
        self.summary = self.Gtk.Label(label="")
        self.summary.set_halign(self.Gtk.Align.START)
        self.summary.set_xalign(0)
        self.summary.set_line_wrap(True)
        self.summary.set_max_width_chars(90)
        self.summary.hide()
        scroll = self.Gtk.ScrolledWindow()
        scroll.set_policy(self.Gtk.PolicyType.AUTOMATIC, self.Gtk.PolicyType.AUTOMATIC)
        scroll.set_vexpand(True)
        scroll.set_min_content_height(220)
        self.logview = self.Gtk.TextView(editable=False, cursor_visible=False, wrap_mode=self.Gtk.WrapMode.WORD_CHAR)
        self.logview.get_style_context().add_class("ws-log")
        scroll.add(self.logview)
        box.pack_start(row, False, False, 0)
        box.pack_start(self.bar, False, False, 0)
        box.pack_start(self.summary, False, False, 0)
        box.pack_start(scroll, True, True, 0)
        action.pack_start(box, True, True, 0)
        self.progress_quit, self.reboot_btn = self._use_standalone_buttons(
            page, "_Reboot", self._reboot, self._quit
        )
        self.reboot_btn.set_no_show_all(True)
        self.reboot_btn.hide()
        self.stack.add_named(page, "progress")

    def _poll(self):
        self._read_progress()
        self._read_log()
        if self.proc is None:
            return False
        code = self.proc.poll()
        if code is None:
            return True
        self._read_progress()
        self._read_log()
        self._finish(code)
        return False

    def _read_progress(self):
        path = getattr(self, "progress_path", None)
        if path is None or not path.exists():
            return
        data = path.read_text(encoding="utf-8", errors="replace")
        chunk = data[self._progress_pos :]
        self._progress_pos = len(data)
        for line in chunk.splitlines():
            parts = line.split("\t")
            if not parts:
                continue
            if parts[0] == "PHASE" and len(parts) >= 4:
                n, total, title = parts[1], parts[2], parts[3]
                try:
                    self._phase_total = max(int(total), 1)
                    frac = max(0, int(n) - 1) / self._phase_total
                except ValueError:
                    frac = 0
                self.bar.set_fraction(min(frac, 1))
                self.phase_label.set_text(title)
            elif parts[0] == "DONE" and len(parts) >= 3:
                self._phases_done += 1
                frac = self._phases_done / max(self._phase_total, 1)
                self.bar.set_fraction(min(frac, 1))

    def _read_log(self):
        path = Path(self.model["log"])
        if not path.exists():
            return
        with path.open("r", encoding="utf-8", errors="replace") as handle:
            handle.seek(self._log_pos)
            chunk = handle.read()
            self._log_pos = handle.tell()
        if not chunk:
            return
        buf = self.logview.get_buffer()
        buf.insert(buf.get_end_iter(), chunk)
        # Keep the view on the latest lines without holding the whole install log in the widget.
        text = buf.get_text(buf.get_start_iter(), buf.get_end_iter(), True)
        lines = text.splitlines()
        if len(lines) > 200:
            buf.set_text("\n".join(lines[-200:]) + "\n")
        end = buf.get_end_iter()
        mark = buf.create_mark(None, end, False)
        self.logview.scroll_to_mark(mark, 0, True, 0, 1)

    def _finish(self, code):
        if self._finished:
            return
        self._finished = True
        self.spinner.stop()
        self.bar.set_fraction(1)
        result = {}
        if self.result_path.exists():
            try:
                result = json.loads(self.result_path.read_text(encoding="utf-8"))
            except json.JSONDecodeError:
                result = {}
        self.result = result
        failed = result.get("failed", 0 if code == 0 else 1)
        lines = [
            "Dry run finished. No changes were made." if self.model["dry_run"] else "Setup finished.",
            f"Desktop: {result.get('desktop', self.de)}",
            f"Applied: {result.get('applied', 0)}    Skipped: {result.get('skipped', 0)}    Failed: {failed}",
        ]
        failures = result.get("failures") or []
        if failures:
            lines.append("Failures: " + ", ".join(failures))
        if code not in (0, None) and not result:
            lines.append(f"The installer process exited with status {code}.")
        mok = (result.get("mok") or "").strip()
        if mok:
            lines.append(mok)
        lines.append(f"Log: {self.model['log']}")
        self.summary.set_text("\n".join(lines))
        self.summary.show()
        self.phase_label.set_text("Complete" if failed == 0 and code == 0 else "Finished with problems")
        self.progress_page.set_property("window-name", "INSTALLATION COMPLETE")
        self._paint_nav(self.progress_page)
        if self.model["dry_run"] or failed:
            self.reboot_btn.hide()
        else:
            self.reboot_btn.set_no_show_all(False)
            self.reboot_btn.show()
        self._show("progress")

    def _reboot(self):
        subprocess.Popen(["systemctl", "reboot"], stdin=subprocess.DEVNULL)
        self._exit()

    def _quit(self):
        if self.proc is not None and self.proc.poll() is None:
            return
        if self.auto_accept or self._finished:
            self._exit()
            return
        dialog = self.Gtk.MessageDialog(
            transient_for=self.win,
            modal=True,
            message_type=self.Gtk.MessageType.QUESTION,
            buttons=self.Gtk.ButtonsType.NONE,
            text="Quit the workstation setup?",
        )
        dialog.format_secondary_text("Nothing is installed until you click Begin Installation.")
        dialog.add_button("_Back", self.Gtk.ResponseType.CANCEL)
        dialog.add_button("_Quit", self.Gtk.ResponseType.OK)
        response = dialog.run()
        dialog.destroy()
        if response == self.Gtk.ResponseType.OK:
            self._exit()

    def _on_delete(self, *_args):
        self._quit()
        return True

    def _exit(self):
        if self._bus is not None and self._bus.poll() is None:
            self._bus.terminate()
        if not self.screenshot:
            self.Gtk.main_quit()

    def _flush(self, rounds=25):
        from gi.repository import GLib

        context = GLib.MainContext.default()
        self.win.show_all()
        for _ in range(rounds):
            while context.pending():
                context.iteration(False)

    def _shot(self, name):
        page = self.stack.get_visible_child()
        if page is not None:
            self._paint_nav(page)
        self._flush()
        window = self.win.get_window()
        if window is None:
            raise RuntimeError(f"window is not mapped for {name}")
        pix = self.Gdk.pixbuf_get_from_window(window, 0, 0, window.get_width(), window.get_height())
        path = Path(self.screenshot) / f"{name}.png"
        path.parent.mkdir(parents=True, exist_ok=True)
        pix.savev(str(path), "png", [], [])
        print(f"SHOT {path} {window.get_width()}x{window.get_height()}", flush=True)

    def _exercise(self):
        self.win.resize(1600, 900)
        self.win.move(0, 0)
        self.win.show_all()
        self._shot("01-welcome")
        self._go_hub()
        self._shot("02-hub")
        self._open_spoke("desktop")
        self._shot("03-desktop")
        if "mde" in self._radios:
            self._radios["mde"].set_active(True)
        self._close_spoke("desktop")
        if self.selectors["desktop"].get_property("status") != "MDE":
            raise SystemExit("desktop spoke did not select MDE")
        print("OK desktop spoke", flush=True)
        cats = self.model["categories"].get(self.de, [])
        if cats:
            self._open_spoke(cats[0]["key"])
            self._shot("04-catalog")
            self._close_spoke(cats[0]["key"])
            print("OK catalog spoke", flush=True)
        self._open_spoke("preflight")
        self._close_spoke("preflight")
        self._shot("05-hub-selected")
        if self.model["dry_run"]:
            self._start_apply()
            from gi.repository import GLib

            import time

            context = GLib.MainContext.default()
            for _ in range(600):
                while context.pending():
                    context.iteration(False)
                if self.result is not None:
                    break
                self._poll()
                time.sleep(0.1)
            self._shot("06-progress")
            if not self.result:
                raise SystemExit("apply did not report a result")
            print(
                f"OK apply applied={self.result.get('applied')} failed={self.result.get('failed')}",
                flush=True,
            )
        self._exit()


def main(argv):
    parser = argparse.ArgumentParser(description="Anaconda fullscreen workstation installer")
    parser.add_argument("--compile-model", nargs=2, metavar=("SRC", "DEST"))
    parser.add_argument("--serve-bus", metavar="ADDRESS")
    parser.add_argument("--model")
    parser.add_argument("--script")
    parser.add_argument("--screenshot")
    args = parser.parse_args(argv)
    if args.compile_model:
        compile_model(args.compile_model[0], args.compile_model[1])
        return 0
    if args.serve_bus:
        serve_bus(args.serve_bus)
        return 0
    if not args.model or not args.script:
        parser.error("--model and --script are required")
    model = json.loads(Path(args.model).read_text(encoding="utf-8"))
    app = Installer(model, args.script, screenshot=args.screenshot)
    return app.run()


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except BrokenPipeError:
        raise SystemExit(0)
