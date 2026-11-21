#!/usr/bin/env python3
"""Watch for display connect/disconnect and ask what to do when enabled in Settings."""
import json
import os
import subprocess
import sys
import time

SHARE = os.path.dirname(os.path.abspath(__file__))
for path in (SHARE, "/usr/share/mde", os.path.join(SHARE, "ws-settings"),
             "/usr/share/mde/ws-settings"):
    if path not in sys.path and os.path.isdir(path):
        sys.path.insert(0, path)

SETTINGS = os.path.join(os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config")),
                        "ws-settings", "settings.json")


def load_settings():
    try:
        with open(SETTINGS) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def outputs():
    try:
        import labwc_cfg
        return labwc_cfg.outputs()
    except Exception:
        return []


def fingerprint(outs):
    return tuple(sorted((o.get("name"), bool(o.get("active"))) for o in outs if o.get("name")))


def apply_projection(mode, outs):
    """Best-effort mirror of Settings projection modes via wlr-randr."""
    if not outs:
        return
    if mode == "extend":
        x = 0
        for o in outs:
            name = o["name"]
            w = (o.get("rect") or {}).get("width") or (o.get("current_mode") or {}).get("width") or 1920
            subprocess.run(["wlr-randr", "--output", name, "--on", "--pos", f"{x},0"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            x += int(w)
    elif mode == "duplicate":
        main = next((o for o in outs if o.get("focused")), outs[0])
        mode_info = main.get("current_mode") or {}
        mode_s = f"{mode_info.get('width', 0)}x{mode_info.get('height', 0)}"
        for o in outs:
            cmd = ["wlr-randr", "--output", o["name"], "--on", "--pos", "0,0"]
            if mode_info.get("width"):
                cmd.extend(["--mode", mode_s])
            subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    elif mode.startswith("only:"):
        keep = mode.split(":", 1)[1]
        for o in outs:
            flag = "--on" if o["name"] == keep else "--off"
            subprocess.run(["wlr-randr", "--output", o["name"], flag],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def ask(outs):
    import gi
    gi.require_version("Gtk", "3.0")
    from gi.repository import Gtk

    active = [o for o in outs if o.get("active")]
    dialog = Gtk.MessageDialog(
        message_type=Gtk.MessageType.QUESTION,
        buttons=Gtk.ButtonsType.NONE,
        text="A display was connected",
    )
    dialog.format_secondary_text("Choose how to use your displays.")
    dialog.add_button("Extend", 1)
    dialog.add_button("Duplicate", 2)
    for i, o in enumerate(outs, 1):
        dialog.add_button(f"Show only on {i}", 10 + i)
    dialog.add_button("Arrange…", 3)
    dialog.add_button("Dismiss", Gtk.ResponseType.CANCEL)
    response = dialog.run()
    dialog.destroy()
    if response == 1:
        apply_projection("extend", outs)
    elif response == 2:
        apply_projection("duplicate", outs)
    elif response == 3:
        subprocess.Popen(["ws-settings", "display"],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    elif isinstance(response, int) and response >= 11:
        idx = response - 10
        if 1 <= idx <= len(outs):
            apply_projection(f"only:{outs[idx - 1]['name']}", outs)
    if response in (1, 2) or (isinstance(response, int) and response >= 11):
        if shutil_which("ws-display-save"):
            subprocess.Popen(["ws-display-save"],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def shutil_which(cmd):
    from shutil import which
    return which(cmd)


def main():
    # Wait for the session to settle.
    time.sleep(3)
    prev = fingerprint(outputs())
    while True:
        time.sleep(2)
        s = load_settings()
        if not s.get("ask_on_display_connect"):
            prev = fingerprint(outputs())
            continue
        outs = outputs()
        cur = fingerprint(outs)
        if cur == prev:
            continue
        # Only prompt when a new connector appears (not on disconnect alone).
        prev_names = {n for n, _ in prev}
        cur_names = {n for n, _ in cur}
        added = cur_names - prev_names
        prev = cur
        if not added:
            continue
        try:
            ask(outs)
        except Exception:
            pass


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(0)
