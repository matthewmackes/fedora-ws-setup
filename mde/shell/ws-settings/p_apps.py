# Settings > Apps: Installed apps, Default apps and Startup.
import configparser
import os
import time

from gi.repository import Gio, GLib, Gtk

import wsui as ui

DEFAULT_GROUPS = [
    ("Web browser", "web-browser", ["x-scheme-handler/http", "x-scheme-handler/https", "text/html"]),
    ("Email", "mail-send", ["x-scheme-handler/mailto"]),
    ("File manager", "system-file-manager", ["inode/directory"]),
    ("Photo viewer", "image-x-generic", ["image/jpeg", "image/png", "image/gif", "image/webp", "image/bmp"]),
    ("Music player", "audio-x-generic", ["audio/mpeg", "audio/flac", "audio/ogg", "audio/x-wav", "audio/mp4"]),
    ("Video player", "video-x-generic", ["video/mp4", "video/x-matroska", "video/webm", "video/quicktime"]),
    ("Text editor", "accessories-text-editor", ["text/plain"]),
    ("PDF reader", "application-pdf", ["application/pdf"]),
    ("Archive manager", "package-x-generic", ["application/zip", "application/x-tar", "application/x-7z-compressed"]),
]
AUTOSTART_DIRS = ["/etc/xdg/autostart", os.path.join(ui.CONFIG, "autostart")]


# --- Installed apps -------------------------------------------------------------------

def installed_apps():
    apps = [a for a in Gio.AppInfo.get_all() if isinstance(a, Gio.DesktopAppInfo) and a.should_show()]
    return sorted(apps, key=lambda a: a.get_display_name().casefold())


def package_info(apps, done):
    """Map desktop file → (kind, package id, size, install time) without blocking."""
    files = [a.get_filename() for a in apps if a.get_filename()]
    rpm_files = [f for f in files if f.startswith("/usr/share/")]
    info = {}
    for f in files:
        if "/flatpak/exports/" in f:
            info[f] = ("flatpak", os.path.basename(f)[:-8], 0, os.stat(f).st_mtime)
        elif not f.startswith("/usr/share/"):
            info[f] = ("local", None, 0, os.stat(f).st_mtime)

    def got(_ok, out, _err):
        for f, line in zip(rpm_files, out.splitlines()):
            parts = line.split("\t")
            if len(parts) == 3 and parts[1].isdigit():
                info[f] = ("rpm", parts[0], int(parts[1]), int(parts[2]))
        done(info)
    if rpm_files:
        ui.run(["rpm", "-qf", "--qf", "%{NAME}\t%{SIZE}\t%{INSTALLTIME}\n", *rpm_files], got)
    else:
        done(info)


def build_installed(app, body):
    apps = installed_apps()
    state = {"sort": "name", "filter": "all", "text": "", "info": {}}
    bar = Gtk.Box(spacing=8)
    search = Gtk.SearchEntry(placeholder_text="Search apps")
    search.set_size_request(280, -1)
    bar.pack_start(search, False, False, 0)
    bar.pack_end(ui.combo([("name", "Name (A to Z)"), ("name-desc", "Name (Z to A)"),
                           ("date", "Install date"), ("size", "Size (large to small)")], "name",
                          lambda v: (state.update(sort=v), fill()), 190), False, False, 0)
    bar.pack_end(ui.combo([("all", "All apps"), ("rpm", "Fedora packages"), ("flatpak", "Flatpak"),
                           ("local", "Other")], "all", lambda v: (state.update(filter=v), fill()), 160),
                 False, False, 0)
    body.pack_start(bar, False, False, 8)
    count = ui.label("", "caption")
    body.pack_start(count, False, False, 4)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    body.pack_start(box, False, False, 0)

    def fill():
        ui.clear(box)
        info = state["info"]
        q = state["text"].casefold()
        shown = [a for a in apps if q in a.get_display_name().casefold()
                 and (state["filter"] == "all" or info.get(a.get_filename(), ("rpm",))[0] == state["filter"])]
        key = {"name": lambda a: a.get_display_name().casefold(),
               "name-desc": lambda a: a.get_display_name().casefold(),
               "date": lambda a: -info.get(a.get_filename(), (0, 0, 0, 0))[3],
               "size": lambda a: -info.get(a.get_filename(), (0, 0, 0, 0))[2]}[state["sort"]]
        shown.sort(key=key, reverse=state["sort"] == "name-desc")
        count.set_text(f"{len(shown)} apps found")
        for a in shown:
            kind, pkg, size, when = info.get(a.get_filename(), (None, None, 0, 0))
            parts = [{"rpm": pkg, "flatpak": "Flatpak", "local": "Added by this PC's setup"}.get(kind) or ""]
            if when:
                parts.append(time.strftime("%-m/%-d/%Y", time.localtime(when)))
            more = ui.css(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text="More options"), "btn")
            more.add(Gtk.Image.new_from_icon_name("view-more-horizontal-symbolic", Gtk.IconSize.MENU))
            more.connect("clicked", lambda b, a=a, k=kind, p=pkg: ui.popup_menu(b, app_actions(app, a, k, p)))
            r = ui.row(a.get_display_name(), "  |  ".join(p for p in parts if p),
                       a.get_icon() or "application-x-executable",
                       [ui.label(ui.human_size(size) if size else "", "caption"), more])
            box.pack_start(r, False, False, 0)
        box.show_all()
    search.connect("search-changed", lambda e: (state.update(text=e.get_text()), fill()))
    fill()
    package_info(apps, lambda info: (state.update(info=info), fill()))


def app_actions(app, a, kind, pkg):
    items = [("Open", lambda: ui.spawn(["gtk-launch", a.get_id()]))]
    if kind in ("rpm", "flatpak") and pkg:
        items.append(("Uninstall", lambda: uninstall(app, a, kind, pkg)))
    items.append(("Advanced options", lambda: app_details(app, a, kind, pkg)))
    return items


def uninstall(app, a, kind, pkg):
    def go():
        d = ui.Dialog(app.win, f"Uninstalling {a.get_display_name()}\u2026", None, None)
        d.add_text("This can take a minute.")
        d.run_dialog()
        argv = ["pkcon", "remove", "-y", "--plain", pkg] if kind == "rpm" else \
            ["flatpak", "uninstall", "-y", "--noninteractive", pkg]

        def done(ok, _o, err):
            d.destroy()
            if ok:
                ui.notify(f"{a.get_display_name()} was uninstalled", "", "user-trash")
                GLib.timeout_add(800, lambda: app.refresh() and False)
            else:
                ui.error(app.win, "Couldn\u2019t uninstall", err.strip() or
                         "Another app may depend on it. Try removing it with dnf in a terminal.")
        ui.run(argv, done)
    ui.confirm(app.win, f"Uninstall {a.get_display_name()}?",
               "This app and its related info will be uninstalled.", "Uninstall", go)


def app_details(app, a, kind, pkg):
    d = ui.Dialog(app.win, a.get_display_name(), None, "Close", 520)
    fields = [("Description", a.get_description() or a.get_generic_name() or ""),
              ("Command", a.get_commandline() or ""),
              ("Desktop file", a.get_filename() or "")]
    if kind == "rpm":
        fields.append(("Package", ui.out(["rpm", "-q", pkg]).strip()))
        fields.append(("Source", ui.out(["rpm", "-q", "--qf", "%{VENDOR}  (%{URL})", pkg]).strip()))
    elif kind == "flatpak":
        fields.append(("Flatpak", pkg))
    types = a.get_supported_types() or []
    if types:
        fields.append(("Opens", ", ".join(types[:12]) + (" \u2026" if len(types) > 12 else "")))
    for k, v in fields:
        if v:
            d.body.pack_start(ui.texts(k, v), False, False, 0)
    if kind == "flatpak":
        d.body.pack_start(ui.button("Reset app data", lambda: ui.confirm(
            app.win, "Reset app data?", "The app\u2019s settings and data on this PC will be deleted.", "Reset",
            lambda: ui.run(["flatpak", "uninstall", "--delete-data", "-y", "--noninteractive", pkg]) and None)),
            False, False, 8)
    d.run_dialog()


# --- Default apps -------------------------------------------------------------------------

def build_defaults(app, body):
    body.pack_start(ui.section("Set defaults for file and link types"), False, False, 0)
    for title, icon_name, types in DEFAULT_GROUPS:
        choices = {}
        for t in types:
            for a in Gio.AppInfo.get_all_for_type(t):
                choices.setdefault(a.get_id(), a)
        current = Gio.AppInfo.get_default_for_type(types[0], False)
        if not choices:
            continue

        def set_default(app_id, types=types, choices=choices):
            a = choices[app_id]
            for t in types:
                if t in (a.get_supported_types() or []) or t.startswith("x-scheme-handler/"):
                    try:
                        a.set_as_default_for_type(t)
                    except GLib.Error:
                        pass
        body.pack_start(ui.row(title, current.get_display_name() if current else "Choose a default",
                               icon_name, ui.combo([(i, a.get_display_name()) for i, a in
                                                    sorted(choices.items(), key=lambda x: x[1].get_display_name())],
                                                   current.get_id() if current else None, set_default, 240)),
                        False, False, 0)

    body.pack_start(ui.section("Set a default for a file type"), False, False, 0)
    holder = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    e = ui.entry("", "Enter a file type, like .md or .mp3", 320)

    def lookup(*_):
        ui.clear(holder)
        text = e.get_text().strip()
        if not text:
            return
        mime = text if "/" in text else Gio.content_type_guess("file" + (text if text.startswith(".") else "." + text),
                                                                None)[0]
        apps = Gio.AppInfo.get_all_for_type(mime)
        current = Gio.AppInfo.get_default_for_type(mime, False)
        desc = Gio.content_type_get_description(mime)
        if not apps:
            holder.pack_start(ui.row(desc, f"{mime}: no installed app opens this type", "dialog-question"),
                              False, False, 0)
        else:
            holder.pack_start(ui.row(desc, mime, Gio.content_type_get_icon(mime),
                                     ui.combo([(a.get_id(), a.get_display_name()) for a in apps],
                                              current.get_id() if current else None,
                                              lambda i: next(a for a in apps if a.get_id() == i)
                                              .set_as_default_for_type(mime), 240)), False, False, 0)
        holder.show_all()
    e.connect("activate", lookup)
    body.pack_start(ui.row("File type", None, "text-x-generic-symbolic", [e, ui.button("Search", lookup)]),
                    False, False, 0)
    body.pack_start(holder, False, False, 0)


# --- Startup ---------------------------------------------------------------------------------

def read_desktop(path):
    cp = configparser.RawConfigParser(strict=False, interpolation=None)
    cp.optionxform = str
    try:
        cp.read(path, encoding="utf-8")
        return dict(cp["Desktop Entry"]) if cp.has_section("Desktop Entry") else None
    except (configparser.Error, UnicodeDecodeError):
        return None


def write_desktop(path, entry):
    text = "[Desktop Entry]\n" + "".join(f"{k}={v}\n" for k, v in entry.items())
    ui.write_file(path, text)


def autostart_entries():
    entries = {}
    for d in AUTOSTART_DIRS:
        try:
            names = sorted(os.listdir(d))
        except OSError:
            continue
        for n in names:
            if n.endswith(".desktop"):
                e = read_desktop(os.path.join(d, n))
                if e is not None:
                    entries[n] = (e, d)
    return entries


def _desktop_tokens(value):
    return [x.lower() for x in value.split(";") if x]


def _mde_desktop(tokens):
    """True when OnlyShowIn/NotShowIn mentions MDE or labwc (DesktopNames=MDE:labwc)."""
    return "mde" in tokens or "labwc" in tokens or "sway" in tokens


def enabled_for_sway(e):
    if e.get("Hidden", "").lower() == "true" or e.get("X-GNOME-Autostart-enabled", "").lower() == "false":
        return False
    only = _desktop_tokens(e.get("OnlyShowIn", ""))
    never = _desktop_tokens(e.get("NotShowIn", ""))
    return (not only or _mde_desktop(only)) and not _mde_desktop(never)


def applies_to_sway(e):
    only = _desktop_tokens(e.get("OnlyShowIn", ""))
    return not only or _mde_desktop(only)


def set_startup(name, entry, on):
    user = os.path.join(ui.CONFIG, "autostart", name)
    e = dict(entry)
    e["Hidden"] = "false" if on else "true"
    e.pop("X-GNOME-Autostart-enabled", None)
    never = [x for x in e.get("NotShowIn", "").split(";")
             if x and x.lower() not in ("sway", "mde")]
    if on:
        if never:
            e["NotShowIn"] = ";".join(never) + ";"
        else:
            e.pop("NotShowIn", None)
    write_desktop(user, e)


def build_startup(app, body):
    body.pack_start(ui.label("Apps can start when you sign in. They start minimised, or only as a "
                             "background task.", "caption", wrap=True), False, False, 8)
    entries = autostart_entries()
    shown = sorted(((n, e, d) for n, (e, d) in entries.items() if applies_to_sway(e) or
                    _mde_desktop(_desktop_tokens(e.get("NotShowIn", "")))),
                   key=lambda x: x[1].get("Name", x[0]).casefold())
    for name, e, _d in shown:
        body.pack_start(ui.row(e.get("Name", name[:-8]), e.get("Comment"), e.get("Icon") or "application-x-executable",
                               ui.switch(enabled_for_sway(e), lambda v, n=name, e=e: set_startup(n, e, v))),
                        False, False, 0)

    def add():
        d = ui.Dialog(app.win, "Add an app to startup", "Add")
        apps = installed_apps()
        c = ui.combo([(a.get_id(), a.get_display_name()) for a in apps], None, lambda _v: None, 380)
        d.add_field("App", c)

        def go():
            if c.get_active() < 0:
                return
            a = apps[c.get_active()]
            e = read_desktop(a.get_filename()) or {}
            e["Hidden"] = "false"
            write_desktop(os.path.join(ui.CONFIG, "autostart", a.get_id()), e)
            d.destroy()
            app.refresh()
        d.on_primary(go)
        d.run_dialog()
    body.pack_start(ui.row("Add an app", "Start another app when you sign in", "list-add-symbolic",
                           ui.button("Browse apps", add)), False, False, 8)




PAGES = [
    dict(key="installed", title="Installed apps", parent="apps", icon="view-app-grid",
         caption="Uninstall and manage apps on your PC", keywords="uninstall remove programs features",
         build=build_installed, aliases=("appsfeatures-app", "installed-apps", "programs")),
    dict(key="defaultapps", title="Default apps", parent="apps", icon="preferences-desktop-default-applications",
         caption="Defaults for file and link types, other defaults",
         keywords="default browser email open with file association", build=build_defaults,
         aliases=("default-apps",)),
    dict(key="startup", title="Startup", parent="apps", icon="system-run",
         caption="Apps that start automatically when you sign in", keywords="autostart login boot",
         build=build_startup, aliases=("startupapps",)),
]
