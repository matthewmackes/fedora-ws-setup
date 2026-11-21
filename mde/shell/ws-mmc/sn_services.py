# Services: systemd's system services, with the startup types Windows shows
# (Automatic, Manual, Disabled) and start, stop and restart.
import subprocess
import threading

import mmcui as ui
from mmcui import Action, Node
from gi.repository import GLib, Gtk

KEY = "services"
NAME = "Services"
ICON = "system-run"
DESCRIPTION = "Start, stop and configure the services on this computer"

STARTUP = {"enabled": "Automatic", "enabled-runtime": "Automatic", "static": "Manual",
           "indirect": "Manual", "generated": "Manual", "transient": "Manual", "alias": "Manual",
           "disabled": "Manual", "masked": "Disabled", "masked-runtime": "Disabled"}
LOCKED = {"static", "generated", "transient", "alias"}


def status_text(active, sub):
    if active == "active":
        return "Running"
    if active == "activating":
        return "Start Pending"
    if active == "deactivating":
        return "Stop Pending"
    if active == "failed":
        return "Failed"
    return ""


class Service:
    def __init__(self, unit, description, file_state, active="inactive", sub="dead"):
        self.unit = unit
        self.name = unit.removesuffix(".service")
        self.description = description or self.name
        self.file_state = file_state
        self.active, self.sub = active, sub
        self.startup = STARTUP.get(file_state, file_state)
        self.locked = file_state in LOCKED
        self.detail = None

    @property
    def status(self):
        return status_text(self.active, self.sub)

    @property
    def running(self):
        return self.active == "active"


def systemctl(args, done, timeout=40):
    def work():
        try:
            p = subprocess.run(["systemctl", "--no-pager"] + args, capture_output=True, text=True, timeout=timeout)
            out, err, ok = p.stdout, p.stderr, p.returncode == 0
        except (OSError, subprocess.TimeoutExpired) as e:
            out, err, ok = "", str(e), False
        GLib.idle_add(lambda: done(ok, out, err) and False)
    threading.Thread(target=work, daemon=True).start()


def parse_show(text):
    info = {}
    for line in text.splitlines():
        k, _, v = line.partition("=")
        info[k] = v
    return info


def exec_path(text):
    if "path=" in text:
        return text.split("path=", 1)[1].split(" ;", 1)[0].strip()
    return text


class ServicesView(ui.View):
    def __init__(self, console, node):
        super().__init__(console, node)
        cols = [ui.Column("Name", lambda s: s.description, 280),
                ui.Column("Status", lambda s: s.status, 110),
                ui.Column("Startup Type", lambda s: s.startup, 120),
                ui.Column("Service Name", lambda s: s.name, 220, expand=True)]
        self.list = ui.ListView(cols, icon=lambda s: ui.pick_icon("system-run"), key=lambda s: s.unit,
                                dim=lambda s: s.file_state.startswith("masked"), on_select=self.on_select,
                                on_activate=lambda s: ServiceSheet(self, s), on_menu=self.menu)
        self.desc = ui.label("", wrap=True)
        ext = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4), "extended")
        ext.set_size_request(-1, 72)
        ext.pack_start(self.desc, False, False, 0)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box.pack_start(self.list, True, True, 0)
        box.pack_start(Gtk.Separator(), False, False, 0)
        box.pack_start(ext, False, False, 0)
        self.widget = box
        self.refresh()

    def refresh(self):
        self.status("Loading services\u2026")

        def work():
            try:
                files = subprocess.run(["systemctl", "list-unit-files", "--type=service", "-o", "json", "--no-pager"],
                                       capture_output=True, text=True, timeout=20).stdout
                units = subprocess.run(["systemctl", "list-units", "--type=service", "--all", "-o", "json", "--no-pager"],
                                       capture_output=True, text=True, timeout=20).stdout
                import json
                files, units = json.loads(files or "[]"), json.loads(units or "[]")
            except (OSError, ValueError, subprocess.TimeoutExpired):
                files, units = [], []
            GLib.idle_add(lambda: self.loaded(files, units) and False)
        threading.Thread(target=work, daemon=True).start()

    def loaded(self, files, units):
        active = {u["unit"]: u for u in units}
        rows = []
        for f in files:
            unit = f.get("unit_file", "")
            if "@" in unit or not unit.endswith(".service"):
                continue
            live = active.get(unit, {})
            rows.append(Service(unit, live.get("description", ""), f.get("state", ""),
                                live.get("active", "inactive"), live.get("sub", "dead")))
        # Descriptions for inactive units are missing from list-units. Fill them lazily
        # from the unit name so the list appears immediately; a second pass fills the rest.
        rows.sort(key=lambda s: s.description.lower() or s.name)
        self.list.set_rows(rows)
        running = sum(1 for s in rows if s.running)
        self.status(f"{running} running, {len(rows)} services")
        self.fill_descriptions(rows)
        self.changed()

    def fill_descriptions(self, rows):
        missing = [s for s in rows if not s.description or s.description == s.name]

        def work():
            if not missing:
                return
            names = [s.unit for s in missing]
            try:
                out = subprocess.run(["systemctl", "show", "-p", "Id", "-p", "Description", "--no-pager"] + names,
                                     capture_output=True, text=True, timeout=40).stdout
            except (OSError, subprocess.TimeoutExpired):
                return
            desc = {}
            cur = ""
            for line in out.splitlines():
                k, _, v = line.partition("=")
                if k == "Id":
                    cur = v
                elif k == "Description" and cur:
                    desc[cur] = v
            GLib.idle_add(lambda: self.apply_descriptions(rows, desc) and False)
        threading.Thread(target=work, daemon=True).start()

    def apply_descriptions(self, rows, desc):
        for s in rows:
            if s.unit in desc and desc[s.unit]:
                s.description = desc[s.unit]
        self.list.set_rows(rows)

    def selected_service(self):
        return self.list.selected()

    def selected_title(self):
        s = self.selected_service()
        return s.description if s else None

    def on_select(self):
        s = self.selected_service()
        self.desc.set_text(s.description if s else "")
        self.changed()

    def actions(self):
        return [Action("Export List\u2026", self.export)]

    def item_actions(self):
        s = self.selected_service()
        if not s:
            return []
        return [Action("Start", lambda: self.ctl(s, "start"), ui.pick_icon("media-playback-start-symbolic"),
                       enabled=not s.running and not s.file_state.startswith("masked")),
                Action("Stop", lambda: self.ctl(s, "stop"), ui.pick_icon("media-playback-stop-symbolic"),
                       enabled=s.running),
                Action("Restart", lambda: self.ctl(s, "restart"), ui.pick_icon("view-refresh-symbolic"),
                       enabled=s.running),
                None,
                Action("Properties", lambda: ServiceSheet(self, s), "document-properties-symbolic"),
                Action("Help", self.console.about_node)]

    def menu(self, event):
        ui.popup(self.list.tree, event, self.item_actions() or self.actions())

    def ctl(self, s, verb):
        def done(ok, _out, err):
            if not ok:
                ui.error(self.console.win, f"Couldn\u2019t {verb} {s.description}", err.strip() or "The change was cancelled.")
            GLib.timeout_add(400, lambda: self.refresh() and False)
        systemctl([verb, s.unit], done)

    def set_startup(self, s, startup, done):
        if startup == "Disabled":
            systemctl(["mask", s.unit], done)
            return
        if startup == "Automatic":
            def step(ok, _o, err):
                if not ok and "not masked" not in err.lower():
                    done(ok, _o, err)
                    return
                systemctl(["enable", s.unit], done)
            systemctl(["unmask", s.unit], step)
            return
        else:
            def step(ok, _o, err):
                if not ok and "not masked" not in err.lower():
                    done(ok, _o, err)
                    return
                systemctl(["disable", s.unit], done)
            systemctl(["unmask", s.unit], step)

    def export(self):
        dlg = Gtk.FileChooserNative.new("Export List", self.console.win, Gtk.FileChooserAction.SAVE, "_Save", "_Cancel")
        dlg.set_do_overwrite_confirmation(True)
        dlg.set_current_name("Services.txt")
        if dlg.run() == Gtk.ResponseType.ACCEPT:
            lines = ["Name\tStatus\tStartup Type\tService Name\n"]
            lines += [f"{s.description}\t{s.status}\t{s.startup}\t{s.name}\n" for s in self.list.objects()]
            try:
                ui.write_file(dlg.get_filename(), "".join(lines))
            except OSError as e:
                ui.error(self.console.win, "Couldn\u2019t export the list", str(e))
        dlg.destroy()


class ServiceSheet:
    def __init__(self, view, s):
        self.view, self.s = view, s
        self.info = {}
        self.win = ui.PropertySheet(view.console.win, f"{s.description} Properties", apply=self.apply,
                                    width=480, height=520)
        self.general(self.win.page("General"))
        self.logon = self.win.page("Log On")
        self.recovery = self.win.page("Recovery")
        self.deps = self.win.page("Dependencies")
        self.win.present_sheet()
        systemctl(["show", s.unit, "--no-pager"], self.loaded)

    def loaded(self, ok, out, _err):
        if not ok:
            return
        self.info = parse_show(out)
        path = exec_path(self.info.get("ExecStart", ""))
        self.path_label.set_text(path or "(no executable)")
        self.status_label.set_text(self.s.status or "Stopped")
        user = self.info.get("User") or "root"
        self.logon.pack_start(ui.kv_grid([
            ("Log on as:", "Local System account" if user in ("root", "") else user),
            ("Group:", self.info.get("Group") or ("root" if user in ("root", "") else "")),
        ]), False, False, 0)
        self.logon.show_all()
        restart = self.info.get("Restart") or "no"
        names = {"no": "Take no action", "always": "Restart the service", "on-failure": "Restart the service",
                 "on-abnormal": "Restart the service", "on-abort": "Restart the service", "on-success": "Take no action"}
        self.recovery.pack_start(ui.kv_grid([
            ("First failure:", names.get(restart, restart)),
            ("Second failure:", names.get(restart, restart)),
            ("Subsequent failures:", names.get(restart, restart)),
            ("Restart policy:", restart),
        ]), False, False, 0)
        self.recovery.pack_start(ui.label("The restart policy comes from the service file. "
                                          "Edit it to change how the service recovers.", "caption", wrap=True),
                                 False, False, 0)
        self.recovery.show_all()
        for title, key in (("This service depends on:", "Requires"), ("This service wants:", "Wants"),
                           ("These services depend on this one:", "RequiredBy"),
                           ("These services want this one:", "WantedBy")):
            items = [i for i in self.info.get(key, "").split() if i]
            if items:
                self.deps.pack_start(ui.label(title, "caption"), False, False, 0)
                self.deps.pack_start(ui.simple_list(items, min(44 + 22 * len(items), 140)), False, False, 0)
        self.deps.show_all()

    def general(self, box):
        s = self.s
        box.pack_start(ui.sheet_header(ui.pick_icon("system-run"), s.description), False, False, 0)
        box.pack_start(Gtk.Separator(), False, False, 0)
        box.pack_start(ui.kv_grid([("Service name:", s.name), ("Display name:", s.description)]), False, False, 0)
        box.pack_start(ui.label("Description:", "caption"), False, False, 0)
        box.pack_start(ui.text_box(s.description, 60), False, False, 0)
        box.pack_start(ui.label("Path to executable:", "caption"), False, False, 0)
        self.path_label = ui.label("Loading\u2026", selectable=True, wrap=True)
        box.pack_start(self.path_label, False, False, 0)
        self.startup = Gtk.ComboBoxText()
        for opt in ("Automatic", "Manual", "Disabled"):
            self.startup.append(opt, opt)
        self.startup.set_active_id(s.startup if s.startup in ("Automatic", "Manual", "Disabled") else "Manual")
        self.startup.set_sensitive(not s.locked)
        self.startup.connect("changed", self.win.changed)
        box.pack_start(ui.label("Startup type:", "caption"), False, False, 0)
        box.pack_start(self.startup, False, False, 0)
        if s.locked:
            box.pack_start(ui.label("This service is part of the system and its startup type can\u2019t be changed.",
                                    "caption", wrap=True), False, False, 0)
        box.pack_start(ui.label("Service status:", "caption"), False, False, 0)
        self.status_label = ui.label(s.status or "Stopped")
        box.pack_start(self.status_label, False, False, 0)
        row = Gtk.Box(spacing=8)
        row.pack_start(ui.wsui.button("Start", lambda: self.view.ctl(s, "start"),
                                      sensitive=not s.running and not s.file_state.startswith("masked")), False, False, 0)
        row.pack_start(ui.wsui.button("Stop", lambda: self.view.ctl(s, "stop"), sensitive=s.running), False, False, 0)
        row.pack_start(ui.wsui.button("Restart", lambda: self.view.ctl(s, "restart"), sensitive=s.running), False, False, 0)
        box.pack_start(row, False, False, 0)

    def apply(self):
        want = self.startup.get_active_id()
        if not want or want == self.s.startup or self.s.locked:
            return

        def done(ok, _o, err):
            if not ok:
                ui.error(self.view.console.win, "Couldn\u2019t change the startup type", err.strip() or "The change was cancelled.")
            self.view.refresh()
        self.view.set_startup(self.s, want, done)


def node():
    return Node("Services (Local)", ICON, ServicesView, key=KEY, description=DESCRIPTION)
