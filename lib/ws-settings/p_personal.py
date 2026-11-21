# Settings > Personalization: Background, Colors, Themes, Lock screen, Fonts,
# Start and Taskbar.
import os
import re
import shutil

from gi.repository import Gdk, GdkPixbuf, GLib, Gtk, Pango

import bing
import ws_theme
import wsui as ui

FITS = [("fill", "Fill"), ("fit", "Fit"), ("stretch", "Stretch"), ("tile", "Tile"), ("center", "Centre")]
PICTURE_DIRS = ["/usr/local/share/backgrounds", "/usr/share/backgrounds"]
PICTURE_TYPES = (".jpg", ".jpeg", ".png", ".webp", ".jxl")
PRESETS = [("Light", "light", "light", "#0078d4", "#f3f3f3"),
           ("Dark", "dark", "dark", "#0078d4", "#202020"),
           ("Sunrise", "light", "light", "#da3b01", "#fbeee6"),
           ("Flow", "dark", "dark", "#0099bc", "#1b2a33"),
           ("Glow", "dark", "dark", "#8e8cd8", "#24203a"),
           ("Bloom", "light", "light", "#e3008c", "#f9e8f2")]
WS_START_CONF = os.path.join(ui.CONFIG, "ws-start", "settings.json")
START_LAYOUTS = [("more_pins", "More pins"), ("default", "Default"),
                 ("more_recommendations", "More recommendations")]
START_DEFAULTS = {"layout": "default", "recently_added": True, "most_used": False,
                  "recommended_files": True, "folders": []}
START_FOLDERS = [("settings", "Settings", "preferences-system"),
                 ("explorer", "File Explorer", "system-file-manager"),
                 ("documents", "Documents", "folder-documents"),
                 ("downloads", "Downloads", "folder-download"),
                 ("music", "Music", "folder-music"), ("pictures", "Pictures", "folder-pictures"),
                 ("videos", "Videos", "folder-videos"), ("network", "Network", "network-workgroup"),
                 ("personal", "Personal folder", "user-home")]


def thumbnail(path, w, h, color=None):
    area = Gtk.DrawingArea()
    area.set_size_request(w, h)
    pix = None
    if path and not color:
        try:
            pix = GdkPixbuf.Pixbuf.new_from_file_at_scale(path, w * 2, h * 2, True)
        except GLib.Error:
            pass

    def draw(_a, cr):
        cr.save()
        r = 6
        cr.new_sub_path()
        for i, (x, y) in enumerate(((w - r, r), (w - r, h - r), (r, h - r), (r, r))):
            cr.arc(x, y, r, (i - 1) * 1.5708, i * 1.5708)
        cr.close_path()
        cr.clip()
        if pix:
            k = max(w / pix.get_width(), h / pix.get_height())
            cr.scale(k, k)
            Gdk.cairo_set_source_pixbuf(cr, pix, (w / k - pix.get_width()) / 2, (h / k - pix.get_height()) / 2)
            cr.paint()
        else:
            c = color or "#808080"
            cr.set_source_rgb(*(int(c[i:i + 2], 16) / 255 for i in (1, 3, 5)))
            cr.paint()
        cr.restore()
    area.connect("draw", draw)
    return area


def current_wallpaper():
    s = ui.load()
    if s.get("wallpaper") or s.get("wallpaper_color"):
        return s.get("wallpaper"), s.get("wallpaper_fit", "fill"), s.get("wallpaper_color")
    m = re.search(r'^output \* bg "?([^"\n]+?)"? (\w+)', ui.read_file(os.path.join(ui.CONFIG, "sway", "config")), re.M)
    return (m[1], m[2], None) if m else (None, "fill", None)


def background_mode():
    mode = ui.load().get("wallpaper_mode")
    if mode in ("picture", "color", "spotlight"):
        return mode
    _path, _fit, color = current_wallpaper()
    return "color" if color else "picture"


def set_wallpaper(path=None, fit=None, color=None):
    s = ui.load()
    # Spotlight owns the lock screen. Leaving it makes the lock screen follow the desktop again.
    release_lock = s.get("wallpaper_mode") == "spotlight" and s.get("bing_owns_lock", True)
    recent = [p for p in s.get("wallpaper_recent", []) if p != path and os.path.exists(p)]
    if path:
        recent = [path] + recent
    cur_path, cur_fit, _c = current_wallpaper()
    fit = fit or cur_fit or "fill"
    fields = {"wallpaper_recent": recent[:5], "bing_owns_lock": False}
    if release_lock:
        fields["lock_image"] = None
    if color:
        bing.merge(wallpaper=None, wallpaper_color=color, wallpaper_mode="color", **fields)
        line = f"output * bg {color} solid_color"
    else:
        path = path or cur_path
        bing.merge(wallpaper=path, wallpaper_fit=fit, wallpaper_color=None, wallpaper_mode="picture", **fields)
        line = f'output * bg "{path}" {fit}'
    ui.sway_conf("wallpaper", line)
    accent_applied = False
    if ui.get("accent_auto") and path and not color:
        accent = ws_theme.wallpaper_accent(path)
        if accent:
            ui.save(accent=accent)
            ws_theme.apply()
            accent_applied = True
    if release_lock and not accent_applied:
        ws_theme.apply(reload=False)


def caption(hero, settings, waiting):
    for text, style in bing.preview_lines(settings, waiting):
        lab = ui.label(text, *style, wrap=True, xalign=0.5)
        lab.set_halign(Gtk.Align.CENTER)
        hero.pack_start(lab, False, False, 0)


def pictures():
    """Wallpapers shipped with the system, newest folders first, skipping portrait copies."""
    found = []
    for d in PICTURE_DIRS:
        for root, _dirs, files in sorted(os.walk(d), reverse=True):
            for f in sorted(files):
                if f.lower().endswith(PICTURE_TYPES) and "portrait" not in f.lower():
                    found.append(os.path.join(root, f))
    return found


def choose_picture(app, title, callback, folder=False):
    dlg = Gtk.FileChooserNative.new(title, app.win, Gtk.FileChooserAction.OPEN, "Choose picture", "Cancel")
    f = Gtk.FileFilter()
    f.set_name("Pictures")
    for mime in ("image/png", "image/jpeg", "image/webp", "image/gif"):
        f.add_mime_type(mime)
    dlg.add_filter(f)
    pics = ui.out(["xdg-user-dir", "PICTURES"]).strip()
    if pics:
        dlg.set_current_folder(pics)
    if dlg.run() == Gtk.ResponseType.ACCEPT:
        callback(dlg.get_filename())
    dlg.destroy()


def build_background(app, body):
    path, fit, color = current_wallpaper()
    mode = background_mode()
    hero = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8), "hero")
    prev = thumbnail(path, 320, 180, color)
    prev.set_halign(Gtk.Align.CENTER)
    hero.pack_start(prev, False, False, 0)
    if mode == "spotlight":
        caption(hero, ui.load(), bing.pending)
    body.pack_start(hero, False, False, 0)

    def on_mode(v):
        if v == "spotlight":
            bing.merge(wallpaper_mode="spotlight", bing_owns_lock=True)
            bing.ensure_timer()
            bing.refresh_async(lambda _ok: app.refresh())
            app.refresh()
            return
        if v == "color":
            set_wallpaper(color="#0063b1")
        else:
            set_wallpaper(path=(ui.load().get("wallpaper_recent") or pictures() or [None])[0])
        app.refresh()

    subtitle = ("A new Bing picture each day, on every desktop and the lock screen" if mode == "spotlight"
                else "A picture background applies to all your desktops")
    exp, inner = ui.expander("Personalise your background", subtitle, "preferences-desktop-wallpaper-symbolic",
                             ui.combo([("picture", "Picture"), ("color", "Solid colour"),
                                       ("spotlight", "Windows spotlight")], mode, on_mode, 220), expanded=True)
    if mode == "picture":
        grid = Gtk.FlowBox(max_children_per_line=5, min_children_per_line=3,
                           selection_mode=Gtk.SelectionMode.NONE, column_spacing=8, row_spacing=8)
        recent = [p for p in ui.load().get("wallpaper_recent", []) if os.path.exists(p)]
        for p in (recent + [x for x in pictures() if x not in recent])[:10]:
            b = ui.css(Gtk.Button(tooltip_text=os.path.basename(p), halign=Gtk.Align.START), "thumb")
            if p == path:
                ui.css(b, "active")
            b.add(thumbnail(p, 96, 54))
            b.connect("clicked", lambda _b, p=p: (set_wallpaper(path=p), app.refresh()))
            grid.add(b)
        card = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8), "card", "tall")
        card.pack_start(ui.label("Recent images", "caption"), False, False, 0)
        card.pack_start(grid, False, False, 0)
        inner.pack_start(card, False, False, 0)
        inner.pack_start(ui.row("Choose a photo", None, None, ui.button(
            "Browse photos", lambda: choose_picture(app, "Choose a background",
                                                    lambda f: (set_wallpaper(path=f), app.refresh())))),
            False, False, 0)
        inner.pack_start(ui.row("Choose a fit for your desktop image", None, None,
                                ui.combo(FITS, fit, lambda v: (set_wallpaper(fit=v), app.refresh()), 160)),
                         False, False, 0)
    elif mode == "color":
        inner.pack_start(swatch_card("Choose your background colour", color,
                                     lambda c: (set_wallpaper(color=c), app.refresh()), app), False, False, 0)
    else:
        inner.pack_start(ui.row("Recent images", "Today's picture is kept with your other backgrounds",
                                None), False, False, 0)
    body.pack_start(exp, False, False, 0)


def swatch_card(title, active, callback, app, recent=()):
    card = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8), "card", "tall")
    if recent:
        card.pack_start(ui.label(title, "caption"), False, False, 0)
        card.pack_start(swatch_grid(recent, active, callback), False, False, 0)
        card.pack_start(ui.label("Windows colours", "caption"), False, False, 4)
    else:
        card.pack_start(ui.label("Windows colours" if title == "Recent colours" else title, "caption"),
                        False, False, 0)
    card.pack_start(swatch_grid(ws_theme.PALETTE, active, callback), False, False, 0)

    def custom():
        d = Gtk.ColorChooserDialog(title="Choose a custom colour", transient_for=app.win)
        d.set_use_alpha(False)
        rgba = Gdk.RGBA()
        if active and rgba.parse(active):
            d.set_rgba(rgba)
        if d.run() == Gtk.ResponseType.OK:
            c = d.get_rgba()
            callback("#%02x%02x%02x" % (round(c.red * 255), round(c.green * 255), round(c.blue * 255)))
        d.destroy()
    b = ui.button("View colours", custom)
    b.set_halign(Gtk.Align.START)
    card.pack_start(ui.row("Custom colours", None, None, b), False, False, 0)
    return card


def swatch_grid(colors, active, callback):
    grid = Gtk.FlowBox(max_children_per_line=8, min_children_per_line=8,
                       selection_mode=Gtk.SelectionMode.NONE, column_spacing=4, row_spacing=4,
                       homogeneous=True, halign=Gtk.Align.START)
    for c in colors:
        b = ui.css(Gtk.Button(tooltip_text=c), "swatch")
        if active and c.lower() == active.lower():
            ui.css(b, "active")
        b.add(thumbnail(None, 36, 36, c))
        b.connect("clicked", lambda _b, c=c: callback(c))
        grid.add(b)
    return grid


def set_theme(app, **kw):
    ui.save(**kw)
    s = ui.load()
    recent = [c for c in s.get("accent_recent", []) if c != s["accent"]]
    ui.save(accent_recent=[s["accent"]] + recent[:4])
    ws_theme.apply()
    app.restyle()
    app.refresh()


def build_colors(app, body):
    s = ui.load()
    if s["windows_mode"] == s["app_mode"]:
        mode = s["windows_mode"]
    else:
        mode = "custom"

    def set_mode(v):
        if v == "custom":
            ui.save(windows_mode="dark", app_mode="light")
            set_theme(app)
        else:
            set_theme(app, windows_mode=v, app_mode=v)
    exp, inner = ui.expander("Choose your mode",
                             "Change the colours that appear in the taskbar, menus and your apps",
                             "applications-graphics-symbolic",
                             ui.combo([("light", "Light"), ("dark", "Dark"), ("custom", "Custom")], mode, set_mode, 140),
                             expanded=mode == "custom")
    for key, title in (("windows_mode", "Choose your default Windows mode"),
                       ("app_mode", "Choose your default app mode")):
        inner.pack_start(ui.row(title, None, None,
                                ui.combo([("light", "Light"), ("dark", "Dark")], s[key],
                                         lambda v, k=key: set_theme(app, **{k: v}), 140)), False, False, 0)
    body.pack_start(exp, False, False, 0)

    def pick(c):
        set_theme(app, accent=c, accent_auto=False)

    def set_auto(v):
        if v == "auto":
            path, _f, _c = current_wallpaper()
            accent = ws_theme.wallpaper_accent(path) if path else None
            set_theme(app, accent_auto=True, **({"accent": accent} if accent else {}))
        else:
            set_theme(app, accent_auto=False)
    exp, inner = ui.expander("Accent colour", None, "color-select-symbolic",
                             ui.combo([("manual", "Manual"), ("auto", "Automatic")],
                                      "auto" if s["accent_auto"] else "manual", set_auto, 140),
                             expanded=not s["accent_auto"])
    inner.pack_start(swatch_card("Recent colours", s["accent"], pick, app,
                                 [c for c in s.get("accent_recent", []) if c][:5]), False, False, 0)
    body.pack_start(exp, False, False, 0)
    body.pack_start(ui.row("Show accent colour on title bars and window borders", None,
                           "window-new-symbolic",
                           ui.switch(s["accent_titlebars"], lambda v: set_theme(app, accent_titlebars=v))),
                    False, False, 0)


def theme_dirs(kind):
    """Installed GTK 3 ('gtk-3.0'), icon ('index.theme') or cursor ('cursors') themes."""
    names = set()
    roots = ["/usr/share/themes", "/usr/local/share/themes", os.path.join(ui.HOME, ".themes"),
             os.path.join(ui.HOME, ".local/share/themes")] if kind == "gtk-3.0" else \
        ["/usr/share/icons", "/usr/local/share/icons", os.path.join(ui.HOME, ".icons"),
         os.path.join(ui.HOME, ".local/share/icons")]
    for root in roots:
        try:
            for n in os.listdir(root):
                p = os.path.join(root, n)
                if kind == "gtk-3.0" and os.path.isdir(os.path.join(p, "gtk-3.0")):
                    names.add(n)
                elif kind == "cursors" and os.path.isdir(os.path.join(p, "cursors")):
                    names.add(n)
                elif kind == "icons" and os.path.isfile(os.path.join(p, "index.theme")) \
                        and not os.path.isdir(os.path.join(p, "cursors")) and n not in ("default", "hicolor", "locolor"):
                    names.add(n)
        except OSError:
            continue
    return sorted(names, key=str.casefold)


def gsetting(key, value=None, schema="org.gnome.desktop.interface"):
    if value is None:
        return ui.out(["gsettings", "get", schema, key]).strip().strip("'")
    ui.run(["gsettings", "set", schema, key, str(value)])


def set_ini(key, value):
    ini = os.path.join(ui.CONFIG, "gtk-3.0", "settings.ini")
    text = ui.read_file(ini, "[Settings]\n")
    if re.search(rf"(?m)^{key}=", text):
        text = re.sub(rf"(?m)^{key}=.*$", f"{key}={value}", text)
    else:
        text += f"{key}={value}\n"
    ui.write_file(ini, text)


def build_themes(app, body):
    s = ui.load()
    body.pack_start(ui.section("Current theme"), False, False, 0)
    grid = Gtk.FlowBox(max_children_per_line=3, min_children_per_line=3,
                       selection_mode=Gtk.SelectionMode.NONE, column_spacing=8, row_spacing=8, homogeneous=True)
    for name, wmode, amode, accent, bg in PRESETS:
        b = ui.css(Gtk.Button(), "thumb")
        if s["windows_mode"] == wmode and s["app_mode"] == amode and s["accent"].lower() == accent:
            ui.css(b, "active")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        overlay = Gtk.Overlay()
        overlay.add(thumbnail(None, 160, 90, bg))
        bar = thumbnail(None, 48, 8, accent)
        bar.set_halign(Gtk.Align.CENTER)
        bar.set_valign(Gtk.Align.END)
        bar.set_margin_bottom(12)
        overlay.add_overlay(bar)
        box.pack_start(overlay, False, False, 0)
        box.pack_start(ui.label(name, xalign=0.5), False, False, 0)
        b.add(box)
        b.connect("clicked", lambda _b, w=wmode, a=amode, c=accent: set_theme(
            app, windows_mode=w, app_mode=a, accent=c, accent_auto=False))
        grid.add(b)
    body.pack_start(grid, False, False, 0)
    body.pack_start(ui.section("Theme parts"), False, False, 0)
    light = s.get("gtk_theme") or ws_theme.LIGHT_GTK

    def set_gtk(v):
        ui.save(gtk_theme=v)
        ws_theme.apply()
    body.pack_start(ui.row("App theme", "The look of GTK apps in light mode", "preferences-desktop-theme-symbolic",
                           ui.combo([(n, n) for n in theme_dirs("gtk-3.0")], light, set_gtk, 220)), False, False, 0)
    icons = gsetting("icon-theme")

    def set_icons(v):
        gsetting("icon-theme", v)
        set_ini("gtk-icon-theme-name", v)
        ui.save(icon_theme=v)
    body.pack_start(ui.row("Icons", "Icons in apps and the file manager", "preferences-desktop-icons-symbolic",
                           ui.combo([(n, n) for n in theme_dirs("icons")], icons, set_icons, 220)), False, False, 0)
    body.pack_start(ui.nav_row("Mouse pointer", "Pointer style and size", "input-mouse-symbolic",
                               lambda: app.go("pointer")), False, False, 0)


def build_lockscreen(app, body):
    s = ui.load()
    lock = s.get("lock_image") or current_wallpaper()[0]
    spotlight = s.get("wallpaper_mode") == "spotlight" and s.get("bing_owns_lock", True)
    hero = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8), "hero")
    prev = thumbnail(lock, 320, 180)
    prev.set_halign(Gtk.Align.CENTER)
    hero.pack_start(prev, False, False, 0)
    if spotlight:
        caption(hero, s, False)
    body.pack_start(hero, False, False, 0)

    def set_(**kw):
        bing.merge(**kw)
        ws_theme.apply(reload=False)
        app.refresh()
    if spotlight:
        source = "spotlight"
        options = [("spotlight", "Windows spotlight"), ("picture", "Picture")]
    else:
        source = "desktop" if not s.get("lock_image") else "picture"
        options = [("desktop", "Same as desktop background"), ("picture", "Picture")]

    def on_source(v):
        if v == "spotlight":
            set_(lock_image=s.get("wallpaper") or lock, bing_owns_lock=True)
        elif v == "desktop":
            set_(lock_image=None, bing_owns_lock=False)
        else:
            set_(lock_image=lock, bing_owns_lock=False)

    exp, inner = ui.expander("Personalise your lock screen",
                             "Windows spotlight uses the same Bing picture as your desktop" if spotlight else None,
                             "system-lock-screen-symbolic",
                             ui.combo(options, source, on_source, 220),
                             expanded=source == "picture" or spotlight)
    if source == "picture":
        inner.pack_start(ui.row("Choose a photo", None, None, ui.button(
            "Browse photos", lambda: choose_picture(app, "Choose a lock screen picture",
                                                    lambda f: set_(lock_image=f)))), False, False, 0)
    body.pack_start(exp, False, False, 0)
    for key, title, caption in (("lock_failed_attempts", "Show failed sign-in attempts", None),
                                ("lock_keyboard_layout", "Show the keyboard layout", None),
                                ("lock_caps_lock", "Show when Caps Lock is on", None)):
        body.pack_start(ui.row(title, caption, None,
                               ui.switch(s.get(key, key != "lock_keyboard_layout"),
                                         lambda v, k=key: set_(**{k: v}))), False, False, 0)
    body.pack_start(ui.row("Lock now", "Super+L locks your PC", "system-lock-screen-symbolic",
                           ui.button("Lock", lambda: ui.spawn("ws-lock"))), False, False, 0)
    body.pack_start(ui.section("Related settings"), False, False, 0)
    body.pack_start(ui.nav_row("Screen timeout", "Choose when the screen locks and turns off",
                               "preferences-desktop-screensaver", lambda: app.go("power")), False, False, 0)


def font_families():
    out = ui.out(["fc-list", "--format", "%{family[0]}\n"])
    return sorted({f.strip() for f in out.splitlines() if f.strip()}, key=str.casefold)


def build_fonts(app, body):
    def install():
        dlg = Gtk.FileChooserNative.new("Install fonts", app.win, Gtk.FileChooserAction.OPEN, "Install", "Cancel")
        dlg.set_select_multiple(True)
        f = Gtk.FileFilter()
        f.set_name("Fonts")
        for pat in ("*.ttf", "*.otf", "*.TTF", "*.OTF", "*.ttc"):
            f.add_pattern(pat)
        dlg.add_filter(f)
        if dlg.run() == Gtk.ResponseType.ACCEPT:
            dest = os.path.join(ui.HOME, ".local/share/fonts")
            os.makedirs(dest, exist_ok=True)
            for path in dlg.get_filenames():
                shutil.copy(path, dest)
            ui.run(["fc-cache", "-f", dest], lambda *_: app.refresh())
        dlg.destroy()
    body.pack_start(ui.row("Add fonts", "Choose font files to install for your account",
                           "font-x-generic-symbolic", ui.button("Browse and install fonts", install)),
                    False, False, 0)
    families = font_families()
    head = Gtk.Box(spacing=12)
    head.pack_start(ui.label("Available fonts", "section"), True, True, 0)
    search = Gtk.SearchEntry(placeholder_text="Search fonts", valign=Gtk.Align.CENTER)
    search.set_size_request(240, -1)
    head.pack_end(search, False, False, 0)
    body.pack_start(head, False, False, 0)
    body.pack_start(ui.label(f"{len(families)} font families installed", "caption"), False, False, 4)
    grid = Gtk.FlowBox(max_children_per_line=3, min_children_per_line=2,
                       selection_mode=Gtk.SelectionMode.NONE, column_spacing=4, row_spacing=4, homogeneous=True)
    body.pack_start(grid, False, False, 0)

    def fill(text=""):
        ui.clear(grid)
        q = text.casefold()
        shown = [f for f in families if q in f.casefold()][:60]
        for fam in shown:
            b = ui.css(Gtk.Button(relief=Gtk.ReliefStyle.NONE), "card", "tall")
            box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
            sample = Gtk.Label(xalign=0)
            sample.set_markup(f'<span font_family="{GLib.markup_escape_text(fam)}" size="xx-large">'
                              f'The quick brown fox</span>')
            sample.set_ellipsize(Pango.EllipsizeMode.END)
            box.pack_start(sample, False, False, 0)
            box.pack_start(ui.label(fam), False, False, 0)
            b.add(box)
            b.connect("clicked", lambda _b, f=fam: font_details(app, f))
            grid.add(b)
        grid.show_all()
    search.connect("search-changed", lambda e: fill(e.get_text()))
    fill()


def font_details(app, family):
    d = ui.Dialog(app.win, family, None, "Close", 560)
    for size in ("large", "x-large", "xx-large"):
        lab = Gtk.Label(xalign=0)
        lab.set_markup(f'<span font_family="{GLib.markup_escape_text(family)}" size="{size}">'
                       "The quick brown fox jumps over the lazy dog. 1234567890</span>")
        lab.set_line_wrap(True)
        d.body.pack_start(lab, False, False, 0)
    files = ui.out(["fc-list", f":family={family}", "--format", "%{style[0]}\t%{file}\n"]).strip().splitlines()
    for line in sorted(files)[:12]:
        style, _t, path = line.partition("\t")
        d.body.pack_start(ui.texts(style or "Regular", path), False, False, 0)
    d.run_dialog()


def start_settings():
    return {**START_DEFAULTS, **ui.json_file(WS_START_CONF, {})}


def save_start(**kw):
    s = start_settings()
    s.update(kw)
    import json
    ui.write_file(WS_START_CONF, json.dumps(s, indent=2) + "\n")


def build_start(app, body):
    s = start_settings()
    card = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12), "card", "tall")
    card.pack_start(ui.label("Layout"), False, False, 0)
    radios = Gtk.Box(spacing=24)
    group = None
    for key, name in START_LAYOUTS:
        radio = Gtk.RadioButton.new_with_label_from_widget(group, name)
        group = group or radio
        radio.set_active(s["layout"] == key)
        radio.connect("toggled", lambda r, k=key: r.get_active() and save_start(layout=k))
        radios.pack_start(radio, False, False, 0)
    card.pack_start(radios, False, False, 0)
    body.pack_start(card, False, False, 0)
    for key, title, caption in (
            ("recently_added", "Show recently added apps", None),
            ("most_used", "Show most used apps", "Counts the apps you open from Start"),
            ("recommended_files", "Show recommended files in Start, recent files in File Explorer, "
             "and items in Jump Lists", None)):
        body.pack_start(ui.row(title, caption, None,
                               ui.switch(s[key], lambda v, k=key: save_start(**{k: v}))), False, False, 0)


def build_start_folders(app, body):
    s = start_settings()
    body.pack_start(ui.label("These folders appear on Start next to the power button", "caption"), False, False, 8)

    def set_folder(key, on):
        chosen = [k for k, *_ in START_FOLDERS
                  if (k == key and on) or (k != key and k in start_settings()["folders"])]
        save_start(folders=chosen)
    for key, name, icon_name in START_FOLDERS:
        body.pack_start(ui.row(name, None, icon_name + "-symbolic",
                               ui.switch(key in s["folders"], lambda v, k=key: set_folder(k, v))), False, False, 0)


TRAY_SEEN = os.path.join(os.environ.get("XDG_CACHE_HOME", os.path.expanduser("~/.cache")),
                         "ws-taskbar", "tray-seen.json")


def set_taskbar(**kw):
    ui.save(**kw)
    ws_theme.apply_taskbar()


def build_taskbar(app, body):
    s = ui.load()
    body.pack_start(ui.section("Taskbar items"), False, False, 0)
    body.pack_start(ui.row("Search", None, "system-search-symbolic",
                           ui.combo([("hide", "Hide"), ("icon", "Search icon only"),
                                     ("label", "Search icon and label"), ("box", "Search box")],
                                    s["taskbar_search"], lambda v: set_taskbar(taskbar_search=v), 200)),
                    False, False, 0)
    body.pack_start(ui.row("Task view", None, "view-dual-symbolic",
                           ui.switch(s["taskbar_taskview"], lambda v: set_taskbar(taskbar_taskview=v))),
                    False, False, 0)

    body.pack_start(ui.section("System tray icons"), False, False, 0)
    body.pack_start(ui.row("Hidden icon menu", "Show the hidden icon menu (^) for icons you turn off below",
                           "go-up-symbolic",
                           ui.switch(s["tray_overflow"], lambda v: set_taskbar(tray_overflow=v))), False, False, 0)
    seen = ui.json_file(TRAY_SEEN, {})
    exp, inner = ui.expander("Other system tray icons", "Show or hide additional system tray icons",
                             "view-more-horizontal-symbolic")
    if not seen:
        inner.pack_start(ui.row("No apps have shown a tray icon yet", None, None), False, False, 0)

    def set_tray(ident, shown):
        hidden = [i for i in ui.get("tray_hidden") if i != ident] + ([] if shown else [ident])
        set_taskbar(tray_hidden=hidden)
    for ident, title in sorted(seen.items(), key=lambda kv: kv[1].lower()):
        inner.pack_start(ui.row(title, None, None, ui.switch(ident not in s["tray_hidden"],
                                                            lambda v, i=ident: set_tray(i, v))), False, False, 0)
    body.pack_start(exp, False, False, 0)

    exp, inner = ui.expander("Taskbar behaviours", "Taskbar alignment, badging, automatically hide, and "
                             "multiple displays", "preferences-desktop-symbolic")
    inner.pack_start(ui.row("Taskbar alignment", None, None,
                            ui.combo([("center", "Center"), ("left", "Left")], s["taskbar_align"],
                                     lambda v: set_taskbar(taskbar_align=v), 140)), False, False, 0)
    for key, title, caption in (
        ("taskbar_autohide", "Automatically hide the taskbar", None),
        ("taskbar_badges", "Show badges on taskbar apps", "Counts of unread notifications"),
        ("taskbar_flash", "Show flashing on taskbar apps", "When an app needs your attention"),
        ("taskbar_all_displays", "Show my taskbar on all displays", None),
        ("taskbar_corner", "Select the far corner of the taskbar to show the desktop", None),
        ("clock_seconds", "Show seconds in system tray clock", "Uses more power"),
    ):
        inner.pack_start(ui.row(title, caption, None,
                                ui.switch(s.get(key, False), lambda v, k=key: set_taskbar(**{k: v}))),
                         False, False, 0)
    inner.pack_start(ui.row("Combine taskbar buttons and hide labels", None, None,
                            ui.combo([("always", "Always"), ("full", "When taskbar is full"), ("never", "Never")],
                                     s["taskbar_combine"], lambda v: set_taskbar(taskbar_combine=v), 200)),
                     False, False, 0)
    body.pack_start(exp, False, False, 0)


def build_personalization(app, body):
    path, _fit, color = current_wallpaper()
    hero = ui.css(Gtk.Box(spacing=24), "hero")
    hero.pack_start(thumbnail(path, 240, 135, color), False, False, 0)
    col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, valign=Gtk.Align.CENTER)
    col.pack_start(ui.label("Select a theme to apply", "hero-title"), False, False, 0)
    row_ = Gtk.Box(spacing=8)
    s = ui.load()
    for name, wmode, amode, accent, bg in PRESETS[:4]:
        b = ui.css(Gtk.Button(tooltip_text=name), "thumb")
        if s["windows_mode"] == wmode and s["accent"].lower() == accent:
            ui.css(b, "active")
        b.add(thumbnail(None, 64, 40, bg))
        b.connect("clicked", lambda _b, w=wmode, a=amode, c=accent: set_theme(
            app, windows_mode=w, app_mode=a, accent=c, accent_auto=False))
        row_.pack_start(b, False, False, 0)
    col.pack_start(row_, False, False, 0)
    hero.pack_start(col, True, True, 0)
    body.pack_start(hero, False, False, 8)


CATEGORY = {"personalization": build_personalization}

PAGES = [
    dict(key="background", title="Background", parent="personalization", icon="preferences-desktop-wallpaper",
         caption="Background image, colour, Windows spotlight", keywords="wallpaper desktop picture photo bing spotlight",
         build=build_background, aliases=("personalization-background", "wallpaper")),
    dict(key="colors", title="Colours", parent="personalization", icon="preferences-color",
         caption="Accent colour, transparency effects, colour theme",
         keywords="color colour dark mode light mode accent theme night", build=build_colors,
         aliases=("personalization-colors", "colours", "color", "dark-mode")),
    dict(key="themes", title="Themes", parent="personalization", icon="preferences-desktop-theme",
         caption="Install, create, manage", keywords="gtk theme icons cursor look", build=build_themes),
    dict(key="lockscreen", title="Lock screen", parent="personalization", icon="system-lock-screen",
         caption="Lock screen images, apps, animations", keywords="swaylock picture lock",
         build=build_lockscreen, aliases=("lock-screen",)),
    dict(key="fonts", title="Fonts", parent="personalization", icon="preferences-desktop-font",
         caption="Install, manage", keywords="typeface font family install", build=build_fonts),
    dict(key="start", title="Start", parent="personalization", icon="start-here",
         caption="Recent apps and items, folders", keywords="start menu pins recommended layout",
         build=build_start, aliases=("personalization-start",)),
    dict(key="startfolders", title="Folders", parent="start", icon="folder",
         caption="Choose which folders appear on Start next to the power button",
         keywords="start folders power button", build=build_start_folders,
         aliases=("personalization-start-places",)),
    dict(key="taskbar", title="Taskbar", parent="personalization", icon="preferences-desktop",
         caption="Taskbar behaviours, system tray icons", keywords="panel tray clock seconds align hide pin search task view badges",
         build=build_taskbar),
]
