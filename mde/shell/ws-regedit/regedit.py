#!/usr/bin/env python3
# Registry Editor for MDE. Classic tree and Name / Type / Data list, editing
# org.mde GSettings and ~/.config/mde under HKEY_CURRENT_USER\Software\MDE.
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
for extra in (os.path.join(HERE, "..", "ws-settings"), "/usr/share/mde/ws-settings"):
    path = os.path.abspath(extra)
    if os.path.isdir(path) and path not in sys.path:
        sys.path.insert(0, path)

import gi  # noqa: E402

gi.require_version("Gdk", "3.0")
gi.require_version("Gio", "2.0")
gi.require_version("Gtk", "3.0")
from gi.repository import Gdk, Gio, GLib, Gtk, Pango  # noqa: E402

import wsui as ui  # noqa: E402
import registry as reg  # noqa: E402

CSS = """
#settings.regedit { background-color: #f3f3f3; }
#settings.regedit treeview, #settings-dialog treeview {
  font-family: "@FONT@"; font-size: @BODY@pt; color: #1a1a1a; background-color: #ffffff;
}
#settings.regedit treeview:selected, #settings-dialog treeview:selected {
  background-color: alpha(@ACCENT@, 0.16); color: #1a1a1a;
}
#settings.regedit treeview:selected:focus {
  background-color: alpha(@ACCENT@, 0.24);
}
#settings.regedit treeview header button {
  background-color: #ffffff; background-image: none; box-shadow: none; border: none;
  border-right: 1px solid alpha(#000000, 0.06); border-bottom: 1px solid alpha(#000000, 0.08);
  padding: 4px 8px; min-height: 0;
}
#settings.regedit treeview header button label { color: #5d5d5d; font-size: @CAPTION@pt; }
#settings.regedit menubar { background-color: #f3f3f3; padding: 2px 4px 0; box-shadow: none; border: none; }
#settings.regedit menubar > menuitem { padding: 4px 10px; border-radius: 4px; }
#settings.regedit menubar > menuitem:hover { background-color: alpha(#000000, 0.05); }
#settings.regedit menubar > menuitem label { color: #1a1a1a; }
#settings.regedit .address { padding: 0 8px 6px; }
#settings.regedit .address entry {
  min-height: 28px; background-color: #ffffff; background-image: none;
  border: 1px solid alpha(#000000, 0.08); border-bottom-color: alpha(#000000, 0.16);
  border-radius: 4px; box-shadow: none;
}
#settings.regedit .pane {
  background-color: #ffffff; border: 1px solid alpha(#000000, 0.08); border-radius: 6px;
}
#settings.regedit .statusbar { padding: 3px 12px 5px; }
#settings.regedit .statusbar label { font-size: @CAPTION@pt; color: #5d5d5d; }
#settings.regedit paned > separator { background: none; min-width: 6px; }
#settings-dialog textview, #settings-dialog textview text {
  font-family: "@FONT@"; font-size: @BODY@pt; color: #1a1a1a; background-color: #ffffff;
}
"""


def load_css():
    ui.load_css()
    text = (CSS.replace("@FONT@", "Selawik").replace("@BODY@", "10.5")
            .replace("@CAPTION@", "9").replace("@ACCENT@", "#0078d4"))
    provider = Gtk.CssProvider()
    provider.load_from_data(ui.ws_theme.transform(text, which="app_mode").encode())
    Gtk.StyleContext.add_provider_for_screen(
        Gdk.Screen.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 2)


def build_menu(rows, menu=None):
    menu = menu or Gtk.Menu()
    for child in menu.get_children():
        menu.remove(child)
    last_sep = True
    for row in rows:
        if row is None:
            if not last_sep:
                menu.append(Gtk.SeparatorMenuItem())
                last_sep = True
            continue
        label, callback, enabled, children = row
        item = Gtk.MenuItem.new_with_mnemonic(label)
        item.set_sensitive(enabled)
        if children:
            item.set_submenu(build_menu(children))
        elif callback:
            item.connect("activate", lambda _item, cb=callback: GLib.idle_add(_invoke, cb))
        menu.append(item)
        last_sep = False
    if menu.get_children() and isinstance(menu.get_children()[-1], Gtk.SeparatorMenuItem):
        menu.remove(menu.get_children()[-1])
    menu.show_all()
    return menu


def _invoke(callback):
    callback()
    return False


def popup(widget, event, rows):
    if not any(row for row in rows):
        return
    menu = build_menu(rows)
    menu.attach_to_widget(widget, None)
    menu.popup_at_pointer(event)


class Unset:
    name = "(Default)"
    kind = reg.REG_SZ
    unset = True
    choices = ()
    data = ""


UNSET = Unset()


def local_path(arg):
    if arg.startswith("file://"):
        from urllib.parse import unquote, urlparse
        return unquote(urlparse(arg).path)
    return arg


class Editor:
    def __init__(self, reg_file=None):
        self.reg_file = reg_file
        self.registry = reg.Registry()
        self.current = None
        self.suspend = 0
        self.reload_id = 0
        self.pending = False
        self.find_state = {"query": "", "keys": True, "values": True, "data": True}
        self.address_edit = False
        self.unwatch = lambda: None
        self.monitors = []
        load_css()
        self.build()
        self.reload(select=reg.MDE_PREFIX, first=True)
        self.unwatch = self.registry.watch(self.on_external)
        if reg_file:
            GLib.idle_add(self.offer_import, reg_file)

    def build(self):
        win = self.win = Gtk.Window(title="Registry Editor")
        win.set_name("settings")
        ui.css(win, "regedit")
        win.set_icon_name("preferences-system")
        win.set_default_size(980, 640)
        win.connect("delete-event", self.on_close)
        win.connect("key-press-event", self.on_key)
        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        win.add(root)

        menubar = Gtk.MenuBar()
        for title, actions in (("_File", self.file_actions), ("_Edit", self.edit_actions),
                               ("_View", self.view_actions), ("_Favorites", self.favorite_actions),
                               ("_Help", self.help_actions)):
            item = Gtk.MenuItem.new_with_mnemonic(title)
            menu = Gtk.Menu()
            item.set_submenu(menu)
            item.connect("activate", lambda _item, menu=menu, actions=actions: build_menu(actions(), menu))
            menubar.append(item)
        root.pack_start(menubar, False, False, 0)

        address_box = ui.css(Gtk.Box(), "address")
        self.address = Gtk.Entry(placeholder_text="Key path")
        self.address.connect("activate", self.on_address)
        self.address.connect("focus-in-event", lambda *_a: self._address(True))
        self.address.connect("focus-out-event", lambda *_a: self._address(False))
        address_box.pack_start(self.address, True, True, 0)
        root.pack_start(address_box, False, False, 0)

        self.paned = Gtk.Paned()
        self.paned.set_margin_start(8)
        self.paned.set_margin_end(8)
        self.paned.set_margin_bottom(8)
        root.pack_start(self.paned, True, True, 0)

        self.store = Gtk.TreeStore(str, str, object)
        self.tree = Gtk.TreeView(model=self.store, headers_visible=False, enable_search=False)
        column = Gtk.TreeViewColumn()
        icon = Gtk.CellRendererPixbuf(stock_size=Gtk.IconSize.MENU)
        text = Gtk.CellRendererText(ellipsize=Pango.EllipsizeMode.END)
        column.pack_start(icon, False)
        column.pack_start(text, True)
        column.add_attribute(icon, "icon-name", 0)
        column.add_attribute(text, "text", 1)
        self.tree.append_column(column)
        self.tree.get_selection().connect("changed", self.on_tree)
        self.tree.connect("button-press-event", self.on_tree_button)
        tree_sw = Gtk.ScrolledWindow()
        tree_sw.add(self.tree)
        tree_frame = ui.css(Gtk.Frame(shadow_type=Gtk.ShadowType.NONE), "pane")
        tree_frame.add(tree_sw)
        tree_frame.set_size_request(260, -1)
        self.paned.pack1(tree_frame, False, False)

        self.values = Gtk.ListStore(str, str, str, object)
        self.list = Gtk.TreeView(model=self.values, enable_search=False)
        self.list.get_selection().connect("changed", lambda *_a: None)
        self.list.connect("row-activated", lambda *_a: self.modify())
        self.list.connect("button-press-event", self.on_list_button)
        for title, col in (("Name", 0), ("Type", 1), ("Data", 2)):
            cell = Gtk.CellRendererText(ellipsize=Pango.EllipsizeMode.END)
            column = Gtk.TreeViewColumn(title, cell)
            column.set_resizable(True)
            column.set_expand(col == 2)
            column.set_cell_data_func(cell, self._paint_cell, col)
            self.list.append_column(column)
        self.list.get_column(0).set_min_width(180)
        self.list.get_column(1).set_min_width(120)
        list_sw = Gtk.ScrolledWindow()
        list_sw.add(self.list)
        list_frame = ui.css(Gtk.Frame(shadow_type=Gtk.ShadowType.NONE), "pane")
        list_frame.add(list_sw)
        self.paned.pack2(list_frame, True, False)

        status = ui.css(Gtk.Box(), "statusbar")
        self.status = ui.label("")
        status.pack_start(self.status, True, True, 0)
        root.pack_start(status, False, False, 0)
        win.show_all()
        GLib.idle_add(self._place)

    def _place(self):
        self.paned.set_position(280)
        return False

    def _address(self, editing):
        self.address_edit = editing
        if not editing and self.current is not None:
            self.address.set_text(self.current.path or "Computer")
        return False

    def _paint_cell(self, _column, cell, model, iterator, col):
        value = model[iterator][3]
        italic = Pango.Style.ITALIC if getattr(value, "unset", False) else Pango.Style.NORMAL
        cell.set_property("style", italic)
        cell.set_property("text", model[iterator][col])

    def on_close(self, *_args):
        self.unwatch()
        for monitor in self.monitors:
            monitor.cancel()
        Gtk.main_quit()

    def on_key(self, _win, event):
        focus = self.win.get_focus()
        typing = isinstance(focus, (Gtk.Entry, Gtk.TextView))
        if event.keyval == Gdk.KEY_F5:
            self.reload()
            return True
        if event.keyval == Gdk.KEY_F3:
            self.find_next()
            return True
        if event.state & Gdk.ModifierType.CONTROL_MASK and event.keyval in (Gdk.KEY_f, Gdk.KEY_F):
            self.find_dialog()
            return True
        if typing:
            return False
        if event.keyval in (Gdk.KEY_Return, Gdk.KEY_KP_Enter):
            self.modify()
            return True
        if event.keyval == Gdk.KEY_Delete:
            self.delete()
            return True
        if event.keyval == Gdk.KEY_F2:
            self.rename()
            return True
        return False

    def on_external(self, *_args):
        if self.suspend:
            self.pending = True
            return
        if self.reload_id:
            return
        self.reload_id = GLib.timeout_add(200, self._reload_idle)

    def _reload_idle(self):
        self.reload_id = 0
        if not self.suspend:
            self.reload()
        return False

    def guard(self):
        editor = self

        class Guard:
            def __enter__(self):
                editor.suspend += 1

            def __exit__(self, *_exc):
                editor.suspend -= 1
                if editor.pending and editor.suspend == 0:
                    editor.pending = False
                    editor.reload()
        return Guard()

    def reload(self, select=None, value=None, first=False):
        selected = select if select is not None else (self.current.path if self.current else reg.MDE_PREFIX)
        chosen = value if value is not None else self.selected_name()
        expanded = [] if first else self.expanded_paths()
        for monitor in self.monitors:
            monitor.cancel()
        self.monitors.clear()
        computer = self.registry.tree()
        self.store.clear()
        self.add_key(None, computer)
        if first:
            expanded = ["", "HKEY_CURRENT_USER", r"HKEY_CURRENT_USER\Software", reg.MDE_PREFIX]
        for path in expanded:
            iterator = self.iter_for(path)
            if iterator is not None:
                self.tree.expand_row(self.store.get_path(iterator), False)
        self.arm_monitors(computer)
        iterator = self.iter_for(selected) or self.iter_for(reg.MDE_PREFIX)
        if iterator is not None:
            self.tree.expand_to_path(self.store.get_path(iterator))
            self.tree.get_selection().select_iter(iterator)
            self.show_key(self.store[iterator][2], chosen)

    def add_key(self, parent, key):
        icon = "computer" if key.kind in ("computer", "hive") else "folder"
        iterator = self.store.append(parent, [icon, key.name, key])
        for child in key.children:
            self.add_key(iterator, child)

    def expanded_paths(self):
        paths = []

        def collect(_tree, path):
            iterator = self.store.get_iter(path)
            if iterator is not None:
                paths.append(self.store[iterator][2].path)
        self.tree.map_expanded_rows(collect)
        return paths

    def iter_for(self, path):
        found = []
        want = (path or "").lower()

        def walk(model, _path, iterator):
            if model[iterator][2].path.lower() == want:
                found.append(iterator.copy())
                return True
            return False
        self.store.foreach(walk)
        return found[0] if found else None

    def arm_monitors(self, key):
        if key.dirpath and os.path.isdir(key.dirpath):
            monitor = Gio.File.new_for_path(key.dirpath).monitor_directory(Gio.FileMonitorFlags.NONE, None)
            monitor.connect("changed", lambda *_a: self.on_external())
            self.monitors.append(monitor)
        for child in key.children:
            self.arm_monitors(child)

    def on_tree(self, selection):
        model, iterator = selection.get_selected()
        if iterator is None:
            return
        self.show_key(model[iterator][2], None)

    def show_key(self, key, value_name):
        self.current = key
        shown = key.path or "Computer"
        if not self.address_edit:
            self.address.set_text(shown)
        self.status.set_text(shown)
        self.values.clear()
        self.values.append(["(Default)", reg.REG_SZ, "(value not set)", UNSET])
        for value in key.values:
            self.values.append([value.name, value.kind, reg.format_data(value), value])
        if value_name:
            for row in self.values:
                if row[0] == value_name:
                    self.list.get_selection().select_iter(row.iter)
                    break

    def selected_value(self):
        model, iterator = self.list.get_selection().get_selected()
        if iterator is None:
            return None
        return model[iterator][3]

    def selected_name(self):
        value = self.selected_value()
        if value is None or getattr(value, "unset", False):
            return None
        return value.name

    def on_address(self, entry):
        path = entry.get_text().strip()
        key = self.registry.key_at(path)
        if key is None:
            ui.error(self.win, "Registry Editor", f"Cannot find the key '{path}'.")
            if self.current is not None:
                entry.set_text(self.current.path or "Computer")
            return
        iterator = self.iter_for(key.path)
        if iterator is not None:
            self.tree.expand_to_path(self.store.get_path(iterator))
            self.tree.get_selection().select_iter(iterator)
            self.tree.scroll_to_cell(self.store.get_path(iterator), None, True, 0.3, 0)

    def file_actions(self):
        return [("_Import…", self.import_dialog, True, None),
                ("_Export…", self.export_dialog, self.current is not None and bool(self.current.path), None),
                None,
                ("E_xit", self.on_close, True, None)]

    def edit_actions(self):
        key = self.current
        structural = bool(key and key.structural)
        value = self.selected_value()
        can_value = value is not None and not getattr(value, "unset", False)
        can_rename = (can_value and not value.schema_backed) or (
            not can_value and key is not None and key.kind == "directory" and key.path.lower() != reg.FILES_PREFIX.lower())
        can_delete = can_value or can_rename
        return [("_Modify", self.modify, can_value, None),
                ("_New", None, structural, [
                    ("_Key", self.new_key, True, None),
                    ("_String Value", lambda: self.new_value(reg.REG_SZ), True, None),
                    ("_Binary Value", lambda: self.new_value(reg.REG_BINARY), True, None),
                ]),
                None,
                ("_Delete", self.delete, can_delete, None),
                ("_Rename", self.rename, can_rename, None),
                ("Copy Key _Name", self.copy_name, key is not None, None),
                None,
                ("_Find…", self.find_dialog, True, None),
                ("Find _Next", self.find_next, bool(self.find_state["query"]), None)]

    def view_actions(self):
        return [("_Refresh", self.reload, True, None)]

    def favorite_actions(self):
        path = self.current.path if self.current else ""
        saved = self.registry.favorites()
        rows = [("_Add to Favorites", self.add_favorite, bool(path), None)]
        if path and path in saved:
            rows.append(("_Remove Favorite", self.remove_favorite, True, None))
        if saved:
            rows.append(None)
        for favorite in saved:
            rows.append((favorite.replace("_", "__"), lambda p=favorite: self.goto_favorite(p), True, None))
        return rows

    def help_actions(self):
        return [("_About Registry Editor", self.about, True, None)]

    def on_tree_button(self, tree, event):
        if event.button != 3:
            return False
        hit = tree.get_path_at_pos(int(event.x), int(event.y))
        if hit:
            tree.get_selection().select_path(hit[0])
            popup(tree, event, self.edit_actions())
        return True

    def on_list_button(self, view, event):
        if event.button != 3:
            return False
        hit = view.get_path_at_pos(int(event.x), int(event.y))
        if hit:
            view.get_selection().select_path(hit[0])
        popup(view, event, self.edit_actions())
        return True

    def modify(self):
        value = self.selected_value()
        if value is None:
            return
        if getattr(value, "unset", False):
            ui.error(self.win, "Registry Editor", "This key has no default value.")
            return
        if value.kind == reg.REG_DWORD:
            self.edit_dword(value)
        elif value.kind == reg.REG_BINARY:
            self.edit_binary(value)
        elif value.kind == reg.REG_MULTI_SZ:
            self.edit_multi(value)
        else:
            self.edit_string(value)

    def edit_string(self, value, create=None):
        dialog = ui.Dialog(self.win, "Edit String", "OK", width=460)
        if value.choices:
            combo = Gtk.ComboBoxText()
            for choice in value.choices:
                combo.append_text(choice)
            combo.set_active(value.choices.index(value.data) if value.data in value.choices else 0)
            dialog.add_field("Value data:", combo)

            def current():
                return combo.get_active_text() or ""
        else:
            entry = Gtk.Entry()
            entry.set_text("" if value.data is None else str(value.data))
            dialog.add_field("Value data:", entry)

            def current():
                return entry.get_text()

            def apply():
                self._commit(value, value.kind, current(), dialog, create)
            entry.connect("activate", lambda *_a: apply())
            dialog.on_primary(apply)
            dialog.run_dialog()
            return

        def apply():
            self._commit(value, value.kind, current(), dialog, create)
        dialog.on_primary(apply)
        dialog.run_dialog()

    def edit_dword(self, value):
        dialog = ui.Dialog(self.win, "Edit DWORD (32-bit) Value", "OK", width=420)
        entry = Gtk.Entry()
        entry.set_text(f"{int(value.data) & 0xFFFFFFFF:x}")
        dialog.add_field("Value data:", entry)
        base = {"hex": True}
        hex_radio = Gtk.RadioButton.new_with_label(None, "Hexadecimal")
        dec_radio = Gtk.RadioButton.new_with_label_from_widget(hex_radio, "Decimal")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        box.pack_start(hex_radio, False, False, 0)
        box.pack_start(dec_radio, False, False, 0)
        dialog.add_field("Base:", box)

        def on_toggle(button):
            if not button.get_active():
                return
            want_hex = button is hex_radio
            if want_hex == base["hex"]:
                return
            try:
                number = int(entry.get_text().strip() or "0", 16 if base["hex"] else 10)
            except ValueError:
                base["hex"] = want_hex
                return
            if number < 0 or number > 0xFFFFFFFF:
                base["hex"] = want_hex
                return
            base["hex"] = want_hex
            entry.set_text(f"{number:x}" if want_hex else str(number))
        hex_radio.connect("toggled", on_toggle)
        dec_radio.connect("toggled", on_toggle)

        def apply():
            try:
                number = int(entry.get_text().strip(), 16 if base["hex"] else 10)
            except ValueError:
                ui.error(self.win, "Registry Editor", "Enter a number.")
                return
            if number < 0 or number > 0xFFFFFFFF:
                ui.error(self.win, "Registry Editor", "Enter a number from 0 to 4294967295.")
                return
            self._commit(value, reg.REG_DWORD, number, dialog, None)
        entry.connect("activate", lambda *_a: apply())
        dialog.on_primary(apply)
        dialog.run_dialog()

    def edit_binary(self, value, create=None):
        dialog = ui.Dialog(self.win, "Edit Binary Value", "OK", width=560)
        dialog.set_resizable(True)
        view = Gtk.TextView(monospace=True, wrap_mode=Gtk.WrapMode.WORD_CHAR)
        view.get_buffer().set_text(self._hex_text(value.data or b""))
        scroll = Gtk.ScrolledWindow(min_content_height=220)
        scroll.add(view)
        dialog.add_field("Value data:", scroll)

        def apply():
            text = view.get_buffer().get_text(view.get_buffer().get_start_iter(), view.get_buffer().get_end_iter(), False)
            try:
                data = self._parse_hex(text)
            except reg.RegistryError as exc:
                ui.error(self.win, "Registry Editor", str(exc))
                return
            self._commit(value, reg.REG_BINARY, data, dialog, create)
        dialog.on_primary(apply)
        dialog.run_dialog()

    def edit_multi(self, value):
        dialog = ui.Dialog(self.win, "Edit Multi-String", "OK", width=520)
        dialog.set_resizable(True)
        view = Gtk.TextView(wrap_mode=Gtk.WrapMode.WORD_CHAR)
        view.get_buffer().set_text("\n".join(value.data or []))
        scroll = Gtk.ScrolledWindow(min_content_height=200)
        scroll.add(view)
        dialog.add_field("Value data (one string on each line):", scroll)

        def apply():
            text = view.get_buffer().get_text(view.get_buffer().get_start_iter(), view.get_buffer().get_end_iter(), False)
            lines = text.splitlines()
            self._commit(value, reg.REG_MULTI_SZ, lines, dialog, None)
        dialog.on_primary(apply)
        dialog.run_dialog()

    def _commit(self, value, kind, data, dialog, create):
        path = self.current.path
        name = create or value.name
        try:
            with self.guard():
                if create:
                    self.registry.new_value(path, name, kind, data)
                else:
                    self.registry.set_value(path, name, kind, data)
        except reg.RegistryError as exc:
            ui.error(self.win, "Registry Editor", str(exc))
            return
        dialog.destroy()
        self.reload(select=path, value=name)

    def _hex_text(self, data):
        rows = []
        for index in range(0, len(data), 16):
            rows.append(" ".join(f"{byte:02x}" for byte in data[index:index + 16]))
        return "\n".join(rows)

    def _parse_hex(self, text):
        tokens = text.replace(",", " ").split()
        if any(len(token) == 0 or len(token) > 2 or any(ch not in "0123456789abcdefABCDEF" for ch in token) for token in tokens):
            raise reg.RegistryError("Enter hex bytes, such as 0a ff.")
        return bytes(int(token, 16) for token in tokens)

    def new_key(self):
        if not self.current or not self.current.structural:
            return
        name = self._unique("New Key", [child.name for child in self.current.children])
        dialog = ui.Dialog(self.win, "New Key", "OK")
        entry = Gtk.Entry()
        entry.set_text(name)
        entry.connect("activate", lambda *_a: apply())
        dialog.add_field("Key name:", entry)

        def apply():
            try:
                with self.guard():
                    created = self.registry.new_key(self.current.path, entry.get_text().strip())
            except reg.RegistryError as exc:
                ui.error(self.win, "Registry Editor", str(exc))
                return
            dialog.destroy()
            self.reload(select=created)
        dialog.on_primary(apply)
        dialog.run_dialog()

    def new_value(self, kind):
        if not self.current or not self.current.structural:
            return
        name = self._unique("New Value", [item.name for item in self.current.values])
        dialog = ui.Dialog(self.win, "New Value", "OK")
        entry = Gtk.Entry()
        entry.set_text(name)
        dialog.add_field("Value name:", entry)

        def apply():
            chosen = entry.get_text().strip()
            try:
                reg._safe_name(chosen)
            except reg.RegistryError as exc:
                ui.error(self.win, "Registry Editor", str(exc))
                return
            if any(item.name == chosen for item in self.current.values):
                ui.error(self.win, "Registry Editor", f"'{chosen}' already exists.")
                return
            dialog.destroy()
            blank = reg.Value(chosen, kind, b"" if kind == reg.REG_BINARY else "")
            if kind == reg.REG_BINARY:
                self.edit_binary(blank, create=chosen)
            else:
                self.edit_string(blank, create=chosen)
        entry.connect("activate", lambda *_a: apply())
        dialog.on_primary(apply)
        dialog.run_dialog()

    def _unique(self, stem, existing):
        if stem not in existing:
            return stem
        number = 1
        while f"{stem} #{number}" in existing:
            number += 1
        return f"{stem} #{number}"

    def delete(self):
        value = self.selected_value()
        key = self.current
        if value is not None and not getattr(value, "unset", False):
            if value.schema_backed:
                title, text, action = ("Confirm Reset", f'Reset the value "{value.name}" to its default?', "Reset")
                def go():
                    with self.guard():
                        self.registry.reset_value(key.path, value.name)
                    self.reload(select=key.path, value=value.name)
            else:
                title, text, action = ("Confirm Delete", f'Delete the value "{value.name}"?', "Delete")
                def go():
                    with self.guard():
                        self.registry.delete_value(key.path, value.name)
                    self.reload(select=key.path)
            ui.confirm(self.win, title, text, action, self._safe(go))
            return
        if key is None or key.kind != "directory" or key.path.lower() == reg.FILES_PREFIX.lower():
            return
        ui.confirm(self.win, "Confirm Delete",
                   f'Delete the key "{key.path}" and everything under it?', "Delete",
                   self._safe(lambda: self._delete_key(key)))

    def _delete_key(self, key):
        parent = key.path.rsplit("\\", 1)[0]
        with self.guard():
            self.registry.delete_key(key.path)
        self.reload(select=parent)

    def _safe(self, callback):
        def run():
            try:
                callback()
            except reg.RegistryError as exc:
                ui.error(self.win, "Registry Editor", str(exc))
        return run

    def rename(self):
        value = self.selected_value()
        key = self.current
        if value is not None and not getattr(value, "unset", False):
            if value.schema_backed:
                return
            self._rename_dialog(value.name, lambda new: self.registry.rename(key.path, new, value.name), key.path, new_value=True)
            return
        if key is None or key.kind != "directory" or key.path.lower() == reg.FILES_PREFIX.lower():
            return
        parent = key.path.rsplit("\\", 1)[0]
        self._rename_dialog(key.name, lambda new: self.registry.rename(key.path, new), parent + "\\" )

    def _rename_dialog(self, current, action, select_path, new_value=False):
        dialog = ui.Dialog(self.win, "Rename", "OK")
        entry = Gtk.Entry()
        entry.set_text(current)
        dialog.add_field("New name:", entry)

        def apply():
            new = entry.get_text().strip()
            try:
                with self.guard():
                    action(new)
            except reg.RegistryError as exc:
                ui.error(self.win, "Registry Editor", str(exc))
                return
            dialog.destroy()
            if new_value:
                self.reload(select=select_path, value=new)
            else:
                self.reload(select=select_path + new)
        entry.connect("activate", lambda *_a: apply())
        dialog.on_primary(apply)
        dialog.run_dialog()

    def copy_name(self):
        if self.current is None:
            return
        text = self.current.path or "Computer"
        Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).set_text(text, -1)
        self.status.set_text(text)

    def find_dialog(self):
        dialog = ui.Dialog(self.win, "Find", "Find Next", width=420)
        entry = Gtk.Entry()
        entry.set_text(self.find_state["query"])
        dialog.add_field("Find what:", entry)
        keys = Gtk.CheckButton.new_with_label("Keys")
        values = Gtk.CheckButton.new_with_label("Values")
        data = Gtk.CheckButton.new_with_label("Data")
        keys.set_active(self.find_state["keys"])
        values.set_active(self.find_state["values"])
        data.set_active(self.find_state["data"])
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        for check in (keys, values, data):
            box.pack_start(check, False, False, 0)
        dialog.add_field("Look at:", box)

        def apply():
            self.find_state = {"query": entry.get_text(), "keys": keys.get_active(),
                               "values": values.get_active(), "data": data.get_active()}
            self.find_next()
        entry.connect("activate", lambda *_a: apply())
        dialog.on_primary(apply)
        dialog.run_dialog()

    def find_next(self):
        query = self.find_state["query"]
        if not query:
            self.find_dialog()
            return
        start = self.current.path if self.current else ""
        match = self.registry.find(
            query, keys=self.find_state["keys"], values=self.find_state["values"],
            data=self.find_state["data"], start_path=start, start_value=self.selected_name(), after=True)
        if match is None:
            ui.error(self.win, "Registry Editor", f'Finished searching the registry. Cannot find "{query}".')
            return
        iterator = self.iter_for(match.path)
        if iterator is not None:
            self.tree.expand_to_path(self.store.get_path(iterator))
            self.tree.get_selection().select_iter(iterator)
            self.show_key(self.store[iterator][2], match.value_name)
        if match.wrapped:
            ui.error(self.win, "Registry Editor", "Finished searching the registry.")

    def add_favorite(self):
        if not self.current or not self.current.path:
            return
        saved = self.registry.favorites()
        if self.current.path not in saved:
            saved.append(self.current.path)
            self.registry.set_favorites(saved)

    def remove_favorite(self):
        if not self.current:
            return
        self.registry.set_favorites([item for item in self.registry.favorites() if item != self.current.path])

    def goto_favorite(self, path):
        if self.registry.key_at(path) is None:
            self.registry.set_favorites([item for item in self.registry.favorites() if item != path])
            ui.error(self.win, "Registry Editor", f"Cannot find the key '{path}'.")
            return
        self.reload(select=path)

    def import_dialog(self):
        chooser = Gtk.FileChooserDialog(title="Import Registry File", parent=self.win,
                                         action=Gtk.FileChooserAction.OPEN)
        chooser.add_button("Cancel", Gtk.ResponseType.CANCEL)
        chooser.add_button("Open", Gtk.ResponseType.ACCEPT)
        self._reg_filter(chooser)
        if chooser.run() == Gtk.ResponseType.ACCEPT:
            path = chooser.get_filename()
            chooser.destroy()
            self.offer_import(path)
        else:
            chooser.destroy()

    def offer_import(self, path):
        path = local_path(path)
        if not os.path.isfile(path):
            ui.error(self.win, "Registry Editor", f"Cannot open '{path}'.")
            return False
        ui.confirm(
            self.win, "Registry Editor",
            f"Adding information can change or reset MDE settings. Are you sure you want to add the information in {os.path.basename(path)} to the registry?",
            "Yes", self._safe(lambda: self._import(path)))
        return False

    def _import(self, path):
        with self.guard():
            result = self.registry.import_text(reg.read_reg_file(path))
        self.reload()
        message = f"The keys and values in {os.path.basename(path)} have been added to the registry."
        if result.skipped:
            message += f" Skipped {result.skipped} entries that Registry Editor cannot store."
        ui.error(self.win, "Registry Editor", message)

    def export_dialog(self):
        if self.current is None or not self.current.path:
            return
        chooser = Gtk.FileChooserDialog(title="Export Registry File", parent=self.win,
                                         action=Gtk.FileChooserAction.SAVE)
        chooser.add_button("Cancel", Gtk.ResponseType.CANCEL)
        chooser.add_button("Save", Gtk.ResponseType.ACCEPT)
        chooser.set_current_name(self.current.name + ".reg")
        chooser.set_do_overwrite_confirmation(True)
        self._reg_filter(chooser)
        if chooser.run() == Gtk.ResponseType.ACCEPT:
            path = chooser.get_filename()
            if not path.lower().endswith(".reg"):
                path += ".reg"
            try:
                reg.write_reg_file(path, self.registry.export_text(self.current.path))
            except OSError as exc:
                ui.error(self.win, "Registry Editor", str(exc))
        chooser.destroy()

    def _reg_filter(self, chooser):
        pattern = Gtk.FileFilter()
        pattern.set_name("Registration Files (*.reg)")
        pattern.add_pattern("*.reg")
        chooser.add_filter(pattern)

    def about(self):
        dialog = ui.Dialog(self.win, "Registry Editor", None, "Close", width=460)
        dialog.add_text("Edits MDE settings under HKEY_CURRENT_USER\\Software\\MDE.")
        dialog.add_text("Shell, Taskbar, Start and Window Layout are MDE settings. Files is ~/.config/mde.")
        dialog.run_dialog()

    def run(self):
        Gtk.main()


def main(argv):
    args = [arg for arg in argv[1:] if arg]
    if args and args[0] in ("-h", "--help"):
        print("Usage: regedit [file.reg]")
        return 0
    reg_file = local_path(args[0]) if args else None
    GLib.set_prgname("regedit")
    try:
        Editor(reg_file).run()
    except reg.RegistryError as exc:
        print(exc, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
