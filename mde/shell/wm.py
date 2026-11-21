#!/usr/bin/env python3
"""Talk to labwc through Wayland protocols.

labwc has no Sway-style socket. Window lists and minimize/focus/close use
wlr-foreign-toplevel-management. Output power uses wlr-output-power-management.
Output geometry uses wl_output. Keyboard layout is remembered locally and
toggled with wtype when that tool is installed.
"""
import json
import os
import select
import struct
import subprocess
import sys
import time

SHELL_APP_IDS = {
    "ws-taskbar", "ws-start", "ws-watermark", "ws-winx", "ws-confirm",
    "ws-drawer", "ws-window-buttons", "ws-osd", "ws-power-menu",
    "swaync", "wofi", "nwg-bar", "swayosd", "nwg-drawer",
}

RUNTIME = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
SHOW_DESKTOP_FILE = os.path.join(RUNTIME, "ws-show-desktop.json")
LAYOUT_FILE = os.path.join(RUNTIME, "ws-keyboard-layout")
CONFIG_DIR = os.path.join(os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config")),
                          "mde", "labwc")

WL_OUTPUT_MODE_CURRENT = 0x1
STATE_MAXIMIZED, STATE_MINIMIZED, STATE_ACTIVATED, STATE_FULLSCREEN = 0, 1, 2, 3
TRANSFORM_NAMES = {
    0: "normal", 1: "90", 2: "180", 3: "270",
    4: "flipped", 5: "flipped-90", 6: "flipped-180", 7: "flipped-270",
}


def _pack(fmt, *args):
    return struct.pack("<" + fmt, *args)


def _msg(oid, opcode, payload):
    return _pack("IHH", oid, opcode, 8 + len(payload)) + payload


def _str(text):
    raw = text.encode() + b"\x00"
    pad = (4 - (len(raw) % 4)) % 4
    return _pack("I", len(raw)) + raw + b"\x00" * pad


class Connection:
    """One Wayland connection. Safe to construct only inside a labwc session."""

    def __init__(self):
        name = os.environ.get("WAYLAND_DISPLAY", "wayland-1")
        path = name if name.startswith("/") else os.path.join(RUNTIME, name)
        self.sock = socket_connect(path)
        self.sock.setblocking(False)
        self.buf = b""
        self._next = 2
        self.registry_id = 1
        self.handlers = {}
        self.globals = []
        self.outputs = {}
        self.heads = {}
        self.modes = {}
        self.windows = {}
        self.seat_id = None
        self.toplevel_manager = None
        self.power_manager = None
        self.output_manager = None
        self._cb_done = False
        self._pending_cb = None
        self.dirty = False
        self.watched = False
        self._send(1, 1, _pack("I", self._alloc_registry()))
        self.roundtrip()
        self._bind()
        self.roundtrip()
        # Heads and modes arrive across the first configuration done event.
        if self.output_manager is not None:
            self.roundtrip()

    def _alloc_registry(self):
        self.registry_id = self._next
        self._next += 1
        self.handlers[self.registry_id] = self._on_registry
        return self.registry_id

    def alloc(self):
        oid = self._next
        self._next += 1
        return oid

    def _send(self, oid, opcode, payload):
        data = _msg(oid, opcode, payload)
        view = memoryview(data)
        while view:
            try:
                sent = self.sock.send(view)
            except BlockingIOError:
                select.select([], [self.sock], [], 1)
                continue
            view = view[sent:]

    def _pump(self):
        while True:
            try:
                chunk = self.sock.recv(65536)
            except BlockingIOError:
                break
            if not chunk:
                raise ConnectionError("wayland display closed")
            self.buf += chunk
        while len(self.buf) >= 8:
            oid, opcode, size = struct.unpack_from("<IHH", self.buf)
            if size < 8 or len(self.buf) < size:
                break
            body = self.buf[8:size]
            self.buf = self.buf[size:]
            self._dispatch(oid, opcode, body)

    def _dispatch(self, oid, opcode, body):
        if oid == 1:
            self._on_display(opcode, body)
            return
        handler = self.handlers.get(oid)
        if handler:
            handler(opcode, body)

    def _on_display(self, opcode, body):
        if opcode == 1 and len(body) >= 4:
            deleted = struct.unpack_from("<I", body)[0]
            self.handlers.pop(deleted, None)
            self.windows.pop(deleted, None)
            self.outputs.pop(deleted, None)

    def roundtrip(self, timeout=2.0):
        cb = self.alloc()
        self._pending_cb = cb
        self._cb_done = False

        def on_cb(opcode, body):
            if opcode == 0:
                self._cb_done = True

        self.handlers[cb] = on_cb
        self._send(1, 0, _pack("I", cb))
        end = time.time() + timeout
        while time.time() < end and not self._cb_done:
            self._pump()
            if not self._cb_done:
                select.select([self.sock], [], [], 0.05)
        self.handlers.pop(cb, None)
        if not self._cb_done:
            raise TimeoutError("labwc did not answer")

    def _on_registry(self, opcode, body):
        if opcode != 0:
            return
        name, off = _u32(body, 0)
        interface, off = _string(body, off)
        version, off = _u32(body, off)
        self.globals.append((name, interface, version))

    def _bind(self):
        for name, interface, version in self.globals:
            if interface == "wl_seat" and self.seat_id is None:
                self.seat_id = self._bind_global(name, interface, min(version, 5))
            elif interface == "wl_output":
                oid = self._bind_global(name, interface, min(version, 4))
                self.outputs[oid] = {
                    "name": "", "x": 0, "y": 0, "width": 0, "height": 0,
                    "phys_w": 0, "phys_h": 0, "refresh": 0, "scale": 1,
                    "transform": "normal", "current": False,
                    "make": "", "model": "",
                }
                self.handlers[oid] = lambda op, body, oid=oid: self._on_output(oid, op, body)
            elif interface == "zwlr_foreign_toplevel_manager_v1" and self.toplevel_manager is None:
                self.toplevel_manager = self._bind_global(name, interface, min(version, 3))
                self.handlers[self.toplevel_manager] = self._on_toplevel_manager
            elif interface == "zwlr_output_power_manager_v1" and self.power_manager is None:
                self.power_manager = self._bind_global(name, interface, min(version, 1))
            elif interface == "zwlr_output_manager_v1" and self.output_manager is None:
                # labwc always offers this. wl_output can be missing while the
                # seat is inactive, and the shell still needs geometry/scale.
                self.output_manager = self._bind_global(name, interface, min(version, 4))
                self.handlers[self.output_manager] = self._on_output_manager

    def _bind_global(self, name, interface, version):
        oid = self.alloc()
        payload = _pack("I", name) + _str(interface) + _pack("II", version, oid)
        self._send(self.registry_id, 0, payload)
        return oid

    def _on_output(self, oid, opcode, body):
        out = self.outputs.get(oid)
        if not out:
            return
        if opcode == 0 and len(body) >= 16:
            x, y, pw, ph = struct.unpack_from("<iiii", body)
            out["x"], out["y"] = x, y
            out["phys_w"], out["phys_h"] = pw, ph
            off = 20  # skip subpixel
            make, off = _string(body, off)
            model, off = _string(body, off)
            out["make"], out["model"] = make, model
            if off + 4 <= len(body):
                transform = struct.unpack_from("<i", body, off)[0]
                out["transform"] = TRANSFORM_NAMES.get(transform, "normal")
        elif opcode == 1 and len(body) >= 16:
            flags, w, h, refresh = struct.unpack_from("<Iiii", body)
            if flags & WL_OUTPUT_MODE_CURRENT or not out["current"]:
                out["width"], out["height"], out["refresh"] = w, h, refresh
                out["current"] = bool(flags & WL_OUTPUT_MODE_CURRENT)
            out.setdefault("modes", [])
            entry = {"width": w, "height": h, "refresh": refresh}
            if entry not in out["modes"]:
                out["modes"].append(entry)
        elif opcode == 3 and len(body) >= 4:
            out["scale"] = struct.unpack_from("<i", body)[0] or 1
        elif opcode == 4:
            out["name"], _ = _string(body, 0)
        self.dirty = True

    def _on_output_manager(self, opcode, body):
        if opcode == 0 and len(body) >= 4:
            hid = struct.unpack_from("<I", body)[0]
            self.heads[hid] = {
                "name": "", "description": "", "make": "", "model": "", "serial": "",
                "x": 0, "y": 0, "width": 0, "height": 0,
                "refresh": 0, "scale": 1.0, "transform": "normal",
                "enabled": False, "current_mode": 0,
                "adaptive_sync": "disabled",
            }
            self.handlers[hid] = lambda op, body, hid=hid: self._on_head(hid, op, body)
            self.dirty = True
        elif opcode == 1:
            self.dirty = True

    def _on_head(self, hid, opcode, body):
        head = self.heads.get(hid)
        if not head:
            return
        if opcode == 0:
            head["name"], _ = _string(body, 0)
        elif opcode == 1:
            head["description"], _ = _string(body, 0)
        elif opcode == 3 and len(body) >= 4:
            mid = struct.unpack_from("<I", body)[0]
            self.modes[mid] = {"width": 0, "height": 0, "refresh": 0, "head": hid,
                               "preferred": False}
            self.handlers[mid] = lambda op, body, mid=mid: self._on_mode(mid, op, body)
        elif opcode == 4 and len(body) >= 4:
            head["enabled"] = struct.unpack_from("<i", body)[0] == 1
        elif opcode == 5 and len(body) >= 4:
            head["current_mode"] = struct.unpack_from("<I", body)[0]
        elif opcode == 6 and len(body) >= 8:
            head["x"], head["y"] = struct.unpack_from("<ii", body)
        elif opcode == 7 and len(body) >= 4:
            transform = struct.unpack_from("<I", body)[0]
            head["transform"] = TRANSFORM_NAMES.get(transform, "normal")
        elif opcode == 8 and len(body) >= 4:
            # wl_fixed_t: 24.8
            head["scale"] = struct.unpack_from("<i", body)[0] / 256.0 or 1.0
        elif opcode == 9:
            self.heads.pop(hid, None)
            self.handlers.pop(hid, None)
        elif opcode == 10:
            head["make"], _ = _string(body, 0)
        elif opcode == 11:
            head["model"], _ = _string(body, 0)
        elif opcode == 12:
            head["serial"], _ = _string(body, 0)
        elif opcode == 13 and len(body) >= 4:
            # zwlr_output_head_v1.adaptive_sync_state since v4
            state = struct.unpack_from("<I", body)[0]
            head["adaptive_sync"] = "enabled" if state == 1 else "disabled"
        self.dirty = True

    def _on_mode(self, mid, opcode, body):
        mode = self.modes.get(mid)
        if not mode:
            return
        if opcode == 0 and len(body) >= 8:
            mode["width"], mode["height"] = struct.unpack_from("<ii", body)
        elif opcode == 1 and len(body) >= 4:
            mode["refresh"] = struct.unpack_from("<i", body)[0]
        elif opcode == 2:
            mode["preferred"] = True
        elif opcode == 3:
            self.modes.pop(mid, None)
            self.handlers.pop(mid, None)
        self.dirty = True

    def _on_toplevel_manager(self, opcode, body):
        if opcode != 0 or len(body) < 4:
            return
        hid = struct.unpack_from("<I", body)[0]
        self.windows[hid] = {
            "id": hid, "pid": 0, "app_id": "", "wm_class": "", "title": "",
            "workspace": "1", "output": "", "output_ids": set(),
            "focused": False, "urgent": False, "floating": True, "hidden": False,
            "maximized": False, "fullscreen": False,
        }
        self.handlers[hid] = lambda op, body, hid=hid: self._on_toplevel(hid, op, body)
        self.dirty = True

    def _on_toplevel(self, hid, opcode, body):
        win = self.windows.get(hid)
        if win is None:
            return
        if opcode == 0:
            win["title"], _ = _string(body, 0)
        elif opcode == 1:
            win["app_id"], _ = _string(body, 0)
        elif opcode == 2 and len(body) >= 4:
            oid = struct.unpack_from("<I", body)[0]
            win["output_ids"].add(oid)
            name = (self.outputs.get(oid) or {}).get("name") or ""
            if name:
                win["output"] = name
        elif opcode == 3 and len(body) >= 4:
            oid = struct.unpack_from("<I", body)[0]
            win["output_ids"].discard(oid)
        elif opcode == 4:
            data, _ = _array(body, 0)
            states = set()
            for i in range(0, len(data) - 3, 4):
                states.add(struct.unpack_from("<I", data, i)[0])
            win["maximized"] = STATE_MAXIMIZED in states
            win["hidden"] = STATE_MINIMIZED in states
            win["focused"] = STATE_ACTIVATED in states
            win["fullscreen"] = STATE_FULLSCREEN in states
        elif opcode == 6:
            self._send(hid, 7, b"")
            self.windows.pop(hid, None)
            self.handlers.pop(hid, None)
        self.dirty = True

    def window_list(self):
        found = []
        for win in self.windows.values():
            if win["app_id"] in SHELL_APP_IDS:
                continue
            names = [(self.outputs.get(oid) or {}).get("name") or "" for oid in win["output_ids"]]
            output = next((n for n in names if n), win["output"])
            found.append({
                "id": win["id"], "pid": 0, "app_id": win["app_id"], "wm_class": "",
                "title": win["title"], "workspace": "1", "output": output,
                "focused": win["focused"], "urgent": False, "floating": True,
                "hidden": win["hidden"], "maximized": win["maximized"],
                "fullscreen": win["fullscreen"],
            })
        return found

    def _head_modes(self, hid):
        found = []
        for mid, mode in self.modes.items():
            if mode.get("head") != hid or not mode.get("width"):
                continue
            found.append({
                "width": mode["width"], "height": mode["height"],
                "refresh": mode.get("refresh") or 0,
                "preferred": bool(mode.get("preferred")),
            })
        found.sort(key=lambda m: (-m["width"] * m["height"], -m["refresh"]))
        return found

    def output_list(self):
        focused = ""
        for win in self.windows.values():
            if win["focused"] and win["output"]:
                focused = win["output"]
        by_name = {}
        # Prefer wlr-output-management heads: they include disabled outputs,
        # mode lists, make/model/serial and adaptive sync.
        for hid, head in self.heads.items():
            if not head["name"]:
                continue
            mode = self.modes.get(head["current_mode"]) or {}
            width = mode.get("width") or 0
            height = mode.get("height") or 0
            refresh = mode.get("refresh") or 0
            scale = head["scale"] or 1
            modes = self._head_modes(hid)
            if not width and modes:
                width, height, refresh = modes[0]["width"], modes[0]["height"], modes[0]["refresh"]
            row = {
                "name": head["name"],
                "active": bool(head["enabled"] and width),
                "focused": head["name"] == focused,
                "scale": scale,
                "transform": head["transform"],
                "make": head.get("make") or "",
                "model": head.get("model") or "",
                "serial": head.get("serial") or "",
                "description": head.get("description") or "",
                "adaptive_sync_status": head.get("adaptive_sync") or "disabled",
                "rect": {
                    "x": head["x"], "y": head["y"],
                    "width": int(width / scale) if width and scale else 0,
                    "height": int(height / scale) if height and scale else 0,
                },
                "current_mode": {
                    "width": width, "height": height, "refresh": refresh,
                },
                "modes": modes,
            }
            by_name[head["name"]] = row
        # Fill gaps from wl_output (make/model, modes) when the seat is active.
        for out in self.outputs.values():
            if not out["name"]:
                continue
            scale = out["scale"] or 1
            row = by_name.get(out["name"])
            if row is None:
                if not out["width"]:
                    continue
                row = {
                    "name": out["name"],
                    "active": True,
                    "focused": out["name"] == focused,
                    "scale": scale,
                    "transform": out["transform"],
                    "make": out.get("make") or "",
                    "model": out.get("model") or "",
                    "serial": "",
                    "description": "",
                    "adaptive_sync_status": "disabled",
                    "rect": {
                        "x": out["x"], "y": out["y"],
                        "width": int(out["width"] / scale),
                        "height": int(out["height"] / scale),
                    },
                    "current_mode": {
                        "width": out["width"], "height": out["height"],
                        "refresh": out["refresh"],
                    },
                    "modes": list(out.get("modes") or []),
                }
                by_name[out["name"]] = row
            else:
                if out.get("make") and not row["make"]:
                    row["make"] = out["make"]
                if out.get("model") and not row["model"]:
                    row["model"] = out["model"]
                if out.get("modes") and not row["modes"]:
                    row["modes"] = list(out["modes"])
                if out["width"] and not row["current_mode"]["width"]:
                    row["current_mode"] = {
                        "width": out["width"], "height": out["height"],
                        "refresh": out["refresh"],
                    }
                    row["rect"] = {
                        "x": out["x"], "y": out["y"],
                        "width": int(out["width"] / scale),
                        "height": int(out["height"] / scale),
                    }
                    row["active"] = True
        rows = list(by_name.values())
        rows.sort(key=lambda r: (not r["active"], r["rect"]["x"], r["rect"]["y"], r["name"]))
        if rows and not any(r["focused"] for r in rows):
            for r in rows:
                if r["active"]:
                    r["focused"] = True
                    break
            else:
                rows[0]["focused"] = True
        return rows

    def _handle(self, ident):
        ident = int(ident)
        if ident not in self.windows:
            raise KeyError(ident)
        return ident

    def activate(self, ident):
        hid = self._handle(ident)
        if self.windows[hid]["hidden"]:
            self._send(hid, 3, b"")
        if self.seat_id:
            self._send(hid, 4, _pack("I", self.seat_id))
        self._flush()

    def close(self, ident):
        self._send(self._handle(ident), 5, b"")
        self._flush()

    def minimize(self, ident):
        self._send(self._handle(ident), 2, b"")
        self._flush()

    def unminimize(self, ident):
        hid = self._handle(ident)
        self._send(hid, 3, b"")
        if self.seat_id:
            self._send(hid, 4, _pack("I", self.seat_id))
        self._flush()

    def toggle_maximize(self, ident):
        hid = self._handle(ident)
        self._send(hid, 1 if self.windows[hid]["maximized"] else 0, b"")
        self._flush()

    def _flush(self):
        if self.watched:
            return
        self.roundtrip()

    def snap_down(self):
        focused = next((w for w in self.windows.values() if w["focused"]), None)
        if not focused:
            return
        if focused["maximized"]:
            self._send(focused["id"], 1, b"")
        else:
            self._send(focused["id"], 2, b"")
        self._flush()

    def show_desktop(self):
        visible = [w for w in self.window_list() if not w["hidden"]]
        if visible:
            _write_json(SHOW_DESKTOP_FILE, [w["id"] for w in visible])
            for w in visible:
                self._send(w["id"], 2, b"")
        else:
            ids = _read_json(SHOW_DESKTOP_FILE, [])
            for ident in ids:
                if ident in self.windows:
                    self._send(ident, 3, b"")
            try:
                os.remove(SHOW_DESKTOP_FILE)
            except OSError:
                pass
        self._flush()

    def output_power(self, mode):
        if not self.power_manager:
            raise RuntimeError("labwc did not offer output power control")
        flag = 0 if mode == "off" else 1
        for oid in list(self.outputs):
            power = self.alloc()
            self._send(self.power_manager, 0, _pack("II", power, oid))
            self._send(power, 0, _pack("I", flag))
        self.roundtrip()

    def focused_output(self):
        for row in self.output_list():
            if row.get("focused"):
                return row["name"]
        rows = self.output_list()
        return rows[0]["name"] if rows else ""


def socket_connect(path):
    import socket
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(path)
    return sock


def _u32(buf, off):
    return struct.unpack_from("<I", buf, off)[0], off + 4


def _string(buf, off):
    n, off = _u32(buf, off)
    raw = buf[off:off + n]
    off += (n + 3) & ~3
    return raw.split(b"\x00", 1)[0].decode("utf-8", "replace"), off


def _array(buf, off):
    n, off = _u32(buf, off)
    data = buf[off:off + n]
    off += (n + 3) & ~3
    return data, off


def _read_json(path, default):
    try:
        with open(path) as handle:
            return json.load(handle)
    except (OSError, ValueError):
        return default


def _write_json(path, data):
    tmp = path + ".tmp"
    with open(tmp, "w") as handle:
        json.dump(data, handle)
    os.replace(tmp, path)


def keyboard_layouts():
    """Layout names from the labwc environment file, plus the last chosen index."""
    names, _variant, _options = _xkb_env()
    index = 0
    try:
        index = int(open(LAYOUT_FILE).read().strip() or "0")
    except (OSError, ValueError):
        index = 0
    if names:
        index %= len(names)
    return names, index


def keyboard_devices():
    names, index = keyboard_layouts()
    if not names:
        return []
    return [{"type": "keyboard", "xkb_layout_names": names, "xkb_active_layout_index": index}]


def _xkb_env():
    layout, variant, options = "us", "", ""
    path = os.path.join(CONFIG_DIR, "environment")
    try:
        text = open(path).read()
    except OSError:
        text = ""
    for line in text.splitlines():
        if line.startswith("XKB_DEFAULT_LAYOUT="):
            layout = line.split("=", 1)[1].strip()
        elif line.startswith("XKB_DEFAULT_VARIANT="):
            variant = line.split("=", 1)[1].strip()
        elif line.startswith("XKB_DEFAULT_OPTIONS="):
            options = line.split("=", 1)[1].strip()
    names = [n for n in layout.split(",") if n]
    return names or ["us"], variant, options


def set_layout(index):
    names, current = keyboard_layouts()
    if not names:
        return
    index = int(index) % len(names)
    _write_json_text(LAYOUT_FILE, str(index))
    steps = (index - current) % len(names)
    if steps and _which("wtype"):
        for _ in range(steps):
            subprocess.run(["wtype", "-M", "alt", "-P", "shift", "-m", "alt", "-p", "shift"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def _write_json_text(path, text):
    tmp = path + ".tmp"
    with open(tmp, "w") as handle:
        handle.write(text)
    os.replace(tmp, path)


def _which(cmd):
    for directory in os.environ.get("PATH", "").split(":"):
        if directory and os.path.isfile(os.path.join(directory, cmd)):
            return True
    return False


def _connect():
    try:
        return Connection()
    except (OSError, TimeoutError) as exc:
        print(f"labwc is not available: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc


def _selftest():
    payload = _str("labwc") + _pack("I", 3)
    text, off = _string(payload, 0)
    version, off = _u32(payload, off)
    assert text == "labwc" and version == 3 and off == len(payload)
    header = _msg(1, 1, payload)
    oid, opcode, size = struct.unpack_from("<IHH", header)
    assert (oid, opcode, size) == (1, 1, len(header))


def main(argv):
    cmd = argv[1] if len(argv) > 1 else "windows"
    if cmd == "selftest":
        _selftest()
        return 0
    if cmd == "layouts":
        names, index = keyboard_layouts()
        json.dump({"names": names, "index": index}, sys.stdout)
        print()
        return 0
    if cmd == "layout":
        set_layout(argv[2] if len(argv) > 2 else "0")
        return 0
    conn = _connect()
    if cmd == "windows":
        json.dump(conn.window_list(), sys.stdout)
        print()
    elif cmd == "outputs":
        json.dump(conn.output_list(), sys.stdout)
        print()
    elif cmd == "focused-output":
        print(conn.focused_output())
    elif cmd == "activate":
        conn.activate(argv[2])
    elif cmd == "close":
        conn.close(argv[2])
    elif cmd == "minimize":
        conn.minimize(argv[2])
    elif cmd == "unminimize":
        conn.unminimize(argv[2])
    elif cmd == "toggle-maximize":
        conn.toggle_maximize(argv[2])
    elif cmd == "snap-down":
        conn.snap_down()
    elif cmd == "show-desktop":
        conn.show_desktop()
    elif cmd == "output-power":
        conn.output_power(argv[2] if len(argv) > 2 else "off")
    elif cmd == "subscribe":
        print(json.dumps({"event": "windows"}), flush=True)
        while True:
            select.select([conn.sock], [], [], None)
            conn.dirty = False
            conn._pump()
            if conn.dirty:
                print(json.dumps({"event": "windows", "windows": conn.window_list()}), flush=True)
    else:
        print("usage: wm.py windows|outputs|focused-output|activate ID|close ID|"
              "minimize ID|unminimize ID|toggle-maximize ID|snap-down|show-desktop|"
              "output-power off|on|layouts|layout N|subscribe|selftest", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv))
    except BrokenPipeError:
        raise SystemExit(0)
