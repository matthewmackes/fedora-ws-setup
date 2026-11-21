"""System tray: a StatusNotifierWatcher and host, with dbusmenu menus."""

from common import Gdk, Gio, GLib, Gtk, css_class, load_json, save_json, TRAY_SEEN_FILE

try:
    from gi.repository import GdkPixbuf
except ImportError:
    GdkPixbuf = None

WATCHER = "org.kde.StatusNotifierWatcher"
WATCHER_PATH = "/StatusNotifierWatcher"
ITEM_IFACE = "org.kde.StatusNotifierItem"
MENU_IFACE = "com.canonical.dbusmenu"
WATCHER_XML = """
<node>
  <interface name="org.kde.StatusNotifierWatcher">
    <method name="RegisterStatusNotifierItem"><arg name="service" type="s" direction="in"/></method>
    <method name="RegisterStatusNotifierHost"><arg name="service" type="s" direction="in"/></method>
    <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
    <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
    <property name="ProtocolVersion" type="i" access="read"/>
    <signal name="StatusNotifierItemRegistered"><arg type="s"/></signal>
    <signal name="StatusNotifierItemUnregistered"><arg type="s"/></signal>
    <signal name="StatusNotifierHostRegistered"/>
  </interface>
</node>
"""


class Item:
    """One tray icon: its properties and the button that shows it."""

    def __init__(self, tray, bus_name, path):
        self.tray, self.bus_name, self.path = tray, bus_name, path
        self.key = bus_name + path
        self.props = {}
        self.buttons = []
        self.sub = tray.bus.signal_subscribe(bus_name, ITEM_IFACE, None, path, None,
                                             Gio.DBusSignalFlags.NONE, self.on_signal)
        self.refresh()

    def on_signal(self, *_args):
        GLib.timeout_add(50, self.refresh)

    def refresh(self):
        def done(bus, res):
            try:
                self.props = bus.call_finish(res).unpack()[0]
            except GLib.Error:
                return
            ident = self.ident
            seen = load_json(TRAY_SEEN_FILE, {})
            title = self.props.get("Title") or ident
            if seen.get(ident) != title:
                seen[ident] = title
                save_json(TRAY_SEEN_FILE, seen)
            self.tray.changed()
        self.tray.bus.call(self.bus_name, self.path, "org.freedesktop.DBus.Properties", "GetAll",
                           GLib.Variant("(s)", (ITEM_IFACE,)), None, Gio.DBusCallFlags.NONE, 2000,
                           None, done)
        return False

    @property
    def ident(self):
        return self.props.get("Id") or self.props.get("Title") or self.key

    @property
    def visible(self):
        return bool(self.props) and self.props.get("Status") != "Passive"

    def tooltip(self):
        tip = self.props.get("ToolTip")
        if tip and len(tip) == 4 and (tip[2] or tip[3]):
            return "\n".join(t for t in (tip[2], tip[3]) if t)
        return self.props.get("Title") or self.ident

    def image(self, size, scale):
        p = self.props
        attention = p.get("Status") == "NeedsAttention"
        name = (attention and p.get("AttentionIconName")) or p.get("IconName")
        pixmaps = (attention and p.get("AttentionIconPixmap")) or p.get("IconPixmap")
        theme = Gtk.IconTheme.get_default()
        if p.get("IconThemePath"):
            theme = Gtk.IconTheme()
            theme.set_custom_theme(Gtk.Settings.get_default().props.gtk_icon_theme_name)
            theme.append_search_path(p["IconThemePath"])
        image = Gtk.Image()
        if name:
            if name.startswith("/"):
                try:
                    pix = GdkPixbuf.Pixbuf.new_from_file_at_size(name, size * scale, size * scale)
                    image.set_from_surface(Gdk.cairo_surface_create_from_pixbuf(pix, scale, None))
                    return image
                except GLib.Error:
                    pass
            info = theme.lookup_icon_for_scale(name, size, scale, Gtk.IconLookupFlags.FORCE_SIZE)
            if info:
                try:
                    pix = info.load_icon()
                    image.set_from_surface(Gdk.cairo_surface_create_from_pixbuf(pix, scale, None))
                    return image
                except GLib.Error:
                    pass
        if pixmaps and GdkPixbuf:
            want = size * scale
            w, h, data = min(pixmaps, key=lambda m: abs(m[0] - want))
            rgba = bytearray(len(data))
            rgba[0::4], rgba[1::4], rgba[2::4], rgba[3::4] = data[1::4], data[2::4], data[3::4], data[0::4]
            pix = GdkPixbuf.Pixbuf.new_from_bytes(GLib.Bytes.new(bytes(rgba)), GdkPixbuf.Colorspace.RGB,
                                                  True, 8, w, h, w * 4)
            if w != want:
                pix = pix.scale_simple(want, want, GdkPixbuf.InterpType.BILINEAR)
            image.set_from_surface(Gdk.cairo_surface_create_from_pixbuf(pix, scale, None))
            return image
        image.set_from_icon_name("image-missing", Gtk.IconSize.MENU)
        image.set_pixel_size(size)
        return image

    def button(self, scale):
        b = css_class(Gtk.Button(relief=Gtk.ReliefStyle.NONE, tooltip_text=self.tooltip()), "tray-icon")
        b.add(self.image(16, scale))
        b.add_events(Gdk.EventMask.SCROLL_MASK)
        b.connect("button-release-event", self.on_click)
        b.connect("scroll-event", self.on_scroll)
        return b

    def call(self, method, args, fail=None):
        def done(bus, res):
            try:
                bus.call_finish(res)
            except GLib.Error:
                if fail:
                    fail()
        self.tray.bus.call(self.bus_name, self.path, ITEM_IFACE, method, args, None,
                           Gio.DBusCallFlags.NONE, 2000, None, done)

    def on_click(self, widget, event):
        x, y = int(event.x_root), int(event.y_root)
        if event.button == 1:
            if self.props.get("ItemIsMenu") and self.props.get("Menu"):
                self.show_menu(widget, event)
            else:
                self.call("Activate", GLib.Variant("(ii)", (x, y)),
                          fail=lambda: self.show_menu(widget, event))
        elif event.button == 2:
            self.call("SecondaryActivate", GLib.Variant("(ii)", (x, y)))
        elif event.button == 3:
            if self.props.get("Menu"):
                self.show_menu(widget, event)
            else:
                self.call("ContextMenu", GLib.Variant("(ii)", (x, y)))
        self.tray.clicked()
        return True

    def on_scroll(self, _w, event):
        delta = {Gdk.ScrollDirection.UP: 1, Gdk.ScrollDirection.DOWN: -1}.get(event.direction)
        if delta:
            self.call("Scroll", GLib.Variant("(is)", (delta, "vertical")))
        return True

    def show_menu(self, widget, event):
        DBusMenu(self.tray.bus, self.bus_name, self.props["Menu"]).popup(widget, event)

    def destroy(self):
        self.tray.bus.signal_unsubscribe(self.sub)


class DBusMenu:
    def __init__(self, bus, bus_name, path):
        self.bus, self.bus_name, self.path = bus, bus_name, path

    def call_sync(self, method, args, reply=None):
        try:
            return self.bus.call_sync(self.bus_name, self.path, MENU_IFACE, method, args, reply,
                                      Gio.DBusCallFlags.NONE, 2000, None).unpack()
        except GLib.Error:
            return None

    def popup(self, widget, event):
        self.call_sync("AboutToShow", GLib.Variant("(i)", (0,)))
        layout = self.call_sync("GetLayout", GLib.Variant("(iias)", (0, -1, [])))
        if not layout:
            return
        menu = self.build(layout[1][2])
        css_class(menu, "ws-menu")
        menu.show_all()
        menu.popup_at_widget(widget, Gdk.Gravity.NORTH, Gdk.Gravity.SOUTH, event)

    def build(self, children):
        menu = Gtk.Menu()
        group = None
        for child in children:
            ident, props, sub = child
            if not props.get("visible", True):
                continue
            if props.get("type") == "separator":
                menu.append(Gtk.SeparatorMenuItem())
                group = None
                continue
            text = props.get("label", "")
            toggle = props.get("toggle-type", "")
            if toggle == "checkmark":
                item = Gtk.CheckMenuItem.new_with_mnemonic(text)
                item.set_active(props.get("toggle-state", 0) == 1)
            elif toggle == "radio":
                item = Gtk.RadioMenuItem.new_with_mnemonic_from_widget(group, text)
                group = item
                item.set_active(props.get("toggle-state", 0) == 1)
            else:
                item = Gtk.MenuItem.new_with_mnemonic(text)
            item.set_sensitive(props.get("enabled", True))
            if props.get("children-display") == "submenu" or sub:
                item.set_submenu(self.build(sub))
            else:
                item.connect("activate", lambda _i, i=ident: self.event(i))
            menu.append(item)
        return menu

    def event(self, ident):
        self.bus.call(self.bus_name, self.path, MENU_IFACE, "Event",
                      GLib.Variant("(isvu)", (ident, "clicked", GLib.Variant("i", 0), Gtk.get_current_event_time())),
                      None, Gio.DBusCallFlags.NONE, 2000, None, None)


class Tray:
    """Owns org.kde.StatusNotifierWatcher (or follows another watcher) and keeps the item list.
    on_change() is called when icons appear, change or go away."""

    def __init__(self, on_change, on_click=None):
        self.on_change = on_change
        self.on_click = on_click
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self.items = {}
        self.watches = {}
        self.owner = False
        self.pending = False
        self.node = Gio.DBusNodeInfo.new_for_xml(WATCHER_XML)
        self.external = None
        self.reg_id = self.bus.register_object(WATCHER_PATH, self.node.interfaces[0],
                                               self.on_method, self.on_property, None)
        Gio.bus_own_name_on_connection(self.bus, WATCHER,
                                       Gio.BusNameOwnerFlags.REPLACE | Gio.BusNameOwnerFlags.ALLOW_REPLACEMENT,
                                       self.on_acquired, self.on_lost)

    # Watcher

    def on_acquired(self, _bus, _name):
        if self.external:
            self.bus.signal_unsubscribe(self.external)
            self.external = None
        self.owner = True
        self.bus.emit_signal(None, WATCHER_PATH, WATCHER, "StatusNotifierHostRegistered", None)

    def on_lost(self, _bus, _name):
        self.owner = False
        self.follow_external()

    def on_method(self, _bus, sender, _path, _iface, method, params, invocation):
        if method == "RegisterStatusNotifierItem":
            service = params.unpack()[0]
            if service.startswith("/"):
                self.add(sender, service)
            else:
                self.add(service, "/StatusNotifierItem")
        invocation.return_value(None)

    def on_property(self, _bus, _sender, _path, _iface, prop):
        if prop == "RegisteredStatusNotifierItems":
            return GLib.Variant("as", list(self.items))
        if prop == "IsStatusNotifierHostRegistered":
            return GLib.Variant("b", True)
        if prop == "ProtocolVersion":
            return GLib.Variant("i", 0)
        return None

    def follow_external(self):
        def on_signal(_bus, _sender, _path, _iface, signal, params):
            ref = params.unpack()[0]
            if signal == "StatusNotifierItemRegistered":
                self.add_ref(ref)
            else:
                self.remove(ref if "/" in ref else ref + "/StatusNotifierItem")
        self.external = self.bus.signal_subscribe(None, WATCHER, None, WATCHER_PATH, None,
                                                  Gio.DBusSignalFlags.NONE, on_signal)
        try:
            refs = self.bus.call_sync(WATCHER, WATCHER_PATH, "org.freedesktop.DBus.Properties", "Get",
                                      GLib.Variant("(ss)", (WATCHER, "RegisteredStatusNotifierItems")),
                                      None, Gio.DBusCallFlags.NONE, 2000, None).unpack()[0]
        except GLib.Error:
            refs = []
        for ref in refs:
            self.add_ref(ref)

    def add_ref(self, ref):
        i = ref.find("/")
        if i < 0:
            self.add(ref, "/StatusNotifierItem")
        else:
            self.add(ref[:i], ref[i:])

    # Items

    def add(self, bus_name, path):
        key = bus_name + path
        if key in self.items:
            return
        self.items[key] = Item(self, bus_name, path)
        if bus_name not in self.watches:
            self.watches[bus_name] = Gio.bus_watch_name_on_connection(
                self.bus, bus_name, Gio.BusNameWatcherFlags.NONE, None, self.on_vanished)
        if self.owner:
            self.bus.emit_signal(None, WATCHER_PATH, WATCHER, "StatusNotifierItemRegistered",
                                 GLib.Variant("(s)", (key,)))
        self.changed()

    def on_vanished(self, _bus, name):
        for key in [k for k, item in self.items.items() if item.bus_name == name]:
            self.remove(key)
        watch = self.watches.pop(name, None)
        if watch:
            Gio.bus_unwatch_name(watch)

    def remove(self, key):
        item = self.items.pop(key, None)
        if item:
            item.destroy()
            if self.owner:
                self.bus.emit_signal(None, WATCHER_PATH, WATCHER, "StatusNotifierItemUnregistered",
                                     GLib.Variant("(s)", (key,)))
            self.changed()

    def changed(self):
        if not self.pending:
            self.pending = True
            GLib.timeout_add(80, self.flush)

    def flush(self):
        self.pending = False
        self.on_change()
        return False

    def clicked(self):
        if self.on_click:
            self.on_click()

    def visible_items(self):
        return [i for i in self.items.values() if i.visible]
