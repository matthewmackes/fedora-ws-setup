# Event Viewer: the journal, split into the Windows logs. Application is your
# session's journal, System is this boot's system journal, Security is the
# audit trail and Setup is package installs.
import json
import os
import subprocess
import threading

import mmcui as ui
from mmcui import Action, Node
from gi.repository import GLib, Gtk

KEY = "eventvwr"
NAME = "Event Viewer"
ICON = "text-x-generic"
DESCRIPTION = "View events that have happened on this computer"

LEVELS = {0: "Critical", 1: "Critical", 2: "Critical", 3: "Error", 4: "Warning",
          5: "Information", 6: "Information", 7: "Verbose"}
LEVEL_ORDER = ["Critical", "Error", "Warning", "Information", "Verbose"]
ICONS = {"Critical": "dialog-error", "Error": "dialog-error", "Warning": "dialog-warning",
         "Information": "dialog-information", "Verbose": "text-x-generic"}


def user_env():
    env = dict(os.environ)
    runtime = f"/run/user/{os.getuid()}"
    if os.path.isdir(runtime):
        env["XDG_RUNTIME_DIR"] = runtime
    return env


def message_of(e):
    msg = e.get("MESSAGE", "")
    if isinstance(msg, list):
        try:
            msg = bytes(msg).decode("utf-8", "replace")
        except (TypeError, ValueError):
            msg = " ".join(str(x) for x in msg)
    return str(msg).strip()


class Event:
    def __init__(self, raw):
        self.raw = raw
        try:
            pri = int(raw.get("PRIORITY", "6"))
        except ValueError:
            pri = 6
        self.level = LEVELS.get(pri, "Information")
        self.time = ui.when(int(raw.get("__REALTIME_TIMESTAMP", "0") or 0))
        self.usec = int(raw.get("__REALTIME_TIMESTAMP", "0") or 0)
        self.source = raw.get("SYSLOG_IDENTIFIER") or raw.get("_COMM") or raw.get("_SYSTEMD_UNIT") or "System"
        self.category = raw.get("_SYSTEMD_UNIT") or raw.get("SYSLOG_FACILITY") or ""
        self.message = message_of(raw)
        self.pid = raw.get("_PID", "")
        self.key = raw.get("__CURSOR") or f"{self.usec}:{self.source}:{self.message[:40]}"

    @property
    def icon(self):
        return ui.pick_icon(ICONS.get(self.level, "dialog-information"))


QUERIES = {
    "admin": ["journalctl", "--system", "-b", "-p", "warning", "-o", "json", "-n", "400", "--no-pager"],
    "application": ["journalctl", "--user", "-o", "json", "-n", "400", "--no-pager"],
    "security": ["journalctl", "--system", "-b", "-o", "json", "-n", "300", "--no-pager", "SYSLOG_FACILITY=4",
                 "+", "SYSLOG_FACILITY=10", "+", "_TRANSPORT=audit"],
    "setup": ["journalctl", "--system", "-b", "-o", "json", "-n", "200", "--no-pager", "SYSLOG_IDENTIFIER=dnf",
              "+", "SYSLOG_IDENTIFIER=rpm", "+", "SYSLOG_IDENTIFIER=packagekit"],
    "system": ["journalctl", "--system", "-b", "-o", "json", "-n", "400", "--no-pager"],
}


def fetch(kind, done):
    argv = QUERIES[kind]
    env = user_env() if "--user" in argv else None

    def work():
        try:
            p = subprocess.run(argv, capture_output=True, text=True, timeout=25, env=env)
            events, err = [], p.stderr.strip()
            for line in p.stdout.splitlines():
                try:
                    events.append(Event(json.loads(line)))
                except ValueError:
                    continue
            events.sort(key=lambda e: e.usec, reverse=True)
        except (OSError, subprocess.TimeoutExpired) as e:
            events, err = [], str(e)
        GLib.idle_add(lambda: done(events, err) and False)
    threading.Thread(target=work, daemon=True).start()


class SummaryView(ui.View):
    """The Event Viewer home page: how many errors and warnings this boot, and the latest."""

    def __init__(self, console, node):
        super().__init__(console, node)
        self.box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.widget = ui.scrolled(self.box)
        self.refresh()

    def refresh(self):
        ui.clear(self.box)
        self.box.pack_start(ui.css(ui.label("Event Viewer", "banner-title"), "banner"), False, False, 0)
        self.box.pack_start(ui.label("Loading events\u2026", "caption"), False, False, 0)
        self.box.show_all()
        fetch("system", self.loaded)

    def loaded(self, events, err):
        ui.clear(self.box)
        head = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4), "banner")
        head.pack_start(ui.label("Overview and Summary", "banner-title"), False, False, 0)
        head.pack_start(ui.label("A summary of administrative events on this computer.", "caption"), False, False, 0)
        self.box.pack_start(head, False, False, 0)
        if err and not events:
            self.box.pack_start(ui.label(err, "caption", wrap=True), False, False, 0)
        counts = {n: 0 for n in LEVEL_ORDER}
        for e in events:
            counts[e.level] = counts.get(e.level, 0) + 1
        self.box.pack_start(ui.label("Summary of Administrative Events", "summary-section"), False, False, 0)
        self.box.pack_start(ui.kv_grid([(n, str(counts[n])) for n in ("Critical", "Error", "Warning")]), False, False, 0)
        self.box.pack_start(ui.label(f"{counts['Information']} information, {counts['Verbose']} verbose, "
                                     f"{len(events)} events in this boot\u2019s sample.", "caption"), False, False, 0)
        recent = [e for e in events if e.level in ("Critical", "Error", "Warning")][:12]
        self.box.pack_start(ui.label("Recently seen", "summary-section"), False, False, 0)
        rows = [(e.level, e.time, e.source, e.message.split("\n", 1)[0][:140]) for e in recent]
        self.box.pack_start(ui.simple_list(rows or [("Information", "", "", "No warnings or errors in this sample.")],
                                           260, headers=["Level", "Date and Time", "Source", "Event"]), False, False, 0)
        self.box.show_all()
        self.status(f"{counts['Error']} errors, {counts['Warning']} warnings")

    def actions(self):
        return []


class LogView(ui.View):
    def __init__(self, console, node, kind):
        super().__init__(console, node)
        self.kind = kind
        self.events = []
        self.levels = set(LEVEL_ORDER)
        self.query = ""
        cols = [ui.Column("Level", lambda e: e.level, 100),
                ui.Column("Date and Time", lambda e: e.time, 180, sort=lambda e: e.usec),
                ui.Column("Source", lambda e: e.source, 160),
                ui.Column("Task Category", lambda e: e.category, 200, expand=True)]
        self.list = ui.ListView(cols, icon=lambda e: e.icon, key=lambda e: e.key, on_select=self.on_select,
                                on_activate=lambda e: EventSheet(self, e), on_menu=self.menu)
        self.preview_title = ui.label("")
        self.preview_body = ui.text_box("", 90)
        head = ui.css(Gtk.Box(), "preview-head")
        head.pack_start(self.preview_title, True, True, 0)
        prev = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL), "preview")
        prev.pack_start(head, False, False, 0)
        prev.pack_start(self.preview_body, False, False, 0)
        paned = Gtk.Paned(orientation=Gtk.Orientation.VERTICAL)
        paned.pack1(self.list, True, False)
        paned.pack2(prev, False, False)
        paned.set_position(360)
        self.widget = paned
        self.refresh()

    def refresh(self):
        self.status("Loading events\u2026")
        fetch(self.kind, self.loaded)

    def loaded(self, events, err):
        self.events = events
        self.apply_filter()
        self.status(err if err and not events else f"{len(events)} events")

    def shown_events(self):
        out = []
        q = self.query.lower()
        for e in self.events:
            if e.level not in self.levels:
                continue
            if q and q not in e.message.lower() and q not in e.source.lower():
                continue
            out.append(e)
        return out

    def apply_filter(self):
        rows = self.shown_events()
        self.list.set_rows(rows)
        if rows and self.list.selected() is None:
            self.list.tree.get_selection().select_path(Gtk.TreePath.new_first())
        self.status(f"{len(rows)} events")
        self.changed()

    def on_select(self):
        e = self.list.selected()
        self.preview_title.set_text(f"{e.level}  \u2014  {e.source}  \u2014  {e.time}" if e else "")
        self.preview_body.view.get_buffer().set_text(e.message if e else "")
        self.changed()

    def selected_title(self):
        e = self.list.selected()
        return e.source if e else None

    def actions(self):
        return [Action("Filter Current Log\u2026", self.filter_log, ui.pick_icon("edit-find-symbolic", "system-search-symbolic")),
                Action("Find\u2026", self.find),
                Action("Save All Events As\u2026", self.save),
                Action("Clear Log\u2026", self.clear, enabled=self.kind in ("system", "application"))]

    def item_actions(self):
        e = self.list.selected()
        if not e:
            return []
        return [Action("Properties", lambda: EventSheet(self, e), "document-properties-symbolic"),
                Action("Copy", lambda: ui.clipboard(e.message)),
                None, Action("Find\u2026", self.find)]

    def menu(self, event):
        ui.popup(self.list.tree, event, self.item_actions() or self.actions())

    def filter_log(self):
        dlg = ui.Dialog(self.console.win, "Filter Current Log", "OK", width=380)
        dlg.add_text("Show events of these levels:")
        checks = {}
        for name in LEVEL_ORDER:
            b = Gtk.CheckButton(label=name, active=name in self.levels)
            checks[name] = b
            dlg.body.pack_start(b, False, False, 0)

        def go():
            self.levels = {n for n, b in checks.items() if b.get_active()} or set(LEVEL_ORDER)
            dlg.destroy()
            self.apply_filter()
        dlg.on_primary(go)
        dlg.run_dialog()

    def find(self):
        ui.ask_text(self.console.win, "Find", "Find what:", self.query, "Find Next", self.find_next)

    def find_next(self, text):
        self.query = text
        self.apply_filter()

    def save(self):
        dlg = Gtk.FileChooserNative.new("Save All Events As", self.console.win, Gtk.FileChooserAction.SAVE,
                                        "_Save", "_Cancel")
        dlg.set_current_name(f"{self.node.title}.txt")
        dlg.set_do_overwrite_confirmation(True)
        if dlg.run() == Gtk.ResponseType.ACCEPT:
            lines = [f"{e.level}\t{e.time}\t{e.source}\t{e.category}\t{e.message.replace(chr(10), ' ')}\n"
                     for e in self.shown_events()]
            try:
                ui.write_file(dlg.get_filename(), "".join(lines))
            except OSError as e:
                ui.error(self.console.win, "Couldn\u2019t save the log", str(e))
        dlg.destroy()

    def clear(self):
        scope = "your user journal" if self.kind == "application" else "the system journal"
        ui.confirm(self.console.win, f"Clear {self.node.title}",
                   f"This permanently deletes {scope}. Individual logs share one journal, so events from other "
                   "logs in it are cleared too.", "Clear",
                   lambda: ui.helper(["journal-clear", "user" if self.kind == "application" else "system",
                                      str(os.getuid())],
                                     lambda ok, msg: (self.refresh() if ok else
                                                      ui.error(self.console.win, "Couldn\u2019t clear the log", msg))))


class EventSheet:
    def __init__(self, view, e):
        win = ui.PropertySheet(view.console.win, "Event Properties", width=520, height=460, buttons=False)
        box = win.page("General")
        box.pack_start(ui.sheet_header(e.icon, e.source), False, False, 0)
        box.pack_start(Gtk.Separator(), False, False, 0)
        box.pack_start(ui.kv_grid([
            ("Level:", e.level), ("Date:", e.time), ("Source:", e.source),
            ("Task category:", e.category or None), ("Process ID:", e.pid or None),
            ("Computer:", os.uname().nodename),
        ]), False, False, 0)
        box.pack_start(ui.label("Description:", "caption"), False, False, 0)
        box.pack_start(ui.text_box(e.message, 160), True, True, 0)
        details = win.page("Details")
        rows = []
        for k in sorted(e.raw):
            if k.startswith("__"):
                continue
            v = e.raw[k]
            if isinstance(v, list):
                v = message_of({"MESSAGE": v}) if k == "MESSAGE" else " ".join(str(x) for x in v)
            rows.append((k, str(v)[:300]))
        details.pack_start(ui.simple_list(rows, 320, headers=["Field", "Value"]), True, True, 0)
        win.present_sheet()


def log_node(title, kind, description):
    return Node(title, ui.pick_icon("text-x-generic"), lambda c, n: LogView(c, n, kind),
                key=f"event-{kind}", description=description)


def node():
    return Node("Event Viewer (Local)", ICON, SummaryView, key=KEY, description=DESCRIPTION, children=[
        Node("Custom Views", "folder", key="event-custom", description="Saved filters", children=[
            log_node("Administrative Events", "admin", "Warnings, errors and critical events from this boot"),
        ]),
        Node("Windows Logs", "folder", key="event-windows", description="The standard logs", children=[
            log_node("Application", "application", "Events from your session"),
            log_node("Security", "security", "Logon, authentication and audit events"),
            log_node("Setup", "setup", "Software installed and removed"),
            log_node("System", "system", "Events from this boot"),
        ]),
    ])
