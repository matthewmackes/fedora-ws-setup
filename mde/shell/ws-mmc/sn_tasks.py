# Task Scheduler: systemd timers, system and yours. Create Basic Task writes a
# timer and a oneshot service under ~/.config/systemd/user.
import json
import os
import re
import subprocess
import threading

import mmcui as ui
from mmcui import Action, Node
from gi.repository import GLib, Gtk

KEY = "taskschd"
NAME = "Task Scheduler"
ICON = "x-office-calendar"
DESCRIPTION = "Schedule tasks to run automatically"

USER_DIR = os.path.join(ui.wsui.CONFIG, "systemd", "user")
NAME_OK = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,40}$")


def user_env():
    env = dict(os.environ)
    runtime = f"/run/user/{os.getuid()}"
    if os.path.isdir(runtime):
        env["XDG_RUNTIME_DIR"] = runtime
    return env


def systemctl(args, user, done, timeout=40):
    argv = ["systemctl", "--no-pager"] + (["--user"] if user else []) + args
    env = user_env() if user else None

    def work():
        try:
            p = subprocess.run(argv, capture_output=True, text=True, timeout=timeout, env=env)
            ok, out, err = p.returncode == 0, p.stdout, p.stderr
        except (OSError, subprocess.TimeoutExpired) as e:
            ok, out, err = False, "", str(e)
        GLib.idle_add(lambda: done(ok, out, err) and False)
    threading.Thread(target=work, daemon=True).start()


class Task:
    def __init__(self, unit, activates, nxt, last, user, description=""):
        self.unit = unit
        self.activates = activates or ""
        self.next = int(nxt or 0)
        self.last = int(last or 0)
        self.user = user
        self.name = unit.removesuffix(".timer")
        self.description = description or self.name
        self.own = user and self.name.startswith("ws-task-")

    @property
    def status(self):
        return "Ready" if self.next else "Disabled"

    @property
    def next_text(self):
        return ui.when(self.next) if self.next else "Never"

    @property
    def last_text(self):
        return ui.when(self.last) if self.last else "Never"

    @property
    def author(self):
        return os.environ.get("USER", "") if self.user else "SYSTEM"

    @property
    def key(self):
        return ("user:" if self.user else "system:") + self.unit


def load_timers(done):
    def read(user):
        try:
            out = subprocess.run(["systemctl", "--no-pager"] + (["--user"] if user else []) +
                                 ["list-timers", "--all", "-o", "json"],
                                 capture_output=True, text=True, timeout=20,
                                 env=user_env() if user else None).stdout
            return json.loads(out or "[]")
        except (OSError, ValueError, subprocess.TimeoutExpired):
            return []

    def work():
        tasks = []
        for user in (False, True):
            for t in read(user):
                tasks.append(Task(t.get("unit", ""), t.get("activates", ""), t.get("next") or 0,
                                  t.get("last") or 0, user))
        GLib.idle_add(lambda: done(tasks) and False)
    threading.Thread(target=work, daemon=True).start()


class TaskView(ui.View):
    def __init__(self, console, node):
        super().__init__(console, node)
        cols = [ui.Column("Name", lambda t: t.name, 240),
                ui.Column("Status", lambda t: t.status, 90),
                ui.Column("Next Run Time", lambda t: t.next_text, 180, sort=lambda t: t.next),
                ui.Column("Last Run Time", lambda t: t.last_text, 180, sort=lambda t: t.last),
                ui.Column("Author", lambda t: t.author, 100),
                ui.Column("Task To Run", lambda t: t.activates, 200, expand=True)]
        self.list = ui.ListView(cols, icon=lambda t: ui.pick_icon("x-office-calendar"), key=lambda t: t.key,
                                dim=lambda t: t.status == "Disabled", on_select=self.changed,
                                on_activate=lambda t: TaskSheet(self, t), on_menu=self.menu)
        self.widget = self.list
        self.refresh()

    def refresh(self):
        self.status("Loading tasks\u2026")
        load_timers(self.loaded)

    def loaded(self, tasks):
        tasks.sort(key=lambda t: (not t.user, t.name.lower()))
        self.list.set_rows(tasks)
        self.status(f"{len(tasks)} tasks")
        self.changed()

    def selected_title(self):
        t = self.list.selected()
        return t.name if t else None

    def actions(self):
        return [Action("Create Basic Task\u2026", self.create, ui.pick_icon("list-add-symbolic"))]

    def item_actions(self):
        t = self.list.selected()
        if not t:
            return []
        return [Action("Run", lambda: self.run(t), ui.pick_icon("media-playback-start-symbolic"),
                       enabled=bool(t.activates)),
                Action("End", lambda: self.end(t), ui.pick_icon("media-playback-stop-symbolic"),
                       enabled=bool(t.activates)),
                None,
                Action("Enable", lambda: self.enable(t, True), enabled=t.status != "Ready"),
                Action("Disable", lambda: self.enable(t, False), enabled=t.user or t.status == "Ready"),
                None,
                Action("Delete", lambda: self.delete(t), "edit-delete-symbolic", enabled=t.own),
                None,
                Action("Properties", lambda: TaskSheet(self, t), "document-properties-symbolic")]

    def toolbar(self):
        return [Action("Create Basic Task", self.create, ui.pick_icon("list-add-symbolic"), tip="Create Basic Task")]

    def menu(self, event):
        ui.popup(self.list.tree, event, self.item_actions() or self.actions())

    def after(self, title):
        def done(ok, _o, err):
            if not ok:
                ui.error(self.console.win, title, (err or "").strip() or "The change was cancelled.")
            GLib.timeout_add(400, lambda: self.refresh() and False)
        return done

    def run(self, t):
        systemctl(["start", t.activates], t.user, self.after(f"Couldn\u2019t run {t.name}"))

    def end(self, t):
        systemctl(["stop", t.activates], t.user, self.after(f"Couldn\u2019t stop {t.name}"))

    def enable(self, t, on):
        args = ["enable", "--now", t.unit] if on else ["disable", "--now", t.unit]
        systemctl(args, t.user, self.after(f"Couldn\u2019t {'enable' if on else 'disable'} {t.name}"))

    def delete(self, t):
        if not t.own:
            return
        ui.confirm(self.console.win, "Task Scheduler",
                   f"Do you want to delete the task {t.name}?", "Yes", lambda: self.remove_files(t))

    def remove_files(self, t):
        base = t.name
        for ext in (".timer", ".service"):
            try:
                os.remove(os.path.join(USER_DIR, base + ext))
            except OSError:
                pass
        systemctl(["daemon-reload"], True, self.after("Couldn\u2019t delete the task"))

    def create(self):
        CreateTask(self)


class CreateTask:
    def __init__(self, view):
        self.view = view
        dlg = self.dlg = ui.Dialog(view.console.win, "Create Basic Task", "Finish", width=480)
        dlg.add_text("Create a task that runs a program on a schedule. It runs as you, when you are logged in.")
        self.name = dlg.add_field("Name:", ui.wsui.entry("", "My Task", 360))
        self.desc = dlg.add_field("Description:", ui.wsui.entry("", "", 360))
        self.trigger = Gtk.ComboBoxText()
        for key, label in (("daily", "Daily"), ("weekly", "Weekly"), ("logon", "When I log on"),
                           ("once", "One time")):
            self.trigger.append(key, label)
        self.trigger.set_active(0)
        dlg.add_field("When do you want the task to start?", self.trigger)
        row = Gtk.Box(spacing=8)
        self.hour = Gtk.SpinButton.new_with_range(0, 23, 1)
        self.minute = Gtk.SpinButton.new_with_range(0, 59, 1)
        self.hour.set_value(9)
        row.pack_start(self.hour, False, False, 0)
        row.pack_start(ui.label(":"), False, False, 0)
        row.pack_start(self.minute, False, False, 0)
        row.pack_start(ui.label("24-hour time", "caption"), False, False, 0)
        dlg.add_field("Start time:", row)
        self.program = dlg.add_field("Program:", ui.wsui.entry("", "/usr/bin/notify-send Hello", 360))
        dlg.on_primary(self.finish)
        dlg.run_dialog()

    def finish(self):
        name = self.name.get_text().strip().replace(" ", "-")
        if not NAME_OK.match(name):
            ui.error(self.dlg, "Create Basic Task", "Use letters, digits, a dot or a dash in the name.")
            return
        program = self.program.get_text().strip()
        if not program:
            ui.error(self.dlg, "Create Basic Task", "Type the program to run.")
            return
        unit = f"ws-task-{name}"
        desc = self.desc.get_text().strip() or name
        trigger = self.trigger.get_active_id()
        hh, mm = int(self.hour.get_value()), int(self.minute.get_value())
        if trigger == "daily":
            when = f"OnCalendar=*-*-* {hh:02d}:{mm:02d}:00"
        elif trigger == "weekly":
            when = f"OnCalendar=Mon *-*-* {hh:02d}:{mm:02d}:00"
        elif trigger == "logon":
            when = "OnStartupSec=30s"
        else:
            import datetime
            day = datetime.date.today().isoformat()
            when = f"OnCalendar={day} {hh:02d}:{mm:02d}:00"
        os.makedirs(USER_DIR, exist_ok=True)
        ui.write_file(os.path.join(USER_DIR, unit + ".service"),
                      f"[Unit]\nDescription={desc}\n\n[Service]\nType=oneshot\nExecStart={program}\n")
        ui.write_file(os.path.join(USER_DIR, unit + ".timer"),
                      f"[Unit]\nDescription={desc}\n\n[Timer]\n{when}\nPersistent=true\n\n"
                      f"[Install]\nWantedBy=timers.target\n")
        self.dlg.destroy()
        systemctl(["daemon-reload"], True, lambda ok, _o, err: (
            ui.error(self.view.console.win, "Couldn\u2019t create the task", err.strip()) if not ok else
            systemctl(["enable", "--now", unit + ".timer"], True, self.view.after("Couldn\u2019t enable the task"))))


class TaskSheet:
    def __init__(self, view, t):
        self.view, self.t = view, t
        win = ui.PropertySheet(view.console.win, f"{t.name} Properties", width=480, height=460, buttons=False)
        gen = win.page("General")
        gen.pack_start(ui.sheet_header(ui.pick_icon("x-office-calendar"), t.name), False, False, 0)
        gen.pack_start(Gtk.Separator(), False, False, 0)
        loc = "your account, only when you are logged on" if t.user else "the system account, whether or not you are logged on"
        gen.pack_start(ui.kv_grid([
            ("Name:", t.name), ("Author:", t.author), ("Status:", t.status),
            ("Description:", t.description),
        ]), False, False, 0)
        gen.pack_start(ui.label(f"When running the task, use {loc}.", wrap=True), False, False, 0)
        trig = win.page("Triggers")
        trig.pack_start(ui.simple_list([(t.next_text, "Enabled" if t.next else "Disabled")], 80,
                                       headers=["Next run", "Status"]), False, False, 0)
        trig.pack_start(ui.kv_grid([("Next run:", t.next_text), ("Last run:", t.last_text)]), False, False, 0)
        act = win.page("Actions")
        act.pack_start(ui.label("Start a program", "summary-section"), False, False, 0)
        act.pack_start(ui.kv_grid([("Program:", t.activates or "(none)")]), False, False, 0)
        if t.own:
            service = os.path.join(USER_DIR, t.name + ".service")
            text = ui.read_file(service)
            start = ""
            for line in text.splitlines():
                if line.startswith("ExecStart="):
                    start = line.split("=", 1)[1]
            act.pack_start(ui.kv_grid([("Command:", start or None)]), False, False, 0)
        hist = win.page("History")
        hist.pack_start(ui.label("The task\u2019s recent runs are in Event Viewer, under Windows Logs \u203a System "
                                 "for system tasks and Application for yours.", wrap=True), False, False, 0)
        b = ui.wsui.button("Open Event Viewer", lambda: ui.spawn(["ws-mmc", "eventvwr.msc"]))
        b.set_halign(Gtk.Align.START)
        hist.pack_start(b, False, False, 0)
        win.present_sheet()


def node():
    return Node("Task Scheduler (Local)", ICON, TaskView, key=KEY, description=DESCRIPTION)
