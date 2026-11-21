#!/usr/bin/env python3
# Windows 11 colours for the Sway shell: Light or Dark mode and the accent
# colour, from ~/.config/ws-settings/settings.json. The shell's styles are
# rendered light with the default accent (the copies in base/); transform()
# turns that into the chosen theme. Used by ws-settings, ws-start and ws-winx,
# and as "ws_theme.py apply" to restyle the panel, notifications and menus.
# Installed by lib/look_sway.sh, which fills in the @...@ values.
import colorsys
import json
import os
import re
import subprocess
import sys

SHARE = os.path.dirname(os.path.abspath(__file__))
BASE = os.path.join(SHARE, "base")
BASE_ACCENT = "@ACCENT@"
BASE_ACCENT_TEXT = "@ACCENT_TEXT@"
LIGHT_GTK = "@GTK_THEME@"
DARK_GTK = "adw-gtk3-dark"

# Windows 11 accent colours (Settings > Personalization > Colors), in rows of 8.
PALETTE = [
    "#ffb900", "#ff8c00", "#f7630c", "#ca5010", "#da3b01", "#ef6950", "#d13438", "#ff4343",
    "#e74856", "#e81123", "#ea005e", "#c30052", "#e3008c", "#bf0077", "#c239b3", "#9a0089",
    "#0078d4", "#0063b1", "#8e8cd8", "#6b69d6", "#8764b8", "#744da9", "#b146c2", "#881798",
    "#0099bc", "#2d7d9a", "#00b7c3", "#038387", "#00b294", "#018574", "#00cc6a", "#10893e",
    "#7a7574", "#5d5a58", "#68768a", "#515c6b", "#567c73", "#486860", "#498205", "#107c10",
    "#767676", "#4c4a48", "#69797e", "#4a5459", "#647c64", "#525e54", "#847545", "#7e735f",
]

DEFAULTS = {"windows_mode": "light", "app_mode": "light", "accent": BASE_ACCENT,
            "accent_auto": False, "accent_titlebars": True, "gtk_theme": None,
            "lock_image": None, "lock_failed_attempts": True, "lock_keyboard_layout": False,
            "lock_caps_lock": True, "taskbar_align": "center", "taskbar_search": "hide",
            "taskbar_taskview": True, "taskbar_autohide": False, "taskbar_badges": True,
            "taskbar_flash": True, "taskbar_all_displays": True, "taskbar_corner": True,
            "taskbar_combine": "always", "taskbar_all_desktops": False,
            "tray_hidden": ["nm-applet", "blueman"], "tray_overflow": True}

# Fluent light literals used by the shell styles, and their dark equivalents.
# Shadows ("px alpha(#000000") and the light layer fill are handled first.
DARK_MAP = [
    ("alpha(#ffffff, 0.7)", "alpha(#WHITE0, 0.06)"),
    ("alpha(#ffffff, 0.5)", "alpha(#WHITE0, 0.04)"),
    ("alpha(#ffffff,", "alpha(#WHITE0,"),
    ("alpha(#f9f9f9,", "alpha(#2c2c2c,"),
    ("alpha(#000000,", "alpha(#WHITE0,"),
    ("#ffffff", "#1c1c1c"),
    ("#fbfbfb", "#2b2b2b"),
    ("#f9f9f9", "#2c2c2c"),
    ("#f5f5f7", "#202020"),
    ("#f3f3f3", "#202020"),
    ("#f0f0f2", "#2b2b2b"),
    ("#e0e0e0", "#3a3a3a"),
    ("#d0d0d5", "#454545"),
    ("#b8b8bd", "#5a5a5a"),
    ("#a0a0a5", "#7a7a7a"),
    ("#767676", "#9a9a9a"),
    ("#606984", "#8a93ab"),
    ("#5d5d5d", "#c5c5c5"),
    ("#1a1a1a", "#ffffff"),
]


# Windows 11 contrast themes: base mode, extra colour swaps, accent fill and link.
CONTRAST = {
    "aquatic": {"name": "Aquatic", "base": "dark", "accent": "#8ee3f0", "link": "#75e9fc",
                "map": [("#202020", "#202020"), ("#c5c5c5", "#ffffff")]},
    "desert": {"name": "Desert", "base": "light", "accent": "#903909", "link": "#1c5e75",
               "map": [("#f3f3f3", "#fffaef"), ("#fbfbfb", "#fffaef"), ("#f9f9f9", "#fffaef"),
                       ("#1a1a1a", "#3d3d3d"), ("#5d5d5d", "#3d3d3d")]},
    "dusk": {"name": "Dusk", "base": "dark", "accent": "#a1bfde", "link": "#70ebde",
             "map": [("#ffffff", "#b6f6f0"), ("#c5c5c5", "#b6f6f0"), ("#202020", "#2d3236"),
                     ("#2b2b2b", "#2d3236"), ("#2c2c2c", "#2d3236"), ("#1c1c1c", "#2d3236")]},
    "night_sky": {"name": "Night sky", "base": "dark", "accent": "#d6b4fd", "link": "#8080ff",
                  "map": [("#c5c5c5", "#ffffff"), ("#202020", "#000000"), ("#2b2b2b", "#000000"),
                          ("#2c2c2c", "#000000"), ("#1c1c1c", "#000000")]},
}


def config_dir():
    return os.path.join(os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config")))


def settings_path():
    return os.path.join(config_dir(), "ws-settings", "settings.json")


def load():
    try:
        with open(settings_path()) as f:
            data = json.load(f)
    except (OSError, ValueError):
        data = {}
    return {**DEFAULTS, **(data if isinstance(data, dict) else {})}


def hls(hexcolor):
    r, g, b = (int(hexcolor[i:i + 2], 16) / 255 for i in (1, 3, 5))
    return colorsys.rgb_to_hls(r, g, b)


def from_hls(h, l, s):
    r, g, b = colorsys.hls_to_rgb(h, max(0.0, min(1.0, l)), max(0.0, min(1.0, s)))
    return "#%02x%02x%02x" % (round(r * 255), round(g * 255), round(b * 255))


def shades(accent):
    """Fluent accent ramp: (fill, text) for light mode and for dark mode."""
    h, l, s = hls(accent)
    light = (accent, from_hls(h, l * 0.87, s))
    dark = (from_hls(h, max(l, 0.69), s), from_hls(h, max(l, 0.80), s))
    return {"light": light, "dark": dark}


def readable_on(fill):
    r, g, b = (int(fill[i:i + 2], 16) / 255 for i in (1, 3, 5))
    return "#000000" if 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.55 else "#ffffff"


def contrast(theme):
    return CONTRAST.get(theme.get("contrast") or "")


def is_dark(theme=None, which="windows_mode"):
    theme = theme or load()
    c = contrast(theme)
    return c["base"] == "dark" if c else theme[which] == "dark"


def accents(theme=None, which="windows_mode"):
    theme = theme or load()
    c = contrast(theme)
    if c:
        return c["accent"], c["link"]
    return shades(theme["accent"])["dark" if is_dark(theme, which) else "light"]


def transform(text, theme=None, which="windows_mode"):
    """Restyle text written for light mode with the default accent."""
    theme = theme or load()
    fill, link = accents(theme, which)
    text = re.sub(r"(px\s+)alpha\(#000000", r"\1alpha(#SHADOW0", text)
    if is_dark(theme, which):
        for light, dark in DARK_MAP:
            text = text.replace(light, dark)
    c = contrast(theme)
    if c:
        swaps = dict(c["map"])
        text = re.sub("|".join(re.escape(k) for k in swaps), lambda m: swaps[m.group(0)], text)
    text = text.replace(BASE_ACCENT_TEXT, "#ACCENTTEXT").replace(BASE_ACCENT, "#ACCENTFILL")
    text = text.replace("#ACCENTTEXT", link).replace("#ACCENTFILL", fill)
    return text.replace("#SHADOW0", "#000000").replace("#WHITE0", "#ffffff")


def wallpaper_accent(path):
    """Pick an accent from a picture: the most common saturated hue."""
    try:
        import gi
        gi.require_version("GdkPixbuf", "2.0")
        from gi.repository import GdkPixbuf
        pix = GdkPixbuf.Pixbuf.new_from_file_at_scale(path, 64, 64, True)
    except Exception:
        return None
    data, n, stride = pix.get_pixels(), pix.get_n_channels(), pix.get_rowstride()
    buckets = {}
    for y in range(pix.get_height()):
        for x in range(pix.get_width()):
            i = y * stride + x * n
            h, l, s = colorsys.rgb_to_hls(data[i] / 255, data[i + 1] / 255, data[i + 2] / 255)
            if s < 0.25 or not 0.2 < l < 0.8:
                continue
            key = int(h * 24)
            w, sh, sl, ss = buckets.get(key, (0, 0, 0, 0))
            buckets[key] = (w + s, sh + h * s, sl + l * s, ss + s * s)
    if not buckets:
        return None
    w, sh, sl, ss = max(buckets.values())
    h, l, s = sh / w, sl / w, ss / w
    return from_hls(h, min(max(l, 0.35), 0.5), max(s, 0.6))


# --- Applying the theme to the shell -----------------------------------------

STYLES = [("swaync.css", "swaync/style.css"),
          ("wofi.css", "wofi/style.css"), ("swayosd.css", "swayosd/style.css"),
          ("nwg-bar.css", "nwg-bar/style.css"), ("qt5.conf", "qt5ct/colors/padOS.conf"),
          ("qt6.conf", "qt6ct/colors/padOS.conf")]


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        f.write(text)
    os.replace(tmp, path)


def swaylock_colors(theme):
    dark = is_dark(theme)
    fill = accents(theme)[0].lstrip("#")
    panel, inside, fg, ring = ("202020", "2c2c2ce6", "ffffff", "3a3a3a") if dark else \
        ("f3f3f3", "ffffffe6", "1a1a1a", "e0e0e0")
    keys = {"color": panel, "ring-color": ring, "ring-ver-color": fill, "key-hl-color": fill,
            "caps-lock-key-hl-color": fill}
    for k in ("inside", "inside-ver", "inside-wrong", "inside-clear", "inside-caps-lock"):
        keys[k + "-color"] = inside
    for k in ("text", "text-ver", "text-clear", "text-caps-lock"):
        keys[k + "-color"] = fg
    return keys


def sway_colors(theme):
    path = os.path.join(BASE, "sway-colors.conf")
    try:
        with open(path) as f:
            text = transform(f.read(), theme)
    except OSError:
        return []
    lines = [line for line in text.splitlines() if line.startswith("client.")]
    if not theme["accent_titlebars"]:
        fg = "#ffffff" if is_dark(theme) else "#1a1a1a"
        bg = "#202020" if is_dark(theme) else "#f3f3f3"
        fill = accents(theme)[0]
        lines = [re.sub(r"^client\.focused\s.*", f"client.focused {fill} {bg} {fg} {fill} {fill}", line)
                 for line in lines]
    return lines


def apply_taskbar():
    """The taskbar re-reads Settings and restyles itself on SIGUSR1."""
    subprocess.run(["pkill", "-USR1", "-u", str(os.getuid()), "-f", "ws-taskbar/taskbar.py"],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def apply_greeter():
    """Share the lock picture and display scale with the sign-in screen."""
    sync = "/usr/local/share/ws-greeter/sync.py"
    if os.path.exists(sync):
        subprocess.Popen([sys.executable, sync], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                         start_new_session=True)


def apply_swaync(theme=None):
    """Notification timeout from Settings > Accessibility > Visual effects."""
    theme = theme or load()
    if not theme.get("notify_timeout"):
        return
    path = os.path.join(config_dir(), "swaync", "config.json")
    try:
        with open(path) as f:
            conf = json.load(f)
    except (OSError, ValueError):
        return
    conf["timeout"] = conf["timeout-low"] = int(theme["notify_timeout"])
    write(path, json.dumps(conf, indent=2) + "\n")


def apply(reload=True):
    theme = load()
    cfg = config_dir()
    for base, target in STYLES:
        try:
            with open(os.path.join(BASE, base)) as f:
                write(os.path.join(cfg, target), transform(f.read(), theme))
        except OSError:
            pass
    lock = os.path.join(cfg, "swaylock", "config")
    try:
        with open(lock) as f:
            lines = f.read().splitlines()
        keys = swaylock_colors(theme)
        if theme["lock_image"]:
            keys["image"] = theme["lock_image"]
        flags = {"show-failed-attempts": theme["lock_failed_attempts"],
                 "show-keyboard-layout": theme["lock_keyboard_layout"],
                 "indicator-caps-lock": theme["lock_caps_lock"]}
        lines = [line for line in lines if line not in flags]
        lines = [f"{line.split('=')[0]}={keys[line.split('=')[0]]}"
                 if line.split("=")[0] in keys else line for line in lines]
        lines += [flag for flag, on in flags.items() if on]
        write(lock, "\n".join(lines) + "\n")
    except OSError:
        pass
    apply_swaync(theme)
    colors = sway_colors(theme)
    write(os.path.join(cfg, "sway", "config.d", "60-ws-theme.conf"),
          "# Window colours from Settings > Personalization > Colors.\n"
          + "".join(line + "\n" for line in colors))
    app_dark = is_dark(theme, "app_mode")
    gtk = DARK_GTK if app_dark and os.path.isdir(f"/usr/share/themes/{DARK_GTK}") else \
        theme["gtk_theme"] or LIGHT_GTK
    ini = os.path.join(cfg, "gtk-3.0", "settings.ini")
    try:
        with open(ini) as f:
            text = f.read()
        text = re.sub(r"(?m)^gtk-theme-name=.*$", f"gtk-theme-name={gtk}", text)
        text = re.sub(r"(?m)^gtk-application-prefer-dark-theme=.*$",
                      f"gtk-application-prefer-dark-theme={int(app_dark)}", text)
        write(ini, text)
    except OSError:
        pass
    apply_greeter()
    if not reload:
        return
    quiet = {"stdout": subprocess.DEVNULL, "stderr": subprocess.DEVNULL}
    for cmd in (["gsettings", "set", "org.gnome.desktop.interface", "gtk-theme", gtk],
                ["gsettings", "set", "org.gnome.desktop.interface", "color-scheme",
                 "prefer-dark" if app_dark else "prefer-light"]):
        subprocess.run(cmd, **quiet)
    for line in colors:
        subprocess.run(["swaymsg", line], **quiet)
    apply_taskbar()
    subprocess.run(["swaync-client", "-rs"], **quiet)
    subprocess.run(["pkill", "-u", str(os.getuid()), "-f", "ws-start --daemon"], **quiet)
    subprocess.run(["pkill", "-u", str(os.getuid()), "-x", "swayosd-server"], **quiet)


if __name__ == "__main__":
    if sys.argv[1:2] == ["apply"]:
        apply(reload="--files-only" not in sys.argv)
    elif sys.argv[1:2] == ["filter"]:
        sys.stdout.write(transform(sys.stdin.read()))
    else:
        sys.exit("usage: ws_theme.py apply [--files-only] | filter")
