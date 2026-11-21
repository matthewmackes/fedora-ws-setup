#!/usr/bin/env python3
# ws-mmc: the Microsoft Management Console for the Sway session. A console has a
# tree of snap-ins on the left, the selected node's results in the middle and an
# Actions pane on the right.
#
#   ws-mmc                   an empty console (File > Add/Remove Snap-in)
#   ws-mmc compmgmt.msc      Computer Management (devmgmt.msc, diskmgmt.msc,
#                            lusrmgr.msc, services.msc, eventvwr.msc, taskschd.msc,
#                            perfmon.msc work the same way)
#   ws-mmc ~/Console1.msc    a console saved from File > Save As
import json
import os
import sys

SHARE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, SHARE)

import importlib  # noqa: E402
import traceback  # noqa: E402

import mmcui as ui  # noqa: E402
from mmcui import Action, Node  # noqa: E402
from gi.repository import Gdk, GLib, Gtk, Pango  # noqa: E402

MODULES = {}
for _name in ("sn_tasks", "sn_events", "sn_users", "sn_perf", "sn_devices", "sn_disks", "sn_services"):
    try:
        _m = importlib.import_module(_name)
        MODULES[_m.KEY] = _m
    except Exception:
        traceback.print_exc()


def snap_node(key):
    return MODULES[key].node() if key in MODULES else None


def compmgmt_node():
    def kids(*keys):
        return [n for n in (snap_node(k) for k in keys) if n]
    return Node("Computer Management (Local)", "computer", key="compmgmt", children=[
        Node("System Tools", "applications-system", key="systemtools",
             children=kids("taskschd", "eventvwr", "lusrmgr", "perfmon", "devmgmt"),
             description="Tools for managing this computer"),
        Node("Storage", "drive-harddisk", key="storage", children=kids("diskmgmt"),
             description="Disk drives and volumes"),
        Node("Services and Applications", "system-run", key="svcapps", children=kids("services"),
             description="Services that run on this computer"),
    ], description="Manage this computer: system tools, storage, services")


class SnapIn:
    def __init__(self, key, name, icon, description, node):
        self.key, self.name, self.icon, self.description, self.node = key, name, icon, description, node


SNAPINS = {s.key: s for s in [
    SnapIn("compmgmt", "Computer Management", "computer",
           "Computer Management and its system tools, storage and services", compmgmt_node)] + [
    SnapIn(m.KEY, m.NAME, m.ICON, m.DESCRIPTION, m.node) for m in MODULES.values()]}

# Built-in consoles: snap-ins, whether they sit under "Console Root", and the panes shown.
CONSOLES = {
    "compmgmt.msc": {"name": "Computer Management", "snapins": ["compmgmt"]},
    "devmgmt.msc": {"name": "Device Manager", "snapins": ["devmgmt"], "tree": False, "actions": False},
    "diskmgmt.msc": {"name": "Disk Management", "snapins": ["diskmgmt"], "tree": False, "actions": False},
    "lusrmgr.msc": {"name": "Local Users and Groups", "snapins": ["lusrmgr"]},
    "services.msc": {"name": "Services", "snapins": ["services"]},
    "eventvwr.msc": {"name": "Event Viewer", "snapins": ["eventvwr"]},
    "taskschd.msc": {"name": "Task Scheduler", "snapins": ["taskschd"]},
    "perfmon.msc": {"name": "Performance Monitor", "snapins": ["perfmon"]},
}


def load_spec(arg):
    """A console spec from a built-in name or a saved .msc file."""
    if not arg:
        return {"name": "Console1", "snapins": [], "root": True}
    base = os.path.basename(arg).lower()
    if base in CONSOLES and not os.path.isfile(arg):
        return dict(CONSOLES[base], builtin=base)
    try:
        with open(os.path.expanduser(arg)) as f:
            data = json.load(f)
    except (OSError, ValueError):
        return None
    if data.get("console") in CONSOLES:
        data = {**CONSOLES[data["console"]], **data, "builtin": data["console"]}
    data["path"] = os.path.abspath(os.path.expanduser(arg))
    data["snapins"] = [s for s in data.get("snapins", []) if s in SNAPINS]
    return data


class Console:
    def __init__(self, spec):
        self.spec = spec
        self.views = {}
        self.view = None
        self.node = None
        self.history, self.future = [], []
        self.dirty = False
        self.build()
        self.load_tree()

    # Window

    def build(self):
        win = self.win = Gtk.Window()
        win.set_name("settings")
        ui.css(win, "mmc")
        area = Gdk.Display.get_default().get_monitor(0).get_workarea()
        win.set_default_size(min(1180, area.width - 48), min(760, area.height - 72))
        win.connect("delete-event", self.on_close)
        win.connect("key-press-event", self.on_key)
        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        win.add(root)

        menubar = Gtk.MenuBar()
        self.file_menu = self.menubar_item(menubar, "_File", self.file_actions)
        self.action_menu = self.menubar_item(menubar, "_Action", self.action_actions)
        self.view_menu = self.menubar_item(menubar, "_View", self.view_actions)
        self.menubar_item(menubar, "_Help", lambda: [
            Action("About Management Console", self.about),
            Action(f"About {self.node.title}" if self.node else "About", self.about_node)])
        root.pack_start(menubar, False, False, 0)

        bar = self.toolbar = ui.css(Gtk.Box(spacing=2), "toolbar")
        self.back_b = self.tool(bar, "go-previous-symbolic", "Back", self.go_back)
        self.fwd_b = self.tool(bar, "go-next-symbolic", "Forward", self.go_forward)
        self.up_b = self.tool(bar, "go-up-symbolic", "Up one level", self.go_up)
        self.tree_b = self.tool(bar, "view-dual-symbolic", "Show/Hide Console Tree", self.toggle_tree)
        bar.pack_start(Gtk.Separator(orientation=Gtk.Orientation.VERTICAL), False, False, 0)
        self.refresh_b = self.tool(bar, "view-refresh-symbolic", "Refresh", self.refresh)
        self.props_b = self.tool(bar, "document-properties-symbolic", "Properties", self.properties)
        self.tool(bar, "help-browser-symbolic", "Help", self.about_node)
        self.extra_sep = Gtk.Separator(orientation=Gtk.Orientation.VERTICAL)
        bar.pack_start(self.extra_sep, False, False, 0)
        self.extra_tools = Gtk.Box(spacing=2)
        bar.pack_start(self.extra_tools, False, False, 0)
        self.actions_b = self.tool(bar, ui.pick_icon("view-sidebar-end-symbolic", "sidebar-show-right-symbolic",
                                                     "view-right-pane-symbolic", "view-paged-symbolic"),
                                   "Show/Hide Action Pane",
                                   self.toggle_actions, end=True)
        root.pack_start(bar, False, False, 0)

        self.outer = Gtk.Paned()
        self.outer.set_margin_start(8)
        self.outer.set_margin_end(8)
        root.pack_start(self.outer, True, True, 0)

        self.store = Gtk.TreeStore(str, str, object)
        self.tree = Gtk.TreeView(model=self.store, headers_visible=False, enable_search=False)
        col = Gtk.TreeViewColumn()
        pix = Gtk.CellRendererPixbuf(stock_size=Gtk.IconSize.MENU)
        pix.set_padding(2, 3)
        cell = Gtk.CellRendererText(ellipsize=Pango.EllipsizeMode.END)
        col.pack_start(pix, False)
        col.pack_start(cell, True)
        col.add_attribute(pix, "icon-name", 0)
        col.add_attribute(cell, "text", 1)
        self.tree.append_column(col)
        self.tree.get_selection().connect("changed", self.on_tree_select)
        self.tree.connect("button-press-event", self.on_tree_button)
        tree_sw = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER)
        tree_sw.add(self.tree)
        self.tree_pane = ui.css(Gtk.Frame(shadow_type=Gtk.ShadowType.NONE), "pane")
        self.tree_pane.add(tree_sw)
        self.tree_pane.set_size_request(220, -1)
        self.outer.pack1(self.tree_pane, False, False)

        self.inner = Gtk.Paned()
        self.outer.pack2(self.inner, True, False)
        self.center = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "pane")
        self.center.set_size_request(380, -1)
        self.inner.pack1(self.center, True, False)

        self.actions_pane = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "actions")
        sw = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER)
        sw.add(self.actions_pane)
        self.actions_frame = ui.css(Gtk.Frame(shadow_type=Gtk.ShadowType.NONE), "pane")
        self.actions_frame.add(sw)
        self.actions_frame.set_size_request(220, -1)
        self.inner.pack2(self.actions_frame, False, False)

        status = ui.css(Gtk.Box(), "statusbar")
        self.status_label = ui.label("")
        status.pack_start(self.status_label, True, True, 0)
        root.pack_start(status, False, False, 0)

        root.show_all()
        self.tree_pane.set_visible(self.spec.get("tree", True))
        self.actions_frame.set_visible(self.spec.get("actions", True))
        GLib.idle_add(self.place_panes)

    def place_panes(self):
        width = self.win.get_allocated_width()
        self.outer.set_position(240)
        self.inner.set_position(max(400, width - (240 if self.tree_pane.get_visible() else 0) - 16 - 250))
        return False

    def menubar_item(self, bar, text, actions):
        item = Gtk.MenuItem.new_with_mnemonic(text)
        menu = Gtk.Menu()
        item.set_submenu(menu)
        item.connect("activate", lambda *_: ui.build_menu(actions(), menu))
        bar.append(item)
        return menu

    def tool(self, bar, icon, tip, callback, end=False):
        b = Gtk.Button(tooltip_text=tip, relief=Gtk.ReliefStyle.NONE)
        b.add(ui.icon_image(icon))
        b.connect("clicked", lambda *_: callback())
        (bar.pack_end if end else bar.pack_start)(b, False, False, 0)
        return b

    # Console tree

    def root_nodes(self):
        snapins = [SNAPINS[k].node() for k in self.spec.get("snapins", []) if k in SNAPINS]
        if self.spec.get("root", False) or len(snapins) != 1:
            return [Node("Console Root", "folder", children=snapins, key="root",
                         description="Use File > Add/Remove Snap-in to add tools")]
        return snapins

    def load_tree(self):
        self.store.clear()
        for v in self.views.values():
            v.hidden()
        self.views.clear()
        self.view = None
        self.node = None
        self.history.clear()
        self.future.clear()
        for n in self.root_nodes():
            self.add_node(None, n)
        self.tree.expand_row(Gtk.TreePath.new_first(), False)
        first = self.store.get_iter_first()
        if first is not None:
            path = self.store.get_path(first)
            if len(self.store) == 1 and self.store[first][2].key == "compmgmt":
                self.tree.expand_row(path, False)
            self.tree.get_selection().select_path(path)
        self.update_title()

    def add_node(self, parent, node):
        it = self.store.append(parent, [node.icon, node.title, node])
        for c in node.children:
            self.add_node(it, c)

    def on_tree_select(self, sel):
        model, it = sel.get_selected()
        if it is None:
            return
        node = model[it][2]
        if node is self.node:
            return
        if self.node is not None and not getattr(self, "navigating", False):
            self.history.append(self.node)
            self.future.clear()
        self.show_node(node)

    def on_tree_button(self, tree, event):
        if event.button != 3:
            return False
        hit = tree.get_path_at_pos(int(event.x), int(event.y))
        if hit:
            tree.get_selection().select_path(hit[0])
            actions = self.view.actions() if self.view else []
            actions = actions + [None, Action("Refresh", self.refresh), None,
                                 Action("Help", self.about_node)]
            ui.popup(tree, event, actions)
        return True

    def iter_of(self, node):
        found = []

        def walk(model, path, it):
            if model[it][2] is node:
                found.append(it.copy())
                return True
            return False
        self.store.foreach(walk)
        return found[0] if found else None

    def select_node(self, node, record=True):
        it = self.iter_of(node)
        if it is None:
            return
        path = self.store.get_path(it)
        self.tree.expand_to_path(path)
        self.navigating = not record
        self.tree.get_selection().select_path(path)
        self.navigating = False

    # Results pane

    def show_node(self, node):
        if self.view:
            self.view.hidden()
        self.node = node
        view = self.views.get(id(node))
        if view is None:
            try:
                view = node.view(self, node) if node.view else FolderView(self, node)
            except Exception as e:
                view = ErrorView(self, node, e)
            self.views[id(node)] = view
        self.view = view
        for c in self.center.get_children():
            self.center.remove(c)
        self.center.pack_start(view.widget, True, True, 0)
        view.widget.show_all()
        view.shown()
        self.status_label.set_text(getattr(view, "status_text", ""))
        self.update_actions(view)
        self.update_title()
        self.back_b.set_sensitive(bool(self.history))
        self.fwd_b.set_sensitive(bool(self.future))
        it = self.iter_of(node)
        self.up_b.set_sensitive(bool(it is not None and self.store.iter_parent(it)))

    def update_actions(self, view):
        if view is not self.view:
            return
        ui.clear(self.actions_pane)
        self.section(self.node.title, view.actions() + [None, Action("Refresh", self.refresh, "view-refresh-symbolic"),
                                                         None, Action("Help", self.about_node, "help-browser-symbolic")])
        title = view.selected_title()
        if title:
            self.section(title, view.item_actions())
        self.actions_pane.show_all()
        ui.clear(self.extra_tools)
        tools = view.toolbar()
        for a in tools:
            b = self.tool(self.extra_tools, a.icon, a.tip or a.label, a.callback)
            b.set_sensitive(a.enabled)
        self.extra_tools.show_all()
        self.extra_sep.set_visible(bool(tools))
        self.props_b.set_sensitive(any(a and a.label == "Properties" and a.enabled
                                       for a in view.item_actions()))

    def section(self, title, actions):
        head = ui.css(Gtk.Box(), "actions-head")
        head.pack_start(ui.label(title), True, True, 0)
        self.actions_pane.pack_start(head, False, False, 0)
        for a in actions:
            if a is None:
                continue
            b = Gtk.Button(relief=Gtk.ReliefStyle.NONE, sensitive=a.enabled)
            row = Gtk.Box(spacing=8)
            row.pack_start(ui.icon_image(a.icon) if a.icon else Gtk.Box(width_request=16), False, False, 0)
            row.pack_start(ui.label(a.label), True, True, 0)
            if a.children:
                row.pack_end(ui.icon_image("pan-end-symbolic", 12), False, False, 0)
                b.connect("button-press-event", lambda w, e, c=a.children: ui.popup(w, e, c) or True)
            elif a.callback:
                b.connect("clicked", lambda _b, cb=a.callback: cb())
            b.add(row)
            self.actions_pane.pack_start(b, False, False, 0)

    def set_status(self, view, text):
        view.status_text = text
        if view is self.view:
            self.status_label.set_text(text)

    def update_title(self):
        name = self.spec.get("name", "Console1")
        if self.spec.get("builtin") and not self.spec.get("path"):
            title = name
            if self.node and self.node.title not in (name, "Computer Management (Local)") \
                    and self.spec.get("tree", True):
                title = f"{name} - [{self.node.title}]" if name == "Computer Management" else name
        else:
            title = f"{name} - [{self.node.title if self.node else 'Console Root'}]"
        self.win.set_title(title)
        snap = self.spec.get("snapins", [])
        icon = SNAPINS[snap[0]].icon if len(snap) == 1 and snap[0] in SNAPINS else "applications-system"
        self.win.set_icon_name(icon)

    # Navigation and commands

    def go_back(self):
        if self.history:
            self.future.append(self.node)
            self.select_node(self.history.pop(), record=False)

    def go_forward(self):
        if self.future:
            self.history.append(self.node)
            self.select_node(self.future.pop(), record=False)

    def go_up(self):
        it = self.iter_of(self.node)
        parent = self.store.iter_parent(it) if it is not None else None
        if parent is not None:
            self.select_node(self.store[parent][2])

    def open_child(self, node):
        self.select_node(node)

    def toggle_tree(self):
        self.tree_pane.set_visible(not self.tree_pane.get_visible())
        self.dirty = True

    def toggle_actions(self):
        self.actions_frame.set_visible(not self.actions_frame.get_visible())
        self.dirty = True

    def refresh(self):
        if self.view:
            self.view.refresh()

    def properties(self):
        if self.view:
            self.view.properties()

    def on_key(self, _w, event):
        mods = event.state & Gtk.accelerator_get_default_mod_mask()
        k = event.keyval
        if k == Gdk.KEY_F5:
            self.refresh()
        elif k == Gdk.KEY_Return and mods == Gdk.ModifierType.MOD1_MASK:
            self.properties()
        elif k == Gdk.KEY_Left and mods == Gdk.ModifierType.MOD1_MASK:
            self.go_back()
        elif k == Gdk.KEY_Right and mods == Gdk.ModifierType.MOD1_MASK:
            self.go_forward()
        elif k == Gdk.KEY_Delete and not mods and self.view and not isinstance(self.win.get_focus(), Gtk.Entry):
            self.view.delete()
        elif mods == Gdk.ModifierType.CONTROL_MASK and k in (Gdk.KEY_m, Gdk.KEY_o, Gdk.KEY_s, Gdk.KEY_n):
            {Gdk.KEY_m: self.add_remove, Gdk.KEY_o: self.open_console,
             Gdk.KEY_s: self.save, Gdk.KEY_n: self.new_console}[k]()
        else:
            return False
        return True

    # Menus

    def file_actions(self):
        return [Action("New", self.new_console), Action("Open\u2026", self.open_console),
                Action("Save", self.save), Action("Save As\u2026", self.save_as), None,
                Action("Add/Remove Snap-in\u2026", self.add_remove), None,
                Action("Exit", lambda: self.on_close() or self.win.destroy())]

    def action_actions(self):
        if not self.view:
            return [Action("Help", self.about_node)]
        acts = list(self.view.actions())
        items = self.view.item_actions() if self.view.selected_title() else []
        if items:
            acts += [None] + items
        return acts + [None, Action("Refresh", self.refresh), None, Action("Help", self.about_node)]

    def view_actions(self):
        acts = [Action("Customize", children=[
            Action("Console tree", self.toggle_tree, checked=self.tree_pane.get_visible()),
            Action("Action pane", self.toggle_actions, checked=self.actions_frame.get_visible())])]
        extra = self.view.view_actions() if self.view else []
        return acts + ([None] + extra if extra else [])

    # Console files

    def new_console(self):
        ui.spawn(["ws-mmc"])

    def open_console(self):
        d = Gtk.FileChooserNative.new("Open Console", self.win, Gtk.FileChooserAction.OPEN, "_Open", "_Cancel")
        f = Gtk.FileFilter()
        f.set_name("Microsoft Common Console Document (*.msc)")
        f.add_pattern("*.msc")
        d.add_filter(f)
        if d.run() == Gtk.ResponseType.ACCEPT:
            path = d.get_filename()
            spec = load_spec(path)
            if spec is None:
                ui.error(self.win, "Can\u2019t open this console", f"{path} isn\u2019t a console file.")
            else:
                self.spec = spec
                self.load_tree()
                self.tree_pane.set_visible(spec.get("tree", True))
                self.actions_frame.set_visible(spec.get("actions", True))
        d.destroy()

    def save(self):
        if self.spec.get("path"):
            self.write(self.spec["path"])
        else:
            self.save_as()

    def save_as(self):
        d = Gtk.FileChooserNative.new("Save As", self.win, Gtk.FileChooserAction.SAVE, "_Save", "_Cancel")
        d.set_do_overwrite_confirmation(True)
        docs = GLib.get_user_special_dir(GLib.UserDirectory.DIRECTORY_DOCUMENTS) or ui.HOME
        d.set_current_folder(docs)
        d.set_current_name(f"{self.spec.get('name', 'Console1')}.msc")
        if d.run() == Gtk.ResponseType.ACCEPT:
            path = d.get_filename()
            if not path.endswith(".msc"):
                path += ".msc"
            self.spec["name"] = os.path.splitext(os.path.basename(path))[0]
            self.write(path)
        d.destroy()

    def write(self, path):
        data = {"version": 1, "name": self.spec.get("name", "Console1"),
                "snapins": self.spec.get("snapins", []), "root": self.spec.get("root", False),
                "tree": self.tree_pane.get_visible(), "actions": self.actions_frame.get_visible()}
        try:
            ui.write_file(path, json.dumps(data, indent=2) + "\n")
        except OSError as e:
            ui.error(self.win, "Can\u2019t save the console", str(e))
            return
        self.spec["path"] = path
        self.dirty = False
        self.update_title()

    def on_close(self, *_):
        if self.dirty and not self.spec.get("builtin"):
            d = ui.Dialog(self.win, f"Save console settings to {self.spec.get('name', 'Console1')}?",
                          "Save", "Don\u2019t save")
            d.on_primary(lambda: (d.destroy(), self.save(), self.win.destroy()))
            dont_save = d.primary.get_parent().get_children()[-1]
            dont_save.connect("clicked", lambda *_: GLib.idle_add(self.win.destroy))
            d.run_dialog()
            return True
        return False

    def add_remove(self):
        AddRemove(self)

    def apply_snapins(self, keys):
        self.spec = {**self.spec, "snapins": keys, "root": True}
        self.spec.pop("builtin", None)
        if self.spec.get("name") in [c["name"] for c in CONSOLES.values()]:
            self.spec["name"] = "Console1"
        self.dirty = True
        self.load_tree()

    # Help

    def about(self):
        d = ui.Dialog(self.win, "Management Console", None, "OK")
        d.add_text("Management Console for this PC.\nSnap-ins manage devices, disks, users and "
                   "groups, services, scheduled tasks, events and performance through the system\u2019s "
                   "own services (udev, UDisks, AccountsService, systemd and the journal).")
        d.run_dialog()

    def about_node(self):
        if not self.node:
            return self.about()
        d = ui.Dialog(self.win, self.node.title, None, "OK")
        d.add_text(self.node.description or "A Management Console snap-in.")
        d.run_dialog()


class FolderView(ui.View):
    """A node without its own results: list its children."""

    def __init__(self, console, node):
        super().__init__(console, node)
        self.list = ui.ListView([ui.Column("Name", lambda n: n.title, 260),
                                 ui.Column("Description", lambda n: n.description, 420, expand=True)],
                                icon=lambda n: n.icon, on_activate=console.open_child,
                                on_select=self.changed, on_menu=self.menu)
        self.list.set_rows(node.children)
        self.widget = self.list if node.children else ui.empty(
            "There are no items to show in this view." + (
                "\n\nUse File \u203a Add/Remove Snap-in to add tools to this console."
                if node.key == "root" else ""))

    def actions(self):
        if self.node.key == "root":
            return [Action("Add/Remove Snap-in\u2026", self.console.add_remove, "list-add-symbolic")]
        return []

    def selected_title(self):
        n = self.list.selected()
        return n.title if n else None

    def item_actions(self):
        n = self.list.selected()
        return [Action("Open", lambda: self.console.open_child(n))] if n else []

    def menu(self, event):
        ui.popup(self.list.tree, event, self.item_actions())


class ErrorView(ui.View):
    def __init__(self, console, node, exc):
        super().__init__(console, node)
        import traceback
        traceback.print_exception(exc)
        self.widget = ui.empty(f"{node.title} couldn\u2019t open.\n\n{exc}")


class AddRemove:
    """File > Add/Remove Snap-in: available snap-ins on the left, the console's on the right."""

    def __init__(self, console):
        self.console = console
        self.keys = list(console.spec.get("snapins", []))
        d = self.d = ui.Dialog(console.win, "Add or Remove Snap-ins", "OK", "Cancel", width=760)
        d.set_resizable(True)
        d.add_text("Choose the snap-ins for this console. Snap-ins run with your account; changes "
                   "that need an administrator ask for your password.")
        row = Gtk.Box(spacing=12)
        self.avail = ui.ListView([ui.Column("Available snap-ins", lambda s: s.name, 260, expand=True)],
                                 icon=lambda s: s.icon, key=lambda s: s.key,
                                 on_select=self.describe, on_activate=lambda s: self.add())
        self.avail.set_rows(list(SNAPINS.values()))
        self.avail.set_size_request(280, 300)
        row.pack_start(ui.css(self.frame(self.avail), "listframe"), True, True, 0)
        mid = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, valign=Gtk.Align.CENTER)
        mid.pack_start(ui.wsui.button("Add >", self.add), False, False, 0)
        row.pack_start(mid, False, False, 0)
        self.chosen = ui.ListView([ui.Column("Selected snap-ins", lambda k: SNAPINS[k].name, 260, expand=True)],
                                  icon=lambda k: SNAPINS[k].icon, key=lambda k: k)
        self.chosen.set_size_request(280, 300)
        row.pack_start(ui.css(self.frame(self.chosen), "listframe"), True, True, 0)
        side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, valign=Gtk.Align.CENTER)
        side.pack_start(ui.wsui.button("Remove", self.remove), False, False, 0)
        side.pack_start(ui.wsui.button("Move Up", lambda: self.move(-1)), False, False, 0)
        side.pack_start(ui.wsui.button("Move Down", lambda: self.move(1)), False, False, 0)
        row.pack_start(side, False, False, 0)
        d.body.pack_start(row, True, True, 0)
        self.desc = ui.label("", "caption", wrap=True)
        d.body.pack_start(self.desc, False, False, 0)
        d.on_primary(self.ok)
        self.update()
        d.run_dialog()

    def frame(self, child):
        f = Gtk.Frame(shadow_type=Gtk.ShadowType.NONE)
        f.add(child)
        return f

    def describe(self):
        s = self.avail.selected()
        self.desc.set_text(f"Description: {s.description}" if s else "")

    def update(self):
        self.chosen.set_rows(self.keys)

    def add(self):
        s = self.avail.selected()
        if s:
            self.keys.append(s.key)
            self.update()

    def remove(self):
        k = self.chosen.selected()
        if k in self.keys:
            self.keys.remove(k)
            self.update()

    def move(self, step):
        k = self.chosen.selected()
        if k not in self.keys:
            return
        i = self.keys.index(k)
        j = max(0, min(len(self.keys) - 1, i + step))
        self.keys[i], self.keys[j] = self.keys[j], self.keys[i]
        self.update()
        self.chosen.select_key(k)

    def ok(self):
        self.d.destroy()
        if self.keys != self.console.spec.get("snapins"):
            self.console.apply_snapins(self.keys)


def main():
    arg = next((a for a in sys.argv[1:] if not a.startswith("-")), "")
    spec = load_spec(arg)
    if spec is None:
        sys.exit(f"ws-mmc: {arg} is not a console file or a built-in console")
    GLib.set_prgname("ws-mmc")
    GLib.set_application_name(spec.get("name", "Management Console"))
    ui.load_css()
    console = Console(spec)
    console.win.connect("destroy", lambda *_: Gtk.main_quit())
    console.win.show()
    Gtk.main()


if __name__ == "__main__":
    main()
