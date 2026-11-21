# Shared pieces for ws-mmc: console nodes and actions, the results-pane list and
# tree views, property sheets, sizes, and the root helper that polkit guards.
# Styles build on ws-settings (wsui): the console window is named "settings" so
# buttons, entries and dialogs match the Settings app.
import os
import sys

sys.path.append("/usr/share/mde/ws-settings")

import gi  # noqa: E402

gi.require_version("Gdk", "3.0")
gi.require_version("Gtk", "3.0")
from gi.repository import Gdk, GLib, GObject, Gtk, Pango  # noqa: E402

import wsui  # noqa: E402,F401
from wsui import (Dialog, call, clear, confirm, css, error, json_file, label, out, prop,  # noqa: E402,F401
                  proxy, read_file, run, spawn, write_file)

HELPER = "/usr/libexec/ws-mmc-helper"
HOME = os.path.expanduser("~")

CSS = """
#settings.mmc { background-color: #f3f3f3; }
#settings.mmc treeview, #settings-dialog treeview {
  font-family: "@FONT@"; font-size: @BODY@pt; color: #1a1a1a; background-color: #ffffff;
}
#settings.mmc treeview:selected, #settings-dialog treeview:selected {
  background-color: alpha(@ACCENT@, 0.16); color: #1a1a1a;
}
#settings.mmc treeview:selected:focus, #settings-dialog treeview:selected:focus {
  background-color: alpha(@ACCENT@, 0.24);
}
#settings.mmc treeview.dim, #settings-dialog treeview.dim { color: #8a8a8a; }
#settings.mmc treeview header button, #settings-dialog treeview header button {
  background-color: #ffffff; background-image: none; box-shadow: none; border: none;
  border-right: 1px solid alpha(#000000, 0.06); border-bottom: 1px solid alpha(#000000, 0.08);
  padding: 4px 8px; min-height: 0;
}
#settings.mmc treeview header button label, #settings-dialog treeview header button label {
  color: #5d5d5d; font-size: @CAPTION@pt;
}
#settings.mmc treeview header button:hover { background-color: #f6f6f6; }
#settings.mmc menubar { background-color: #f3f3f3; padding: 2px 4px 0; box-shadow: none; border: none; }
#settings.mmc menubar > menuitem { padding: 4px 10px; border-radius: 4px; }
#settings.mmc menubar > menuitem:hover { background-color: alpha(#000000, 0.05); }
#settings.mmc menubar > menuitem label { color: #1a1a1a; }
#settings.mmc .toolbar { padding: 2px 6px 4px; }
#settings.mmc .toolbar button {
  min-width: 32px; min-height: 30px; padding: 0; border-radius: 4px;
  background: none; border: none; box-shadow: none; transition: background-color 83ms linear;
}
#settings.mmc .toolbar button:hover { background-color: alpha(#000000, 0.05); }
#settings.mmc .toolbar button:active { background-color: alpha(#000000, 0.03); }
#settings.mmc .toolbar separator { margin: 6px 4px; min-width: 1px; background-color: alpha(#000000, 0.1); }
#settings.mmc .pane {
  background-color: #ffffff; border: 1px solid alpha(#000000, 0.08); border-radius: 6px;
}
#settings.mmc .pane-plain { background-color: #ffffff; }
#settings.mmc .actions { background-color: #fbfbfb; border-radius: 6px; }
#settings.mmc .actions-head {
  background-color: alpha(#000000, 0.045); padding: 6px 10px; margin-bottom: 2px;
}
#settings.mmc .actions-head label { font-weight: 600; }
#settings.mmc .actions button {
  background: none; border: none; box-shadow: none; border-radius: 4px;
  padding: 2px 10px 2px 14px; min-height: 26px; transition: background-color 83ms linear;
}
#settings.mmc .actions button:hover { background-color: alpha(#000000, 0.05); }
#settings.mmc .actions button:disabled label { color: alpha(#000000, 0.36); }
#settings.mmc .statusbar { padding: 3px 12px 5px; }
#settings.mmc .statusbar label { font-size: @CAPTION@pt; color: #5d5d5d; }
#settings.mmc paned > separator { background: none; min-width: 6px; min-height: 6px; }
#settings.mmc .empty { color: #5d5d5d; padding: 40px; }
#settings.mmc .banner { padding: 16px 20px 8px; }
#settings.mmc .banner-title { font-size: 15pt; font-weight: 600; }
#settings.mmc .summary-section { font-weight: 600; margin: 16px 20px 6px; }
#settings.mmc .preview { border-top: 1px solid alpha(#000000, 0.08); }
#settings.mmc .preview-head { padding: 6px 12px; background-color: alpha(#000000, 0.03); }
#settings.mmc .preview-head label { font-weight: 600; }
#settings.mmc textview, #settings-dialog textview, #settings.mmc textview text, #settings-dialog textview text {
  font-family: "@FONT@"; font-size: @BODY@pt; color: #1a1a1a; background-color: #ffffff;
}
#settings.mmc .extended { padding: 16px; border-right: 1px solid alpha(#000000, 0.06); }
#settings.mmc .extended-title { font-size: 13pt; font-weight: 600; }
#settings.mmc button.link, #settings-dialog button.link { padding: 0; min-height: 22px; }
#settings-dialog notebook { background: none; }
#settings-dialog notebook header { background: none; border: none; box-shadow: none; }
#settings-dialog notebook header tabs { margin: 0 16px; }
#settings-dialog notebook tab {
  padding: 6px 12px; border: none; border-bottom: 2px solid transparent; background: none; min-height: 0;
}
#settings-dialog notebook tab label { color: #5d5d5d; }
#settings-dialog notebook tab:checked { border-bottom-color: @ACCENT@; }
#settings-dialog notebook tab:checked label { color: #1a1a1a; font-weight: 600; }
#settings-dialog notebook stack { background-color: #fbfbfb; }
#settings-dialog .sheet-page { padding: 20px; }
#settings-dialog .sheet-title { font-size: 12pt; font-weight: 600; }
#settings-dialog .status-box {
  background-color: #ffffff; border: 1px solid alpha(#000000, 0.1); border-radius: 4px; padding: 10px;
}
#settings-dialog .key { color: #5d5d5d; }
#settings-dialog .listframe { border: 1px solid alpha(#000000, 0.1); border-radius: 4px; }
#settings-dialog check, #settings-dialog radio { border-color: alpha(#000000, 0.45); background-image: none; }
#settings-dialog check:checked, #settings-dialog radio:checked {
  background-color: @ACCENT@; border-color: @ACCENT@; color: #ffffff;
}
#settings-dialog combobox button {
  min-height: 30px; padding: 0 12px; background-color: #ffffff; background-image: none;
  border: 1px solid alpha(#000000, 0.06); border-bottom-color: alpha(#000000, 0.16); border-radius: 4px;
  box-shadow: none;
}
"""

_provider = None


def load_css():
    wsui.load_css()
    global _provider
    screen = Gdk.Screen.get_default()
    if _provider:
        Gtk.StyleContext.remove_provider_for_screen(screen, _provider)
    _provider = Gtk.CssProvider()
    _provider.load_from_data(wsui.ws_theme.transform(CSS, which="app_mode").encode())
    Gtk.StyleContext.add_provider_for_screen(screen, _provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 2)


# --- Console model -------------------------------------------------------------------

class Action:
    """A command for the Actions pane, the Action menu, context menus and the toolbar.
    children makes a submenu; checked (True/False) makes a check item."""

    def __init__(self, label, callback=None, icon=None, enabled=True, checked=None, children=None,
                 tip=None):
        self.label, self.callback, self.icon, self.enabled = label, callback, icon, enabled
        self.checked, self.children, self.tip = checked, children, tip


class Node:
    """A console tree item. view(console) builds its results pane; without one, the
    results pane lists the children."""

    def __init__(self, title, icon, view=None, children=(), description="", key=None):
        self.title, self.icon, self.view, self.description = title, icon, view, description
        self.children = list(children)
        self.key = key or title


class View:
    """The results pane of a node. Subclasses set self.widget and override what they need."""

    def __init__(self, console, node):
        self.console, self.node, self.widget = console, node, None

    def actions(self):
        """Actions for the node (top section of the Actions pane)."""
        return []

    def view_actions(self):
        """Extra entries for the View menu."""
        return []

    def toolbar(self):
        """Extra toolbar buttons (Actions with icons)."""
        return []

    def selected_title(self):
        return None

    def item_actions(self):
        """Actions for the selected item (second Actions section and the context menu)."""
        return []

    def refresh(self):
        pass

    def properties(self):
        """Alt+Enter and the toolbar Properties button."""
        for a in self.item_actions():
            if a and a.label == "Properties" and a.enabled and a.callback:
                a.callback()
                return

    def delete(self):
        for a in self.item_actions():
            if a and a.label in ("Delete", "Uninstall device") and a.enabled and a.callback:
                a.callback()
                return

    def shown(self):
        pass

    def hidden(self):
        pass

    def changed(self):
        self.console.update_actions(self)

    def status(self, text):
        self.console.set_status(self, text)


def build_menu(actions, menu=None):
    menu = menu or Gtk.Menu()
    for child in menu.get_children():
        menu.remove(child)
    last_sep = True
    for a in actions:
        if a is None:
            if not last_sep:
                menu.append(Gtk.SeparatorMenuItem())
                last_sep = True
            continue
        last_sep = False
        if a.checked is not None:
            item = Gtk.CheckMenuItem(label=a.label, active=a.checked)
        else:
            item = Gtk.MenuItem(label=a.label)
        item.set_sensitive(a.enabled)
        if a.children:
            item.set_submenu(build_menu(a.children))
        elif a.callback:
            item.connect("activate", lambda _i, cb=a.callback: GLib.idle_add(lambda: cb() and False))
        menu.append(item)
    if menu.get_children() and isinstance(menu.get_children()[-1], Gtk.SeparatorMenuItem):
        menu.remove(menu.get_children()[-1])
    menu.show_all()
    return menu


def popup(widget, event, actions):
    actions = [a for a in actions]
    if not any(a for a in actions):
        return
    menu = build_menu(actions)
    menu.attach_to_widget(widget, None)
    menu.popup_at_pointer(event)


def icon_image(name, size=16):
    image = Gtk.Image.new_from_icon_name(name or "application-x-executable", Gtk.IconSize.MENU)
    image.set_pixel_size(size)
    return image


def has_icon(name):
    return Gtk.IconTheme.get_default().has_icon(name)


def pick_icon(*names):
    for n in names:
        if n and has_icon(n):
            return n
    return "application-x-executable"


# --- Results pane views --------------------------------------------------------------

class Column:
    """title, text(obj) -> str, width in px, sort(obj) -> key (defaults to the text)."""

    def __init__(self, title, text, width=120, sort=None, expand=False, align=0.0):
        self.title, self.text, self.width, self.sort = title, text, width, sort
        self.expand, self.align = expand, align


class ListView(Gtk.ScrolledWindow):
    """A details list: sortable columns, an icon in the first column, rows that carry
    their object, a context menu and double-click to activate."""

    def __init__(self, columns, icon=None, key=None, dim=None, on_select=None, on_activate=None,
                 on_menu=None, multiple=False, headers=True):
        super().__init__(hscrollbar_policy=Gtk.PolicyType.AUTOMATIC, vexpand=True, hexpand=True)
        self.columns = columns
        self.icon_of = icon or (lambda o: None)
        self.key_of = key or id
        self.dim_of = dim or (lambda o: False)
        self.on_select, self.on_activate, self.on_menu = on_select, on_activate, on_menu
        types = [str, bool] + [str] * len(columns) + [GObject.TYPE_PYOBJECT]
        self.store = Gtk.ListStore(*types)
        self.obj_col = len(types) - 1
        self.sorted = Gtk.TreeModelSort(model=self.store)
        self.tree = Gtk.TreeView(model=self.sorted, headers_visible=headers, enable_search=True)
        self.tree.set_search_column(2)
        self.tree.get_selection().set_mode(Gtk.SelectionMode.MULTIPLE if multiple else Gtk.SelectionMode.BROWSE)
        for i, col in enumerate(columns):
            tc = Gtk.TreeViewColumn(col.title)
            tc.set_resizable(True)
            tc.set_min_width(40)
            tc.set_fixed_width(col.width)
            tc.set_sizing(Gtk.TreeViewColumnSizing.FIXED)
            tc.set_expand(col.expand)
            if i == 0:
                pix = Gtk.CellRendererPixbuf(stock_size=Gtk.IconSize.MENU)
                pix.set_padding(4, 3)
                tc.pack_start(pix, False)
                tc.add_attribute(pix, "icon-name", 0)
            cell = Gtk.CellRendererText(ellipsize=Pango.EllipsizeMode.END, xalign=col.align)
            cell.set_padding(4, 3)
            tc.pack_start(cell, True)
            tc.add_attribute(cell, "text", i + 2)
            tc.set_cell_data_func(cell, self._dim_cell)
            tc.set_sort_column_id(i + 2)
            self.sorted.set_sort_func(i + 2, self._sorter, col)
            self.tree.append_column(tc)
        self.tree.get_selection().connect("changed", lambda *_: self.on_select and self.on_select())
        self.tree.connect("row-activated", lambda *_: self.on_activate and self.selected() is not None
                          and self.on_activate(self.selected()))
        self.tree.connect("button-press-event", self._on_button)
        self.add(self.tree)

    def _dim_cell(self, _col, cell, model, it, _data):
        cell.set_property("foreground", "#8a8a8a" if model[it][1] else None)

    def _sorter(self, model, a, b, col):
        oa, ob = model[a][self.obj_col], model[b][self.obj_col]
        if oa is None or ob is None:
            return 0
        ka = col.sort(oa) if col.sort else (col.text(oa) or "").lower()
        kb = col.sort(ob) if col.sort else (col.text(ob) or "").lower()
        try:
            return (ka > kb) - (ka < kb)
        except TypeError:
            return 0

    def _on_button(self, tree, event):
        if event.button != 3:
            return False
        hit = tree.get_path_at_pos(int(event.x), int(event.y))
        if hit:
            sel = tree.get_selection()
            if not sel.path_is_selected(hit[0]):
                sel.unselect_all()
                sel.select_path(hit[0])
        if self.on_menu:
            self.on_menu(event)
        return True

    def set_rows(self, objects):
        keep = {self.key_of(o) for o in self.selected_all()}
        scroll = self.get_vadjustment().get_value()
        self.store.clear()
        for o in objects:
            self.store.append([self.icon_of(o), bool(self.dim_of(o))] +
                              [c.text(o) or "" for c in self.columns] + [o])
        sel = self.tree.get_selection()
        for row in self.sorted:
            if self.key_of(row[self.obj_col]) in keep:
                sel.select_iter(row.iter)
        GLib.idle_add(lambda: self.get_vadjustment().set_value(scroll) and False)

    def selected(self):
        objs = self.selected_all()
        return objs[0] if objs else None

    def selected_all(self):
        model, paths = self.tree.get_selection().get_selected_rows()
        return [model[p][self.obj_col] for p in paths]

    def select_key(self, key):
        for row in self.sorted:
            if self.key_of(row[self.obj_col]) == key:
                self.tree.get_selection().select_iter(row.iter)
                self.tree.scroll_to_cell(row.path, None, False, 0, 0)
                return True
        return False

    def objects(self):
        return [row[self.obj_col] for row in self.store]


def scrolled(child, frame=True):
    sw = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER, vexpand=True)
    sw.add(child)
    return sw


def empty(text):
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
    box.pack_start(label(text, "empty", wrap=True, xalign=0.5), False, False, 0)
    return box


# --- Property sheets -----------------------------------------------------------------

class PropertySheet(Gtk.Window):
    """A Windows property sheet: tabs, then OK, Cancel and Apply. apply() is called for
    OK and Apply once something was changed."""

    def __init__(self, parent, title, apply=None, width=460, height=540, buttons=True):
        super().__init__(title=title, transient_for=parent, modal=False)
        self.set_name("settings-dialog")
        self.set_default_size(width, height)
        self.set_type_hint(Gdk.WindowTypeHint.DIALOG)
        self.apply_cb = apply
        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.notebook = Gtk.Notebook(vexpand=True)
        outer.pack_start(self.notebook, True, True, 0)
        foot = css(Gtk.Box(spacing=8), "dialog-foot")
        if buttons:
            self.ok = css(Gtk.Button(label="OK"), "btn", "accent")
            self.ok.connect("clicked", lambda *_: self.finish(True))
            cancel = css(Gtk.Button(label="Cancel"), "btn")
            cancel.connect("clicked", lambda *_: self.destroy())
            self.apply_button = css(Gtk.Button(label="Apply", sensitive=False), "btn")
            self.apply_button.connect("clicked", lambda *_: self.finish(False))
            for b in (self.apply_button, cancel, self.ok):
                b.set_size_request(96, -1)
                foot.pack_end(b, False, False, 0)
        else:
            close = css(Gtk.Button(label="Close"), "btn", "accent")
            close.set_size_request(96, -1)
            close.connect("clicked", lambda *_: self.destroy())
            foot.pack_end(close, False, False, 0)
            self.apply_button = None
        outer.pack_start(foot, False, False, 0)
        self.add(outer)
        self.connect("key-press-event", lambda _w, e: e.keyval == Gdk.KEY_Escape and (self.destroy() or True))

    def page(self, title):
        box = css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12), "sheet-page")
        sw = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER)
        sw.add(box)
        self.notebook.append_page(sw, Gtk.Label(label=title))
        return box

    def changed(self, *_):
        if self.apply_button:
            self.apply_button.set_sensitive(True)

    def finish(self, close):
        if self.apply_button and self.apply_button.get_sensitive() and self.apply_cb:
            self.apply_cb()
            self.apply_button.set_sensitive(False)
        if close:
            self.destroy()

    def present_sheet(self):
        self.show_all()
        self.present()


def sheet_header(icon_name, title, size=32):
    row = Gtk.Box(spacing=16)
    row.pack_start(icon_image(icon_name, size), False, False, 0)
    if isinstance(title, Gtk.Widget):
        row.pack_start(title, True, True, 0)
    else:
        row.pack_start(label(title, "sheet-title", wrap=True), True, True, 0)
    return row


def kv_grid(rows, selectable=True):
    """Key/value pairs in two columns. Rows with a None value are skipped."""
    grid = Gtk.Grid(column_spacing=16, row_spacing=8)
    r = 0
    for key, value in rows:
        if value is None:
            continue
        grid.attach(label(key, "key"), 0, r, 1, 1)
        if isinstance(value, Gtk.Widget):
            grid.attach(value, 1, r, 1, 1)
        else:
            v = label(str(value), wrap=True, selectable=selectable)
            v.set_hexpand(True)
            grid.attach(v, 1, r, 1, 1)
        r += 1
    return grid


def text_box(text, height=120):
    view = Gtk.TextView(editable=False, cursor_visible=False, wrap_mode=Gtk.WrapMode.WORD_CHAR)
    view.set_left_margin(8)
    view.set_right_margin(8)
    view.set_top_margin(6)
    view.set_bottom_margin(6)
    view.get_buffer().set_text(text)
    sw = css(Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER), "listframe")
    sw.set_size_request(-1, height)
    sw.add(view)
    sw.view = view
    return sw


def simple_list(rows, height=160, headers=None):
    """A framed list of strings (or tuples of strings for several columns)."""
    ncols = len(rows[0]) if rows and isinstance(rows[0], tuple) else (len(headers) if headers else 1)
    store = Gtk.ListStore(*([str] * ncols))
    for r in rows:
        store.append(list(r) if isinstance(r, tuple) else [r])
    tree = Gtk.TreeView(model=store, headers_visible=bool(headers))
    for i in range(ncols):
        cell = Gtk.CellRendererText(ellipsize=Pango.EllipsizeMode.END)
        cell.set_padding(4, 2)
        col = Gtk.TreeViewColumn(headers[i] if headers else "", cell, text=i)
        col.set_resizable(True)
        col.set_expand(i == ncols - 1)
        tree.append_column(col)
    sw = css(Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.AUTOMATIC), "listframe")
    sw.set_size_request(-1, height)
    sw.add(tree)
    sw.tree, sw.store = tree, store
    return sw


def ask_text(parent, title, prompt, value="", action="OK", callback=None, password=False):
    d = Dialog(parent, title, action)
    e = wsui.entry(value, "", 360, password=password)
    d.add_field(prompt, e)
    e.connect("activate", lambda *_: d.primary.clicked())
    d.on_primary(lambda: (callback(e.get_text().strip()), d.destroy()))
    d.run_dialog()
    return d


# --- Sizes, time and the root helper -------------------------------------------------

def size_text(n, decimals=2):
    if n is None:
        return ""
    n = float(n)
    for unit in ("bytes", "KB", "MB", "GB", "TB", "PB"):
        if abs(n) < 1024 or unit == "PB":
            return f"{n:.0f} bytes" if unit == "bytes" else f"{n:.{decimals}f} {unit}"
        n /= 1024


def when(usec):
    """A local date and time for a microsecond timestamp, as Windows writes it."""
    if not usec:
        return ""
    from datetime import datetime
    t = datetime.fromtimestamp(usec / 1e6)
    return t.strftime("%-m/%-d/%Y %-I:%M:%S %p")


def helper(args, done=None):
    """Run the root helper through pkexec (polkit asks for a password, then remembers
    it for a few minutes). done(ok, message)."""
    def finish(ok, o, e):
        msg = (e or o).strip()
        if "dismissed" in msg.lower() or "not authorized" in msg.lower():
            msg = "The change was cancelled."
        if done:
            done(ok, msg)
    run(["pkexec", HELPER] + [str(a) for a in args], finish)


def sysfs(path, name, default=""):
    try:
        with open(os.path.join(path, name)) as f:
            return f.read().strip()
    except (OSError, UnicodeDecodeError):
        return default


def clipboard(text):
    Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).set_text(text, -1)
