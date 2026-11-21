# Registry Editor's data model. HKEY_CURRENT_USER\Software\MDE is the only
# live hive: Shell, Taskbar, Start and Window Layout are org.mde GSettings,
# and Files mirrors ~/.config/mde (a directory is a key, a file is a value).
# The other four hives are present and empty.

from __future__ import annotations

import json
import os
import shutil
import xml.etree.ElementTree as ET

REG_SZ = "REG_SZ"
REG_DWORD = "REG_DWORD"
REG_BINARY = "REG_BINARY"
REG_MULTI_SZ = "REG_MULTI_SZ"

HIVES = (
    "HKEY_CLASSES_ROOT",
    "HKEY_CURRENT_USER",
    "HKEY_LOCAL_MACHINE",
    "HKEY_USERS",
    "HKEY_CURRENT_CONFIG",
)

MDE_PREFIX = r"HKEY_CURRENT_USER\Software\MDE"
FILES_PREFIX = MDE_PREFIX + r"\Files"

# Shown in this order. A schema id that is not listed still appears, after these.
AREA_NAMES = (
    ("org.mde", "Shell"),
    ("org.mde.taskbar", "Taskbar"),
    ("org.mde.start", "Start"),
    ("org.mde.window-layout", "Window Layout"),
)

HEADER = "Windows Registry Editor Version 5.00"


class RegistryError(Exception):
    pass


class KeySpec:
    def __init__(self, name, gtype, default, choices):
        self.name, self.gtype, self.default, self.choices = name, gtype, default, tuple(choices)


class Area:
    def __init__(self, name, schema, keys):
        self.name, self.schema, self.keys = name, schema, list(keys)


class Value:
    def __init__(self, name, kind, data, *, schema=None, gkey=None, gtype=None,
                 choices=(), default=None, filepath=None):
        self.name, self.kind, self.data = name, kind, data
        self.schema, self.gkey, self.gtype = schema, gkey, gtype
        self.choices, self.default, self.filepath = tuple(choices), default, filepath

    @property
    def schema_backed(self):
        return self.schema is not None


class Key:
    def __init__(self, name, path, kind, *, schema=None, dirpath=None):
        self.name, self.path, self.kind = name, path, kind
        self.schema, self.dirpath = schema, dirpath
        self.children = []
        self.values = []

    @property
    def structural(self):
        """New keys, new values, rename and delete apply under Files."""
        return self.kind == "directory"


class ImportResult:
    def __init__(self):
        self.written = 0
        self.skipped = 0
        self.notes = []

    def skip(self, note):
        self.skipped += 1
        if len(self.notes) < 8:
            self.notes.append(note)

    def write(self):
        self.written += 1


class Match:
    def __init__(self, path, value_name, wrapped):
        self.path, self.value_name, self.wrapped = path, value_name, wrapped


def format_data(value):
    if value.kind == REG_DWORD:
        n = int(value.data) & 0xFFFFFFFF
        return f"0x{n:08x} ({n})"
    if value.kind == REG_BINARY:
        data = value.data or b""
        if not data:
            return "(zero-length binary value)"
        preview = " ".join(f"{b:02x}" for b in data[:16])
        if len(data) > 16:
            preview += " ..."
        return preview
    if value.kind == REG_MULTI_SZ:
        return " ".join(value.data)
    return "" if value.data is None else str(value.data)


def schema_candidates():
    here = os.path.dirname(os.path.abspath(__file__))
    return [
        os.environ.get("MDE_GSCHEMA") or "",
        os.path.abspath(os.path.join(here, "..", "..", "data", "glib-2.0", "schemas", "org.mde.gschema.xml")),
        "/usr/share/glib-2.0/schemas/org.mde.gschema.xml",
    ]


def default_schema_path():
    for path in schema_candidates():
        if path and os.path.isfile(path):
            return path
    return ""


def default_config_root():
    base = os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config"))
    return os.path.join(base, "mde")


def default_favorites_path():
    base = os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config"))
    return os.path.join(base, "ws-regedit", "favorites.json")


def _parse_default(gtype, text):
    text = (text or "").strip()
    if gtype == "b":
        return text == "true"
    if gtype == "i":
        return int(text or "0")
    if gtype == "as":
        if text in ("", "[]"):
            return []
        inner = text[1:-1].strip() if text.startswith("[") else text
        if not inner:
            return []
        out = []
        for part in inner.split(","):
            part = part.strip()
            if len(part) >= 2 and part[0] == "'" and part[-1] == "'":
                part = part[1:-1]
            out.append(part)
        return out
    if len(text) >= 2 and text[0] == "'" and text[-1] == "'":
        return text[1:-1]
    return text


def load_areas(schema_file):
    if not schema_file or not os.path.isfile(schema_file):
        raise RegistryError("Cannot read the MDE settings schema.")
    root = ET.parse(schema_file).getroot()
    found = {}
    order = []
    for schema in root.findall("schema"):
        sid = schema.get("id") or ""
        keys = []
        for key in schema.findall("key"):
            gtype = key.get("type") or "s"
            default_el = key.find("default")
            choices = tuple(c.get("value") for c in key.findall("choices/choice") if c.get("value"))
            keys.append(KeySpec(key.get("name"), gtype, _parse_default(gtype, default_el.text if default_el is not None else ""), choices))
        found[sid] = keys
        order.append(sid)
    areas = []
    used = set()
    for sid, title in AREA_NAMES:
        if sid in found:
            areas.append(Area(title, sid, found[sid]))
            used.add(sid)
    for sid in order:
        if sid in used or not sid.startswith("org.mde"):
            continue
        title = sid.split(".")[-1].replace("-", " ").title()
        areas.append(Area(title, sid, found[sid]))
    return areas


def _gtype_kind(gtype):
    if gtype == "b" or gtype == "i":
        return REG_DWORD
    if gtype == "as":
        return REG_MULTI_SZ
    return REG_SZ


def _gtype_data(gtype, raw):
    if gtype == "b":
        return 1 if raw else 0
    if gtype == "i":
        return int(raw) & 0xFFFFFFFF
    if gtype == "as":
        return list(raw or [])
    return "" if raw is None else str(raw)


def _safe_name(name):
    if not name or name in (".", "..") or "/" in name or "\\" in name or "\x00" in name:
        raise RegistryError(f"'{name}' is not a valid name.")
    if len(name) > 255:
        raise RegistryError("That name is too long.")
    return name


def _classify(data):
    if b"\x00" in data:
        return REG_BINARY, data
    try:
        return REG_SZ, data.decode("utf-8")
    except UnicodeDecodeError:
        return REG_BINARY, data


def _encode_multisz(strings):
    return b"".join(s.encode("utf-16le") + b"\x00\x00" for s in strings) + b"\x00\x00"


def _decode_multisz(data):
    strings = []
    i = 0
    while i + 1 < len(data):
        j = i
        while j + 1 < len(data) and not (data[j] == 0 and data[j + 1] == 0):
            j += 2
        if j == i:
            if j + 2 >= len(data):
                break
            strings.append("")
            i = j + 2
            continue
        strings.append(data[i:j].decode("utf-16le"))
        i = j + 2
    return strings


def _hex_dump(prefix, data):
    body = ",".join(f"{b:02x}" for b in data)
    # Break long lines the way Registry Editor does, on a comma.
    limit = 76
    first = f"{prefix}:{body}"
    if len(first) <= limit:
        return first
    lines = []
    chunk = prefix + ":"
    for i, part in enumerate(body.split(",")):
        piece = part if i == 0 else "," + part
        if len(chunk) + len(piece) > limit:
            lines.append(chunk + ",\\")
            chunk = "  " + part
        else:
            chunk += piece
    lines.append(chunk)
    return "\n".join(lines)


def _escape_reg(text):
    return text.replace("\\", "\\\\").replace('"', '\\"').replace("\r", "\\r").replace("\n", "\\n")


def _unfold(text):
    lines = []
    buf = ""
    for line in text.splitlines():
        joined = buf + line if buf else line
        buf = ""
        slashes = len(joined) - len(joined.rstrip("\\"))
        if slashes % 2 == 1:
            buf = joined[:-1]
        else:
            lines.append(joined)
    if buf:
        lines.append(buf)
    return lines


def _parse_quoted(text, i):
    if i >= len(text) or text[i] != '"':
        raise RegistryError("A registry value is missing quotes.")
    i += 1
    out = []
    while i < len(text):
        if text[i] == "\\" and i + 1 < len(text):
            mapping = {"\\": "\\", '"': '"', "n": "\n", "r": "\r"}
            out.append(mapping.get(text[i + 1], text[i + 1]))
            i += 2
            continue
        if text[i] == '"':
            return "".join(out), i + 1
        out.append(text[i])
        i += 1
    raise RegistryError("A registry value is missing its closing quote.")


def _parse_hex_list(text):
    text = text.replace(" ", "").replace("\t", "")
    if not text:
        return b""
    try:
        return bytes(int(part, 16) for part in text.split(",") if part != "")
    except ValueError as exc:
        raise RegistryError("A binary value contains invalid hex.") from exc


def _decode_utf16(data):
    if data.endswith(b"\x00\x00"):
        data = data[:-2]
    return data.decode("utf-16le")


def parse_value_line(line):
    line = line.strip()
    if line.startswith("@"):
        name, rest = "(Default)", line[1:].strip()
    elif line.startswith('"'):
        name, idx = _parse_quoted(line, 0)
        rest = line[idx:].strip()
    else:
        raise RegistryError(f"Cannot read this line: {line[:80]}")
    if not rest.startswith("="):
        raise RegistryError(f"Cannot read this line: {line[:80]}")
    rest = rest[1:].strip()
    if rest == "-":
        return name, "delete", None
    if rest.startswith('"'):
        text, _end = _parse_quoted(rest, 0)
        return name, REG_SZ, text
    lower = rest.lower()
    if lower.startswith("dword:"):
        try:
            return name, REG_DWORD, int(rest.split(":", 1)[1], 16)
        except ValueError as exc:
            raise RegistryError("A DWORD value is not valid hex.") from exc
    if lower.startswith("hex(7):"):
        return name, REG_MULTI_SZ, _decode_multisz(_parse_hex_list(rest.split(":", 1)[1]))
    if lower.startswith("hex(1):") or lower.startswith("hex(2):"):
        return name, REG_SZ, _decode_utf16(_parse_hex_list(rest.split(":", 1)[1]))
    if lower.startswith("hex:"):
        return name, REG_BINARY, _parse_hex_list(rest.split(":", 1)[1])
    raise RegistryError(f"Unsupported value data: {rest[:40]}")


def read_reg_file(path):
    with open(path, "rb") as handle:
        raw = handle.read()
    if raw.startswith(b"\xff\xfe") or raw.startswith(b"\xfe\xff"):
        return raw.decode("utf-16")
    return raw.decode("utf-8-sig")


def write_reg_file(path, text):
    # Version 5.00 files are UTF-16 LE. utf-16 writes the BOM.
    with open(path, "wb") as handle:
        handle.write(text.encode("utf-16"))


class DictSettings:
    """In-memory GSettings stand-in for tests."""

    def __init__(self, initial=None):
        self.data = dict(initial or {})

    def get(self, schema, key, gtype, default):
        return self.data.get((schema, key), default)

    def set(self, schema, key, gtype, value):
        self.data[(schema, key)] = value

    def reset(self, schema, key):
        self.data.pop((schema, key), None)

    def watch(self, _schemas, _callback):
        return lambda: None


class GioSettings:
    """Live org.mde GSettings, or the same JSON fallback Settings uses."""

    def __init__(self):
        self._cache = {}
        self._fallback = os.path.join(
            os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config")),
            "ws-settings", "gsettings-fallback.json")

    def _gio(self, schema):
        if schema in self._cache:
            return self._cache[schema]
        settings = None
        try:
            import gi

            gi.require_version("Gio", "2.0")
            from gi.repository import Gio

            source = Gio.SettingsSchemaSource.get_default()
            compiled = None if source is None else source.lookup(schema, True)
            # A missing key aborts the process, so remember which keys this
            # install actually compiled.
            if compiled is not None:
                settings = (Gio.Settings.new(schema), compiled)
        except Exception:
            settings = None
        self._cache[schema] = settings
        return settings

    def _load_fallback(self):
        try:
            with open(self._fallback) as handle:
                data = json.load(handle)
            return data if isinstance(data, dict) else {}
        except (OSError, ValueError):
            return {}

    def _save_fallback(self, data):
        os.makedirs(os.path.dirname(self._fallback), exist_ok=True)
        tmp = self._fallback + ".tmp"
        with open(tmp, "w") as handle:
            json.dump(data, handle, indent=2)
            handle.write("\n")
        os.replace(tmp, self._fallback)

    def get(self, schema, key, gtype, default):
        pair = self._gio(schema)
        if pair is None:
            raw = self._load_fallback().get(schema, {}).get(key, default)
            return _gtype_data(gtype, raw) if gtype in ("b", "i") else raw
        if not pair[1].has_key(key):
            return default
        settings = pair[0]
        try:
            if gtype == "s":
                return settings.get_string(key)
            if gtype == "b":
                return 1 if settings.get_boolean(key) else 0
            if gtype == "i":
                return int(settings.get_int(key)) & 0xFFFFFFFF
            if gtype == "as":
                return list(settings.get_strv(key))
        except Exception:
            return default
        return default

    def set(self, schema, key, gtype, value):
        pair = self._gio(schema)
        if pair is None:
            data = self._load_fallback()
            bucket = data.setdefault(schema, {})
            if gtype == "b":
                bucket[key] = bool(value)
            elif gtype == "i":
                bucket[key] = int(value)
            elif gtype == "as":
                bucket[key] = list(value)
            else:
                bucket[key] = value
            self._save_fallback(data)
            return
        if not pair[1].has_key(key):
            raise RegistryError(f"'{key}' is not in the installed settings schema.")
        settings = pair[0]
        if gtype == "s":
            ok = settings.set_string(key, value)
        elif gtype == "b":
            ok = settings.set_boolean(key, bool(value))
        elif gtype == "i":
            ok = settings.set_int(key, int(value))
        elif gtype == "as":
            ok = settings.set_strv(key, list(value))
        else:
            raise RegistryError(f"Cannot store a {gtype} value.")
        if not ok:
            raise RegistryError(f"Could not store '{key}'.")
        self._sync()

    def reset(self, schema, key):
        pair = self._gio(schema)
        if pair is None:
            data = self._load_fallback()
            data.get(schema, {}).pop(key, None)
            self._save_fallback(data)
            return
        if not pair[1].has_key(key):
            raise RegistryError(f"'{key}' is not in the installed settings schema.")
        pair[0].reset(key)
        self._sync()

    def watch(self, schemas, callback):
        tokens = []
        for schema in schemas:
            pair = self._gio(schema)
            if pair is None:
                continue
            settings = pair[0]
            tokens.append((settings, settings.connect("changed", lambda *_a: callback())))
        def unwatch():
            for settings, token in tokens:
                settings.disconnect(token)
        return unwatch

    @staticmethod
    def _sync():
        try:
            import gi

            gi.require_version("Gio", "2.0")
            from gi.repository import Gio

            Gio.Settings.sync()
        except Exception:
            pass


class Registry:
    def __init__(self, config_root=None, schema_file=None, settings=None, favorites_path=None):
        self.config_root = config_root or default_config_root()
        self.schema_file = schema_file if schema_file is not None else default_schema_path()
        self.areas = load_areas(self.schema_file)
        self.settings = settings if settings is not None else GioSettings()
        self.favorites_path = favorites_path or default_favorites_path()
        self._by_path = {}

    def watch(self, callback):
        return self.settings.watch([area.schema for area in self.areas], callback)

    def tree(self):
        computer = Key("Computer", "", "computer")
        by_schema = {area.schema: area for area in self.areas}
        for hive in HIVES:
            node = Key(hive, hive, "hive")
            if hive == "HKEY_CURRENT_USER":
                software = Key("Software", hive + r"\Software", "folder")
                mde = Key("MDE", MDE_PREFIX, "folder")
                for schema, title in AREA_NAMES:
                    area = by_schema.get(schema)
                    if area:
                        mde.children.append(self._schema_key(area))
                known = {schema for schema, _title in AREA_NAMES}
                for area in self.areas:
                    if area.schema not in known:
                        mde.children.append(self._schema_key(area))
                mde.children.append(self._files_key(FILES_PREFIX, self.config_root))
                software.children.append(mde)
                node.children.append(software)
            computer.children.append(node)
        self._index(computer)
        return computer

    def _index(self, key):
        self._by_path = {}

        def walk(node):
            self._by_path[node.path.lower()] = node
            for child in node.children:
                walk(child)
        walk(key)

    def _schema_key(self, area):
        key = Key(area.name, MDE_PREFIX + "\\" + area.name, "schema", schema=area.schema)
        for spec in area.keys:
            raw = self.settings.get(area.schema, spec.name, spec.gtype, spec.default)
            data = raw if spec.gtype in ("s", "as") else _gtype_data(spec.gtype, raw)
            if spec.gtype == "s":
                data = "" if data is None else str(data)
            elif spec.gtype == "as":
                data = list(data or [])
            key.values.append(Value(
                spec.name, _gtype_kind(spec.gtype), data,
                schema=area.schema, gkey=spec.name, gtype=spec.gtype,
                choices=spec.choices, default=spec.default))
        return key

    def _files_key(self, reg_path, dirpath):
        key = Key(reg_path.split("\\")[-1], reg_path, "directory", dirpath=dirpath)
        if not os.path.isdir(dirpath):
            return key
        entries = []
        try:
            entries = sorted(os.scandir(dirpath), key=lambda item: item.name.lower())
        except OSError:
            return key
        dirs, files = [], []
        for item in entries:
            if item.is_symlink():
                continue
            if item.is_dir():
                dirs.append(item)
            elif item.is_file():
                files.append(item)
        for item in files:
            try:
                with open(item.path, "rb") as handle:
                    blob = handle.read()
            except OSError:
                continue
            kind, data = _classify(blob)
            key.values.append(Value(item.name, kind, data, filepath=item.path))
        for item in dirs:
            key.children.append(self._files_key(reg_path + "\\" + item.name, item.path))
        return key

    def key_at(self, path):
        if not self._by_path:
            self.tree()
        text = (path or "").replace("/", "\\").strip("\\")
        if text.lower().startswith("computer\\"):
            text = text.split("\\", 1)[1]
        if text.lower() in ("", "computer"):
            return self._by_path.get("")
        return self._by_path.get(text.lower())

    def _require(self, path):
        self.tree()
        key = self.key_at(path)
        if key is None:
            raise RegistryError(f"Cannot find the key '{path}'.")
        return key

    def _value(self, key, name):
        for value in key.values:
            if value.name == name:
                return value
        raise RegistryError(f"Cannot find the value '{name}'.")

    def set_value(self, path, name, kind, data):
        key = self._require(path)
        value = self._value(key, name)
        if value.schema_backed:
            self._write_schema(value, kind, data)
        else:
            self._write_file(value.filepath, kind, data)
        return self._require(path)

    def _write_schema(self, value, kind, data):
        if kind != value.kind:
            raise RegistryError(f"'{value.name}' is a {value.kind} value.")
        if value.gtype == "s":
            text = str(data)
            if value.choices and text not in value.choices:
                raise RegistryError("'" + "', '".join(value.choices) + "' are the only values this setting accepts.")
            self.settings.set(value.schema, value.gkey, "s", text)
        elif value.gtype == "b":
            self.settings.set(value.schema, value.gkey, "b", int(data) != 0)
        elif value.gtype == "i":
            number = int(data)
            if number < 0 or number > 0x7FFFFFFF:
                raise RegistryError("This setting accepts a number from 0 to 2147483647.")
            self.settings.set(value.schema, value.gkey, "i", number)
        elif value.gtype == "as":
            if not isinstance(data, (list, tuple)) or any(not isinstance(item, str) for item in data):
                raise RegistryError("This setting stores one string on each line.")
            self.settings.set(value.schema, value.gkey, "as", list(data))
        else:
            raise RegistryError(f"Cannot store '{value.name}'.")

    def _write_file(self, filepath, kind, data):
        if kind == REG_SZ:
            blob = str(data).encode("utf-8")
        elif kind == REG_BINARY:
            blob = bytes(data)
        else:
            raise RegistryError("Files are stored as REG_SZ or REG_BINARY.")
        parent = os.path.dirname(filepath)
        self._ensure_dir(parent)
        tmp = filepath + ".regedit-tmp"
        with open(tmp, "wb") as handle:
            handle.write(blob)
        os.replace(tmp, filepath)

    def reset_value(self, path, name):
        key = self._require(path)
        value = self._value(key, name)
        if not value.schema_backed:
            raise RegistryError(f"'{name}' is a file. Delete removes it.")
        self.settings.reset(value.schema, value.gkey)

    def delete_value(self, path, name):
        key = self._require(path)
        value = self._value(key, name)
        if value.schema_backed:
            self.settings.reset(value.schema, value.gkey)
            return
        self._unlink(value.filepath)

    def delete_key(self, path):
        key = self._require(path)
        if key.kind != "directory" or key.path.lower() == FILES_PREFIX.lower():
            raise RegistryError("This key cannot be deleted.")
        self._reject_link(key.dirpath)
        if not self._under_root(key.dirpath):
            raise RegistryError("That folder is outside MDE settings.")
        shutil.rmtree(key.dirpath)

    def new_key(self, path, name):
        parent = self._require(path)
        if not parent.structural:
            raise RegistryError("New keys can be created under Files only.")
        name = _safe_name(name)
        dest = os.path.join(parent.dirpath, name)
        if os.path.exists(dest):
            raise RegistryError(f"'{name}' already exists.")
        self._ensure_dir(parent.dirpath)
        os.mkdir(dest)
        return parent.path + "\\" + name

    def new_value(self, path, name, kind, data):
        parent = self._require(path)
        if not parent.structural:
            raise RegistryError("New values can be created under Files only.")
        if kind not in (REG_SZ, REG_BINARY):
            raise RegistryError("New file values are REG_SZ or REG_BINARY.")
        name = _safe_name(name)
        dest = os.path.join(parent.dirpath, name)
        if os.path.exists(dest):
            raise RegistryError(f"'{name}' already exists.")
        self._ensure_dir(parent.dirpath)
        self._write_file(dest, kind, data)

    def rename(self, path, new_name, value_name=None):
        new_name = _safe_name(new_name)
        key = self._require(path)
        if value_name is not None:
            value = self._value(key, value_name)
            if value.schema_backed:
                raise RegistryError("Schema value names are fixed.")
            dest = os.path.join(os.path.dirname(value.filepath), new_name)
            if os.path.exists(dest):
                raise RegistryError(f"'{new_name}' already exists.")
            self._reject_link(value.filepath)
            os.rename(value.filepath, dest)
            return
        if key.kind != "directory" or key.path.lower() == FILES_PREFIX.lower():
            raise RegistryError("This key cannot be renamed.")
        dest = os.path.join(os.path.dirname(key.dirpath), new_name)
        if os.path.exists(dest):
            raise RegistryError(f"'{new_name}' already exists.")
        self._reject_link(key.dirpath)
        os.rename(key.dirpath, dest)

    def export_text(self, path):
        key = self._require(path)
        lines = [HEADER, ""]
        self._export_key(key, lines)
        return "\n".join(lines).rstrip() + "\n"

    def _export_key(self, key, lines):
        if key.path:
            lines.append(f"[{key.path}]")
            for value in key.values:
                lines.append(self._export_value(value))
            lines.append("")
        for child in key.children:
            self._export_key(child, lines)

    def _export_value(self, value):
        name = '"' + _escape_reg(value.name) + '"'
        if value.kind == REG_SZ:
            return f'{name}="{_escape_reg(str(value.data))}"'
        if value.kind == REG_DWORD:
            return f"{name}=dword:{int(value.data) & 0xFFFFFFFF:08x}"
        if value.kind == REG_MULTI_SZ:
            return f"{name}={_hex_dump('hex(7)', _encode_multisz(list(value.data)))}"
        return f"{name}={_hex_dump('hex', bytes(value.data or b''))}"

    def import_text(self, text):
        unfolded = _unfold(text)
        first = next((line.strip() for line in unfolded if line.strip()), "")
        if first != HEADER:
            raise RegistryError("This is not a Version 5.00 registry file.")
        result = ImportResult()
        current = None
        for line in unfolded:
            stripped = line.strip()
            if not stripped or stripped.startswith(";") or stripped == HEADER:
                continue
            if stripped.startswith("["):
                if not stripped.endswith("]"):
                    raise RegistryError(f"Cannot read this key: {stripped[:80]}")
                body = stripped[1:-1]
                if body.startswith("-"):
                    self._import_delete_key(body[1:], result)
                    current = None
                else:
                    current = body
                continue
            if current is None:
                continue
            name, kind, data = parse_value_line(stripped)
            self._import_assignment(current, name, kind, data, result)
        return result

    def _schema_accepts(self, spec, kind, data):
        if kind != _gtype_kind(spec.gtype):
            return False
        if spec.gtype == "s" and spec.choices and str(data) not in spec.choices:
            return False
        if spec.gtype == "i":
            number = int(data)
            if number < 0 or number > 0x7FFFFFFF:
                return False
        if spec.gtype == "as" and (not isinstance(data, list) or any(not isinstance(item, str) for item in data)):
            return False
        return True

    def _coerce_schema(self, spec, data):
        if spec.gtype == "b":
            return 1 if int(data) else 0
        if spec.gtype == "i":
            return int(data)
        if spec.gtype == "as":
            return list(data)
        return str(data)

    def _import_delete_key(self, path, result):
        target = self._classify_path(path)
        if target is None:
            result.skip(f"Skipped deleting {path}.")
            return
        where, extra = target
        if where == "schema":
            area, _spec = extra
            if area is None:
                result.skip(f"Skipped deleting {path}.")
                return
            for spec in area.keys:
                self.settings.reset(area.schema, spec.name)
                result.write()
            return
        dirpath, leaf = extra
        if not leaf:
            result.skip("Skipped deleting Files itself.")
            return
        if os.path.isdir(dirpath) and not os.path.islink(dirpath) and self._under_root(dirpath):
            self._reject_link(dirpath)
            shutil.rmtree(dirpath)
            result.write()
        else:
            result.skip(f"Skipped deleting {path}, which is not a folder.")

    def _classify_path(self, path):
        """Return ('schema', (area, spec|None)) or ('file', (dirpath, leaf)) or None.

        spec is None when the value name is not a schema key. leaf is the last
        path component under Files; an empty leaf means the Files key itself.
        """
        text = path.replace("/", "\\").strip("\\")
        lower = text.lower()
        files = FILES_PREFIX.lower()
        if lower == files or lower.startswith(files + "\\"):
            rest = text[len(FILES_PREFIX):].lstrip("\\")
            parts = [part for part in rest.split("\\") if part]
            try:
                for part in parts:
                    _safe_name(part)
            except RegistryError:
                return None
            return "file", (os.path.join(self.config_root, *parts), parts[-1] if parts else "")
        prefix = MDE_PREFIX.lower() + "\\"
        if not lower.startswith(prefix):
            return None
        rest = text[len(MDE_PREFIX) + 1:]
        head, _, tail = rest.partition("\\")
        for area in self.areas:
            if area.name.lower() != head.lower():
                continue
            if tail:
                return None
            return "schema", (area, None)
        return None

    def _area_value(self, area, name):
        for spec in area.keys:
            if spec.name == name:
                return spec
        return None

    def _import_assignment(self, path, name, kind, data, result):
        if name == "(Default)":
            result.skip(f"Skipped the default value of {path}.")
            return
        target = self._classify_path(path)
        if target is None:
            result.skip(f"Skipped {path}, which Registry Editor cannot store.")
            return
        where, extra = target
        if where == "schema":
            area, _spec = extra
            spec = self._area_value(area, name) if area else None
            if spec is None:
                result.skip(f"Skipped '{name}', which is not in the schema.")
                return
            if kind == "delete":
                self.settings.reset(area.schema, spec.name)
                result.write()
                return
            if not self._schema_accepts(spec, kind, data):
                result.skip(f"Skipped '{name}' because its type or value is not allowed.")
                return
            value = Value(spec.name, _gtype_kind(spec.gtype), data, schema=area.schema,
                          gkey=spec.name, gtype=spec.gtype, choices=spec.choices, default=spec.default)
            self._write_schema(value, value.kind, self._coerce_schema(spec, data))
            result.write()
            return
        dirpath, _leaf = extra
        if kind == "delete":
            filepath = os.path.join(dirpath, name)
            if os.path.isfile(filepath) and not os.path.islink(filepath):
                self._unlink(filepath)
                result.write()
            else:
                result.skip(f"Skipped deleting '{name}', which is not a file.")
            return
        if kind not in (REG_SZ, REG_BINARY):
            result.skip(f"Skipped '{name}' because Files stores text and binary values.")
            return
        try:
            _safe_name(name)
        except RegistryError:
            result.skip(f"Skipped '{name}' because the name is not valid.")
            return
        self._ensure_dir(dirpath)
        self._write_file(os.path.join(dirpath, name), kind, data if kind == REG_BINARY else str(data))
        result.write()

    def find(self, query, *, keys=True, values=True, data=True, start_path="", start_value=None, after=True):
        needle = (query or "").lower()
        if not needle:
            return None
        root = self.tree()
        items = []

        def walk(node):
            items.append((node.path, None, node.name))
            for value in node.values:
                items.append((node.path, value.name, value))
            for child in node.children:
                walk(child)
        walk(root)
        start = 0
        for index, (path, value_name, _payload) in enumerate(items):
            if path.lower() == (start_path or "").lower() and value_name == start_value:
                start = index + (1 if after else 0)
                break
        order = list(range(start, len(items))) + list(range(0, start))
        for offset, index in enumerate(order):
            path, value_name, payload = items[index]
            if value_name is None:
                if keys and needle in payload.lower():
                    return Match(path, None, offset > 0 and index < start)
            else:
                hit = values and needle in value_name.lower()
                if data and needle in self._search_text(payload):
                    hit = True
                if hit:
                    return Match(path, value_name, offset > 0 and index < start)
        return None

    def _search_text(self, value):
        if value.kind == REG_DWORD:
            n = int(value.data) & 0xFFFFFFFF
            return f"0x{n:08x} {n} {format_data(value)}".lower()
        if value.kind == REG_BINARY:
            return " ".join(f"{b:02x}" for b in value.data).lower()
        if value.kind == REG_MULTI_SZ:
            return "\n".join(value.data).lower()
        return str(value.data).lower()

    def favorites(self):
        try:
            with open(self.favorites_path) as handle:
                data = json.load(handle)
            if isinstance(data, list):
                return [item for item in data if isinstance(item, str)]
        except (OSError, ValueError):
            pass
        return []

    def set_favorites(self, paths):
        os.makedirs(os.path.dirname(self.favorites_path), exist_ok=True)
        tmp = self.favorites_path + ".tmp"
        with open(tmp, "w") as handle:
            json.dump(list(paths), handle, indent=2)
            handle.write("\n")
        os.replace(tmp, self.favorites_path)

    def _ensure_dir(self, path):
        if os.path.islink(path) or not self._under_root(path):
            raise RegistryError("That folder is outside MDE settings.")
        os.makedirs(path, exist_ok=True)
        if not self._under_root(path):
            raise RegistryError("That folder is outside MDE settings.")

    def _unlink(self, path):
        self._reject_link(path)
        if not self._under_root(path):
            raise RegistryError("That file is outside MDE settings.")
        os.remove(path)

    def _reject_link(self, path):
        if os.path.islink(path):
            raise RegistryError("Registry Editor does not change symbolic links.")

    def _under_root(self, path):
        root = os.path.realpath(self.config_root)
        real = os.path.realpath(path)
        return real == root or real.startswith(root + os.sep)
