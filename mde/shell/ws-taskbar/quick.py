"""System status (network, volume, battery, Bluetooth, brightness, power mode) and the
Windows 11 Quick Settings flyout."""

import re

from common import (Flyout, Gio, GLib, Gtk, css_class, clear, label, out, run, save_setting,
                    settings, sway_exec, symbolic, which, ws_motion)

PROFILE_NAMES = {"power-saver": "Best power efficiency", "balanced": "Balanced",
                 "performance": "Best performance"}


def system_proxy(name, path, iface):
    try:
        return Gio.DBusProxy.new_for_bus_sync(Gio.BusType.SYSTEM, Gio.DBusProxyFlags.NONE, None,
                                              name, path, iface, None)
    except GLib.Error:
        return None


def prop(proxy, name, default=None):
    if not proxy:
        return default
    v = proxy.get_cached_property(name)
    return v.unpack() if v is not None else default


def set_prop(proxy, iface, name, value, done=None):
    if not proxy:
        return

    def finish(p, res):
        try:
            p.call_finish(res)
        except GLib.Error:
            pass
        if done:
            done()
    proxy.call("org.freedesktop.DBus.Properties.Set", GLib.Variant("(ssv)", (iface, name, value)),
               Gio.DBusCallFlags.ALLOW_INTERACTIVE_AUTHORIZATION, 5000, None, finish)


class Status:
    """Live system state. Listeners get called (without arguments) on any change."""

    def __init__(self):
        self.listeners = []
        self.volume, self.muted, self.sink = 0, False, ""
        self.brightness = None
        self.nm = system_proxy("org.freedesktop.NetworkManager", "/org/freedesktop/NetworkManager",
                               "org.freedesktop.NetworkManager")
        self.active = self.device = self.ap = None
        if self.nm:
            self.nm.connect("g-properties-changed", lambda *_: self.on_nm())
            self.on_nm(notify=False)
        self.battery = system_proxy("org.freedesktop.UPower", "/org/freedesktop/UPower/devices/DisplayDevice",
                                    "org.freedesktop.UPower.Device")
        if self.battery:
            self.battery.connect("g-properties-changed", lambda *_: self.notify())
        self.bt = system_proxy("org.bluez", "/org/bluez/hci0", "org.bluez.Adapter1")
        if self.bt and self.bt.get_cached_property("Powered") is None:
            self.bt = None
        if self.bt:
            self.bt.connect("g-properties-changed", lambda *_: self.notify())
        self.power = (system_proxy("org.freedesktop.UPower.PowerProfiles", "/org/freedesktop/UPower/PowerProfiles",
                                   "org.freedesktop.UPower.PowerProfiles")
                      or system_proxy("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles",
                                      "net.hadess.PowerProfiles"))
        if self.power and self.power.get_cached_property("ActiveProfile") is None:
            self.power = None
        if self.power:
            self.power.connect("g-properties-changed", lambda *_: self.notify())
        self.read_volume()
        self.read_brightness()
        self.watch_audio()

    def notify(self):
        for callback in self.listeners:
            callback()

    # Network

    def on_nm(self, notify=True):
        path = prop(self.nm, "PrimaryConnection", "/")
        if path in ("/", None):
            path = next(iter(prop(self.nm, "ActiveConnections", []) or []), "/")
        if not self.active or self.active.get_object_path() != path:
            self.active = self.device = self.ap = None
            if path and path != "/":
                self.active = system_proxy("org.freedesktop.NetworkManager", path,
                                           "org.freedesktop.NetworkManager.Connection.Active")
                devices = prop(self.active, "Devices", [])
                if devices and "wireless" in (prop(self.active, "Type", "") or ""):
                    self.device = system_proxy("org.freedesktop.NetworkManager", devices[0],
                                               "org.freedesktop.NetworkManager.Device.Wireless")
                    if self.device:
                        self.device.connect("g-properties-changed", lambda *_: self.on_ap())
                    self.on_ap(notify=False)
        if notify:
            self.notify()

    def on_ap(self, notify=True):
        ap_path = prop(self.device, "ActiveAccessPoint", "/")
        if not self.ap or self.ap.get_object_path() != ap_path:
            self.ap = None
            if ap_path and ap_path != "/":
                self.ap = system_proxy("org.freedesktop.NetworkManager", ap_path,
                                       "org.freedesktop.NetworkManager.AccessPoint")
                if self.ap:
                    self.ap.connect("g-properties-changed", lambda *_: self.notify())
        if notify:
            self.notify()

    @property
    def wifi_enabled(self):
        return bool(prop(self.nm, "WirelessEnabled", False))

    @property
    def airplane(self):
        return bool(self.nm) and not self.wifi_enabled and not (self.bt and self.bt_powered) and \
            settings().get("airplane", False)

    def network(self):
        """(icon, tooltip)"""
        if not self.nm:
            return "network-offline-symbolic", "Network unavailable"
        if self.airplane:
            return "airplane-mode-symbolic", "Airplane mode"
        state = prop(self.nm, "State", 0)
        kind = prop(self.active, "Type", "") or ""
        name = prop(self.active, "Id", "") or ""
        internet = "Internet access" if state == 70 else "No internet access"
        if 40 <= state < 50:
            return "network-wireless-acquiring-symbolic", "Connecting"
        if not self.active or state < 50:
            if self.wifi_enabled:
                return "network-wireless-offline-symbolic", "Not connected"
            return "network-offline-symbolic", "Not connected"
        if "wireless" in kind:
            strength = prop(self.ap, "Strength", 0) or 0
            level = ("excellent" if strength > 75 else "good" if strength > 50 else
                     "ok" if strength > 25 else "weak")
            return f"network-wireless-signal-{level}-symbolic", f"{name}\n{internet}"
        if "vpn" in kind or "wireguard" in kind:
            return "network-vpn-symbolic", f"{name}\n{internet}"
        return "network-wired-symbolic", f"{name or 'Ethernet'}\n{internet}"

    def set_wifi(self, on, done=None):
        set_prop(self.nm, "org.freedesktop.NetworkManager", "WirelessEnabled", GLib.Variant("b", on), done)

    # Bluetooth

    @property
    def bt_powered(self):
        return bool(prop(self.bt, "Powered", False))

    def set_bt(self, on):
        if on:
            run(["rfkill", "unblock", "bluetooth"],
                lambda *_: set_prop(self.bt, "org.bluez.Adapter1", "Powered", GLib.Variant("b", True)))
        else:
            set_prop(self.bt, "org.bluez.Adapter1", "Powered", GLib.Variant("b", False))

    def set_airplane(self, on):
        save_setting(airplane=on)
        self.set_wifi(not on, self.notify)
        if self.bt:
            self.set_bt(not on)

    # Battery

    def battery_state(self):
        """(present, percent, icon, tooltip)"""
        if not self.battery or not prop(self.battery, "IsPresent", False) or prop(self.battery, "Type", 0) != 2:
            return False, 0, "", ""
        pct = prop(self.battery, "Percentage", 0.0)
        state = prop(self.battery, "State", 0)
        level = min(100, int(pct // 10 * 10))
        if state == 4 or (state == 1 and pct >= 99):
            icon, text = "battery-level-100-charged-symbolic", "Fully charged"
        elif state in (1, 5):
            icon = f"battery-level-{level}-charging-symbolic"
            secs = prop(self.battery, "TimeToFull", 0)
            text = "Charging" + (f", {secs // 3600} h {secs % 3600 // 60} min until full" if secs else "")
        else:
            icon = f"battery-level-{level}-symbolic"
            secs = prop(self.battery, "TimeToEmpty", 0)
            text = f"{secs // 3600} h {secs % 3600 // 60} min remaining" if secs else "On battery"
        return True, round(pct), icon, f"Battery: {round(pct)}%\n{text}"

    # Power mode

    def profiles(self):
        return [p["Profile"] for p in prop(self.power, "Profiles", [])] if self.power else []

    @property
    def profile(self):
        return prop(self.power, "ActiveProfile", "balanced")

    def set_profile(self, name):
        iface = self.power.get_interface_name() if self.power else ""
        set_prop(self.power, iface, "ActiveProfile", GLib.Variant("s", name))

    # Audio

    def read_volume(self):
        text = out(["pactl", "get-sink-volume", "@DEFAULT_SINK@"])
        m = re.search(r"(\d+)%", text)
        self.volume = int(m.group(1)) if m else 0
        self.muted = "yes" in out(["pactl", "get-sink-mute", "@DEFAULT_SINK@"])
        self.sink = out(["pactl", "get-default-sink"]).strip()

    def watch_audio(self):
        try:
            proc = Gio.Subprocess.new(["pactl", "subscribe"], Gio.SubprocessFlags.STDOUT_PIPE)
        except GLib.Error:
            return
        stream = Gio.DataInputStream.new(proc.get_stdout_pipe())
        self.audio_pending = False

        def changed():
            self.audio_pending = False
            self.read_volume()
            self.notify()
            return False

        def on_line(s, res):
            try:
                line, _n = s.read_line_finish_utf8(res)
            except GLib.Error:
                line = None
            if line is None:
                GLib.timeout_add_seconds(3, self.watch_audio)
                return
            if ("sink" in line or "server" in line) and "sink-input" not in line and not self.audio_pending:
                self.audio_pending = True
                GLib.timeout_add(60, changed)
            s.read_line_async(GLib.PRIORITY_DEFAULT, None, on_line)
        stream.read_line_async(GLib.PRIORITY_DEFAULT, None, on_line)
        self.audio_proc = proc
        return False

    def volume_icon(self):
        if self.muted or self.volume == 0:
            return "audio-volume-muted-symbolic"
        return ("audio-volume-low-symbolic" if self.volume < 34 else
                "audio-volume-medium-symbolic" if self.volume < 67 else "audio-volume-high-symbolic")

    def set_volume(self, value):
        self.volume = value
        run(["pactl", "set-sink-volume", "@DEFAULT_SINK@", f"{value}%"])
        if self.muted and value > 0:
            run(["pactl", "set-sink-mute", "@DEFAULT_SINK@", "0"])

    def toggle_mute(self):
        run(["pactl", "set-sink-mute", "@DEFAULT_SINK@", "toggle"])

    # Brightness

    def read_brightness(self):
        if not which("brightnessctl"):
            return
        fields = out(["brightnessctl", "-m", "-c", "backlight", "info"]).strip().split(",")
        if len(fields) >= 4 and fields[3].endswith("%"):
            self.brightness = int(fields[3][:-1])

    def set_brightness(self, value):
        self.brightness = value
        run(["brightnessctl", "-q", "-c", "backlight", "set", f"{max(1, value)}%"])


# --- Quick Settings flyout -----------------------------------------------------------------

def split_tile(icon_name, title, active, on_toggle, on_more=None, sensitive=True):
    """A Windows 11 quick-settings tile: the button toggles, the chevron opens details."""
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
    tile = css_class(Gtk.Box(), "tile")
    if active:
        css_class(tile, "on")
    main = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, hexpand=True), "tile-main")
    main.add(symbolic(icon_name, 16))
    main.connect("clicked", lambda *_: on_toggle())
    main.set_sensitive(sensitive)
    tile.pack_start(main, True, True, 0)
    if on_more:
        css_class(tile, "split")
        more = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "tile-more")
        more.add(symbolic("go-next-symbolic", 12))
        more.connect("clicked", lambda *_: on_more())
        more.set_sensitive(sensitive)
        tile.pack_start(more, False, False, 0)
    box.pack_start(tile, False, False, 0)
    box.pack_start(label(title, "tile-label", xalign=0.5, ellipsize=True), False, False, 0)
    return box


class QuickSettings(Flyout):
    def __init__(self, status, monitor=None):
        super().__init__("quick-settings", Gtk.Align.END, monitor)
        self.status = status
        self.stack = Gtk.Stack(transition_type=Gtk.StackTransitionType.SLIDE_LEFT_RIGHT,
                               transition_duration=ws_motion.NORMAL, vhomogeneous=False,
                               interpolate_size=True)
        self.panel.pack_start(self.stack, True, True, 0)
        self.main = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.stack.add_named(self.main, "main")
        self.sub = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.stack.add_named(self.sub, "sub")
        status.listeners.append(self.on_status)
        self.dragging = False

    def open(self):
        self.status.read_brightness()
        self.build_main()
        self.stack.set_visible_child_name("main")
        super().open()

    def on_status(self):
        if self.get_visible() and self.stack.get_visible_child_name() == "main" and not self.dragging:
            self.build_main()

    def build_main(self):
        clear(self.main)
        s = self.status
        top = css_class(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=24), "qs-body")
        grid = Gtk.Grid(column_spacing=12, row_spacing=16, column_homogeneous=True)
        tiles = []
        if s.nm:
            wifi_name = (prop(s.active, "Id", "") if s.device else "") or "Wi-Fi"
            tiles.append(split_tile("network-wireless-signal-excellent-symbolic", wifi_name,
                                    s.wifi_enabled, lambda: s.set_wifi(not s.wifi_enabled),
                                    self.show_wifi))
        if s.bt:
            tiles.append(split_tile("bluetooth-active-symbolic", "Bluetooth", s.bt_powered,
                                    lambda: s.set_bt(not s.bt_powered),
                                    (lambda: self.launch("blueman-manager")) if which("blueman-manager") else None))
        if s.nm:
            tiles.append(split_tile("airplane-mode-symbolic", "Airplane mode", s.airplane,
                                    lambda: s.set_airplane(not s.airplane)))
        conf = settings()
        tiles.append(split_tile("night-light-symbolic", "Night light", conf.get("night_light"),
                                self.toggle_night_light, lambda: self.launch("ws-settings nightlight"),
                                sensitive=which("wlsunset")))
        dnd = out(["swaync-client", "-D"]).strip() == "true" if which("swaync-client") else False
        tiles.append(split_tile("notifications-disabled-symbolic", "Do not disturb", dnd,
                                lambda: (run(["swaync-client", "-dn" if not dnd else "-df"],
                                             lambda *_: self.build_main()))))
        if s.power:
            name = s.profile
            icon = {"power-saver": "power-profile-power-saver-symbolic",
                    "performance": "power-profile-performance-symbolic"}.get(name, "power-profile-balanced-symbolic")
            tiles.append(split_tile(icon, PROFILE_NAMES.get(name, name.title()), name != "balanced",
                                    self.cycle_profile, lambda: self.launch("ws-settings power")))
        for i, tile in enumerate(tiles):
            grid.attach(tile, i % 3, i // 3, 1, 1)
        top.pack_start(grid, False, False, 0)

        sliders = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        if s.brightness is not None:
            sliders.pack_start(self.slider_row("display-brightness-symbolic", s.brightness, s.set_brightness),
                               False, False, 0)
        vol = self.slider_row(s.volume_icon(), s.volume, s.set_volume, icon_click=s.toggle_mute,
                              more=self.show_sound)
        sliders.pack_start(vol, False, False, 0)
        top.pack_start(sliders, False, False, 0)
        self.main.pack_start(top, False, False, 0)

        footer = css_class(Gtk.Box(spacing=4), "qs-footer")
        present, pct, icon, tip = s.battery_state()
        if present:
            bat = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text=tip), "qs-link")
            row = Gtk.Box(spacing=8)
            row.pack_start(symbolic(icon, 16), False, False, 0)
            row.pack_start(label(f"{pct}%"), False, False, 0)
            bat.add(row)
            bat.connect("clicked", lambda *_: self.launch("ws-settings power"))
            footer.pack_start(bat, False, False, 0)
        gear = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="All settings"), "qs-icon")
        gear.add(symbolic("emblem-system-symbolic", 16))
        gear.connect("clicked", lambda *_: self.launch("ws-settings"))
        footer.pack_end(gear, False, False, 0)
        self.main.pack_start(footer, False, False, 0)
        self.main.show_all()

    def slider_row(self, icon_name, value, on_change, icon_click=None, more=None):
        row = Gtk.Box(spacing=8)
        icon = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "qs-icon")
        icon.add(symbolic(icon_name, 16))
        if icon_click:
            icon.connect("clicked", lambda *_: icon_click())
        else:
            icon.set_can_focus(False)
            css_class(icon, "flat-icon")
        row.pack_start(icon, False, False, 0)
        scale = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 1)
        scale.set_value(value)
        scale.set_draw_value(False)
        scale.set_hexpand(True)
        scale.set_tooltip_text(f"{value}")
        pending = {"id": 0}

        def changed(sc):
            v = int(sc.get_value())
            sc.set_tooltip_text(f"{v}")
            if pending["id"]:
                GLib.source_remove(pending["id"])
            pending["id"] = GLib.timeout_add(40, lambda: (on_change(v), pending.update(id=0)) and False)
        scale.connect("value-changed", changed)
        scale.connect("button-press-event", lambda *_: setattr(self, "dragging", True))
        scale.connect("button-release-event", lambda *_: setattr(self, "dragging", False))
        row.pack_start(scale, True, True, 0)
        if more:
            b = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="Select a sound output"), "qs-icon")
            b.add(symbolic("go-next-symbolic", 12))
            b.connect("clicked", lambda *_: more())
            row.pack_start(b, False, False, 0)
        return row

    def launch(self, cmd):
        self.close_flyout()
        sway_exec(cmd)

    def toggle_night_light(self):
        save_setting(night_light=not settings().get("night_light"))
        run(["pkill", "-u", str(__import__("os").getuid()), "-f", "ws-nightlight"])
        GLib.timeout_add(200, lambda: self.build_main() and False)

    def cycle_profile(self):
        names = self.status.profiles()
        if names:
            cur = self.status.profile
            self.status.set_profile(names[(names.index(cur) + 1) % len(names)] if cur in names else names[0])

    # Detail pages

    def sub_page(self, title, footer_text=None, footer_cmd=None):
        clear(self.sub)
        head = css_class(Gtk.Box(spacing=8), "qs-head")
        back = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="Back"), "qs-icon")
        back.add(symbolic("go-previous-symbolic", 16))
        back.connect("clicked", lambda *_: (self.build_main(), self.stack.set_visible_child_name("main")))
        head.pack_start(back, False, False, 0)
        head.pack_start(label(title, "qs-title"), False, False, 0)
        self.sub.pack_start(head, False, False, 0)
        scroll = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER, propagate_natural_height=True,
                                    max_content_height=360)
        body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        css_class(body, "qs-list")
        scroll.add(body)
        self.sub.pack_start(scroll, True, True, 0)
        if footer_text:
            footer = css_class(Gtk.Box(), "qs-footer")
            link = css_class(Gtk.Button(label=footer_text, relief=Gtk.ReliefStyle.NONE), "qs-link")
            link.connect("clicked", lambda *_: self.launch(footer_cmd))
            footer.pack_start(link, False, False, 0)
            self.sub.pack_start(footer, False, False, 0)
        self.sub.show_all()
        self.stack.set_visible_child_name("sub")
        return body

    def show_sound(self):
        body = self.sub_page("Sound output", "More volume settings", "ws-settings sound")

        def fill(_ok, text):
            import json
            try:
                sinks = json.loads(text)
            except ValueError:
                sinks = []
            current = out(["pactl", "get-default-sink"]).strip()
            for sink in sinks:
                name = sink.get("description") or sink.get("name")
                row = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "qs-row")
                if sink.get("name") == current:
                    css_class(row, "selected")
                box = Gtk.Box(spacing=12)
                box.pack_start(symbolic("audio-speakers-symbolic", 16), False, False, 0)
                box.pack_start(label(name, ellipsize=True), True, True, 0)
                row.add(box)
                row.connect("clicked", lambda _b, n=sink.get("name"): run(
                    ["pactl", "set-default-sink", n], lambda *_: self.show_sound()))
                body.pack_start(row, False, False, 0)
            body.show_all()
        run(["pactl", "-f", "json", "list", "sinks"], fill)

    def show_wifi(self):
        body = self.sub_page("Wi-Fi", "More Wi-Fi settings", "nm-connection-editor")
        head = self.sub.get_children()[0]
        switch = Gtk.Switch(active=self.status.wifi_enabled, valign=Gtk.Align.CENTER)
        switch.connect("notify::active", lambda sw, _p: self.status.set_wifi(
            sw.get_active(), lambda: GLib.timeout_add(1500, lambda: self.show_wifi() and False)))
        head.pack_end(switch, False, False, 0)
        head.show_all()
        if not self.status.wifi_enabled:
            body.pack_start(label("Wi-Fi is turned off", "qs-empty", xalign=0.5), False, False, 12)
            body.show_all()
            return
        spinner = Gtk.Spinner(active=True)
        body.pack_start(spinner, False, False, 12)
        body.show_all()

        def fill(_ok, text):
            known = {line.split(":")[0] for line in out(["nmcli", "-t", "-f", "NAME,TYPE", "con", "show"]).splitlines()
                     if line.endswith("802-11-wireless")}
            spinner.destroy()
            seen = set()
            for line in text.splitlines():
                parts = re.split(r"(?<!\\):", line)
                if len(parts) < 4:
                    continue
                in_use, signal, security, ssid = parts[0] == "*", parts[1], parts[2], parts[3].replace("\\:", ":")
                if not ssid or ssid in seen:
                    continue
                seen.add(ssid)
                body.pack_start(self.wifi_row(ssid, int(signal or 0), security, in_use, ssid in known),
                                False, False, 0)
            if not seen:
                body.pack_start(label("No Wi-Fi networks found", "qs-empty", xalign=0.5), False, False, 12)
            body.show_all()
        run(["nmcli", "-t", "-f", "IN-USE,SIGNAL,SECURITY,SSID", "dev", "wifi", "list", "--rescan", "auto"], fill)

    def wifi_row(self, ssid, signal, security, in_use, known):
        outer = css_class(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "qs-row-box")
        if in_use:
            css_class(outer, "selected")
        row = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "qs-row")
        box = Gtk.Box(spacing=12)
        level = "excellent" if signal > 75 else "good" if signal > 50 else "ok" if signal > 25 else "weak"
        box.pack_start(symbolic(f"network-wireless-signal-{level}-symbolic", 16), False, False, 0)
        texts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        texts.pack_start(label(ssid, ellipsize=True), False, False, 0)
        state = "Connected, secured" if in_use and security else "Connected" if in_use else \
            "Secured" if security else "Open"
        texts.pack_start(label(state, "caption"), False, False, 0)
        box.pack_start(texts, True, True, 0)
        row.add(box)
        outer.pack_start(row, False, False, 0)
        details = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, no_show_all=True)
        css_class(details, "qs-details")
        outer.pack_start(details, False, False, 0)

        def expand(*_):
            if details.get_visible():
                details.hide()
                return
            clear(details)
            msg = label("", "caption", wrap=True)
            if in_use:
                b = css_class(Gtk.Button(label="Disconnect", halign=Gtk.Align.END), "qs-button")
                b.connect("clicked", lambda *_: run(["nmcli", "con", "down", "id", ssid],
                                                    lambda *_: self.show_wifi()))
                details.pack_start(b, False, False, 0)
            else:
                entry = None
                if security and not known:
                    details.pack_start(label("Enter the network security key", "caption"), False, False, 0)
                    entry = Gtk.Entry(visibility=False, input_purpose=Gtk.InputPurpose.PASSWORD)
                    details.pack_start(entry, False, False, 0)
                b = css_class(Gtk.Button(label="Connect", halign=Gtk.Align.END), "qs-button", "accent")

                def connect(*_):
                    msg.set_text("Connecting\u2026")
                    b.set_sensitive(False)
                    if known:
                        argv = ["nmcli", "con", "up", "id", ssid]
                    else:
                        argv = ["nmcli", "dev", "wifi", "connect", ssid]
                        if entry is not None:
                            argv += ["password", entry.get_text()]

                    def done(ok, _out):
                        if ok:
                            self.show_wifi()
                        else:
                            b.set_sensitive(True)
                            msg.set_text("Couldn't connect to this network.")
                    run(argv, done)
                b.connect("clicked", connect)
                if entry is not None:
                    entry.connect("activate", connect)
                details.pack_start(b, False, False, 0)
            details.pack_start(msg, False, False, 0)
            details.show_all()
            details.show()
        row.connect("clicked", expand)
        return outer
