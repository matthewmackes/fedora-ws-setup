#!/usr/bin/env python3
"""Share the lock-screen picture and display scale with the sign-in screen, which runs
as the lightdm user and can't read home folders."""

import json
import os
import pwd
import shutil
import subprocess

SHARED = "/var/lib/ws-greeter"
SETTINGS = os.path.expanduser("~/.config/ws-settings/settings.json")
EXTENSIONS = (".jpg", ".jpeg", ".png", ".webp")


def settings():
    try:
        with open(SETTINGS) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def output_scale():
    try:
        outputs = json.loads(subprocess.run(["swaymsg", "-rt", "get_outputs"], capture_output=True,
                                            text=True, timeout=2).stdout)
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return None
    active = [o for o in outputs if o.get("active")]
    internal = [o for o in active if o.get("name", "").startswith(("eDP", "LVDS", "DSI"))]
    chosen = (internal or active or [None])[0]
    return chosen.get("scale") if chosen else None


def sync():
    if not os.access(SHARED, os.W_OK):
        return False
    user = pwd.getpwuid(os.getuid()).pw_name
    s = settings()
    picture = next((p for p in (s.get("lock_image"), s.get("wallpaper")) if p and os.path.isfile(p)), None)
    target = None
    if picture:
        ext = os.path.splitext(picture)[1].lower()
        target = os.path.join(SHARED, user + (ext if ext in EXTENSIONS else ".jpg"))
        try:
            if not os.path.exists(target) or os.path.getmtime(target) < os.path.getmtime(picture) \
                    or os.path.getsize(target) != os.path.getsize(picture):
                shutil.copyfile(picture, target + ".tmp")
                os.chmod(target + ".tmp", 0o644)
                os.replace(target + ".tmp", target)
        except OSError:
            target = None
    for ext in EXTENSIONS:
        stale = os.path.join(SHARED, user + ext)
        if stale != target and os.path.exists(stale):
            try:
                os.unlink(stale)
            except OSError:
                pass
    data = {"scale": output_scale(), "clock_24h": bool(s.get("clock_24h"))}
    path = os.path.join(SHARED, user + ".json")
    try:
        with open(path + ".tmp", "w") as f:
            json.dump(data, f)
        os.chmod(path + ".tmp", 0o644)
        os.replace(path + ".tmp", path)
    except OSError:
        return False
    return True


if __name__ == "__main__":
    raise SystemExit(0 if sync() else 1)
