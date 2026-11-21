# Settings > Accounts: Your info, Sign-in options and Other users, through
# AccountsService and fprintd on D-Bus (polkit asks for a password).
import os
import pty
import pwd
import select
import subprocess
import threading
import time

from gi.repository import GdkPixbuf, GLib, Gtk

import wsui as ui

TYPES = [(0, "Standard user"), (1, "Administrator")]


def accounts():
    return ui.proxy("org.freedesktop.Accounts", "/org/freedesktop/Accounts", "org.freedesktop.Accounts")


def user_proxy(path):
    return ui.proxy("org.freedesktop.Accounts", path, "org.freedesktop.Accounts.User")


def me():
    return user_proxy(f"/org/freedesktop/Accounts/User{os.getuid()}")


def display_name(u):
    return ui.prop(u, "RealName") or ui.prop(u, "UserName") or "User"


def type_name(u):
    return "Administrator" if ui.prop(u, "AccountType", 0) == 1 else "Standard user"


def set_photo(app, path):
    try:
        pix = GdkPixbuf.Pixbuf.new_from_file(path)
    except GLib.Error as e:
        ui.error(app.win, "Couldn\u2019t use that picture", e.message)
        return
    side = min(pix.get_width(), pix.get_height())
    pix = pix.new_subpixbuf((pix.get_width() - side) // 2, (pix.get_height() - side) // 2, side, side)
    pix = pix.scale_simple(256, 256, GdkPixbuf.InterpType.HYPER)
    face = os.path.join(ui.HOME, ".face")
    pix.savev(face, "png", [], [])
    ui.call(me(), "SetIconFile", "(s)", face, done=lambda *_: None)
    app.avatar.reload()
    app.refresh()


def build_yourinfo(app, body):
    u = me()
    hero = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4), "hero")
    big = ui.Avatar(160)
    big.set_halign(Gtk.Align.CENTER)
    hero.pack_start(big, False, False, 8)
    hero.pack_start(ui.label(display_name(u), "hero-title", xalign=0.5), False, False, 0)
    hero.pack_start(ui.label(ui.prop(u, "UserName") or "", "caption", xalign=0.5), False, False, 0)
    hero.pack_start(ui.label(f"Local account  \u00b7  {type_name(u)}", "caption", xalign=0.5), False, False, 0)
    body.pack_start(hero, False, False, 0)

    body.pack_start(ui.section("Adjust your photo"), False, False, 0)

    def browse():
        from p_personal import choose_picture
        choose_picture(app, "Choose a photo", lambda f: set_photo(app, f))

    def remove():
        try:
            os.unlink(os.path.join(ui.HOME, ".face"))
        except OSError:
            pass
        ui.call(u, "SetIconFile", "(s)", "", done=lambda *_: (app.avatar.reload(), app.refresh()))
    body.pack_start(ui.row("Choose a file", None, "image-x-generic-symbolic",
                           [ui.button("Remove photo", remove), ui.button("Browse files", browse)]), False, False, 0)

    body.pack_start(ui.section("Account settings"), False, False, 0)
    e = ui.entry(ui.prop(u, "RealName") or "", "Your name", 240)

    def save_name():
        name = e.get_text().strip()
        if name:
            ui.call(u, "SetRealName", "(s)", name, done=lambda ok, err: (
                app.account_name.set_text(name), app.refresh()) if ok else ui.error(app.win, "Couldn\u2019t change your name", str(err)))
    body.pack_start(ui.row("Your name", "Shown on Start, the sign-in screen and in Settings",
                           "avatar-default-symbolic", [e, ui.button("Save", save_name)]), False, False, 0)
    body.pack_start(ui.row("Email", ui.prop(u, "Email") or "Not set", "mail-unread-symbolic"), False, False, 0)


# --- Sign-in options ------------------------------------------------------------------------

def change_password(current, new, done):
    """Run passwd on a pseudo-terminal, answering its prompts."""
    pid, fd = pty.fork()
    if pid == 0:
        os.execvp("passwd", ["passwd"])
    answers = [current, new, new]
    output = b""
    deadline = time.time() + 30
    while time.time() < deadline:
        r, _w, _x = select.select([fd], [], [], 0.5)
        if fd in r:
            try:
                chunk = os.read(fd, 1024)
            except OSError:
                break
            if not chunk:
                break
            output += chunk
            if chunk.rstrip().endswith(b":") and answers:
                os.write(fd, answers.pop(0).encode() + b"\n")
    _pid, status = os.waitpid(pid, 0)
    text = output.decode(errors="ignore").strip().splitlines()
    message = next((l for l in reversed(text) if l.strip() and not l.strip().endswith(":")), "")
    done(os.waitstatus_to_exitcode(status) == 0, message)


def build_signin(app, body):
    body.pack_start(ui.section("Ways to sign in"), False, False, 0)
    exp, inner = ui.expander("Password", "Sign in with your account\u2019s password", "dialog-password-symbolic")
    cur = ui.entry("", "Current password", 260, password=True)
    new = ui.entry("", "New password", 260, password=True)
    again = ui.entry("", "Confirm password", 260, password=True)
    status = ui.label("", "caption", wrap=True)

    def go():
        if new.get_text() != again.get_text():
            status.set_text("The passwords don\u2019t match.")
            return
        if not new.get_text():
            status.set_text("Enter a new password.")
            return
        status.set_text("Changing your password\u2026")

        def finished(ok, msg):
            GLib.idle_add(lambda: (status.set_text("Your password was changed." if ok else
                                                   (msg or "The password wasn\u2019t changed.")),
                                   [w.set_text("") for w in (cur, new, again)]) and False)
        threading.Thread(target=change_password, args=(cur.get_text(), new.get_text(), finished),
                         daemon=True).start()
    for w, t in ((cur, "Current password"), (new, "New password"), (again, "Confirm password")):
        inner.pack_start(ui.row(t, None, None, w), False, False, 0)
    foot = ui.css(Gtk.Box(spacing=12), "card")
    foot.pack_start(status, True, True, 0)
    foot.pack_end(ui.button("Change", go, accent=True), False, False, 0)
    inner.pack_start(foot, False, False, 0)
    body.pack_start(exp, False, False, 0)

    fp = fingerprint_device()
    if fp:
        user = pwd.getpwuid(os.getuid()).pw_name
        fingers = []
        try:
            fingers = fp.call_sync("ListEnrolledFingers", GLib.Variant("(s)", (user,)), 0, 5000, None).unpack()[0]
        except GLib.Error:
            pass
        exp, inner = ui.expander("Fingerprint recognition",
                                 f"{len(fingers)} finger{'s' if len(fingers) != 1 else ''} set up" if fingers
                                 else "Sign in and unlock with your fingerprint", "fingerprint-symbolic")
        inner.pack_start(ui.row("Add a finger", None, None,
                                ui.button("Set up", lambda: enroll(app, fp, user), accent=True)), False, False, 0)
        if fingers:
            def delete():
                def go():
                    try:
                        fp.call_sync("Claim", GLib.Variant("(s)", (user,)),
                                     ui.Gio.DBusCallFlags.ALLOW_INTERACTIVE_AUTHORIZATION, 30000, None)
                        fp.call_sync("DeleteEnrolledFingers2", None,
                                     ui.Gio.DBusCallFlags.ALLOW_INTERACTIVE_AUTHORIZATION, 30000, None)
                    except GLib.Error as e:
                        ui.error(app.win, "Couldn\u2019t remove fingerprints", e.message)
                    finally:
                        try:
                            fp.call_sync("Release", None, 0, 5000, None)
                        except GLib.Error:
                            pass
                    app.refresh()
                ui.confirm(app.win, "Remove all fingerprints?", "You\u2019ll sign in with your password.",
                           "Remove", go)
            inner.pack_start(ui.row("Remove all fingerprints", ", ".join(f.replace("-", " ") for f in fingers),
                                    None, ui.button("Remove", delete)), False, False, 0)
        body.pack_start(exp, False, False, 0)

    body.pack_start(ui.section("Additional settings"), False, False, 0)
    s = ui.load()

    def set_lock(v):
        ui.save(lock_on_sleep=v)
        ui.ok(["pkill", "-u", str(os.getuid()), "-x", "swayidle"])
    body.pack_start(ui.row("Require sign-in when the PC wakes from sleep", None, "system-lock-screen-symbolic",
                           ui.switch(s["lock_on_sleep"], set_lock)), False, False, 0)
    u = me()
    auto = ui.prop(u, "AutomaticLogin", False)
    body.pack_start(ui.row("Automatic sign-in", "Skip the sign-in screen when the PC starts", "system-users-symbolic",
                           ui.switch(auto, lambda v: ui.call(u, "SetAutomaticLogin", "(b)", v, done=lambda ok, e: ok or
                                                             (ui.error(app.win, "Automatic sign-in", str(e)), app.refresh())))),
                    False, False, 0)
    body.pack_start(ui.nav_row("Screen and sleep", "Choose when your PC locks", "system-lock-screen",
                               lambda: app.go("power")), False, False, 0)


def fingerprint_device():
    mgr = ui.proxy("net.reactivated.Fprint", "/net/reactivated/Fprint/Manager", "net.reactivated.Fprint.Manager")
    if not mgr:
        return None
    try:
        path = mgr.call_sync("GetDefaultDevice", None, 0, 3000, None).unpack()[0]
    except GLib.Error:
        return None
    return ui.proxy("net.reactivated.Fprint", path, "net.reactivated.Fprint.Device")


def enroll(app, fp, user):
    d = ui.Dialog(app.win, "Set up fingerprint", None, "Cancel")
    msg = ui.label("Touch the fingerprint sensor.", wrap=True)
    bar = Gtk.ProgressBar()
    d.body.pack_start(ui.icon("fingerprint-symbolic", 64), False, False, 8)
    d.body.pack_start(msg, False, False, 0)
    d.body.pack_start(bar, False, False, 0)
    state = {"n": 0, "done": False}
    stages = ui.prop(fp, "num-enroll-stages", 5) or 5
    texts = {"enroll-stage-passed": "Lift your finger, then touch the sensor again.",
             "enroll-retry-scan": "Try again.", "enroll-swipe-too-short": "Swipe again, a bit slower.",
             "enroll-finger-not-centered": "Centre your finger on the sensor.",
             "enroll-remove-and-retry": "Lift your finger and try again.",
             "enroll-completed": "All set! You can now sign in with your fingerprint.",
             "enroll-failed": "Fingerprint setup failed.", "enroll-data-full": "The sensor is full.",
             "enroll-duplicate": "This finger is already set up."}

    def on_signal(_p, _sender, name, params):
        if name != "EnrollStatus":
            return
        result, done = params.unpack()
        if result == "enroll-stage-passed":
            state["n"] += 1
            bar.set_fraction(min(1, state["n"] / stages))
        msg.set_text(texts.get(result, result))
        if done:
            state["done"] = True
            bar.set_fraction(1 if result == "enroll-completed" else bar.get_fraction())
            stop()
    handler = fp.connect("g-signal", on_signal)

    def stop():
        for m in ("EnrollStop", "Release"):
            try:
                fp.call_sync(m, None, 0, 5000, None)
            except GLib.Error:
                pass

    def closed(*_):
        fp.disconnect(handler)
        if not state["done"]:
            stop()
        app.refresh()
    d.connect("destroy", closed)
    d.run_dialog()
    finger = "right-index-finger"
    try:
        fp.call_sync("Claim", GLib.Variant("(s)", (user,)),
                     ui.Gio.DBusCallFlags.ALLOW_INTERACTIVE_AUTHORIZATION, 30000, None)
        fp.call_sync("EnrollStart", GLib.Variant("(s)", (finger,)),
                     ui.Gio.DBusCallFlags.ALLOW_INTERACTIVE_AUTHORIZATION, 30000, None)
    except GLib.Error as e:
        msg.set_text(e.message.split(":")[-1].strip() or e.message)
        state["done"] = True


# --- Other users --------------------------------------------------------------------------------

def crypt_password(password):
    try:
        out = subprocess.run(["openssl", "passwd", "-6", "-stdin"], input=password, capture_output=True,
                             text=True, timeout=10).stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        out = ""
    return out


def build_others(app, body):
    acc = accounts()
    try:
        paths = acc.call_sync("ListCachedUsers", None, 0, 5000, None).unpack()[0] if acc else []
    except GLib.Error:
        paths = []
    mine = f"/org/freedesktop/Accounts/User{os.getuid()}"

    def add():
        d = ui.Dialog(app.win, "Create a user for this PC", "Create")
        user = d.add_field("User name", ui.entry("", "For example, alex", 360))
        full = d.add_field("Full name", ui.entry("", "Alex Smith", 360))
        kind = {"v": 0}
        d.add_field("Account type", ui.combo(TYPES, 0, lambda v: kind.__setitem__("v", v), 360))
        pw = d.add_field("Password", ui.entry("", "Make it hard to guess", 360, password=True))
        note = ui.label("", "caption", wrap=True)
        d.body.pack_start(note, False, False, 0)

        def go():
            name = user.get_text().strip().lower()
            if not name or not name.isascii() or not (name[0].isalpha() and name.replace("-", "").replace("_", "").isalnum()):
                note.set_text("Use lower-case letters and numbers, starting with a letter.")
                return
            if not pw.get_text():
                note.set_text("Enter a password.")
                return

            def created(ok, result):
                if not ok:
                    note.set_text(str(result))
                    return
                new = user_proxy(result[0])
                ui.call(new, "SetPassword", "(ss)", crypt_password(pw.get_text()), "",
                        done=lambda *_: (d.destroy(), app.refresh()))
            ui.call(acc, "CreateUser", "(ssi)", name, full.get_text().strip() or name, kind["v"], done=created)
        d.on_primary(go)
        d.run_dialog()
    body.pack_start(ui.row("Add other user", None, "list-add-symbolic",
                           ui.button("Add account", add, accent=True)), False, False, 8)
    body.pack_start(ui.section("Other users"), False, False, 0)
    others = [user_proxy(p) for p in paths if p != mine]
    if not others:
        body.pack_start(ui.label("No other users are set up on this PC.", "caption"), False, False, 0)
    for u in others:
        if not u:
            continue
        exp, inner = ui.expander(display_name(u), f"{ui.prop(u, 'UserName')}  \u00b7  {type_name(u)}",
                                 "avatar-default-symbolic")
        inner.pack_start(ui.row("Account options", None, None,
                                ui.combo(TYPES, ui.prop(u, "AccountType", 0),
                                         lambda v, u=u: ui.call(u, "SetAccountType", "(i)", v,
                                                                done=lambda *_: app.refresh()), 200)), False, False, 0)

        def remove(u=u):
            uid = ui.prop(u, "Uid")
            ui.confirm(app.win, f"Delete {display_name(u)}\u2019s account and data?",
                       "Deleting this account removes their files, including documents, pictures and "
                       "downloads, from this PC.", "Delete account and data",
                       lambda: ui.call(acc, "DeleteUser", "(xb)", uid, True, done=lambda *_: app.refresh()))
        inner.pack_start(ui.row("Account and data", None, None, ui.button("Remove", remove, danger=True)),
                         False, False, 0)
        body.pack_start(exp, False, False, 0)


def build_accounts(app, body):
    u = me()
    hero = ui.css(Gtk.Box(spacing=20), "hero")
    hero.pack_start(ui.Avatar(96), False, False, 0)
    t = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.CENTER)
    t.pack_start(ui.label(display_name(u), "hero-title"), False, False, 0)
    t.pack_start(ui.label(ui.prop(u, "UserName") or "", "caption"), False, False, 0)
    t.pack_start(ui.label(f"Local account  \u00b7  {type_name(u)}", "caption"), False, False, 0)
    hero.pack_start(t, True, True, 0)
    body.pack_start(hero, False, False, 8)


CATEGORY = {"accounts": build_accounts}

PAGES = [
    dict(key="yourinfo", title="Your info", parent="accounts", icon="avatar-default",
         caption="Profile photo, name", keywords="picture photo avatar name account",
         build=build_yourinfo, aliases=("your-info",)),
    dict(key="signin", title="Sign-in options", parent="accounts", icon="dialog-password",
         caption="Password, fingerprint, automatic sign-in",
         keywords="password fingerprint login lock autologin", build=build_signin,
         aliases=("signinoptions", "password")),
    dict(key="otherusers", title="Other users", parent="accounts", icon="system-users",
         caption="Add, remove and manage other user accounts", keywords="users family add account delete",
         build=build_others, aliases=("otherusers", "family-group")),
]
