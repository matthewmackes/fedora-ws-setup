# Settings > Advanced: every settings and system tool installed on this PC,
# as an icon grid grouped like the Control Panel "All Tasks" view, plus apps
# moved here from the Start menu All apps list.
from gi.repository import Gdk, GLib, Gtk

import advanced_list
import wsui as ui

GROUPS = advanced_list.GROUPS


def group_of(cats):
    for title, keys in GROUPS:
        if keys & cats:
            return title
    return "Other settings"


def build_advanced(app, body):
    body.pack_start(ui.label("Every settings and system tool installed on this PC, for options that go "
                             "beyond Settings. Apps you move here from Start are listed first.",
                             "caption", wrap=True), False, False, 8)
    search = Gtk.SearchEntry(placeholder_text="Search tools")
    search.set_size_request(280, -1)
    search.set_halign(Gtk.Align.START)
    body.pack_start(search, False, False, 8)
    holder = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    body.pack_start(holder, False, False, 0)
    automatic = advanced_list.automatic_entries()

    def ask_remove(desktop_app):
        name = desktop_app.get_display_name()

        def remove():
            advanced_list.remove_moved(desktop_app.get_id() or name)
            fill(search.get_text())

        ui.confirm(app.win, "Remove this app from Advanced?",
                   f"{name} will return to the All apps list.",
                   "Remove", remove)

    def on_right_click(button, event, desktop_app):
        if event.button != 3:
            return False
        menu = Gtk.Menu()
        item = Gtk.MenuItem(label="Remove from Advanced")
        item.connect("activate", lambda *_a, target=desktop_app: GLib.idle_add(lambda: ask_remove(target) or False))
        menu.append(item)
        menu.show_all()
        menu.popup_at_widget(button, Gdk.Gravity.NORTH_WEST, Gdk.Gravity.SOUTH_WEST, event)
        return True

    def add_button(grid, desktop_app, removable):
        button = ui.css(Gtk.Button(tooltip_text=desktop_app.get_description() or desktop_app.get_display_name()),
                        "tool")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.pack_start(ui.icon(desktop_app.get_icon() or "applications-system", 32), False, False, 0)
        lab = ui.label(desktop_app.get_display_name(), xalign=0.5, wrap=True)
        lab.set_justify(Gtk.Justification.CENTER)
        lab.set_max_width_chars(14)
        box.pack_start(lab, False, False, 0)
        button.add(box)
        button.connect("clicked", lambda _b, target=desktop_app: ui.spawn(["gtk-launch", target.get_id()])
                       if target.get_id() else ui.spawn(target.get_commandline()))
        if removable:
            button.connect("button-press-event", lambda b, event, target=desktop_app: on_right_click(b, event, target))
        grid.add(button)

    def fill(q=""):
        ui.clear(holder)
        moved = advanced_list.moved_apps()
        moved_ids = {desktop_app.get_id() for desktop_app in moved if desktop_app.get_id()}
        groups = {}
        matched = []
        for desktop_app in moved:
            hay = f"{desktop_app.get_display_name()} {desktop_app.get_description() or ''} " \
                  f"{' '.join(desktop_app.get_keywords() or [])}"
            if q.casefold() in hay.casefold():
                matched.append(desktop_app)
        if matched:
            groups["Moved from Start"] = matched
        for desktop_app, cats in automatic:
            if desktop_app.get_id() and desktop_app.get_id() in moved_ids:
                continue
            hay = f"{desktop_app.get_display_name()} {desktop_app.get_description() or ''} " \
                  f"{' '.join(desktop_app.get_keywords() or [])}"
            if q.casefold() in hay.casefold():
                groups.setdefault(group_of(cats), []).append(desktop_app)
        order = ["Moved from Start"] + [title for title, _keys in GROUPS] + ["Other settings"]
        for title in order:
            if title not in groups:
                continue
            holder.pack_start(ui.section(title), False, False, 0)
            grid = Gtk.FlowBox(max_children_per_line=6, min_children_per_line=3, homogeneous=True,
                               selection_mode=Gtk.SelectionMode.NONE, column_spacing=4, row_spacing=4)
            for desktop_app in groups[title]:
                add_button(grid, desktop_app, title == "Moved from Start")
            card = ui.css(Gtk.Box(), "card", "tall")
            card.pack_start(grid, True, True, 0)
            holder.pack_start(card, False, False, 0)
        if not groups:
            holder.pack_start(ui.label("No tools match.", "caption"), False, False, 0)
        holder.show_all()

    search.connect("search-changed", lambda entry: fill(entry.get_text()))
    fill()


CATEGORY = {"advanced": build_advanced}
PAGES = []
