# Settings > Gaming, Accessibility, and Privacy & security.
import json
import os
import re
import subprocess

from gi.repository import Gtk

import ws_theme
import wsui as ui

CURSOR_SIZES = [24, 32, 40, 48, 64, 72, 96]
SERVICES = [("ssh", "Remote sign-in (SSH)"), ("mdns", "Find devices on the network (mDNS)"),
            ("samba-client", "Browse shared folders (Samba client)"), ("samba", "Share folders (Samba)"),
            ("kdeconnect", "KDE Connect / GSConnect"), ("cockpit", "Computer Management (Cockpit)"),
            ("http", "Web server (HTTP)"), ("https", "Secure web server (HTTPS)"),
            ("dhcpv6-client", "IPv6 addresses (DHCPv6)"), ("ipp-client", "Network printers (IPP)")]


def gset(schema, key, value=None):
    if value is None:
        return ui.out(["gsettings", "get", schema, key]).strip().strip("'")
    ui.run(["gsettings", "set", schema, key, str(value).lower() if isinstance(value, bool) else str(value)])


# --- Gaming ------------------------------------------------------------------------------------

def build_gamemode(app, body):
    if not ui.which("gamemoded"):
        body.pack_start(ui.infobar("Game Mode needs the gamemode package.",
                                   action=("Install", lambda: ui.install(app, "gamemode"))), False, False, 0)
        return
    masked = ui.out(["systemctl", "--user", "is-enabled", "gamemoded"]).strip() == "masked"

    def toggle(v):
        ui.run(["systemctl", "--user", "unmask" if v else "mask", "gamemoded"], lambda *_: app.refresh())
    body.pack_start(ui.row("Game Mode", "Optimise your PC for play by giving games priority for the processor "
                           "and graphics card", "applications-games-symbolic", ui.switch(not masked, toggle)),
                    False, False, 0)
    status = ui.out(["gamemoded", "-s"]).strip()
    body.pack_start(ui.row("Status", status.capitalize() or "Not running", "dialog-information-symbolic"),
                    False, False, 0)
    body.pack_start(ui.label("Steam: set a game\u2019s launch options to \u201cgamemoderun %command%\u201d. "
                             "Other games: start them with gamemoderun.", "caption", wrap=True), False, False, 8)


def screenshot_dir():
    d = ui.get("screenshot_dir")
    return d or os.path.join(ui.out(["xdg-user-dir", "PICTURES"]).strip() or os.path.join(ui.HOME, "Pictures"),
                             "Screenshots")


def build_captures(app, body):
    d = screenshot_dir()

    def change():
        dlg = Gtk.FileChooserNative.new("Choose a folder for captures", app.win,
                                        Gtk.FileChooserAction.SELECT_FOLDER, "Select folder", "Cancel")
        if os.path.isdir(d):
            dlg.set_current_folder(d)
        if dlg.run() == Gtk.ResponseType.ACCEPT:
            ui.save(screenshot_dir=dlg.get_filename())
            app.refresh()
        dlg.destroy()
    body.pack_start(ui.row("Captures location", d.replace(ui.HOME, "~", 1), "folder-pictures-symbolic",
                           [ui.button("Change folder", change),
                            ui.button("Open folder", lambda: (os.makedirs(d, exist_ok=True), ui.spawn(["xdg-open", d])))]),
                    False, False, 0)
    body.pack_start(ui.section("Shortcuts"), False, False, 0)
    for key, what in (("Print", "Capture the whole screen"), ("Shift+Print", "Capture an area and edit it"),
                      ("Super+Print", "Capture the focused window")):
        body.pack_start(ui.row(what, key, "camera-photo-symbolic"), False, False, 0)


# --- Accessibility -------------------------------------------------------------------------------

def build_textsize(app, body):
    scale = float(gset("org.gnome.desktop.interface", "text-scaling-factor") or 1)
    hero = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "hero")
    preview = ui.label("Text size preview", xalign=0.5)
    hero.pack_start(preview, True, True, 24)
    body.pack_start(hero, False, False, 0)

    def show(v):
        preview.set_markup(f'<span size="{int(10.5 * v / 100 * 1024)}">Text size preview</span>')
    state = {"v": round(scale * 100)}
    show(state["v"])
    s = ui.slider(state["v"], 100, 225, lambda v: (state.update(v=v), show(v)), step=5, live=True,
                  fmt=lambda v: f"{v}%")
    body.pack_start(ui.row("Text size", "Drag the slider until the sample text is easy to read, then select Apply",
                           "format-text-larger-symbolic",
                           [s, ui.button("Apply", lambda: gset("org.gnome.desktop.interface", "text-scaling-factor",
                                                                f"{state['v'] / 100:.2f}"), accent=True)]),
                    False, False, 0)
    body.pack_start(ui.label("Apps pick up the new size when they restart.", "caption"), False, False, 4)


def build_visual(app, body):
    s = ui.load()
    body.pack_start(ui.row("Always show scrollbars", None, "view-continuous-symbolic",
                           ui.switch(gset("org.gnome.desktop.interface", "overlay-scrolling") == "false",
                                     lambda v: gset("org.gnome.desktop.interface", "overlay-scrolling", not v))),
                    False, False, 0)
    body.pack_start(ui.row("Animation effects", None, "applications-graphics-symbolic",
                           ui.switch(gset("org.gnome.desktop.interface", "enable-animations") != "false",
                                     lambda v: gset("org.gnome.desktop.interface", "enable-animations", v))),
                    False, False, 0)

    def set_timeout(v):
        ui.save(notify_timeout=v)
        ws_theme.apply_swaync()
        ui.run(["swaync-client", "-R"])
    body.pack_start(ui.row("Dismiss notifications after this amount of time", None,
                           "preferences-system-notifications-symbolic",
                           ui.combo([(5, "5 seconds"), (7, "7 seconds"), (15, "15 seconds"), (30, "30 seconds"),
                                     (60, "1 minute"), (300, "5 minutes")], s.get("notify_timeout", 5),
                                    set_timeout, 160)), False, False, 0)


def cursor():
    seat = re.search(r"seat \* xcursor_theme (\S+) (\d+)", ui.read_file(ui.SWAY_SETTINGS))
    if seat:
        return seat[1], int(seat[2])
    return gset("org.gnome.desktop.interface", "cursor-theme") or "Adwaita", \
        int(gset("org.gnome.desktop.interface", "cursor-size") or 24)


def set_cursor(theme, size):
    ui.sway_conf("cursor", f"seat * xcursor_theme {theme} {size}")
    gset("org.gnome.desktop.interface", "cursor-theme", theme)
    gset("org.gnome.desktop.interface", "cursor-size", size)
    from p_personal import set_ini
    set_ini("gtk-cursor-theme-name", theme)
    set_ini("gtk-cursor-theme-size", size)


def build_pointer(app, body):
    from p_personal import theme_dirs
    theme, size = cursor()
    body.pack_start(ui.row("Mouse pointer style", None, "input-mouse-symbolic",
                           ui.combo([(t, t) for t in theme_dirs("cursors")], theme,
                                    lambda t: set_cursor(t, cursor()[1]), 240)), False, False, 0)
    body.pack_start(ui.row("Size", None, "zoom-in-symbolic",
                           ui.slider(size, 24, 96, lambda v: set_cursor(cursor()[0], v), step=8,
                                     fmt=lambda v: f"{v} px")), False, False, 0)
    body.pack_start(ui.label("Some apps use the new pointer after they restart.", "caption"), False, False, 4)


def build_contrast(app, body):
    s = ui.load()
    hero = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4), "hero")
    hero.pack_start(ui.label("Contrast themes use distinct colours to make text and apps easier to see.",
                             wrap=True), False, False, 0)
    body.pack_start(hero, False, False, 0)

    def apply(v):
        ui.save(contrast=v or None)
        ws_theme.apply()
        app.restyle()
        app.refresh()
    body.pack_start(ui.row("Contrast themes", None, "preferences-desktop-accessibility-symbolic",
                           ui.combo([("", "None")] + [(k, v["name"]) for k, v in ws_theme.CONTRAST.items()],
                                    s.get("contrast") or "", apply, 180)), False, False, 0)
    body.pack_start(ui.label("Keyboard shortcut: none. Turning a theme off returns to your colour mode.",
                             "caption"), False, False, 4)


def shortcuts():
    text = ui.read_file("/usr/local/bin/ws-shortcuts")
    m = re.search(r"<<'LIST'\n(.*?)\nLIST", text, re.S)
    return [tuple(x.strip() for x in line.split("\u2014", 1)) for line in (m[1].splitlines() if m else [])
            if "\u2014" in line]


def build_keyboard(app, body):
    body.pack_start(ui.label("Sway uses the Super (Windows) key for most shortcuts.", "caption"), False, False, 8)
    search = Gtk.SearchEntry(placeholder_text="Search shortcuts")
    body.pack_start(search, False, False, 8)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    body.pack_start(box, False, False, 0)

    def fill(q=""):
        ui.clear(box)
        for keys, what in shortcuts():
            if q.casefold() in f"{keys} {what}".casefold():
                box.pack_start(ui.row(what, None, None, ui.label(keys, "caption")), False, False, 0)
        box.show_all()
    search.connect("search-changed", lambda e: fill(e.get_text()))
    fill()


# --- Privacy & security --------------------------------------------------------------------------

def secure_boot():
    for name in os.listdir("/sys/firmware/efi/efivars") if os.path.isdir("/sys/firmware/efi/efivars") else []:
        if name.startswith("SecureBoot-"):
            try:
                with open(os.path.join("/sys/firmware/efi/efivars", name), "rb") as f:
                    return f.read()[-1] == 1
            except OSError:
                return None
    return None


def build_security(app, body):
    fw = ui.out(["systemctl", "is-active", "firewalld"]).strip() == "active"
    selinux = ui.out(["getenforce"]).strip() or "Unknown"
    crypt = "crypt" in ui.out(["lsblk", "-no", "TYPE"])
    sb = secure_boot()
    updates = ui.json_file(os.path.join(ui.STATE, "updates.json"), {})
    pending = len(updates.get("packages", []))
    items = [("Firewall & network protection", "On" if fw else "Off \u2014 your PC may be exposed", fw,
              "security-high", "firewall"),
             ("App & browser control (SELinux)", selinux, selinux == "Enforcing", "security-medium", None),
             ("Device encryption", "On" if crypt else "Off \u2014 files can be read if the drive is removed", crypt,
              "drive-harddisk-encrypted", None),
             ("Secure Boot", {True: "On", False: "Off", None: "Not available"}[sb], bool(sb), "security-high", None),
             ("Updates", f"{pending} updates available" if pending else "You\u2019re up to date", not pending,
              "system-software-update", "update")]
    body.pack_start(ui.section("Protection areas"), False, False, 0)
    for title, caption, good, icon_name, target in items:
        badge = ui.label("\u2714" if good else "\u26a0", "ok-badge" if good else "caption")
        if target:
            body.pack_start(ui.nav_row(title, caption, icon_name, lambda t=target: app.go(t), badge), False, False, 0)
        else:
            body.pack_start(ui.row(title, caption, icon_name, badge), False, False, 0)


def build_firewall(app, body):
    active = ui.out(["systemctl", "is-active", "firewalld"]).strip() == "active"

    def toggle(v):
        ui.run(["systemctl", "start" if v else "stop", "firewalld"], lambda *_: app.refresh())
    body.pack_start(ui.row("Firewall", "Helps protect your PC from unwanted network traffic",
                           "security-high-symbolic", ui.switch(active, toggle)), False, False, 0)
    if not active:
        return
    zones = ui.out(["firewall-cmd", "--get-zones"], timeout=5).split()
    default = ui.out(["firewall-cmd", "--get-default-zone"], timeout=5).strip()
    body.pack_start(ui.row("Network profile", "The default zone for networks you connect to",
                           "network-workgroup-symbolic",
                           ui.combo([(z, z) for z in zones], default, lambda z: ui.run(
                               ["firewall-cmd", f"--set-default-zone={z}"], lambda *_: app.refresh()), 220)),
                    False, False, 0)
    exp, inner = ui.expander("Allow an app through the firewall", f"Services allowed in the {default} zone",
                             "applications-system-symbolic")
    loading = ui.label("Select Show to list the allowed services. You may be asked for your password.",
                       "caption", wrap=True)
    holder = ui.css(Gtk.Box(spacing=12), "card")
    holder.pack_start(loading, True, True, 0)

    def show():
        loading.set_text("Loading\u2026")

        def got(ok, out, err):
            if not ok:
                loading.set_text(err.strip() or "Couldn\u2019t read the firewall settings.")
                return
            holder.destroy()
            allowed = out.split()
            for svc, title in SERVICES:
                inner.pack_start(ui.row(title, svc, None, ui.switch(svc in allowed, lambda v, s=svc: ui.run(
                    ["sh", "-c", f"firewall-cmd --{'add' if v else 'remove'}-service={s} && "
                     f"firewall-cmd --permanent --{'add' if v else 'remove'}-service={s}"]))), False, False, 0)
            others = [s for s in allowed if s not in dict(SERVICES)]
            if others:
                inner.pack_start(ui.row("Also allowed", ", ".join(others), None), False, False, 0)
            inner.show_all()
        ui.run(["firewall-cmd", "--list-services"], got)
    holder.pack_end(ui.button("Show", show), False, False, 0)
    inner.pack_start(holder, False, False, 0)
    body.pack_start(exp, False, False, 0)


def flatpak_permissions(table, item):
    out = ui.out(["flatpak", "permissions", table, item]) if ui.which("flatpak") else ""
    apps = {}
    for line in out.splitlines()[1:]:
        parts = line.split()
        if len(parts) >= 4:
            apps[parts[2]] = parts[3]
    return apps


def app_name(app_id):
    try:
        from gi.repository import Gio
        info = Gio.DesktopAppInfo.new(app_id + ".desktop")
        return info.get_display_name(), info.get_icon()
    except TypeError:
        return app_id, "application-x-executable"


def flatpak_section(app, body, table, item, what):
    perms = flatpak_permissions(table, item)
    body.pack_start(ui.section(f"Let apps access your {what}"), False, False, 0)
    if not perms:
        body.pack_start(ui.label(f"Flatpak apps that ask for your {what} will appear here.", "caption"),
                        False, False, 0)
    for app_id, value in sorted(perms.items()):
        name, gicon = app_name(app_id)
        body.pack_start(ui.row(name, app_id, gicon, ui.switch(value == "yes", lambda v, a=app_id: ui.run(
            ["flatpak", "permission-set", table, item, a, "yes" if v else "no"]))), False, False, 0)
    body.pack_start(ui.label(f"Apps installed as Fedora packages can always use your {what}.", "caption",
                             wrap=True), False, False, 8)


def build_camera(app, body):
    devices = []
    for n in sorted(os.listdir("/sys/class/video4linux")) if os.path.isdir("/sys/class/video4linux") else []:
        name = ui.read_file(f"/sys/class/video4linux/{n}/name").strip()
        if ui.read_file(f"/sys/class/video4linux/{n}/index").strip() in ("", "0"):
            devices.append((n, name))
    body.pack_start(ui.section("Cameras"), False, False, 0)
    if not devices:
        body.pack_start(ui.label("No cameras were found.", "caption"), False, False, 0)
    for n, name in devices:
        users = subprocess.run(["fuser", f"/dev/{n}"], capture_output=True, text=True).stdout.split()
        procs = sorted({ui.read_file(f"/proc/{p.rstrip('m')}/comm").strip() for p in users if p.rstrip("m").isdigit()})
        body.pack_start(ui.row(name, f"In use by {', '.join(procs)}" if procs else "Not in use", "camera-web-symbolic"),
                        False, False, 0)
    flatpak_section(app, body, "devices", "camera", "camera")


def build_microphone(app, body):
    default = ui.out(["pactl", "get-default-source"]).strip()
    muted = "yes" in ui.out(["pactl", "get-source-mute", "@DEFAULT_SOURCE@"])
    body.pack_start(ui.row("Microphone access", "When off, the microphone is muted for every app",
                           "audio-input-microphone-symbolic",
                           ui.switch(not muted, lambda v: ui.run(["pactl", "set-source-mute", "@DEFAULT_SOURCE@",
                                                                   "0" if v else "1"]))), False, False, 0)
    body.pack_start(ui.section("Recent activity"), False, False, 0)
    try:
        streams = json.loads(ui.out(["pactl", "-f", "json", "list", "source-outputs"]) or "[]")
    except ValueError:
        streams = []
    streams = [s for s in streams if not s.get("properties", {}).get("media.name", "").startswith("Peak detect")]
    if not streams:
        body.pack_start(ui.label("No apps are using your microphone right now.", "caption"), False, False, 0)
    for s in streams:
        p = s.get("properties", {})
        body.pack_start(ui.row(p.get("application.name", "App"), "Using your microphone now",
                               p.get("application.icon_name") or "audio-input-microphone"), False, False, 0)
    body.pack_start(ui.label(f"Default microphone: {default}", "caption", wrap=True), False, False, 8)


def build_location(app, body):
    on = gset("org.gnome.system.location", "enabled") == "true"
    body.pack_start(ui.row("Location services", "Apps can ask for your location through the location portal",
                           "find-location-symbolic",
                           ui.switch(on, lambda v: gset("org.gnome.system.location", "enabled", v))), False, False, 0)
    flatpak_section(app, body, "location", "location", "location")


def build_activity(app, body):
    on = gset("org.gnome.desktop.privacy", "remember-recent-files") != "false"
    body.pack_start(ui.row("Store my activity history on this device",
                           "Recent files show up in Start, File Explorer and file pickers",
                           "document-open-recent-symbolic",
                           ui.switch(on, lambda v: gset("org.gnome.desktop.privacy", "remember-recent-files", v))),
                    False, False, 0)
    age = int(gset("org.gnome.desktop.privacy", "recent-files-max-age") or -1)
    body.pack_start(ui.row("Keep activity history for", None, "x-office-calendar-symbolic",
                           ui.combo([(1, "1 day"), (7, "7 days"), (30, "30 days"), (-1, "Forever")], age,
                                    lambda v: gset("org.gnome.desktop.privacy", "recent-files-max-age", v), 160)),
                    False, False, 0)

    def clear():
        for path in (os.path.join(ui.HOME, ".local/share/recently-used.xbel"),
                     os.path.join(ui.HOME, ".local/state/ws-start/usage.json")):
            try:
                os.unlink(path)
            except OSError:
                pass
        ui.notify("Activity history cleared", "", "edit-clear")
    body.pack_start(ui.row("Clear activity history", "Recent files and Start\u2019s most used apps",
                           "edit-clear-symbolic", ui.button("Clear history", clear)), False, False, 0)


PAGES = [
    dict(key="gamemode", title="Game Mode", parent="gaming", icon="applications-games",
         caption="Optimise your PC for play", keywords="game mode performance gamemode", build=build_gamemode,
         aliases=("gaming-gamemode",)),
    dict(key="captures", title="Captures", parent="gaming", icon="camera-photo",
         caption="Save location, screenshot shortcuts", keywords="screenshot capture print screen folder",
         build=build_captures, aliases=("gaming-gamedvr", "screenshots")),
    dict(key="textsize", title="Text size", parent="accessibility", icon="format-text-larger",
         caption="Text size that appears throughout Windows and your apps", keywords="font size bigger larger zoom",
         build=build_textsize, aliases=("easeofaccess-display",)),
    dict(key="visual", title="Visual effects", parent="accessibility", icon="applications-graphics",
         caption="Scrollbars, animation effects, notification timeout",
         keywords="animations scrollbars transparency effects", build=build_visual,
         aliases=("easeofaccess-visualeffects",)),
    dict(key="pointer", title="Mouse pointer", parent="accessibility", icon="input-mouse",
         caption="Mouse pointer colour, size", keywords="cursor pointer size theme mouse", build=build_pointer,
         aliases=("easeofaccess-mousepointer", "cursor")),
    dict(key="contrast", title="Contrast themes", parent="accessibility", icon="preferences-desktop-accessibility",
         caption="Colour themes for low vision, light sensitivity",
         keywords="high contrast low vision colours", build=build_contrast,
         aliases=("easeofaccess-highcontrast",)),
    dict(key="shortcuts", title="Keyboard", parent="accessibility", icon="preferences-desktop-keyboard-shortcuts",
         caption="Keyboard shortcuts for this desktop", keywords="shortcuts hotkeys keys super",
         build=build_keyboard, aliases=("easeofaccess-keyboard", "keyboard-shortcuts")),
    dict(key="security", title="Windows Security", parent="privacy", icon="security-high",
         caption="Firewall, SELinux, encryption, Secure Boot",
         keywords="security antivirus protection selinux encryption secure boot", build=build_security,
         aliases=("windowsdefender",)),
    dict(key="firewall", title="Firewall & network protection", parent="security", icon="security-high",
         caption="Firewall status, network profile, allowed apps", keywords="firewall ports firewalld zone",
         build=build_firewall, inline=True),
    dict(key="camera", title="Camera", parent="privacy", icon="camera-web",
         caption="Cameras, apps that use the camera", keywords="webcam camera privacy", build=build_camera,
         aliases=("privacy-webcam",)),
    dict(key="microphone", title="Microphone", parent="privacy", icon="audio-input-microphone",
         caption="Microphone access, recent activity", keywords="mic microphone privacy", build=build_microphone,
         aliases=("privacy-microphone",)),
    dict(key="location", title="Location", parent="privacy", icon="find-location",
         caption="Location services, apps that use your location", keywords="gps location geoclue",
         build=build_location, aliases=("privacy-location",)),
    dict(key="activity", title="Activity history", parent="privacy", icon="document-open-recent",
         caption="Recent files, clear history", keywords="history recent files privacy", build=build_activity,
         aliases=("privacy-activityhistory",)),
]
