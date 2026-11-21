# Local Users and Groups: accounts through AccountsService, groups through the root
# helper (groupadd, groupdel, groupmod, gpasswd and chage have no D-Bus API).
import grp
import os
import pwd

import mmcui as ui
from mmcui import Action, Node
from gi.repository import GLib, Gtk

KEY = "lusrmgr"
NAME = "Local Users and Groups"
ICON = "system-users"
DESCRIPTION = "Manage local users and groups"

AS = "org.freedesktop.Accounts"
AS_PATH = "/org/freedesktop/Accounts"
ME = pwd.getpwuid(os.getuid()).pw_name

# The groups that matter on a desktop, with what membership grants.
GROUPS = {
    "wheel": "Administrators: complete and unrestricted access to the computer (sudo)",
    "users": "Users: prevented from making accidental or intentional system-wide changes",
    "audio": "Direct access to sound devices",
    "video": "Direct access to video devices and screen brightness",
    "render": "Access to GPU render nodes",
    "input": "Direct access to keyboards, mice and other input devices",
    "dialout": "Access to serial ports and modems",
    "lp": "Access to printers and parallel ports",
    "cdrom": "Access to optical drives",
    "disk": "Raw access to disks (members can read every file)",
    "kvm": "Run virtual machines with hardware acceleration",
    "libvirt": "Manage virtual machines without a password",
    "docker": "Use the Docker daemon (equivalent to root)",
    "systemd-journal": "Event Log Readers: read the whole system journal",
    "adm": "Read system log files",
    "wireshark": "Capture network traffic",
    "vboxusers": "Use VirtualBox USB passthrough",
    "plugdev": "Mount removable devices",
    "games": "Shared game data",
}


def accounts():
    return ui.proxy(AS, AS_PATH, AS)


def user_proxy(path):
    return ui.proxy(AS, path, f"{AS}.User")


class User:
    def __init__(self, path, proxy):
        self.path, self.proxy = path, proxy
        p = lambda n, d=None: ui.prop(proxy, n, d)  # noqa: E731
        self.name = p("UserName", "")
        self.full = p("RealName", "")
        self.uid = p("Uid", 0)
        self.admin = p("AccountType", 0) == 1
        self.locked = bool(p("Locked", False))
        self.password_mode = p("PasswordMode", 0)
        self.home = p("HomeDirectory", "")
        self.shell = p("Shell", "")
        self.login = p("LoginTime", 0)
        self.system = bool(p("SystemAccount", False))
        self.icon_file = p("IconFile", "")

    @property
    def description(self):
        if self.uid == 0:
            return "Built-in account for administering the computer"
        kind = "Administrator" if self.admin else "Standard user"
        return f"{kind} (signed in)" if self.name == ME else kind


def load_users(done):
    acc = accounts()
    if not acc:
        done([])
        return

    def got(ok, value):
        paths = list(value[0]) if ok and value else []
        ui.call(acc, "FindUserByName", "(s)", "root", done=lambda ok2, v2: finish(paths + ([v2[0]] if ok2 and v2 else [])))

    def finish(paths):
        users = []
        for path in dict.fromkeys(paths):
            px = user_proxy(path)
            if px:
                users.append(User(path, px))
        done(users)
    ui.call(acc, "ListCachedUsers", None, done=got)


def all_groups():
    return {g.gr_name: g for g in grp.getgrall()}


def members_of(group):
    try:
        g = grp.getgrnam(group)
    except KeyError:
        return []
    names = list(g.gr_mem)
    for p in pwd.getpwall():
        if p.pw_gid == g.gr_gid and p.pw_name not in names:
            names.append(p.pw_name)
    return names


def groups_of(user):
    try:
        gid = pwd.getpwnam(user).pw_gid
    except KeyError:
        gid = None
    return sorted(g.gr_name for g in grp.getgrall() if user in g.gr_mem or g.gr_gid == gid)


def human_users():
    return sorted(p.pw_name for p in pwd.getpwall()
                  if p.pw_uid == 0 or (1000 <= p.pw_uid < 60000 and not p.pw_shell.endswith(("nologin", "false"))))


def crypt_password(password, done):
    ui.run(["openssl", "passwd", "-6", "-stdin"],
           lambda ok, out, err: done(out.strip() if ok and out.startswith("$6$") else None), stdin=password + "\n")


def password_fields(dlg, title_new="New password:"):
    p1 = ui.wsui.entry("", "", 320, password=True)
    p2 = ui.wsui.entry("", "", 320, password=True)
    dlg.add_field(title_new, p1)
    dlg.add_field("Confirm password:", p2)
    return p1, p2


def set_password(parent, user, password, done):
    def hashed(h):
        if not h:
            ui.error(parent, "Couldn\u2019t set the password", "The password couldn\u2019t be encrypted.")
            return
        ui.call(user_proxy(user.path) if isinstance(user, User) else user, "SetPassword", "(ss)", h, "",
                done=done)
    crypt_password(password, hashed)


# --- Users ---------------------------------------------------------------------------

class UsersView(ui.View):
    def __init__(self, console, node):
        super().__init__(console, node)
        cols = [ui.Column("Name", lambda u: u.name, 160),
                ui.Column("Full Name", lambda u: u.full, 200),
                ui.Column("Description", lambda u: u.description, 360, expand=True)]
        self.list = ui.ListView(cols, icon=lambda u: ui.pick_icon("avatar-default", "system-users"),
                                key=lambda u: u.name, dim=lambda u: u.locked, on_select=self.changed,
                                on_activate=lambda u: UserSheet(self, u), on_menu=self.menu)
        self.widget = self.list
        self.refresh()

    def refresh(self):
        load_users(self.loaded)

    def loaded(self, users):
        users.sort(key=lambda u: (u.uid != 0, u.name))
        self.list.set_rows(users)
        self.status(f"{len(users)} users")
        self.changed()

    def selected_title(self):
        u = self.list.selected()
        return u.name if u else None

    def actions(self):
        return [Action("New User\u2026", self.new_user, ui.pick_icon("list-add-symbolic")),
                Action("Export List\u2026", self.export)]

    def item_actions(self):
        u = self.list.selected()
        if not u:
            return []
        return [Action("Set Password\u2026", lambda: self.set_password(u), ui.pick_icon("dialog-password-symbolic",
                                                                                         "system-lock-screen-symbolic")),
                Action("All Tasks", children=[Action("Set Password\u2026", lambda: self.set_password(u))]),
                None,
                Action("Delete", lambda: self.delete(u), "edit-delete-symbolic", enabled=u.uid != 0 and u.name != ME),
                Action("Rename", lambda: self.rename(u), enabled=u.uid != 0),
                None,
                Action("Properties", lambda: UserSheet(self, u), "document-properties-symbolic")]

    def toolbar(self):
        return [Action("New User", self.new_user, ui.pick_icon("list-add-symbolic"), tip="New User")]

    def menu(self, event):
        ui.popup(self.list.tree, event, self.item_actions() or self.actions())

    def done(self, title):
        def finish(ok, value):
            if not ok:
                ui.error(self.console.win, title, str(value))
            self.refresh()
        return finish

    def new_user(self):
        dlg = ui.Dialog(self.console.win, "New User", "Create", width=460)
        name = dlg.add_field("User name:", ui.wsui.entry("", "", 320))
        full = dlg.add_field("Full name:", ui.wsui.entry("", "", 320))
        p1, p2 = password_fields(dlg, "Password:")
        must = Gtk.CheckButton(label="User must change password at next logon", active=True)
        admin = Gtk.CheckButton(label="Administrator (member of wheel)")
        disabled = Gtk.CheckButton(label="Account is disabled")
        for c in (must, admin, disabled):
            dlg.body.pack_start(c, False, False, 0)

        def go():
            n = name.get_text().strip()
            if not n or not n.replace("-", "").replace("_", "").replace(".", "").isalnum() or n[0].isdigit() \
                    or n != n.lower():
                ui.error(dlg, "Invalid user name", "Use lowercase letters, digits, - and _, starting with a letter.")
                return
            if p1.get_text() != p2.get_text():
                ui.error(dlg, "Local Users and Groups", "The passwords you typed do not match.")
                return
            password = p1.get_text()
            dlg.destroy()

            def created(ok, value):
                if not ok:
                    ui.error(self.console.win, "Couldn\u2019t create the user", str(value))
                    return
                path = value[0]
                px = user_proxy(path)
                if password:
                    set_password(self.console.win, px, password, lambda ok, v: (
                        must.get_active() and ui.helper(["user-expire", n]), self.refresh()))
                else:
                    ui.call(px, "SetPasswordMode", "(i)", 1 if must.get_active() else 2, done=lambda *_: self.refresh())
                if disabled.get_active():
                    ui.call(px, "SetLocked", "(b)", True, done=lambda *_: self.refresh())
                self.refresh()
            ui.call(accounts(), "CreateUser", "(ssi)", n, full.get_text().strip(), 1 if admin.get_active() else 0,
                    done=created)
        dlg.on_primary(go)
        dlg.run_dialog()

    def set_password(self, u):
        dlg = ui.Dialog(self.console.win, f"Set Password for {u.name}", "OK", width=460)
        if u.name == ME:
            dlg.add_text("Your password protects your files and keyring. Setting it here replaces it without asking "
                         "for the old one, so saved passwords in the keyring may stop unlocking automatically.")
        else:
            dlg.add_text(f"Resetting this password replaces it without the old one. {u.name} may lose access to "
                         "saved passwords in their keyring.")
        p1, p2 = password_fields(dlg)

        def go():
            if p1.get_text() != p2.get_text():
                ui.error(dlg, "Local Users and Groups", "The passwords you typed do not match.")
                return
            pw = p1.get_text()
            dlg.destroy()
            if not pw:
                ui.call(u.proxy, "SetPasswordMode", "(i)", 2, done=self.done("Couldn\u2019t set the password"))
            else:
                set_password(self.console.win, u, pw, lambda ok, v: (
                    ui.error(self.console.win, "Local Users and Groups", "The password has been set.") if ok else
                    ui.error(self.console.win, "Couldn\u2019t set the password", str(v)), self.refresh()))
        dlg.on_primary(go)
        dlg.run_dialog()

    def delete(self, u):
        dlg = ui.Dialog(self.console.win, "Local Users and Groups", "Yes", "No", width=480)
        dlg.add_text("Each user account has a unique identifier in addition to their user name. Deleting a user "
                     "account deletes this identifier and it cannot be restored, even if you create a new account "
                     "with an identical user name. This can prevent the user from accessing files they currently "
                     "have permission to.\n\nAre you sure you want to delete the user " + u.name + "?")
        files = Gtk.CheckButton(label=f"Also delete the home folder ({u.home})")
        dlg.body.pack_start(files, False, False, 0)

        def go():
            dlg.destroy()
            ui.call(accounts(), "DeleteUser", "(xb)", u.uid, files.get_active(),
                    done=self.done("Couldn\u2019t delete the user"))
        dlg.on_primary(go)
        dlg.run_dialog()

    def rename(self, u):
        ui.ask_text(self.console.win, f"Rename {u.name}", "New user name:", u.name, "Rename",
                    lambda new: new and new != u.name and ui.call(
                        u.proxy, "SetUserName", "(s)", new, done=self.done("Couldn\u2019t rename the user")))

    def export(self):
        export_list(self.console.win, "Users", ["Name", "Full Name", "Description"],
                    [(u.name, u.full, u.description) for u in self.list.objects()])


def export_list(parent, name, headers, rows):
    dlg = Gtk.FileChooserNative.new("Export List", parent, Gtk.FileChooserAction.SAVE, "_Save", "_Cancel")
    dlg.set_do_overwrite_confirmation(True)
    dlg.set_current_name(f"{name}.txt")
    if dlg.run() == Gtk.ResponseType.ACCEPT:
        text = "\t".join(headers) + "\n" + "".join("\t".join(r) + "\n" for r in rows)
        try:
            ui.write_file(dlg.get_filename(), text)
        except OSError as e:
            ui.error(parent, "Couldn\u2019t export the list", str(e))
    dlg.destroy()


class UserSheet:
    def __init__(self, view, u):
        self.view, self.u = view, u
        self.add_groups, self.del_groups = set(), set()
        self.win = ui.PropertySheet(view.console.win, f"{u.name} Properties", apply=self.apply, width=460, height=540)
        self.general(self.win.page("General"))
        self.member_of(self.win.page("Member Of"))
        self.profile(self.win.page("Profile"))
        self.win.present_sheet()

    def general(self, box):
        u = self.u
        box.pack_start(ui.sheet_header(ui.pick_icon("avatar-default"), u.name), False, False, 0)
        box.pack_start(Gtk.Separator(), False, False, 0)
        self.full = ui.wsui.entry(u.full, "", 300)
        self.full.connect("changed", self.win.changed)
        self.desc = ui.label(u.description)
        box.pack_start(ui.kv_grid([("Full name:", self.full), ("Description:", self.desc)]), False, False, 0)
        self.must = Gtk.CheckButton(label="User must change password at next logon")
        self.never = Gtk.CheckButton(label="Password never expires")
        self.disabled = Gtk.CheckButton(label="Account is disabled", active=u.locked)
        self.admin = Gtk.CheckButton(label="Administrator (member of wheel)", active=u.admin)
        for c in (self.must, self.never, self.disabled, self.admin):
            c.connect("toggled", self.win.changed)
            box.pack_start(c, False, False, 0)
        if u.uid == 0 or u.name == ME:
            self.disabled.set_sensitive(False)
            self.admin.set_sensitive(u.uid != 0 and u.name != ME)
        box.pack_start(Gtk.Separator(), False, False, 4)
        last = ui.when(u.login * 1_000_000) if u.login else "Never"
        box.pack_start(ui.kv_grid([("Last logon:", last), ("User ID:", str(u.uid)),
                                   ("Password:", {0: "Set", 1: "Set at next logon", 2: "None"}.get(u.password_mode, ""))]),
                       False, False, 0)

    def member_of(self, box):
        box.pack_start(ui.label("Member of:", "caption"), False, False, 0)
        self.groups = ui.simple_list([(g, GROUPS.get(g, "")) for g in groups_of(self.u.name)], 260,
                                     headers=["Group", "Description"])
        box.pack_start(self.groups, True, True, 0)
        row = Gtk.Box(spacing=8)
        row.pack_start(ui.wsui.button("Add\u2026", self.add_group), False, False, 0)
        row.pack_start(ui.wsui.button("Remove", self.remove_group), False, False, 0)
        box.pack_start(row, False, False, 0)
        box.pack_start(ui.label("Changes to a user\u2019s group membership are not effective until the next time "
                                "the user logs on.", "caption", wrap=True), False, False, 0)

    def add_group(self):
        current = {r[0] for r in self.groups.store}
        choices = [g for g in sorted(all_groups()) if g not in current and
                   (g in GROUPS or all_groups()[g].gr_gid >= 1000) and g not in human_users()]
        pick_names(self.win, "Select Groups", "Groups", choices, self.groups_added)

    def groups_added(self, names):
        for n in names:
            self.groups.store.append([n, GROUPS.get(n, "")])
            self.del_groups.discard(n)
            self.add_groups.add(n)
        self.win.changed()

    def remove_group(self):
        model, it = self.groups.tree.get_selection().get_selected()
        if it is None:
            return
        g = model[it][0]
        try:
            if pwd.getpwnam(self.u.name).pw_gid == grp.getgrnam(g).gr_gid:
                ui.error(self.win, "Local Users and Groups",
                         f"{g} is the primary group of {self.u.name}, so it can\u2019t be removed.")
                return
        except KeyError:
            pass
        model.remove(it)
        self.add_groups.discard(g)
        self.del_groups.add(g)
        self.win.changed()

    def profile(self, box):
        u = self.u
        box.pack_start(ui.label("User profile", "summary-section"), False, False, 0)
        box.pack_start(ui.kv_grid([("Profile path:", u.home), ("Logon script:", u.shell)]), False, False, 0)
        box.pack_start(ui.label("Home folder", "summary-section"), False, False, 0)
        box.pack_start(ui.kv_grid([("Local path:", u.home)]), False, False, 0)
        if u.home and os.path.isdir(u.home):
            b = ui.wsui.button("Open Home Folder", lambda: ui.spawn(["xdg-open", u.home]))
            b.set_halign(Gtk.Align.START)
            box.pack_start(b, False, False, 0)

    def apply(self):
        u = self.u
        done = self.view.done("Couldn\u2019t change the account")
        if self.full.get_text().strip() != u.full:
            ui.call(u.proxy, "SetRealName", "(s)", self.full.get_text().strip(), done=done)
        if self.disabled.get_active() != u.locked:
            ui.call(u.proxy, "SetLocked", "(b)", self.disabled.get_active(), done=done)
        if self.admin.get_active() != u.admin:
            ui.call(u.proxy, "SetAccountType", "(i)", 1 if self.admin.get_active() else 0, done=done)
        args = []
        if self.must.get_active():
            args += ["user-expire", u.name]
        if self.never.get_active():
            args += ["user-never-expires", u.name]
        for g in sorted(self.add_groups):
            args += ["group-member-add", g, u.name]
        for g in sorted(self.del_groups):
            args += ["group-member-del", g, u.name]
        self.add_groups, self.del_groups = set(), set()
        if args:
            ui.helper(["batch"] + args, lambda ok, msg: done(ok, msg or "The change was cancelled."))


def pick_names(parent, title, kind, choices, callback):
    """Windows' Select Users / Select Groups, as a checklist."""
    dlg = ui.Dialog(parent, title, "OK", width=420)
    dlg.add_text(f"Select the {kind.lower()} to add:")
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
    checks = []
    for c in choices:
        b = Gtk.CheckButton(label=c + (f" \u2013 {GROUPS[c].split(':')[0]}" if c in GROUPS and ":" in GROUPS[c] else ""))
        b.name = c
        checks.append(b)
        box.pack_start(b, False, False, 0)
    sw = ui.css(Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER), "listframe")
    sw.set_size_request(-1, 240)
    sw.add(box)
    dlg.body.pack_start(sw, True, True, 0)
    if not choices:
        dlg.add_text(f"There are no more {kind.lower()} to add.")
    dlg.on_primary(lambda: (dlg.destroy(), callback([b.name for b in checks if b.get_active()])))
    dlg.run_dialog()


# --- Groups --------------------------------------------------------------------------

class Group:
    def __init__(self, g):
        self.name, self.gid = g.gr_name, g.gr_gid
        self.description = GROUPS.get(self.name, "" if self.gid >= 1000 else "System group")

    @property
    def protected(self):
        return self.gid < 1000


class GroupsView(ui.View):
    def __init__(self, console, node):
        super().__init__(console, node)
        self.show_all = False
        cols = [ui.Column("Name", lambda g: g.name, 200),
                ui.Column("Description", lambda g: g.description, 460, expand=True)]
        self.list = ui.ListView(cols, icon=lambda g: ui.pick_icon("system-users", "group"), key=lambda g: g.name,
                                on_select=self.changed, on_activate=lambda g: GroupSheet(self, g), on_menu=self.menu)
        self.widget = self.list
        self.refresh()

    def refresh(self):
        users = set(human_users())
        out = []
        for g in grp.getgrall():
            private = g.gr_name in users and g.gr_gid >= 1000
            if self.show_all or g.gr_name in GROUPS or (g.gr_gid >= 1000 and g.gr_gid < 60000 and not private):
                out.append(Group(g))
        out.sort(key=lambda g: (g.name != "wheel", g.name))
        self.list.set_rows(out)
        self.status(f"{len(out)} groups")
        self.changed()

    def selected_title(self):
        g = self.list.selected()
        return g.name if g else None

    def actions(self):
        return [Action("New Group\u2026", self.new_group, ui.pick_icon("list-add-symbolic")),
                Action("Export List\u2026", lambda: export_list(
                    self.console.win, "Groups", ["Name", "Description"],
                    [(g.name, g.description) for g in self.list.objects()])),
                None, Action("View", children=self.view_actions())]

    def view_actions(self):
        return [Action("Show system groups", self.toggle_all, checked=self.show_all)]

    def toggle_all(self):
        self.show_all = not self.show_all
        self.refresh()

    def item_actions(self):
        g = self.list.selected()
        if not g:
            return []
        return [Action("Add to Group\u2026", lambda: GroupSheet(self, g)),
                None,
                Action("Delete", lambda: self.delete(g), "edit-delete-symbolic", enabled=not g.protected),
                Action("Rename", lambda: self.rename(g), enabled=not g.protected),
                None,
                Action("Properties", lambda: GroupSheet(self, g), "document-properties-symbolic")]

    def toolbar(self):
        return [Action("New Group", self.new_group, ui.pick_icon("list-add-symbolic"), tip="New Group")]

    def menu(self, event):
        ui.popup(self.list.tree, event, self.item_actions() or self.actions())

    def after(self, title):
        def done(ok, msg):
            if not ok:
                ui.error(self.console.win, title, msg or "The change was cancelled.")
            GLib.timeout_add(300, lambda: self.refresh() and False)
        return done

    def new_group(self):
        dlg = ui.Dialog(self.console.win, "New Group", "Create", width=440)
        name = dlg.add_field("Group name:", ui.wsui.entry("", "", 320))
        members = ui.simple_list([], 140)
        dlg.add_field("Members:", members)
        dlg.body.pack_start(ui.wsui.button("Add\u2026", lambda: pick_names(
            dlg, "Select Users", "Users", [u for u in human_users() if u not in {r[0] for r in members.store}],
            lambda names: [members.store.append([n]) for n in names])), False, False, 0)

        def go():
            n = name.get_text().strip()
            if not n or n in all_groups():
                ui.error(dlg, "New Group", "Type a group name that isn\u2019t in use.")
                return
            args = ["batch", "group-add", n]
            for r in members.store:
                args += ["group-member-add", n, r[0]]
            dlg.destroy()
            ui.helper(args, self.after("Couldn\u2019t create the group"))
        dlg.on_primary(go)
        dlg.run_dialog()

    def delete(self, g):
        ui.confirm(self.console.win, "Local Users and Groups",
                   "Each group has a unique identifier in addition to its group name. Deleting a group deletes this "
                   "identifier and it cannot be restored, even if you create a new group with an identical name. "
                   f"This can prevent group members from accessing resources they currently have permission to.\n\n"
                   f"Are you sure you want to delete the group {g.name}?", "Yes",
                   lambda: ui.helper(["group-del", g.name], self.after("Couldn\u2019t delete the group")))

    def rename(self, g):
        ui.ask_text(self.console.win, f"Rename {g.name}", "New group name:", g.name, "Rename",
                    lambda new: new and new != g.name and ui.helper(
                        ["group-rename", g.name, new], self.after("Couldn\u2019t rename the group")))


class GroupSheet:
    def __init__(self, view, g):
        self.view, self.g = view, g
        self.adds, self.dels = set(), set()
        self.win = ui.PropertySheet(view.console.win, f"{g.name} Properties", apply=self.apply, width=440, height=520)
        box = self.win.page("General")
        box.pack_start(ui.sheet_header(ui.pick_icon("system-users"), g.name), False, False, 0)
        box.pack_start(Gtk.Separator(), False, False, 0)
        box.pack_start(ui.kv_grid([("Description:", g.description or "\u2014"), ("Group ID:", str(g.gid))]),
                       False, False, 0)
        box.pack_start(ui.label("Members:", "caption"), False, False, 0)
        self.primary = {p.pw_name for p in pwd.getpwall() if p.pw_gid == g.gid}
        self.members = ui.simple_list(sorted(members_of(g.name)), 220)
        box.pack_start(self.members, True, True, 0)
        row = Gtk.Box(spacing=8)
        row.pack_start(ui.wsui.button("Add\u2026", self.add), False, False, 0)
        row.pack_start(ui.wsui.button("Remove", self.remove), False, False, 0)
        box.pack_start(row, False, False, 0)
        box.pack_start(ui.label("Changes to a user\u2019s group membership are not effective until the next time "
                                "the user logs on.", "caption", wrap=True), False, False, 0)
        self.win.present_sheet()

    def add(self):
        current = {r[0] for r in self.members.store}
        pick_names(self.win, "Select Users", "Users", [u for u in human_users() if u not in current], self.added)

    def added(self, names):
        for n in names:
            self.members.store.append([n])
            self.dels.discard(n)
            self.adds.add(n)
        self.win.changed()

    def remove(self):
        model, it = self.members.tree.get_selection().get_selected()
        if it is None:
            return
        n = model[it][0]
        if n in self.primary:
            ui.error(self.win, "Local Users and Groups",
                     f"{self.g.name} is the primary group of {n}, so {n} can\u2019t be removed.")
            return
        if self.g.name == "wheel" and n == ME:
            ui.error(self.win, "Local Users and Groups",
                     "You can\u2019t remove yourself from the administrators group here. Another administrator "
                     "has to do it.")
            return
        model.remove(it)
        self.adds.discard(n)
        self.dels.add(n)
        self.win.changed()

    def apply(self):
        args = ["batch"]
        for n in sorted(self.adds):
            args += ["group-member-add", self.g.name, n]
        for n in sorted(self.dels):
            args += ["group-member-del", self.g.name, n]
        self.adds, self.dels = set(), set()
        if len(args) > 1:
            ui.helper(args, self.view.after("Couldn\u2019t change the members"))


def node():
    return Node(NAME, ICON, children=[
        Node("Users", ui.pick_icon("avatar-default", "system-users"), UsersView, key="lusrmgr-users",
             description="Local user accounts"),
        Node("Groups", ui.pick_icon("system-users", "group"), GroupsView, key="lusrmgr-groups",
             description="Local groups and what membership grants"),
    ], key=KEY, description=DESCRIPTION)
