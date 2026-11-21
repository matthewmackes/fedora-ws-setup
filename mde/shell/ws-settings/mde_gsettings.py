"""Bridge MDE gsettings (org.mde.*) for the Python settings app during the Rust port.

When a schema is not compiled yet, values fall back to a JSON file under
~/.config/ws-settings/ so Settings keeps working in development.
"""

from __future__ import annotations

import json
import os

SCHEMA = "org.mde"
TASKBAR = "org.mde.taskbar"
START = "org.mde.start"
WINDOW_LAYOUT = "org.mde.window-layout"

_FALLBACK_DIR = os.path.join(os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config")),
                             "ws-settings")
_FALLBACK_FILE = os.path.join(_FALLBACK_DIR, "gsettings-fallback.json")


def _settings(schema: str):
    try:
        import gi

        gi.require_version("Gio", "2.0")
        from gi.repository import Gio

        # Gio.Settings.new aborts the process if the schema is missing — look up first.
        source = Gio.SettingsSchemaSource.get_default()
        if source is None or source.lookup(schema, True) is None:
            return None
        return Gio.Settings.new(schema)
    except Exception:
        return None


def _fallback_load() -> dict:
    try:
        with open(_FALLBACK_FILE) as f:
            data = json.load(f)
        return data if isinstance(data, dict) else {}
    except (OSError, ValueError):
        return {}


def _fallback_get(schema: str, key: str, default):
    return _fallback_load().get(schema, {}).get(key, default)


def _fallback_set(schema: str, key: str, value) -> bool:
    data = _fallback_load()
    bucket = data.setdefault(schema, {})
    bucket[key] = value
    os.makedirs(_FALLBACK_DIR, exist_ok=True)
    tmp = _FALLBACK_FILE + ".tmp"
    with open(tmp, "w") as f:
        json.dump(data, f, indent=2)
        f.write("\n")
    os.replace(tmp, _FALLBACK_FILE)
    return True


def get_string(schema: str, key: str, default: str = "") -> str:
    ss = _settings(schema)
    if ss is None:
        return str(_fallback_get(schema, key, default))
    try:
        return str(ss.get_string(key))
    except Exception:
        return str(_fallback_get(schema, key, default))


def get_bool(schema: str, key: str, default: bool = False) -> bool:
    ss = _settings(schema)
    if ss is None:
        return bool(_fallback_get(schema, key, default))
    try:
        return bool(ss.get_boolean(key))
    except Exception:
        return bool(_fallback_get(schema, key, default))


def get_int(schema: str, key: str, default: int = 0) -> int:
    ss = _settings(schema)
    if ss is None:
        return int(_fallback_get(schema, key, default))
    try:
        return int(ss.get_int(key))
    except Exception:
        return int(_fallback_get(schema, key, default))


def get_strv(schema: str, key: str, default=None) -> list:
    if default is None:
        default = []
    ss = _settings(schema)
    if ss is None:
        val = _fallback_get(schema, key, default)
        return list(val) if isinstance(val, list) else list(default)
    try:
        return list(ss.get_strv(key))
    except Exception:
        val = _fallback_get(schema, key, default)
        return list(val) if isinstance(val, list) else list(default)


def set_string(schema: str, key: str, value: str) -> bool:
    ss = _settings(schema)
    if ss is None:
        return _fallback_set(schema, key, str(value))
    try:
        ss.set_string(key, str(value))
        return True
    except Exception:
        return _fallback_set(schema, key, str(value))


def set_bool(schema: str, key: str, value: bool) -> bool:
    ss = _settings(schema)
    if ss is None:
        return _fallback_set(schema, key, bool(value))
    try:
        ss.set_boolean(key, bool(value))
        return True
    except Exception:
        return _fallback_set(schema, key, bool(value))


def set_int(schema: str, key: str, value: int) -> bool:
    ss = _settings(schema)
    if ss is None:
        return _fallback_set(schema, key, int(value))
    try:
        ss.set_int(key, int(value))
        return True
    except Exception:
        return _fallback_set(schema, key, int(value))


def set_strv(schema: str, key: str, value) -> bool:
    value = list(value or [])
    ss = _settings(schema)
    if ss is None:
        return _fallback_set(schema, key, value)
    try:
        ss.set_strv(key, value)
        return True
    except Exception:
        return _fallback_set(schema, key, value)


def accent() -> str:
    return get_string(SCHEMA, "accent", "#0078d4")


def animations_enabled() -> bool:
    return get_bool(SCHEMA, "animations", True)


# --- Window Layout -----------------------------------------------------------

WINDOW_LAYOUT_DEFAULTS = {
    "default-layout": "default",
    "new-windows-floating": False,
    "gaps-inner": 0,
    "gaps-outer": 0,
    "smart-gaps": False,
    "border-tiled": 1,
    "border-floating": 1,
    "titlebar-style": "normal",
    "show-window-buttons": False,
    "smart-borders": "off",
    "color-focused": "#0078d4",
    "color-focused-inactive": "#808080",
    "color-unfocused": "#808080",
    "color-urgent": "#ff3b30",
    "focus-follows-mouse": "no",
    "mouse-warping": "none",
    "focus-on-activation": "focus",
    "floating-modifier": True,
    "workspace-back-and-forth": False,
    "taskbar-all-desktops": False,
    "rules-preset": "win11",
    "custom-rules": [],
    "migrated": False,
}

_STRING_KEYS = {
    "default-layout", "titlebar-style", "smart-borders", "color-focused",
    "color-focused-inactive", "color-unfocused", "color-urgent",
    "focus-follows-mouse", "mouse-warping", "focus-on-activation", "rules-preset",
}
_BOOL_KEYS = {
    "new-windows-floating", "smart-gaps", "floating-modifier",
    "workspace-back-and-forth", "taskbar-all-desktops", "migrated",
    "show-window-buttons",
}
_INT_KEYS = {"gaps-inner", "gaps-outer", "border-tiled", "border-floating"}
_STRV_KEYS = {"custom-rules"}


def window_layout_get(key: str):
    default = WINDOW_LAYOUT_DEFAULTS[key]
    if key in _STRING_KEYS:
        return get_string(WINDOW_LAYOUT, key, default)
    if key in _BOOL_KEYS:
        return get_bool(WINDOW_LAYOUT, key, default)
    if key in _INT_KEYS:
        return get_int(WINDOW_LAYOUT, key, default)
    if key in _STRV_KEYS:
        return get_strv(WINDOW_LAYOUT, key, default)
    return default


def window_layout_set(key: str, value) -> bool:
    if key in _STRING_KEYS:
        return set_string(WINDOW_LAYOUT, key, value)
    if key in _BOOL_KEYS:
        return set_bool(WINDOW_LAYOUT, key, value)
    if key in _INT_KEYS:
        return set_int(WINDOW_LAYOUT, key, int(value))
    if key in _STRV_KEYS:
        return set_strv(WINDOW_LAYOUT, key, value)
    return False


def window_layout_load() -> dict:
    return {k: window_layout_get(k) for k in WINDOW_LAYOUT_DEFAULTS}


def window_layout_save(data: dict) -> None:
    for key, value in data.items():
        if key in WINDOW_LAYOUT_DEFAULTS:
            window_layout_set(key, value)


def window_layout_defaults() -> dict:
    return dict(WINDOW_LAYOUT_DEFAULTS)


def migrate_window_layout_from_settings_json(load_settings) -> dict:
    """One-time copy of Multitasking keys from settings.json into org.mde.window-layout."""
    state = window_layout_load()
    if state.get("migrated"):
        return state
    try:
        legacy = load_settings() or {}
    except Exception:
        legacy = {}
    mapping = {
        "default_layout": "default-layout",
        "gaps": "gaps-inner",
        "smart_gaps": "smart-gaps",
        "focus_follows_mouse": "focus-follows-mouse",
        "mouse_warping": "mouse-warping",
        "workspace_back_and_forth": "workspace-back-and-forth",
        "taskbar_all_desktops": "taskbar-all-desktops",
    }
    for old, new in mapping.items():
        if old in legacy and legacy[old] is not None:
            state[new] = legacy[old]
    accent_color = legacy.get("accent") or get_string(SCHEMA, "accent", "") or "#0078d4"
    if accent_color:
        state["color-focused"] = accent_color
    state["migrated"] = True
    window_layout_save(state)
    return state
