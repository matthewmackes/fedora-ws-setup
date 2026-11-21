# Settings > Personalization > Window Layout: Sway window manager controls with
# a Fluent hero preview. Changes stage until Apply; org.mde.window-layout is
# the source of truth and Apply writes Sway via ui.sway_conf.
import copy
import os

import mde_gsettings as gs
import ws_theme
import wsui as ui
from gi.repository import Gtk  # noqa: E402  — after wsui selects Gtk 3
from p_personal import swatch_card

LAYOUTS = [
    ("default", "Side by side", "Windows share the desktop side by side"),
    ("stacking", "Stacked", "Windows stack on top of each other"),
    ("tabbed", "Tabs", "Windows share a tab strip, like a browser"),
]

FLOAT_APP_IDS = [
    "ws-settings", "ws-mmc", "org.pulseaudio.pavucontrol", "pavucontrol",
    "blueman-manager", "blueman-adapters", "nm-connection-editor",
    "org.gnome.Calculator", "gnome-calculator", "galculator", "qalculate-gtk",
    "swappy", "wdisplays", "nwg-look", "xfce4-appfinder",
]

PRESETS = {
    "win11": {
        "title": "Windows-like",
        "caption": "Dialogs float; Settings and system tools float",
        "dialogs": True,
        "apps": list(FLOAT_APP_IDS),
    },
    "dialogs": {
        "title": "Dialogs only",
        "caption": "Only pop-ups and dialog windows float",
        "dialogs": True,
        "apps": [],
    },
    "power": {
        "title": "Power user",
        "caption": "Dialogs float; no built-in app list — use custom rules",
        "dialogs": True,
        "apps": [],
    },
    "custom": {
        "title": "Custom",
        "caption": "Use the list below",
        "dialogs": True,
        "apps": [],
    },
}

RULES_FILE = os.path.join(ui.CONFIG, "mde", "sway", "config.d", "61-ws-window-rules.conf")

# Module-level staging shared by Window Layout and Window rules while Settings is open.
_STAGED = None
_BASELINE = None
_DIRTY = False
_REFRESH = None


def _hex_rgb(color):
    c = (color or "#808080").lstrip("#")
    if len(c) != 6:
        return 0.5, 0.5, 0.5
    return tuple(int(c[i:i + 2], 16) / 255 for i in (0, 2, 4))


def client_line(state, color, dark=False):
    """Expand a primary colour into a Sway client.* line."""
    fill = color or "#808080"
    if state == "focused":
        fg = "#ffffff"
        bg = fill
    elif state == "urgent":
        fg = "#ffffff"
        bg = fill
    else:
        fg = "#ffffff" if dark else "#1a1a1a"
        bg = "#202020" if dark else "#f3f3f3"
    return f"client.{state} {fill} {bg} {fg} {fill} {fill}"


def ensure_loaded():
    global _STAGED, _BASELINE, _DIRTY
    if _STAGED is not None:
        return _STAGED
    gs.migrate_window_layout_from_settings_json(ui.load)
    data = gs.window_layout_load()
    _STAGED = copy.deepcopy(data)
    _BASELINE = copy.deepcopy(data)
    _DIRTY = False
    return _STAGED


def staged():
    return ensure_loaded()


def is_dirty():
    ensure_loaded()
    return _DIRTY


def mark_dirty():
    global _DIRTY
    _DIRTY = True
    if _REFRESH:
        _REFRESH()


def set_staged(key, value):
    ensure_loaded()
    _STAGED[key] = value
    mark_dirty()


def discard_staged():
    global _STAGED, _DIRTY
    ensure_loaded()
    _STAGED = copy.deepcopy(_BASELINE)
    _DIRTY = False


def reset_to_defaults():
    global _STAGED, _DIRTY
    ensure_loaded()
    _STAGED = gs.window_layout_defaults()
    _STAGED["migrated"] = True
    accent = ui.load().get("accent") or gs.accent() or "#0078d4"
    _STAGED["color-focused"] = accent
    _DIRTY = True


def reset_colours_to_accent():
    ensure_loaded()
    accent = ui.load().get("accent") or gs.accent() or "#0078d4"
    muted = "#68768a" if ws_theme.is_dark() else "#808080"
    set_staged("color-focused", accent)
    set_staged("color-focused-inactive", muted)
    set_staged("color-unfocused", muted)
    set_staged("color-urgent", "#ff3b30")


def rules_lines(state):
    lines = []
    preset = PRESETS.get(state.get("rules-preset"), PRESETS["win11"])
    if preset.get("dialogs"):
        lines += [
            'for_window [window_role="pop-up"] floating enable',
            'for_window [window_role="dialog"] floating enable',
            'for_window [window_type="dialog"] floating enable',
        ]
    apps = list(preset.get("apps") or [])
    custom = []
    for entry in state.get("custom-rules") or []:
        if ":" not in entry:
            continue
        app_id, _, action = entry.partition(":")
        app_id, action = app_id.strip(), action.strip().lower()
        if not app_id:
            continue
        if action == "tiling":
            custom.append((app_id, "floating disable"))
        else:
            custom.append((app_id, "floating enable"))
            apps = [a for a in apps if a.casefold() != app_id.casefold()]
    for app_id in apps:
        lines.append(f'for_window [app_id="(?i)^{app_id}$"] floating enable')
    for app_id, cmd in custom:
        lines.append(f'for_window [app_id="(?i)^{app_id}$"] {cmd}')
    lines += [
        'for_window [app_id=".*"] inhibit_idle fullscreen',
        'for_window [class=".*"] inhibit_idle fullscreen',
    ]
    if state.get("new-windows-floating"):
        lines.insert(0, 'for_window [app_id=".*"] floating enable')
        lines.insert(1, 'for_window [class=".*"] floating enable')
    return lines


def apply_to_desktop(state=None):
    """Persist staged settings to gsettings and emit Sway config."""
    global _STAGED, _BASELINE, _DIRTY
    state = copy.deepcopy(state or ensure_loaded())
    state["migrated"] = True
    gs.window_layout_save(state)

    style = state["titlebar-style"]
    tiled = int(state["border-tiled"])
    floating = int(state["border-floating"])
    if style == "normal":
        ui.sway_conf("default_border", "default_border normal")
        ui.sway_conf("default_floating_border", "default_floating_border normal")
    else:
        ui.sway_conf("default_border", f"default_border pixel {tiled}")
        ui.sway_conf("default_floating_border", f"default_floating_border pixel {floating}")
    # Titles sit on the left so the corner buttons do not cover them.
    if state.get("show-window-buttons", True):
        ui.sway_conf("title_align", "title_align left")
    else:
        ui.sway_conf("title_align", "title_align center")

    ui.sway_conf("smart_borders", f"smart_borders {state['smart-borders']}")
    ui.sway_conf("gaps_inner", f"gaps inner {int(state['gaps-inner'])}")
    ui.sway(f"gaps inner all set {int(state['gaps-inner'])}")
    ui.sway_conf("gaps_outer", f"gaps outer {int(state['gaps-outer'])}")
    ui.sway(f"gaps outer all set {int(state['gaps-outer'])}")
    ui.sway_conf("smart_gaps", f"smart_gaps {'on' if state['smart-gaps'] else 'off'}")
    ui.sway_conf("default_layout", f"workspace_layout {state['default-layout']}")
    ui.sway_conf("focus_follows_mouse", f"focus_follows_mouse {state['focus-follows-mouse']}")
    ui.sway_conf("mouse_warping", f"mouse_warping {state['mouse-warping']}")
    ui.sway_conf("workspace_back_and_forth",
                 f"workspace_auto_back_and_forth {'yes' if state['workspace-back-and-forth'] else 'no'}")
    ui.sway_conf("focus_on_activation",
                 f"focus_on_window_activation {state['focus-on-activation']}")
    if state["floating-modifier"]:
        ui.sway_conf("floating_modifier", "floating_modifier Mod4 normal")
    else:
        ui.sway_conf("floating_modifier", "floating_modifier none")

    dark = ws_theme.is_dark()
    for key, state_name in (
            ("color-focused", "focused"),
            ("color-focused-inactive", "focused_inactive"),
            ("color-unfocused", "unfocused"),
            ("color-urgent", "urgent")):
        line = client_line(state_name, state[key], dark=dark)
        # client.focused_inactive uses underscore in Sway.
        sway_state = state_name
        ui.sway_conf(f"client_{sway_state}", line.replace(f"client.{state_name}", f"client.{sway_state}"))

    # Keep settings.json taskbar flag in sync for the shell.
    ui.save(taskbar_all_desktops=bool(state["taskbar-all-desktops"]),
            focus_follows_mouse=state["focus-follows-mouse"],
            mouse_warping=state["mouse-warping"],
            workspace_back_and_forth=bool(state["workspace-back-and-forth"]),
            default_layout=state["default-layout"],
            gaps=int(state["gaps-inner"]),
            smart_gaps=bool(state["smart-gaps"]))
    ws_theme.apply_taskbar()

    text = "# Window rules from Settings > Personalization > Window Layout.\n"
    text += "\n".join(rules_lines(state)) + "\n"
    ui.write_file(RULES_FILE, text)
    for line in rules_lines(state):
        ui.sway(line)

    _STAGED = copy.deepcopy(state)
    _BASELINE = copy.deepcopy(state)
    _DIRTY = False


_LAYOUT_PAGES = frozenset({"windowlayout", "windowrules"})


def leave_guard(app, cont, next_key=None):
    """If staging is dirty, prompt Apply / Discard / Cancel. Return True if blocked."""
    if not is_dirty():
        return False
    if next_key in _LAYOUT_PAGES:
        return False
    d = ui.Dialog(app.win, "Apply these window layout settings?", None, None, 480)
    d.add_text("You have changes that have not been applied to the desktop.")
    foot = d.get_children()[0].get_children()[-1]
    apply_b = ui.css(Gtk.Button(label="Apply"), "btn", "accent")
    discard_b = ui.css(Gtk.Button(label="Discard"), "btn")
    cancel_b = ui.css(Gtk.Button(label="Cancel"), "btn")
    apply_b.connect("clicked", lambda *_: (d.destroy(), apply_to_desktop(), cont()))
    discard_b.connect("clicked", lambda *_: (d.destroy(), discard_staged(), cont()))
    cancel_b.connect("clicked", lambda *_: d.destroy())
    for b in (apply_b, discard_b, cancel_b):
        foot.pack_start(b, True, True, 0)
    d.run_dialog()
    return True


def attach_leave_guard(app):
    def guard(cont, next_key=None):
        return leave_guard(app, cont, next_key)
    app.leave_guard = guard

    def clear():
        if getattr(app, "leave_guard", None) is guard:
            app.leave_guard = None
    app.cleanups.append(clear)


# --- Hero preview ------------------------------------------------------------

def hero_preview(get_state):
    area = Gtk.DrawingArea()
    area.set_size_request(320, 180)

    def draw(_a, cr):
        s = get_state()
        w, h = area.get_allocated_width(), area.get_allocated_height()
        cr.set_source_rgb(0.92, 0.92, 0.93)
        cr.rectangle(0, 0, w, h)
        cr.fill()
        gap = max(2, int(s.get("gaps-inner", 8)) // 2)
        border = max(1, int(s.get("border-tiled", 1)))
        focused = _hex_rgb(s.get("color-focused"))
        inactive = _hex_rgb(s.get("color-focused-inactive"))
        title = s.get("titlebar-style") == "normal"
        buttons = s.get("show-window-buttons", True)
        layout = s.get("default-layout", "default")
        # Desktop inset
        x0, y0, dw, dh = 12, 12, w - 24, h - 36
        cr.set_source_rgb(0.85, 0.85, 0.87)
        cr.rectangle(x0, y0, dw, dh)
        cr.fill()

        def win(x, y, ww, hh, color, active=False):
            cr.set_source_rgb(*color)
            cr.rectangle(x, y, ww, hh)
            cr.fill()
            inset = border
            cr.set_source_rgb(0.98, 0.98, 0.98)
            cr.rectangle(x + inset, y + inset + (14 if title else 0),
                         max(1, ww - 2 * inset), max(1, hh - 2 * inset - (14 if title else 0)))
            cr.fill()
            if title:
                cr.set_source_rgb(*color)
                cr.rectangle(x, y, ww, 14)
                cr.fill()
                cr.set_source_rgb(1, 1, 1)
                cr.rectangle(x + 6, y + 5, 20, 4)
                cr.fill()
            if active and layout == "tabbed":
                cr.set_source_rgb(*color)
                cr.rectangle(x + 4, y + 2, 36, 10)
                cr.fill()
            if active and buttons:
                cr.set_source_rgb(1, 1, 1) if title else cr.set_source_rgb(0.15, 0.15, 0.15)
                mark_y = y + 5 if title else y + 7
                right = x + ww - 5
                for i in range(4):
                    mx = right - (3 - i) * 7
                    if i == 3:
                        cr.set_line_width(1)
                        cr.move_to(mx, mark_y)
                        cr.line_to(mx + 4, mark_y + 4)
                        cr.move_to(mx + 4, mark_y)
                        cr.line_to(mx, mark_y + 4)
                        cr.stroke()
                    else:
                        cr.rectangle(mx, mark_y + (1 if i == 0 else 0), 4, 1 if i == 0 else 4)
                        cr.fill()

        if layout == "tabbed":
            win(x0 + gap, y0 + gap, dw - 2 * gap, dh - 2 * gap, focused, True)
        elif layout == "stacking":
            win(x0 + gap + 10, y0 + gap + 10, dw - 2 * gap - 10, dh - 2 * gap - 10, inactive)
            win(x0 + gap, y0 + gap, dw - 2 * gap - 10, dh - 2 * gap - 10, focused, True)
        else:
            half = (dw - 3 * gap) // 2
            win(x0 + gap, y0 + gap, half, dh - 2 * gap, focused, True)
            win(x0 + 2 * gap + half, y0 + gap, half, dh - 2 * gap, inactive)
        # Taskbar strip
        cr.set_source_rgb(0.95, 0.95, 0.96)
        cr.rectangle(0, h - 22, w, 22)
        cr.fill()
        cr.set_source_rgb(*focused)
        cr.rectangle(w / 2 - 40, h - 16, 80, 10)
        cr.fill()
        return False

    area.connect("draw", draw)
    return area


def layout_cards(state, on_change):
    row = Gtk.Box(spacing=12, homogeneous=True)
    group = None
    for key, title, caption in LAYOUTS:
        card = ui.css(Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8), "card", "tall")
        radio = Gtk.RadioButton.new_with_label_from_widget(group, title)
        group = group or radio
        radio.set_active(state["default-layout"] == key)
        radio.connect("toggled", lambda r, k=key: r.get_active() and on_change(k))
        card.pack_start(radio, False, False, 0)
        card.pack_start(ui.label(caption, "caption", wrap=True), False, False, 0)
        # Thumbnail
        thumb = Gtk.DrawingArea()
        thumb.set_size_request(-1, 56)

        def paint(_a, cr, k=key):
            tw = thumb.get_allocated_width()
            th = thumb.get_allocated_height()
            cr.set_source_rgb(0.9, 0.9, 0.92)
            cr.rectangle(0, 0, tw, th)
            cr.fill()
            cr.set_source_rgb(0.0, 0.47, 0.83)
            if k == "default":
                cr.rectangle(4, 4, tw / 2 - 6, th - 8)
                cr.fill()
                cr.set_source_rgb(0.5, 0.5, 0.55)
                cr.rectangle(tw / 2 + 2, 4, tw / 2 - 6, th - 8)
                cr.fill()
            elif k == "stacking":
                cr.set_source_rgb(0.5, 0.5, 0.55)
                cr.rectangle(10, 8, tw - 18, th - 14)
                cr.fill()
                cr.set_source_rgb(0.0, 0.47, 0.83)
                cr.rectangle(4, 4, tw - 18, th - 14)
                cr.fill()
            else:
                cr.rectangle(4, 14, tw - 8, th - 18)
                cr.fill()
                cr.set_source_rgb(0.0, 0.47, 0.83)
                cr.rectangle(6, 4, 28, 10)
                cr.fill()
                cr.set_source_rgb(0.5, 0.5, 0.55)
                cr.rectangle(36, 4, 28, 10)
                cr.fill()
            return False
        thumb.connect("draw", paint)
        card.pack_start(thumb, False, False, 0)
        # Make the whole card clickable
        ev = Gtk.EventBox()
        ev.add(card)
        ev.connect("button-press-event", lambda _e, _ev, k=key, r=radio: (r.set_active(True), on_change(k)) or True)
        row.pack_start(ev, True, True, 0)
    return row


def build_window_layout(app, body):
    global _REFRESH
    ensure_loaded()
    attach_leave_guard(app)

    hero = ui.css(Gtk.Box(spacing=16), "hero")
    preview = hero_preview(staged)
    hero.pack_start(preview, False, False, 0)
    col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, valign=Gtk.Align.CENTER)
    col.pack_start(ui.label("Window layout", "hero-title"), False, False, 0)
    col.pack_start(ui.label("Arrange windows, borders, gaps and focus. Apply to update the desktop.",
                            "caption", wrap=True), False, False, 0)
    actions = Gtk.Box(spacing=8)

    def do_apply():
        apply_to_desktop()
        app.refresh()

    def do_discard():
        discard_staged()
        app.refresh()

    apply_btn = ui.button("Apply preview to desktop", do_apply, accent=True)
    discard_btn = ui.button("Discard", do_discard)
    actions.pack_start(apply_btn, False, False, 0)
    actions.pack_start(discard_btn, False, False, 0)
    col.pack_start(actions, False, False, 0)
    hero.pack_start(col, True, True, 0)
    body.pack_start(hero, False, False, 8)

    def bump():
        preview.queue_draw()
        apply_btn.set_sensitive(is_dirty())
        discard_btn.set_sensitive(is_dirty())

    _REFRESH = bump
    app.cleanups.append(lambda: globals().__setitem__("_REFRESH", None))
    bump()

    s = staged()

    # Arrange windows
    exp, inner = ui.expander("Arrange windows", "How windows share a desktop",
                             "view-grid-symbolic", expanded=True)
    inner.pack_start(layout_cards(s, lambda k: (set_staged("default-layout", k), bump())),
                     False, False, 8)
    inner.pack_start(ui.row("Open new windows floating",
                            "New windows open floating instead of tiled",
                            "window-new-symbolic",
                            ui.switch(s["new-windows-floating"],
                                      lambda v: (set_staged("new-windows-floating", v), bump()))),
                     False, False, 0)
    body.pack_start(exp, False, False, 0)

    # Gaps
    exp, inner = ui.expander("Space between windows", None, "view-paged-symbolic", expanded=True)
    inner.pack_start(ui.row("Space between windows", None, None,
                            ui.slider(s["gaps-inner"], 0, 48,
                                      lambda v: (set_staged("gaps-inner", int(v)), bump()),
                                      fmt=lambda v: f"{int(v)} px")), False, False, 0)
    inner.pack_start(ui.row("Space around the edge", None, None,
                            ui.slider(s["gaps-outer"], 0, 48,
                                      lambda v: (set_staged("gaps-outer", int(v)), bump()),
                                      fmt=lambda v: f"{int(v)} px")), False, False, 0)
    inner.pack_start(ui.row("Hide space with one window",
                            "Remove gaps when a window is alone on the desktop",
                            "view-fullscreen-symbolic",
                            ui.switch(s["smart-gaps"],
                                      lambda v: (set_staged("smart-gaps", v), bump()))),
                     False, False, 0)
    body.pack_start(exp, False, False, 0)

    # Borders and title bars
    exp, inner = ui.expander("Borders and title bars", "Width, style and window colours",
                             "window-new-symbolic", expanded=True)
    inner.pack_start(ui.row("Border width for tiled windows", None, None,
                            ui.slider(s["border-tiled"], 0, 8,
                                      lambda v: (set_staged("border-tiled", int(v)), bump()),
                                      fmt=lambda v: f"{int(v)} px")), False, False, 0)
    inner.pack_start(ui.row("Border width for floating windows", None, None,
                            ui.slider(s["border-floating"], 0, 8,
                                      lambda v: (set_staged("border-floating", int(v)), bump()),
                                      fmt=lambda v: f"{int(v)} px")), False, False, 0)
    inner.pack_start(ui.row("Title bars", "Pixel borders hide the title bar",
                            None,
                            ui.combo([("pixel", "Pixel borders"), ("normal", "Title bars")],
                                     s["titlebar-style"],
                                     lambda v: (set_staged("titlebar-style", v), bump()), 180)),
                     False, False, 0)
    inner.pack_start(ui.row("Window buttons",
                            "Minimize, maximize, tile or float, and close, on the focused window",
                            "window-maximize-symbolic",
                            ui.switch(s.get("show-window-buttons", True),
                                      lambda v: (set_staged("show-window-buttons", v), bump()))),
                     False, False, 0)
    inner.pack_start(ui.row("Smart borders", "Hide borders when they are not needed",
                            None,
                            ui.combo([("off", "Off"), ("on", "On"), ("no_gaps", "When gaps are off")],
                                     s["smart-borders"],
                                     lambda v: (set_staged("smart-borders", v), bump()), 180)),
                     False, False, 0)

    def colour_pick(key):
        def pick(c):
            set_staged(key, c)
            bump()
            app.refresh()
        return pick

    for key, title in (
            ("color-focused", "Focused window"),
            ("color-focused-inactive", "Inactive window"),
            ("color-unfocused", "Unfocused window"),
            ("color-urgent", "Urgent window")):
        inner.pack_start(ui.label(title, "caption"), False, False, 4)
        inner.pack_start(swatch_card(title, s[key], colour_pick(key), app), False, False, 0)

    inner.pack_start(ui.row("Reset window colours to theme accent",
                            "Use your accent colour for the focused window",
                            "preferences-color-symbolic",
                            ui.button("Reset colours", lambda: (reset_colours_to_accent(), app.refresh()))),
                     False, False, 0)
    body.pack_start(exp, False, False, 0)

    # Focus
    exp, inner = ui.expander("Focus and pointer", None, "input-mouse-symbolic")
    inner.pack_start(ui.row("Focus follows the mouse",
                            "Windows get focus when you point at them",
                            "input-mouse-symbolic",
                            ui.switch(s["focus-follows-mouse"] == "yes",
                                      lambda v: (set_staged("focus-follows-mouse", "yes" if v else "no"),
                                                 bump()))), False, False, 0)
    inner.pack_start(ui.row("Move the pointer with the focus",
                            "When you switch windows with the keyboard, the pointer follows",
                            "input-mouse-symbolic",
                            ui.switch(s["mouse-warping"] != "none",
                                      lambda v: (set_staged("mouse-warping", "output" if v else "none"),
                                                 bump()))), False, False, 0)
    inner.pack_start(ui.row("Focus when a window asks for attention",
                            None, None,
                            ui.combo([("focus", "Always focus"), ("urgent", "Mark urgent"),
                                      ("smart", "Smart"), ("none", "Do nothing")],
                                     s["focus-on-activation"],
                                     lambda v: (set_staged("focus-on-activation", v), bump()), 160)),
                     False, False, 0)
    inner.pack_start(ui.row("Drag floating windows with Super",
                            "Hold the Windows key and drag to move a floating window",
                            None,
                            ui.switch(s["floating-modifier"],
                                      lambda v: (set_staged("floating-modifier", v), bump()))),
                     False, False, 0)
    body.pack_start(exp, False, False, 0)

    # Desktops
    exp, inner = ui.expander("Desktops", None, "view-dual-symbolic")
    inner.pack_start(ui.row("On the taskbar, show all the open windows", None, "view-dual-symbolic",
                            ui.combo([(False, "Only on the desktop I'm using"),
                                      (True, "On all desktops")],
                                     s["taskbar-all-desktops"],
                                     lambda v: (set_staged("taskbar-all-desktops", v), bump()), 240)),
                     False, False, 0)
    inner.pack_start(ui.row("Go back to the previous desktop",
                            "Pressing the shortcut for the current desktop switches to the last one",
                            "view-dual-symbolic",
                            ui.switch(s["workspace-back-and-forth"],
                                      lambda v: (set_staged("workspace-back-and-forth", v), bump()))),
                     False, False, 0)
    body.pack_start(exp, False, False, 0)

    body.pack_start(ui.section("Related settings"), False, False, 0)
    body.pack_start(ui.row("Reset to defaults",
                            "Restore Window Layout defaults. Apply to update the desktop.",
                            "edit-clear-symbolic",
                            ui.button("Reset", lambda: (reset_to_defaults(), app.refresh()))),
                     False, False, 0)


def build_window_rules(app, body):
    ensure_loaded()
    attach_leave_guard(app)
    s = staged()

    body.pack_start(ui.label("Choose how windows open floating or tiled. Apply changes from "
                             "Window Layout.", "caption", wrap=True), False, False, 8)

    body.pack_start(ui.section("Presets"), False, False, 0)

    def set_preset(key):
        set_staged("rules-preset", key)
        app.refresh()

    group = None
    for key, spec in PRESETS.items():
        radio = Gtk.RadioButton.new_with_label_from_widget(group, spec["title"])
        group = group or radio
        radio.set_active(s["rules-preset"] == key)
        radio.connect("toggled", lambda r, k=key: r.get_active() and set_preset(k))
        body.pack_start(ui.row(spec["title"], spec["caption"], "view-list-symbolic", radio),
                        False, False, 0)

    body.pack_start(ui.section("Custom rules"), False, False, 0)
    body.pack_start(ui.label("Match an app id and choose floating or tiled.", "caption"),
                    False, False, 4)

    rules = list(s.get("custom-rules") or [])

    def save_rules(new_rules):
        set_staged("custom-rules", new_rules)
        set_staged("rules-preset", "custom")
        app.refresh()

    for i, entry in enumerate(rules):
        app_id, _, action = entry.partition(":")
        box = Gtk.Box(spacing=8)
        box.pack_start(ui.label(app_id or entry), True, True, 0)
        box.pack_start(ui.label(action or "floating", "caption"), False, False, 0)

        def remove(_b, idx=i):
            nxt = [r for j, r in enumerate(staged().get("custom-rules") or []) if j != idx]
            save_rules(nxt)

        box.pack_end(ui.button("Remove", remove), False, False, 0)
        body.pack_start(ui.row("", None, "application-x-executable-symbolic", box), False, False, 0)

    def add_rule():
        d = ui.Dialog(app.win, "Add a window rule", "Add")
        e = d.add_field("App id", ui.entry("", "for example org.gnome.Calculator", 320))
        action = {"v": "floating"}
        d.add_field("Behaviour", ui.combo([("floating", "Floating"), ("tiling", "Tiled")],
                                          "floating", lambda v: action.__setitem__("v", v), 160))

        def go():
            app_id = e.get_text().strip()
            if not app_id:
                return
            d.destroy()
            nxt = list(staged().get("custom-rules") or [])
            nxt.append(f"{app_id}:{action['v']}")
            save_rules(nxt)
        d.on_primary(go)
        d.run_dialog()

    body.pack_start(ui.row("Add an app", "Float or tile a specific app by its app id",
                           "list-add-symbolic", ui.button("Add", add_rule)), False, False, 0)

    actions = Gtk.Box(spacing=8)
    actions.pack_start(ui.button("Apply preview to desktop",
                                 lambda: (apply_to_desktop(), app.refresh()), accent=True),
                       False, False, 0)
    actions.pack_start(ui.button("Discard", lambda: (discard_staged(), app.refresh())),
                       False, False, 0)
    body.pack_start(actions, False, False, 8)


# Page entries live in p_personal.PAGES (after Fonts) so hub order stays correct.
PAGES = []
