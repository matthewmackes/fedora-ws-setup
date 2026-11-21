#!/usr/bin/env python3
"""Model tests for Registry Editor. No display and no GSettings required."""

import os
import tempfile
import textwrap
import unittest

import registry as reg


SCHEMA = textwrap.dedent("""\
    <?xml version="1.0" encoding="UTF-8"?>
    <schemalist>
      <schema id="org.mde" path="/org/mde/">
        <key name="accent" type="s"><default>'#0078d4'</default></key>
        <key name="color-scheme" type="s">
          <choices>
            <choice value="prefer-light"/>
            <choice value="prefer-dark"/>
          </choices>
          <default>'prefer-light'</default>
        </key>
        <key name="animations" type="b"><default>true</default></key>
        <key name="panel-height" type="i"><default>48</default></key>
      </schema>
      <schema id="org.mde.taskbar" path="/org/mde/taskbar/">
        <key name="autohide" type="b"><default>false</default></key>
      </schema>
      <schema id="org.mde.start" path="/org/mde/start/">
        <key name="show-recommended" type="b"><default>true</default></key>
      </schema>
      <schema id="org.mde.window-layout" path="/org/mde/window-layout/">
        <key name="custom-rules" type="as"><default>[]</default></key>
      </schema>
    </schemalist>
    """)


class RegistryTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = os.path.join(self.tmp.name, "mde")
        self.schema = os.path.join(self.tmp.name, "org.mde.gschema.xml")
        self.fav = os.path.join(self.tmp.name, "favorites.json")
        with open(self.schema, "w") as handle:
            handle.write(SCHEMA)
        self.settings = reg.DictSettings()
        self.reg = reg.Registry(self.root, self.schema, self.settings, self.fav)

    def tearDown(self):
        self.tmp.cleanup()

    def key(self, path):
        self.reg.tree()
        return self.reg.key_at(path)

    def value(self, path, name):
        for item in self.key(path).values:
            if item.name == name:
                return item
        self.fail(name)

    def test_hives_and_areas(self):
        computer = self.reg.tree()
        self.assertEqual([child.name for child in computer.children], list(reg.HIVES))
        for hive in ("HKEY_CLASSES_ROOT", "HKEY_LOCAL_MACHINE", "HKEY_USERS", "HKEY_CURRENT_CONFIG"):
            self.assertEqual(self.key(hive).children, [])
        mde = self.key(reg.MDE_PREFIX)
        self.assertEqual([child.name for child in mde.children],
                         ["Shell", "Taskbar", "Start", "Window Layout", "Files"])
        self.assertEqual(self.key(reg.FILES_PREFIX).name, "Files")

    def test_types_and_dword_text(self):
        animations = self.value(reg.MDE_PREFIX + r"\Shell", "animations")
        self.assertEqual(animations.kind, reg.REG_DWORD)
        self.assertEqual(animations.data, 1)
        self.assertEqual(reg.format_data(animations), "0x00000001 (1)")
        self.assertEqual(self.value(reg.MDE_PREFIX + r"\Shell", "accent").kind, reg.REG_SZ)
        scheme = self.value(reg.MDE_PREFIX + r"\Shell", "color-scheme")
        self.assertEqual(scheme.choices, ("prefer-light", "prefer-dark"))
        rules = self.value(reg.MDE_PREFIX + r"\Window Layout", "custom-rules")
        self.assertEqual(rules.kind, reg.REG_MULTI_SZ)
        self.assertEqual(rules.data, [])

    def test_files_are_values_and_directories_are_keys(self):
        os.makedirs(os.path.join(self.root, "sway"))
        with open(os.path.join(self.root, "sway", "notes.txt"), "w") as handle:
            handle.write("hello")
        with open(os.path.join(self.root, "sway", "blob.bin"), "wb") as handle:
            handle.write(b"a\x00b")
        os.symlink("notes.txt", os.path.join(self.root, "sway", "link.txt"))
        sway = self.key(reg.FILES_PREFIX + r"\sway")
        self.assertEqual(sway.name, "sway")
        names = {item.name: item for item in sway.values}
        self.assertEqual(names["notes.txt"].kind, reg.REG_SZ)
        self.assertEqual(names["notes.txt"].data, "hello")
        self.assertEqual(names["blob.bin"].kind, reg.REG_BINARY)
        self.assertNotIn("link.txt", names)

    def test_write_reset_and_structure(self):
        shell = reg.MDE_PREFIX + r"\Shell"
        self.reg.set_value(shell, "accent", reg.REG_SZ, "#ff0000")
        self.assertEqual(self.value(shell, "accent").data, "#ff0000")
        self.reg.set_value(shell, "animations", reg.REG_DWORD, 0)
        self.assertEqual(self.value(shell, "animations").data, 0)
        self.reg.set_value(shell, "panel-height", reg.REG_DWORD, 40)
        self.assertEqual(self.value(shell, "panel-height").data, 40)
        with self.assertRaises(reg.RegistryError):
            self.reg.set_value(shell, "color-scheme", reg.REG_SZ, "blue")
        with self.assertRaises(reg.RegistryError):
            self.reg.set_value(shell, "panel-height", reg.REG_DWORD, 9999999999)
        self.reg.reset_value(shell, "accent")
        self.assertEqual(self.value(shell, "accent").data, "#0078d4")
        with self.assertRaises(reg.RegistryError):
            self.reg.new_key(shell, "Extra")
        with self.assertRaises(reg.RegistryError):
            self.reg.delete_key(shell)
        with self.assertRaises(reg.RegistryError):
            self.reg.rename(shell, "Other")
        created = self.reg.new_key(reg.FILES_PREFIX, "pads")
        self.reg.new_value(created, "readme.txt", reg.REG_SZ, "hi")
        self.assertEqual(self.value(created, "readme.txt").data, "hi")
        with self.assertRaises(reg.RegistryError):
            self.reg.new_value(created, "count", reg.REG_DWORD, 1)
        self.reg.rename(created, "notes.txt", "readme.txt")
        self.reg.rename(created, "folder")
        folder = reg.FILES_PREFIX + r"\folder"
        self.assertEqual(self.value(folder, "notes.txt").data, "hi")
        self.reg.delete_value(folder, "notes.txt")
        self.assertFalse(os.path.exists(os.path.join(self.root, "folder", "notes.txt")))
        self.reg.delete_key(folder)
        self.assertFalse(os.path.isdir(os.path.join(self.root, "folder")))

    def test_export_import_round_trip(self):
        shell = reg.MDE_PREFIX + r"\Shell"
        self.reg.set_value(shell, "accent", reg.REG_SZ, 'a"b\\c')
        self.reg.set_value(reg.MDE_PREFIX + r"\Window Layout", "custom-rules",
                           reg.REG_MULTI_SZ, ["one", "two"])
        os.makedirs(os.path.join(self.root, "sway"))
        blob = bytes(range(40))
        with open(os.path.join(self.root, "sway", "blob.bin"), "wb") as handle:
            handle.write(blob)
        text = self.reg.export_text(reg.MDE_PREFIX)
        self.assertTrue(text.startswith(reg.HEADER))
        self.assertIn('"accent"="a\\"b\\\\c"', text)
        self.assertIn("dword:00000001", text)
        self.assertIn("hex(7):", text)
        self.assertIn(",\\", text)
        path = os.path.join(self.tmp.name, "mde.reg")
        reg.write_reg_file(path, text)
        self.assertEqual(reg.read_reg_file(path).replace("\r\n", "\n"), text.replace("\r\n", "\n"))

        fresh = reg.Registry(os.path.join(self.tmp.name, "mde2"), self.schema, reg.DictSettings(), self.fav)
        result = fresh.import_text(text)
        self.assertGreater(result.written, 0)
        self.assertEqual(self._of(fresh, shell, "accent").data, 'a"b\\c')
        self.assertEqual(self._of(fresh, reg.MDE_PREFIX + r"\Window Layout", "custom-rules").data, ["one", "two"])
        self.assertEqual(self._of(fresh, reg.FILES_PREFIX + r"\sway", "blob.bin").data, blob)

    def _of(self, store, path, name):
        store.tree()
        for item in store.key_at(path).values:
            if item.name == name:
                return item
        self.fail(name)

    def test_import_merges_and_skips(self):
        text = "\n".join([
            reg.HEADER,
            "",
            r"[HKEY_CURRENT_USER\Software\MDE\Shell]",
            '"accent"="#112233"',
            '"color-scheme"="prefer-dark"',
            '"missing"="nope"',
            '"panel-height"="12"',
            "",
            r"[HKEY_LOCAL_MACHINE\Software\MDE\Shell]",
            '"accent"="#000000"',
            "",
            r"[HKEY_CURRENT_USER\Software\MDE\Files\pads]",
            '"note.txt"="hello"',
            '"count"=dword:00000002',
            "",
            r"[-HKEY_CURRENT_USER\Software\MDE\Nope]",
            "",
        ])
        result = self.reg.import_text(text)
        self.assertEqual(self.value(reg.MDE_PREFIX + r"\Shell", "accent").data, "#112233")
        self.assertEqual(self.value(reg.MDE_PREFIX + r"\Shell", "color-scheme").data, "prefer-dark")
        self.assertEqual(self.value(reg.MDE_PREFIX + r"\Shell", "panel-height").data, 48)
        self.assertEqual(self.value(reg.FILES_PREFIX + r"\pads", "note.txt").data, "hello")
        self.assertGreaterEqual(result.skipped, 3)
        with self.assertRaises(reg.RegistryError):
            self.reg.import_text("REGEDIT4\n")

    def test_find_and_favorites(self):
        os.makedirs(os.path.join(self.root, "sway"))
        with open(os.path.join(self.root, "sway", "theme.txt"), "w") as handle:
            handle.write("accent-line")
        first = self.reg.find("taskbar", start_path=reg.MDE_PREFIX, start_value=None)
        self.assertEqual(first.path, reg.MDE_PREFIX + r"\Taskbar")
        self.assertIsNone(first.value_name)
        data_hit = self.reg.find("accent-line", keys=False, values=False, data=True, start_path="", after=True)
        self.assertEqual(data_hit.value_name, "theme.txt")
        again = self.reg.find("taskbar", start_path=first.path, start_value=None, after=True)
        self.assertTrue(again.wrapped)
        self.reg.set_favorites([reg.MDE_PREFIX + r"\Start"])
        self.assertEqual(self.reg.favorites(), [reg.MDE_PREFIX + r"\Start"])

    def test_real_mde_schema(self):
        path = os.path.join(os.path.dirname(__file__), "..", "..", "data", "glib-2.0", "schemas", "org.mde.gschema.xml")
        areas = {area.name: area for area in reg.load_areas(os.path.abspath(path))}
        scheme = next(key for key in areas["Shell"].keys if key.name == "color-scheme")
        self.assertIn("prefer-dark", scheme.choices)
        rules = next(key for key in areas["Window Layout"].keys if key.name == "custom-rules")
        self.assertEqual(rules.gtype, "as")


if __name__ == "__main__":
    unittest.main()
