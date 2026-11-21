#!/usr/bin/env python3
"""Write and update the MDE labwc config tree.

labwc reads one directory (rc.xml, autostart, environment, menu.xml, theme).
MDE keeps that directory at ~/.config/mde/labwc and a system copy at
/etc/xdg/mde/labwc. Title-bar colours live in an Openbox theme so Settings
can recolor them and ask labwc to reconfigure.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def user_config_dir():
    base = os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config"))
    return os.path.join(base, "mde", "labwc")


def theme_dirs():
    data = os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
    return [
        os.path.join(user_config_dir(), "themes", "MDE", "openbox-3"),
        os.path.join(data, "themes", "MDE", "openbox-3"),
        "/usr/share/themes/MDE/openbox-3",
    ]


def xml(text):
    return (text.replace("&", "&amp;").replace("<", "&lt;")
            .replace(">", "&gt;").replace('"', "&quot;"))


def xkb_from_localectl():
    layout, variant, options = "us", "", ""
    try:
        out = subprocess.run(["localectl", "status"], capture_output=True, text=True,
                             timeout=3).stdout
    except (OSError, subprocess.TimeoutExpired):
        return layout, variant, options
    for line in out.splitlines():
        if "X11 Layout" in line:
            layout = line.split(":", 1)[1].strip() or layout
        elif "X11 Variant" in line:
            variant = line.split(":", 1)[1].strip()
        elif "X11 Options" in line:
            options = line.split(":", 1)[1].strip()
    return layout, variant, options


def themerc(accent, dark=False):
    accent = accent or "#0078d4"
    if dark:
        inactive_bg, inactive_fg, inactive_border = "#202020", "#ffffff", "#3a3a3a"
    else:
        inactive_bg, inactive_fg, inactive_border = "#f3f3f3", "#1a1a1a", "#e0e0e0"
    return f"""# MDE Windows 11 caption. Regenerated when the accent or colour mode changes.
border.width: 1
padding.width: 8
padding.height: 4
window.button.width: 18
window.button.height: 18
window.button.spacing: 4
window.label.text.justify: left
window.active.title.bg: Solid Flat
window.inactive.title.bg: Solid Flat
window.active.title.bg.color: {accent}
window.inactive.title.bg.color: {inactive_bg}
window.active.label.text.color: #ffffff
window.inactive.label.text.color: {inactive_fg}
window.active.border.color: {accent}
window.inactive.border.color: {inactive_border}
window.active.button.unpressed.image.color: #ffffff
window.inactive.button.unpressed.image.color: {inactive_fg}
window.active.button.hover.bg.color: {accent}
window.active.button.close.unpressed.image.color: #ffffff
window.active.button.close.hover.bg.color: #c42b1c
window.active.button.close.hover.image.color: #ffffff
window.inactive.button.close.unpressed.image.color: {inactive_fg}
menu.items.bg.color: {inactive_bg}
menu.items.text.color: {inactive_fg}
menu.items.active.bg.color: {accent}
menu.items.active.text.color: #ffffff
menu.title.bg.color: {accent}
menu.title.text.color: #ffffff
osd.bg.color: {inactive_bg}
osd.label.text.color: {inactive_fg}
"""


def write_themerc(accent, dark=False):
    text = themerc(accent, dark)
    for directory in theme_dirs():
        try:
            os.makedirs(directory, exist_ok=True)
            path = os.path.join(directory, "themerc")
            tmp = path + ".tmp"
            with open(tmp, "w") as handle:
                handle.write(text)
            os.replace(tmp, path)
        except OSError:
            continue
    reconfigure()


def rc_xml():
    # labwc is already a stacking compositor. ToggleAlwaysOnTop was used as a
    # stand-in for Sway "floating", but it pins Settings and dialogs above the
    # panel and Start menu so most of the desktop cannot be clicked.
    keys = "\n".join(_key(combo, action) for combo, action in KEYBINDS)
    return f"""<?xml version="1.0"?>
<!-- MDE labwc session. Replaced when the desktop is re-applied. -->
<labwc_config>
  <core>
    <decoration>server</decoration>
    <gap>0</gap>
  </core>
  <focus>
    <followMouse>no</followMouse>
    <raiseOnFocus>yes</raiseOnFocus>
  </focus>
  <placement>
    <policy>center</policy>
  </placement>
  <snapping>
    <range>20</range>
    <topMaximize>yes</topMaximize>
    <notifyClient>yes</notifyClient>
  </snapping>
  <desktops>
    <number>1</number>
    <popupTime>0</popupTime>
  </desktops>
  <theme>
    <name>MDE</name>
    <cornerRadius>8</cornerRadius>
    <dropShadows>yes</dropShadows>
    <titlebar>
      <layout>icon:iconify,max,close</layout>
      <showTitle>yes</showTitle>
    </titlebar>
    <font place="ActiveWindow">
      <name>sans</name>
      <size>10</size>
    </font>
    <font place="InactiveWindow">
      <name>sans</name>
      <size>10</size>
    </font>
  </theme>
  <keyboard>
    <repeatRate>30</repeatRate>
    <repeatDelay>500</repeatDelay>
    <numlock>off</numlock>
{keys}
  </keyboard>
  <mouse>
    <!-- Without <default/>, Iconify/Maximize/Close and Title binds never load,
         so caption buttons and most title-bar clicks do nothing. -->
    <default/>
    <doubleClickTime>500</doubleClickTime>
    <context name="Root">
      <mousebind button="Right" action="Press"><action name="ShowMenu" menu="root-menu"/></mousebind>
    </context>
  </mouse>
  <libinput>
    <device category="touchpad">
      <naturalScroll>yes</naturalScroll>
      <tap>yes</tap>
      <disableWhileTyping>yes</disableWhileTyping>
      <middleEmulation>yes</middleEmulation>
    </device>
  </libinput>
</labwc_config>
"""


def _key(combo, action):
    release = ' onRelease="yes"' if combo == "Super_L" else ""
    return f'    <keybind key="{combo}"{release}>\n{action}\n    </keybind>'


def _exec(command):
    return f'      <action name="Execute"><command>{xml(command)}</command></action>'


WM = "python3 /usr/share/mde/wm.py"

KEYBINDS = [
    ("Super_L", _exec("ws-drawer")),
    ("W-slash", _exec("ws-shortcuts")),
    ("W-i", _exec("ws-settings")),
    ("W-x", _exec("ws-winx")),
    ("W-d", _exec("ws-show-desktop")),
    ("A-Tab", _exec("ws-window-switch")),
    ("W-Return", _exec("terminator")),
    ("C-A-t", _exec("terminator")),
    ("W-e", _exec("thunar")),
    ("W-l", _exec("ws-lock")),
    ("W-S-v", _exec("ws-clip")),
    ("C-A-Delete", _exec("ws-power-menu")),
    ("W-S-e", _exec("ws-confirm logout")),
    ("W-S-q", '      <action name="Close"/>'),
    ("A-F4", '      <action name="Close"/>'),
    ("W-S-r", '      <action name="Reconfigure"/>'),
    ("W-S-c", '      <action name="Reconfigure"/>'),
    ("W-f", '      <action name="ToggleFullscreen"/>'),
    ("W-a", _exec("ws-taskbar --quick-settings")),
    ("W-n", _exec("ws-taskbar --notifications")),
    ("W-Left", '      <action name="SnapToEdge" direction="left"/>'),
    ("W-Right", '      <action name="SnapToEdge" direction="right"/>'),
    ("W-Up", '      <action name="ToggleMaximize"/>'),
    ("W-Down", _exec(f"{WM} snap-down")),
    ("Print", _exec("ws-screenshot screen")),
    ("S-Print", _exec("ws-screenshot area")),
    ("W-Print", _exec("ws-screenshot window")),
    ("XF86AudioRaiseVolume", _exec("ws-osd volume-up")),
    ("XF86AudioLowerVolume", _exec("ws-osd volume-down")),
    ("XF86AudioMute", _exec("ws-osd mute")),
    ("XF86AudioMicMute", _exec("ws-osd mic-mute")),
    ("XF86MonBrightnessUp", _exec("ws-osd bright-up")),
    ("XF86MonBrightnessDown", _exec("ws-osd bright-down")),
    ("XF86AudioPlay", _exec("playerctl play-pause")),
    ("XF86AudioNext", _exec("playerctl next")),
    ("XF86AudioPrev", _exec("playerctl previous")),
]
for _n in range(1, 10):
    KEYBINDS.append((f"W-{_n}", _exec(f"ws-taskbar --activate {_n}")))
    KEYBINDS.append((f"W-S-{_n}", _exec(f"ws-taskbar --launch {_n}")))


def menu_xml():
    return """<?xml version="1.0"?>
<openbox_menu>
  <menu id="root-menu" label="Desktop">
    <item label="Display settings">
      <action name="Execute"><command>ws-settings display</command></action>
    </item>
    <item label="Personalize">
      <action name="Execute"><command>ws-settings personalization</command></action>
    </item>
    <item label="Terminal">
      <action name="Execute"><command>terminator</command></action>
    </item>
  </menu>
</openbox_menu>
"""


def environment_text(cursor, cursor_size):
    layout, variant, options = xkb_from_localectl()
    if "grp:" not in options:
        options = ",".join(p for p in (options, "grp:alt_shift_toggle") if p)
    return (
        f"XCURSOR_THEME={cursor}\n"
        f"XCURSOR_SIZE={cursor_size}\n"
        f"XKB_DEFAULT_LAYOUT={layout}\n"
        f"XKB_DEFAULT_VARIANT={variant}\n"
        f"XKB_DEFAULT_OPTIONS={options}\n"
    )


def autostart_text():
    return """#!/bin/sh
# Wallpaper is wallpaper.sh so Settings can change it without rewriting rc.xml.
# shellcheck disable=SC1091
. "${XDG_CONFIG_HOME:-$HOME/.config}/mde/labwc/wallpaper.sh"
# Absolute path: ~/.local/bin is ahead of /usr/bin, and a Sway-era copy there
# waits for SWAYSOCK and exits before the panel starts.
exec /usr/bin/ws-sway-autostart
"""


def wallpaper_script(path=None, fit="fill", color=None):
    if color:
        cmd = f"swaybg -c '{color}'"
    elif path:
        cmd = f"swaybg -i '{path}' -m {fit or 'fill'}"
    else:
        cmd = "swaybg -c '#1a1a1a'"
    return f"""#!/bin/sh
pkill -x swaybg 2>/dev/null || true
{cmd} >/dev/null 2>&1 &
"""


def write_text(path, text, mode=0o644):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w") as handle:
        handle.write(text)
    os.chmod(tmp, mode)
    os.replace(tmp, path)


def write_tree(directory, wallpaper, accent="#0078d4", cursor="Adwaita", cursor_size=24, dark=False):
    os.makedirs(directory, exist_ok=True)
    write_text(os.path.join(directory, "rc.xml"), rc_xml())
    write_text(os.path.join(directory, "menu.xml"), menu_xml())
    write_text(os.path.join(directory, "environment"), environment_text(cursor, cursor_size))
    write_text(os.path.join(directory, "autostart"), autostart_text(), 0o755)
    write_text(os.path.join(directory, "wallpaper.sh"), wallpaper_script(wallpaper), 0o755)
    theme = os.path.join(directory, "themes", "MDE", "openbox-3", "themerc")
    write_text(theme, themerc(accent, dark))
    for dest in ("/usr/share/themes/MDE/openbox-3/themerc",):
        try:
            write_text(dest, themerc(accent, dark))
        except OSError:
            pass


def reconfigure():
    subprocess.run(["labwc", "--reconfigure"], stdout=subprocess.DEVNULL,
                   stderr=subprocess.DEVNULL)


def _patch_rc(pattern, repl):
    path = os.path.join(user_config_dir(), "rc.xml")
    try:
        with open(path) as handle:
            text = handle.read()
    except OSError:
        return
    new = re.sub(pattern, repl, text, count=1)
    if new != text:
        write_text(path, new)
        reconfigure()


def set_wallpaper(path=None, fit="fill", color=None):
    script = os.path.join(user_config_dir(), "wallpaper.sh")
    try:
        write_text(script, wallpaper_script(path, fit, color), 0o755)
    except OSError:
        return
    subprocess.Popen(["sh", script], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                     start_new_session=True)


def set_cursor(theme, size):
    path = os.path.join(user_config_dir(), "environment")
    try:
        with open(path) as handle:
            text = handle.read()
    except OSError:
        text = environment_text(theme, size)
    text = re.sub(r"(?m)^XCURSOR_THEME=.*$", f"XCURSOR_THEME={theme}", text)
    text = re.sub(r"(?m)^XCURSOR_SIZE=.*$", f"XCURSOR_SIZE={size}", text)
    if "XCURSOR_THEME=" not in text:
        text += f"XCURSOR_THEME={theme}\nXCURSOR_SIZE={size}\n"
    try:
        write_text(path, text)
    except OSError:
        pass
    home = os.path.expanduser("~")
    xres = os.path.join(home, ".Xresources")
    lines = []
    try:
        lines = open(xres).read().splitlines()
    except OSError:
        pass
    kept = [ln for ln in lines if not ln.startswith(("Xcursor.theme:", "Xcursor.size:"))]
    kept += [f"Xcursor.theme: {theme}", f"Xcursor.size: {size}"]
    try:
        write_text(xres, "\n".join(kept) + "\n")
    except OSError:
        pass


def apply_legacy(key, line):
    """Translate a Sway settings line into a labwc change. Unknown lines are ignored."""
    line = line or ""
    if key == "wallpaper" or line.startswith("output ") and " bg " in line:
        _apply_wallpaper_line(line)
        return
    if key == "cursor" or line.startswith("seat "):
        match = re.search(r"xcursor_theme\s+(\S+)\s+(\d+)", line)
        if match:
            set_cursor(match.group(1), int(match.group(2)))
        return
    if "repeat_delay" in line:
        match = re.search(r"(\d+)", line)
        if match:
            _patch_rc(r"<repeatDelay>\d+</repeatDelay>", f"<repeatDelay>{match.group(1)}</repeatDelay>")
        return
    if "repeat_rate" in line:
        match = re.search(r"(\d+)", line)
        if match:
            _patch_rc(r"<repeatRate>\d+</repeatRate>", f"<repeatRate>{match.group(1)}</repeatRate>")
        return
    if "xkb_numlock" in line:
        value = "on" if "enabled" in line else "off"
        _patch_rc(r"<numlock>\w+</numlock>", f"<numlock>{value}</numlock>")
        return
    if key == "focus_follows_mouse" or line.startswith("focus_follows_mouse"):
        value = "yes" if line.strip().endswith("yes") else "no"
        _patch_rc(r"<followMouse>\w+</followMouse>", f"<followMouse>{value}</followMouse>")
        return
    if key == "auto_enable_outputs" or "autoEnableOutputs" in line:
        value = "yes" if str(line).strip().split()[-1] in ("yes", "true", "1", "on") else "no"
        _ensure_core_tag("autoEnableOutputs", value)
        return
    if key == "reuse_output_mode" or "reuseOutputMode" in line:
        value = "yes" if str(line).strip().split()[-1] in ("yes", "true", "1", "on") else "no"
        _ensure_core_tag("reuseOutputMode", value)
        return
    if key == "adaptive_sync" or "adaptiveSync" in line:
        token = str(line).strip().split()[-1].lower()
        value = {"yes": "yes", "true": "yes", "on": "yes", "always": "yes",
                 "fullscreen": "fullscreen", "no": "no", "false": "no", "off": "no"}.get(token, "no")
        _ensure_core_tag("adaptiveSync", value)
        return
    if key.startswith("client") or line.startswith("client."):
        parts = line.split()
        if len(parts) >= 4 and parts[0] in ("client.focused", "client.focused_inactive",
                                            "client.unfocused", "client.urgent"):
            _apply_client_color(parts[0], parts[2], parts[3])
        return
    if key in ("gaps_inner", "gaps_outer") or line.startswith("gaps "):
        match = re.search(r"(\d+)", line)
        if match and "inner" in line:
            _patch_rc(r"<gap>\d+</gap>", f"<gap>{match.group(1)}</gap>")


def _ensure_core_tag(name, value):
    """Set or insert <core><name>value</name> in the user rc.xml."""
    path = os.path.join(user_config_dir(), "rc.xml")
    try:
        text = open(path).read()
    except OSError:
        return
    pattern = rf"<{name}>[^<]*</{name}>"
    repl = f"<{name}>{value}</{name}>"
    if re.search(pattern, text):
        text = re.sub(pattern, repl, text)
    elif re.search(r"<core>", text):
        text = re.sub(r"<core>", f"<core>\n    {repl}", text, count=1)
    else:
        text = re.sub(r"(<labwc[^>]*>)", rf"\1\n  <core>\n    {repl}\n  </core>", text, count=1)
    try:
        write_text(path, text)
    except OSError:
        return
    reconfigure()


def _apply_client_color(which, bg, fg):
    path = theme_dirs()[0] + "/themerc"
    try:
        text = open(os.path.join(theme_dirs()[0], "themerc")).read()
    except OSError:
        text = themerc(bg if which == "client.focused" else "#0078d4")
    if which == "client.focused":
        text = re.sub(r"(?m)^window.active.title.bg.color:.*$", f"window.active.title.bg.color: {bg}", text)
        text = re.sub(r"(?m)^window.active.border.color:.*$", f"window.active.border.color: {bg}", text)
        text = re.sub(r"(?m)^window.active.label.text.color:.*$", f"window.active.label.text.color: {fg}", text)
    else:
        text = re.sub(r"(?m)^window.inactive.title.bg.color:.*$", f"window.inactive.title.bg.color: {bg}", text)
        text = re.sub(r"(?m)^window.inactive.border.color:.*$", f"window.inactive.border.color: {bg}", text)
        text = re.sub(r"(?m)^window.inactive.label.text.color:.*$", f"window.inactive.label.text.color: {fg}", text)
    for directory in theme_dirs():
        try:
            write_text(os.path.join(directory, "themerc"), text)
        except OSError:
            continue
    reconfigure()


def _apply_wallpaper_line(line):
    solid = re.search(r"bg\s+(#[0-9A-Fa-f]{6})\s+solid_color", line)
    if solid:
        set_wallpaper(color=solid.group(1))
        return
    picture = re.search(r'bg\s+"([^"]+)"\s+(\S+)', line) or re.search(r"bg\s+(\S+)\s+(\S+)", line)
    if picture and picture.group(2) != "solid_color":
        set_wallpaper(picture.group(1), picture.group(2))


# Fractional output scales still break GTK3 hit-testing under labwc. Keep the
# clamp until a session-wide pointer mapping fix lands (see Display › Advanced).
ALLOW_FRACTIONAL_SCALE = False


def _clamp_scale(scale):
    try:
        value = float(scale)
    except (TypeError, ValueError):
        return "1"
    if ALLOW_FRACTIONAL_SCALE and abs(value - 1.5) < 0.01:
        return "1.5"
    return "2" if value >= 1.5 else "1"


def apply_output_line(line):
    """Run one Sway-style `output …` line through wlr-randr."""
    if not shutil.which("wlr-randr"):
        return False
    line = (line or "").strip()
    enable = re.match(r"output\s+(\S+)\s+(enable|disable|on|off)\s*$", line)
    if enable:
        name, state = enable.groups()
        flag = "--on" if state in ("enable", "on") else "--off"
        subprocess.run(["wlr-randr", "--output", name, flag],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return True
    adaptive = re.match(r"output\s+(\S+)\s+adaptive[_-]sync\s+(\S+)\s*$", line)
    if adaptive:
        name, state = adaptive.groups()
        value = "enabled" if state.lower() in ("on", "enabled", "yes", "true", "1") else "disabled"
        subprocess.run(["wlr-randr", "--output", name, "--adaptive-sync", value],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return True
    preferred = re.match(r"output\s+(\S+)\s+preferred\s*$", line)
    if preferred:
        subprocess.run(["wlr-randr", "--output", preferred.group(1), "--preferred"],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return True
    match = re.match(
        r"output\s+(\S+)\s+mode\s+(\d+x\d+)(?:@([\d.]+)Hz)?\s+scale\s+(\S+)\s+"
        r"transform\s+(\S+)\s+position\s+(-?\d+)\s+(-?\d+)"
        r"(?:\s+adaptive[_-]sync\s+(\S+))?",
        line,
    )
    if not match:
        return False
    name, mode, refresh, scale, transform, x, y, adaptive_state = match.groups()
    if refresh:
        mode = f"{mode}@{refresh}"
    scale = _clamp_scale(scale)
    cmd = ["wlr-randr", "--output", name, "--on", "--mode", mode, "--scale", scale,
           "--transform", transform, "--pos", f"{x},{y}"]
    if adaptive_state:
        value = "enabled" if adaptive_state.lower() in ("on", "enabled", "yes", "true", "1") else "disabled"
        cmd.extend(["--adaptive-sync", value])
    subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return True


def _normalize_wlr_randr(entry):
    """Map wlr-randr --json to the sway-like shape Settings expects."""
    import json  # noqa: F401 — kept local for callers that only import outputs
    scale = float(entry.get("scale") or 1) or 1
    current = entry.get("current_mode") or {}
    if not current and entry.get("modes"):
        current = next((m for m in entry["modes"] if m.get("current")), entry["modes"][0])
    width = int(current.get("width") or 0)
    height = int(current.get("height") or 0)
    refresh = int(current.get("refresh") or 0)
    pos = entry.get("position") or {}
    modes = []
    for m in entry.get("modes") or []:
        modes.append({
            "width": int(m.get("width") or 0),
            "height": int(m.get("height") or 0),
            "refresh": int(m.get("refresh") or 0),
            "preferred": bool(m.get("preferred")),
        })
    adaptive = entry.get("adaptive_sync_status") or entry.get("adaptive_sync") or "disabled"
    if isinstance(adaptive, bool):
        adaptive = "enabled" if adaptive else "disabled"
    enabled = entry.get("enabled", True)
    return {
        "name": entry.get("name") or "",
        "active": bool(enabled and width),
        "focused": bool(entry.get("focused")),
        "scale": scale,
        "transform": entry.get("transform") or "normal",
        "make": entry.get("make") or "",
        "model": entry.get("model") or "",
        "serial": entry.get("serial") or "",
        "description": entry.get("description") or "",
        "adaptive_sync_status": str(adaptive).lower(),
        "rect": {
            "x": int(pos.get("x") or entry.get("x") or 0),
            "y": int(pos.get("y") or entry.get("y") or 0),
            "width": int(width / scale) if width else 0,
            "height": int(height / scale) if height else 0,
        },
        "current_mode": {"width": width, "height": height, "refresh": refresh},
        "modes": modes,
    }


def outputs():
    import json
    if shutil.which("wlr-randr"):
        try:
            raw = subprocess.run(["wlr-randr", "--json"], capture_output=True,
                                 text=True, timeout=3)
            data = json.loads(raw.stdout or "[]")
            if isinstance(data, list) and data:
                rows = [_normalize_wlr_randr(o) for o in data if o.get("name")]
                if rows and not any(r.get("focused") for r in rows):
                    for r in rows:
                        if r.get("active"):
                            r["focused"] = True
                            break
                return rows
        except (OSError, subprocess.TimeoutExpired, ValueError, TypeError):
            pass
    script = os.path.join(HERE, "wm.py")
    if not os.path.isfile(script):
        script = "/usr/share/mde/wm.py"
    try:
        raw = subprocess.run([sys.executable, script, "outputs"], capture_output=True,
                             text=True, timeout=3).stdout
    except (OSError, subprocess.TimeoutExpired):
        return []
    try:
        data = json.loads(raw)
    except ValueError:
        return []
    return data if isinstance(data, list) else []


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["write-tree"])
    parser.add_argument("directory")
    parser.add_argument("--wallpaper", default="")
    parser.add_argument("--accent", default="#0078d4")
    parser.add_argument("--cursor", default="Adwaita")
    parser.add_argument("--cursor-size", type=int, default=24)
    parser.add_argument("--dark", action="store_true")
    args = parser.parse_args()
    write_tree(args.directory, args.wallpaper or None, args.accent, args.cursor,
               args.cursor_size, args.dark)


if __name__ == "__main__":
    main()
