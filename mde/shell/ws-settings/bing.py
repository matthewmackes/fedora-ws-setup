# Windows spotlight: today's Bing picture for the desktop and the lock screen.
# "ws-settings --bing-wallpaper" runs refresh from a user timer, once a day
# and again shortly after sign-in. A failed download leaves the current picture.
import json
import os
import re
import subprocess
import sys
import threading
import urllib.error
import urllib.parse
import urllib.request

import ws_theme

ARCHIVE = "https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=1&mkt="
UA = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0"
# Bing publishes these sizes. The smallest one that covers the display is used.
SIZES = (
    (1024, 768, "1024x768"),
    (1280, 720, "1280x720"),
    (1366, 768, "1366x768"),
    (1920, 1080, "1920x1080"),
    (1920, 1200, "1920x1200"),
    (3840, 2160, "UHD"),
)
LOCK = threading.Lock()
pending = False

TIMER = """[Unit]
Description=Windows spotlight wallpaper (Bing)

[Timer]
OnStartupSec=2min
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
"""
SERVICE = """[Unit]
Description=Windows spotlight wallpaper (Bing)

[Service]
Type=oneshot
ExecStart=/usr/bin/ws-settings --bing-wallpaper
"""


def market_from(lang):
    """Turn a locale such as en_GB.UTF-8 into a Bing market such as en-GB."""
    match = re.match(r"([a-zA-Z]{2})[_-]([a-zA-Z]{2})", lang or "")
    if match:
        return f"{match.group(1).lower()}-{match.group(2).upper()}"
    return "en-US"


def market():
    return market_from(os.environ.get("LC_ALL") or os.environ.get("LANG") or "")


def choose_suffix(width, height):
    """Smallest published Bing size that covers the display, or 4K."""
    covering = [item for item in SIZES if item[0] >= width and item[1] >= height]
    if covering:
        return min(covering, key=lambda item: item[0] * item[1])[2]
    return "UHD"


def caption_lines(title, copyright_text):
    """Title line and the © credit, for the preview under the picture."""
    title = (title or "").strip()
    copyright_text = (copyright_text or "").strip()
    credit = ""
    place = copyright_text
    match = re.search(r"\((©[^)]*)\)\s*$", copyright_text)
    if match:
        credit = match.group(1).strip()
        place = copyright_text[:match.start()].strip(" ,")
    headline = title or place
    if headline and place and headline.casefold() != place.casefold() and place.casefold() not in headline.casefold():
        headline = f"{headline} — {place}"
    if credit:
        return headline, credit
    if headline and copyright_text and headline != copyright_text:
        return headline, copyright_text
    return headline, ""


def preview_lines(settings, waiting):
    """(text, style classes) shown under the background preview."""
    if waiting:
        return [("Getting today's picture…", ("caption",))]
    headline, credit = caption_lines(settings.get("bing_title"), settings.get("bing_copyright"))
    lines = []
    if headline:
        lines.append((headline, ()))
    if credit:
        lines.append((credit, ("caption",)))
    return lines


def image_url(urlbase, suffix):
    base = re.sub(r"_(?:\d+x\d+|UHD)$", "", (urlbase or "").split("&", 1)[0])
    return "https://www.bing.com" + base + "_" + suffix + ".jpg"


def picture_dir():
    data = os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
    return os.path.join(data, "backgrounds", "bing")


def display_size():
    """Pixel size of the primary display. 1920×1080 when Sway does not answer."""
    try:
        script = "/usr/share/mde/wm.py"
        if not os.path.isfile(script):
            script = os.path.join(os.path.dirname(os.path.realpath(__file__)), "..", "wm.py")
        raw = subprocess.run([sys.executable, script, "outputs"], capture_output=True,
                             text=True, timeout=2).stdout
        outputs = json.loads(raw)
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return 1920, 1080
    active = [out for out in outputs if isinstance(out, dict) and out.get("active")]
    origin = [out for out in active
              if (out.get("rect") or {}).get("x") == 0 and (out.get("rect") or {}).get("y") == 0]
    focused = [out for out in active if out.get("focused")]
    chosen = (origin or focused or active or [None])[0]
    if not chosen:
        return 1920, 1080
    mode = chosen.get("current_mode") or {}
    width, height = mode.get("width"), mode.get("height")
    if not width or not height:
        rect = chosen.get("rect") or {}
        scale = chosen.get("scale") or 1
        width = int((rect.get("width") or 1920) * scale)
        height = int((rect.get("height") or 1080) * scale)
    return int(width), int(height)


def _request(url):
    return urllib.request.Request(url, headers={"User-Agent": UA})


def fetch_meta(mkt):
    try:
        with urllib.request.urlopen(_request(ARCHIVE + urllib.parse.quote(mkt)), timeout=20) as resp:
            data = json.load(resp)
        image = data["images"][0]
    except (OSError, urllib.error.URLError, ValueError, KeyError, IndexError, TimeoutError):
        return None
    if not image.get("urlbase") or not image.get("startdate"):
        return None
    return {"date": image.get("startdate") or "", "title": image.get("title") or "",
            "copyright": image.get("copyright") or "", "urlbase": image.get("urlbase") or ""}


def download(url, dest):
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    tmp = dest + ".tmp"
    try:
        with urllib.request.urlopen(_request(url), timeout=60) as resp, open(tmp, "wb") as out:
            while True:
                chunk = resp.read(64 * 1024)
                if not chunk:
                    break
                out.write(chunk)
        with open(tmp, "rb") as check:
            magic = check.read(3)
        if magic != b"\xff\xd8\xff" or os.path.getsize(tmp) < 20000:
            os.unlink(tmp)
            return False
        os.replace(tmp, dest)
        return True
    except (OSError, urllib.error.URLError, TimeoutError):
        try:
            os.unlink(tmp)
        except OSError:
            pass
        return False


def read_settings():
    try:
        with open(ws_theme.settings_path()) as handle:
            data = json.load(handle)
    except (OSError, ValueError):
        data = {}
    return data if isinstance(data, dict) else {}


def merge(**changes):
    """Update settings.json. Safe against a spotlight download finishing together."""
    with LOCK:
        data = read_settings()
        data.update(changes)
        ws_theme.write(ws_theme.settings_path(), json.dumps(data, indent=2) + "\n")
        return data


def sway_conf(key, line):
    """Apply a wallpaper line on labwc."""
    if not line:
        return
    root = os.path.abspath(os.path.join(os.path.dirname(os.path.realpath(__file__)), ".."))
    for path in (root, "/usr/share/mde"):
        if path not in sys.path:
            sys.path.insert(0, path)
    import labwc_cfg
    labwc_cfg.apply_legacy(key, line)


def _remember(recent, path):
    kept = [item for item in recent if item != path and os.path.exists(item)]
    return ([path] + kept)[:5]


def _current(data):
    return data.get("bing_date"), data.get("bing_suffix"), data.get("wallpaper"), data.get("lock_image")


def commit(path, meta, suffix, owns_lock):
    with LOCK:
        data = read_settings()
        if data.get("wallpaper_mode") != "spotlight":
            return False
        changes = {
            "wallpaper": path,
            "wallpaper_fit": "fill",
            "wallpaper_color": None,
            "wallpaper_mode": "spotlight",
            "wallpaper_recent": _remember(data.get("wallpaper_recent") or [], path),
            "bing_title": meta["title"],
            "bing_copyright": meta["copyright"],
            "bing_date": meta["date"],
            "bing_suffix": suffix,
            "bing_owns_lock": owns_lock,
        }
        if owns_lock:
            changes["lock_image"] = path
        same = _current(data) == (meta["date"], suffix, path, path if owns_lock else data.get("lock_image"))
        if data.get("accent_auto") and not same:
            accent = ws_theme.wallpaper_accent(path)
            if accent:
                changes["accent"] = accent
        data.update(changes)
        ws_theme.write(ws_theme.settings_path(), json.dumps(data, indent=2) + "\n")
        accent_changed = "accent" in changes
    sway_conf("wallpaper", f'output * bg "{path}" fill')
    if accent_changed or owns_lock:
        ws_theme.apply(reload=accent_changed)
    return True


def prepare():
    """Download today's picture. None means the current wallpaper stays.

    The file is saved before commit, so a failed later step still has the image.
    """
    data = read_settings()
    if data.get("wallpaper_mode") != "spotlight":
        return None
    width, height = display_size()
    suffix = choose_suffix(width, height)
    meta = fetch_meta(data.get("bing_market") or market())
    if not meta:
        print("Bing could not be reached. The current wallpaper stays.", file=sys.stderr)
        return None
    dest = os.path.join(picture_dir(), f"{meta['date']}-{suffix}.jpg")
    owns = bool(data.get("bing_owns_lock", True))
    ready = (data.get("bing_date") == meta["date"] and data.get("bing_suffix") == suffix
             and data.get("wallpaper") == dest and os.path.isfile(dest)
             and (not owns or data.get("lock_image") == dest))
    if ready:
        return None
    if not (os.path.isfile(dest) and os.path.getsize(dest) > 20000) and not download(image_url(meta["urlbase"], suffix), dest):
        print("Bing could not be reached. The current wallpaper stays.", file=sys.stderr)
        return None
    return dest, meta, suffix, owns


def refresh():
    """Apply today's picture when Windows spotlight is selected.

    A network failure keeps the current wallpaper and returns True so the
    daily timer does not stay in a failed state.
    """
    prepared = prepare()
    if not prepared:
        return True
    return commit(*prepared)


def refresh_async(done):
    """Fetch without blocking the Settings window. done(ok) runs on the UI thread."""
    global pending
    pending = True

    def work():
        try:
            prepared = prepare()
        except Exception:
            prepared = None
        from gi.repository import GLib
        GLib.idle_add(finish, prepared)

    def finish(prepared):
        global pending
        pending = False
        try:
            ok = commit(*prepared) if prepared else True
        except Exception:
            ok = False
        done(ok)
        return False

    threading.Thread(target=work, daemon=True).start()


def ensure_timer():
    """Daily user timer. It does nothing until Windows spotlight is selected."""
    units = os.path.join(ws_theme.config_dir(), "systemd", "user")
    os.makedirs(os.path.join(units, "timers.target.wants"), exist_ok=True)
    ws_theme.write(os.path.join(units, "ws-bing-wallpaper.service"), SERVICE)
    ws_theme.write(os.path.join(units, "ws-bing-wallpaper.timer"), TIMER)
    link = os.path.join(units, "timers.target.wants", "ws-bing-wallpaper.timer")
    try:
        if os.path.islink(link) or os.path.exists(link):
            os.unlink(link)
        os.symlink("../ws-bing-wallpaper.timer", link)
    except OSError:
        pass
    subprocess.run(["systemctl", "--user", "daemon-reload"],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    subprocess.run(["systemctl", "--user", "enable", "ws-bing-wallpaper.timer"],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


if __name__ == "__main__":
    raise SystemExit(0 if refresh() else 1)
