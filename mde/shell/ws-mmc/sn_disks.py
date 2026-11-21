# Disk Management: volumes on top, the graphical disk map below. Layout comes from
# lsblk; every change goes through UDisks2, so polkit asks before anything happens.
import json
import os

import mmcui as ui
from mmcui import Action, Node
from gi.repository import Gdk, Gio, GLib, Gtk, Pango, PangoCairo

KEY = "diskmgmt"
NAME = "Disk Management"
ICON = "drive-harddisk"
DESCRIPTION = "Manage disks, partitions and volumes"

UD = "org.freedesktop.UDisks2"
UD_PATH = "/org/freedesktop/UDisks2"
PREFS = os.path.join(ui.wsui.CONFIG, "ws-mmc", "diskmgmt.json")
MB = 1024 * 1024
SYSTEM_MOUNTS = ("/", "/boot", "/boot/efi", "/efi", "/usr", "/var", "/home")

FS_NAMES = {"ext4": "EXT4", "ext3": "EXT3", "ext2": "EXT2", "xfs": "XFS", "btrfs": "Btrfs", "exfat": "exFAT",
            "ntfs": "NTFS", "vfat": "FAT32", "f2fs": "F2FS", "swap": "Swap", "iso9660": "CDFS", "udf": "UDF",
            "crypto_LUKS": "BitLocker (LUKS)", "LVM2_member": "LVM", "zfs_member": "ZFS"}
FORMATS = [("ext4", "EXT4"), ("xfs", "XFS"), ("btrfs", "Btrfs"), ("exfat", "exFAT"), ("ntfs", "NTFS"),
           ("vfat", "FAT32")]

COLORS = {"primary": (0.12, 0.23, 0.58), "dynamic": (0.50, 0.50, 0.0), "unalloc": (0.0, 0.0, 0.0),
          "logical": (0.23, 0.47, 1.0), "free": (0.18, 0.55, 0.18), "rom": (0.45, 0.45, 0.45)}
LEGEND = [("unalloc", "Unallocated"), ("primary", "Primary partition"), ("dynamic", "Simple volume")]

# Manager.CanResize mode flags
OFFLINE_SHRINK, OFFLINE_GROW, ONLINE_SHRINK, ONLINE_GROW = 2, 4, 8, 16


def escape(kname):
    return "".join(c if c.isalnum() else f"_{ord(c):02x}" for c in kname)


def block_proxy(kname, iface="Block"):
    return ui.proxy(UD, f"{UD_PATH}/block_devices/{escape(kname)}", f"{UD}.{iface}")


def opts(**kw):
    out = {}
    for k, v in kw.items():
        k = k.replace("_", "-")
        if isinstance(v, bool):
            out[k] = GLib.Variant("b", v)
        elif isinstance(v, int):
            out[k] = GLib.Variant("t", v)
        else:
            out[k] = GLib.Variant("s", v)
    return out


def manager():
    return ui.proxy(UD, f"{UD_PATH}/Manager", f"{UD}.Manager")


def manager_sync(method, sig, *args):
    m = manager()
    if not m:
        return None
    try:
        return m.call_sync(method, GLib.Variant(sig, args), Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
    except GLib.Error:
        return None


class Disk:
    def __init__(self, n, dev):
        self.n = n
        self.kname = dev["kname"]
        self.path = dev["path"]
        self.size = dev.get("size") or 0
        self.model = (dev.get("model") or "").strip()
        self.vendor = (dev.get("vendor") or "").strip()
        self.tran = dev.get("tran") or ""
        self.removable = bool(dev.get("rm")) or bool(dev.get("hotplug"))
        self.rom = dev["type"] == "rom"
        self.loop = dev["type"] == "loop"
        self.pttype = dev.get("pttype") or ""
        self.fstype = dev.get("fstype") or ""
        self.serial = dev.get("serial") or ""
        self.ro = bool(dev.get("ro"))
        self.raw = dev
        self.segments = []

    @property
    def title(self):
        return f"CD-ROM {self.n}" if self.rom else f"Disk {self.n}"

    @property
    def kind(self):
        if self.rom:
            return "DVD"
        if self.loop:
            return "Virtual"
        return "Removable" if self.removable else "Basic"

    @property
    def status(self):
        if not self.size:
            return "No Media"
        if not self.pttype and not self.fstype and not self.raw.get("children"):
            return "Not Initialized"
        return "Online"

    @property
    def style(self):
        return {"gpt": "GUID Partition Table (GPT)", "dos": "Master Boot Record (MBR)"}.get(self.pttype, "")

    def unallocated(self):
        return sum(s.size for s in self.segments if s.kind == "unalloc")


class Volume:
    """A partition, logical volume, whole-disk file system or unallocated gap."""

    def __init__(self, disk, kind, dev=None, start=0, size=0):
        self.disk, self.kind, self.dev = disk, kind, dev or {}
        d = self.dev
        self.kname = d.get("kname", "")
        self.path = d.get("path", "")
        self.start = start
        self.size = size or d.get("size") or 0
        self.fstype = d.get("fstype") or ""
        self.fsver = d.get("fsver") or ""
        self.label = d.get("label") or d.get("partlabel") or ""
        self.mounts = [m for m in (d.get("mountpoints") or []) if m]
        self.avail = d.get("fsavail")
        self.fssize = d.get("fssize")
        self.parttype = (d.get("parttypename") or "")
        self.partn = d.get("partn")
        self.uuid = d.get("uuid") or ""
        self.children = []   # logical volumes or unlocked containers inside this partition
        self.key = self.kname or f"{disk.kname}:gap:{start}"

    @property
    def fs_name(self):
        if self.fstype == "vfat":
            return self.fsver or "FAT32"
        return FS_NAMES.get(self.fstype, self.fstype.upper() if self.fstype else ("" if self.kind == "unalloc" else "RAW"))

    @property
    def mount(self):
        return self.mounts[0] if self.mounts else ""

    @property
    def name(self):
        if self.kind == "unalloc":
            return "Unallocated"
        inner = self.children[0] if self.children else None
        if inner:
            return inner.name
        label = "" if self.label in ("", "EFI System Partition") else self.label
        if self.mount and label:
            return f"{label} ({self.mount})"
        if self.mount:
            return self.mount
        if label:
            return label
        return f"(Disk {self.disk.n} partition {self.partn})" if self.partn else f"({self.kname})"

    @property
    def layout_type(self):
        return "Dynamic" if self.kind in ("lvm", "crypt") else "Basic"

    @property
    def color(self):
        if self.kind == "unalloc":
            return "unalloc"
        if self.disk.rom:
            return "rom"
        return "dynamic" if self.kind in ("lvm", "crypt") or self.children else "primary"

    def all_mounts(self):
        return self.mounts + [m for c in self.children for m in c.all_mounts()]

    @property
    def system(self):
        return any(m in SYSTEM_MOUNTS for m in self.all_mounts()) or self.swap_active

    @property
    def swap_active(self):
        if self.fstype != "swap":
            return False
        return any(line.split()[0] in (self.path, f"/dev/{self.kname}")
                   for line in ui.read_file("/proc/swaps").splitlines()[1:] if line.split())

    @property
    def status(self):
        if self.kind == "unalloc":
            return ""
        flags = []
        mounts = self.all_mounts()
        if self.parttype == "EFI System" or "/boot/efi" in mounts or "/efi" in mounts:
            flags.append("EFI System Partition")
        else:
            if "/" in mounts:
                flags.append("Boot")
            if "/boot" in mounts:
                flags.append("System")
            if self.swap_active or any(c.swap_active for c in self.children):
                flags.append("Page File")
            if self.fstype == "LVM2_member" and not self.children:
                flags.append("LVM Physical Volume")
            elif self.fstype == "crypto_LUKS" and not self.children:
                return "Locked (Encrypted)"
            elif self.fstype == "crypto_LUKS":
                flags.append("Encrypted")
            if "Microsoft reserved" in self.parttype:
                flags.append("Reserved Partition")
            elif "recovery" in self.parttype.lower():
                flags.append("Recovery Partition")
            elif self.kind == "part":
                flags.append("Basic Data Partition" if self.disk.pttype == "gpt" else "Primary Partition")
        return f"Healthy ({', '.join(flags)})" if flags else "Healthy"

    @property
    def free(self):
        if self.children:
            return self.children[0].free
        return self.avail if self.mounts else None

    @property
    def capacity(self):
        return self.size


def load():
    out = ui.out(["lsblk", "-J", "-b", "-o", "NAME,KNAME,PATH,TYPE,SIZE,START,FSTYPE,FSVER,LABEL,UUID,PARTTYPENAME,"
                  "PARTLABEL,PARTN,PTTYPE,MOUNTPOINTS,FSAVAIL,FSUSED,FSSIZE,MODEL,VENDOR,TRAN,RM,RO,HOTPLUG,"
                  "SERIAL,PKNAME"], timeout=10)
    try:
        devs = json.loads(out)["blockdevices"]
    except (ValueError, KeyError):
        return [], []
    disks, vols = [], []
    devs = [d for d in devs if d["type"] in ("disk", "rom") or
            (d["type"] == "loop" and d.get("size") and ui.sysfs(f"/sys/class/block/{d['kname']}/loop", "backing_file"))]
    devs = [d for d in devs if not d["kname"].startswith(("zram", "ram"))]
    devs.sort(key=lambda d: (d["type"] == "rom", d["type"] == "loop", bool(d.get("rm") or d.get("hotplug")), d["kname"]))
    nd = nr = 0
    for dev in devs:
        if dev["type"] == "rom":
            disk = Disk(nr, dev)
            nr += 1
        else:
            disk = Disk(nd, dev)
            nd += 1
        disks.append(disk)
        parts = [c for c in dev.get("children", []) if c["type"] == "part"]
        if not parts and disk.size and (disk.fstype or dev.get("children")):
            v = Volume(disk, "whole", dev, 0, disk.size)
            for c in dev.get("children", []):
                v.children.append(inner(disk, c))
            disk.segments = [v]
            vols.extend(v.children or [v])
            continue
        if not parts:
            if disk.size and not disk.rom:
                disk.segments = [Volume(disk, "unalloc", start=0, size=disk.size)]
            continue
        pos = MB
        for p in sorted(parts, key=lambda p: p.get("start") or 0):
            start = (p.get("start") or 0) * 512
            if start - pos >= MB:
                disk.segments.append(Volume(disk, "unalloc", start=pos, size=start - pos))
            v = Volume(disk, "part", p, start, p.get("size") or 0)
            for c in p.get("children", []):
                v.children.append(inner(disk, c))
            disk.segments.append(v)
            vols.extend(v.children if v.children else [v])
            pos = max(pos, start + v.size)
        tail = disk.size - (34 * 512 if disk.pttype == "gpt" else 0)
        if tail - pos >= MB:
            disk.segments.append(Volume(disk, "unalloc", start=pos, size=tail - pos))
    for d in disks:
        if d.rom and d.size:
            v = Volume(d, "whole", d.raw, 0, d.size)
            d.segments = [v]
            vols.append(v)
    return disks, vols


def inner(disk, dev):
    v = Volume(disk, "lvm" if dev["type"] == "lvm" else "crypt", dev)
    for c in dev.get("children", []):
        v.children.append(inner(disk, c))
    return v


class DiskView(ui.View):
    def __init__(self, console, node):
        super().__init__(console, node)
        prefs = ui.json_file(PREFS, {})
        self.top = prefs.get("top", "volumes")
        self.bottom = prefs.get("bottom", True)
        self.disks, self.vols = [], []
        self.sel = None
        self.rects = []
        self.monitor = None
        self.pending = None
        self.loading = False

        cols = [ui.Column("Volume", lambda v: v.name, 200),
                ui.Column("Layout", lambda v: "Simple", 70),
                ui.Column("Type", lambda v: v.layout_type, 70),
                ui.Column("File System", lambda v: v.fs_name, 90),
                ui.Column("Status", lambda v: v.status, 300, expand=True),
                ui.Column("Capacity", lambda v: ui.size_text(v.capacity), 100, sort=lambda v: v.capacity, align=1.0),
                ui.Column("Free Space", lambda v: ui.size_text(v.free) if v.free is not None else "", 100,
                          sort=lambda v: v.free or 0, align=1.0),
                ui.Column("% Free", lambda v: f"{100 * v.free // v.capacity} %" if v.free is not None and v.capacity else "",
                          70, sort=lambda v: (v.free or 0) / (v.capacity or 1), align=1.0)]
        self.vlist = ui.ListView(cols, icon=lambda v: ui.pick_icon("drive-harddisk"), key=lambda v: v.key,
                                 on_select=self.on_list_select, on_activate=lambda v: self.properties(),
                                 on_menu=self.on_list_menu)
        dcols = [ui.Column("Disk", lambda d: d.title, 90),
                 ui.Column("Type", lambda d: d.kind, 90),
                 ui.Column("Capacity", lambda d: ui.size_text(d.size) if d.size else "No Media", 100,
                           sort=lambda d: d.size, align=1.0),
                 ui.Column("Unallocated Space", lambda d: ui.size_text(d.unallocated()), 130,
                           sort=lambda d: d.unallocated(), align=1.0),
                 ui.Column("Status", lambda d: d.status, 110),
                 ui.Column("Device Type", lambda d: d.tran.upper() if d.tran else ("Virtual" if d.loop else ""), 100),
                 ui.Column("Partition Style", lambda d: {"gpt": "GPT", "dos": "MBR"}.get(d.pttype, ""), 100,
                           expand=True)]
        self.dlist = ui.ListView(dcols, icon=lambda d: ui.pick_icon("drive-optical" if d.rom else "drive-harddisk"),
                                 key=lambda d: d.kname, on_select=self.on_disk_select,
                                 on_activate=lambda d: self.disk_properties(d), on_menu=self.on_disk_menu)

        self.area = Gtk.DrawingArea(can_focus=True)
        self.area.add_events(Gdk.EventMask.BUTTON_PRESS_MASK)
        self.area.connect("draw", self.draw)
        self.area.connect("button-press-event", self.on_map_button)
        self.area.connect("key-press-event", self.on_map_key)
        map_sw = Gtk.ScrolledWindow()
        map_sw.add(self.area)
        self.map_sw = map_sw

        self.paned = Gtk.Paned(orientation=Gtk.Orientation.VERTICAL)
        self.top_box = Gtk.Box()
        self.paned.pack1(self.top_box, True, False)
        self.paned.pack2(map_sw, True, False)
        self.paned.set_position(230)
        self.widget = self.paned
        self.place()
        self.refresh()

    # data

    def refresh(self):
        if self.loading:
            return
        self.loading = True

        def done(ok, out, err):
            self.loading = False
            self.apply_load()
        ui.run(["udevadm", "settle", "-t", "3"], done)

    def apply_load(self):
        keep = self.sel.key if self.sel else None
        self.disks, self.vols = load()
        self.vlist.set_rows(self.vols)
        self.dlist.set_rows(self.disks)
        self.sel = None
        if keep:
            for d in self.disks:
                for s in d.segments + [c for s in d.segments for c in s.children]:
                    if s.key == keep:
                        self.sel = s
        self.resize_area()
        self.area.queue_draw()
        self.changed()

    def resize_area(self):
        self.area.set_size_request(-1, len(self.disks) * 86 + 48)

    def place(self):
        for c in self.top_box.get_children():
            self.top_box.remove(c)
        self.top_box.pack_start(self.dlist if self.top == "disks" else self.vlist, True, True, 0)
        self.top_box.show_all()
        self.map_sw.set_visible(self.bottom)

    def set_top(self, top):
        self.top = top
        self.save_prefs()
        self.place()
        self.changed()

    def toggle_bottom(self):
        self.bottom = not self.bottom
        self.save_prefs()
        self.place()

    def save_prefs(self):
        ui.write_file(PREFS, json.dumps({"top": self.top, "bottom": self.bottom}) + "\n")

    # selection

    def select(self, obj, sync=True):
        self.sel = obj
        if sync and isinstance(obj, Volume):
            target = obj.children[0] if obj.children else obj
            self.vlist.select_key(target.key)
        elif sync and isinstance(obj, Disk):
            self.dlist.select_key(obj.kname)
        self.area.queue_draw()
        self.changed()

    def on_list_select(self):
        v = self.vlist.selected()
        if v is None:
            return
        seg = v
        for d in self.disks:
            for s in d.segments:
                if s is v or v in s.children:
                    seg = s
        if seg is not self.sel:
            self.sel = seg
            self.area.queue_draw()
            self.changed()

    def on_disk_select(self):
        d = self.dlist.selected()
        if d is not None and d is not self.sel:
            self.sel = d
            self.area.queue_draw()
            self.changed()

    def selected_title(self):
        s = self.sel
        if isinstance(s, Disk):
            return s.title
        if isinstance(s, Volume):
            return s.name
        return None

    # actions

    def actions(self):
        return [Action("Rescan Disks", self.rescan, ui.pick_icon("system-search-symbolic", "view-refresh-symbolic")),
                None,
                Action("Attach VHD", self.attach_vhd, ui.pick_icon("list-add-symbolic")),
                None,
                Action("All Tasks", children=[Action("Rescan Disks", self.rescan),
                                              Action("Attach VHD", self.attach_vhd)]),
                Action("View", children=self.view_actions())]

    def view_actions(self):
        return [Action("Top", children=[
                    Action("Disk List", lambda: self.set_top("disks"), checked=self.top == "disks"),
                    Action("Volume List", lambda: self.set_top("volumes"), checked=self.top == "volumes")]),
                Action("Bottom", children=[
                    Action("Graphical View", self.toggle_bottom, checked=self.bottom),
                    Action("Hidden", self.toggle_bottom, checked=not self.bottom)])]

    def item_actions(self):
        s = self.sel
        if isinstance(s, Disk):
            return self.disk_actions(s)
        if not isinstance(s, Volume):
            return []
        if s.kind == "unalloc":
            return [Action("New Simple Volume\u2026", lambda: self.new_volume(s), "list-add-symbolic",
                           enabled=bool(s.disk.pttype)),
                    None, Action("Properties", enabled=False), Action("Help", self.console.about_node)]
        target = s.children[0] if s.children else s
        mounted = bool(target.mounts)
        fs_ok = target.fstype and target.fstype not in ("LVM2_member", "crypto_LUKS", "swap")
        can_shrink, can_grow = self.resize_modes(s)
        return [
            Action("Open", lambda: ui.spawn(["xdg-open", target.mount]), "folder-open-symbolic", enabled=mounted),
            Action("Explore", lambda: ui.spawn(["thunar", target.mount]), enabled=mounted),
            None,
            Action("Mark Partition as Active", enabled=False),
            Action("Change Drive Letter and Paths\u2026", lambda: self.paths(target), enabled=bool(fs_ok)),
            Action("Format\u2026", lambda: self.format(target), enabled=not s.system and not s.disk.rom and s.kind != "whole"
                   or (s.kind == "whole" and not s.system and not s.disk.rom)),
            None,
            Action("Extend Volume\u2026", lambda: self.resize(s, True), enabled=can_grow),
            Action("Shrink Volume\u2026", lambda: self.resize(s, False), enabled=can_shrink),
            Action("Add Mirror\u2026", enabled=False),
            Action("Delete Volume\u2026", lambda: self.delete(s), "edit-delete-symbolic",
                   enabled=s.kind == "part" and not s.system),
            None,
            Action("Properties", lambda: VolumeSheet(self, target), "document-properties-symbolic"),
            Action("Help", self.console.about_node),
        ]

    def disk_actions(self, d):
        empty = d.status == "Not Initialized"
        no_parts = not any(s.kind == "part" for s in d.segments)
        system = any(s.system for s in d.segments if isinstance(s, Volume))
        return [
            Action("Initialize Disk", lambda: self.initialize(d), enabled=empty and not d.rom),
            Action("Convert to GPT Disk", lambda: self.initialize(d, "gpt"),
                   enabled=no_parts and d.pttype == "dos" and not system),
            Action("Convert to MBR Disk", lambda: self.initialize(d, "dos"),
                   enabled=no_parts and d.pttype == "gpt" and not system),
            None,
            Action("Eject", lambda: self.drive_call(d, "Eject"), enabled=d.removable or d.rom),
            Action("Detach VHD", lambda: self.detach(d), enabled=d.loop),
            Action("Power Off", lambda: self.drive_call(d, "PowerOff"), enabled=d.removable and not system),
            None,
            Action("Properties", lambda: self.disk_properties(d), "document-properties-symbolic"),
            Action("Help", self.console.about_node),
        ]

    def resize_modes(self, s):
        if s.kind != "part" or s.children or not s.fstype:
            return False, False
        res = manager_sync("CanResize", "(s)", s.fstype)
        if not res or not res[0]:
            return False, False
        mode = res[1]
        mounted = bool(s.mounts)
        shrink = bool(mode & (ONLINE_SHRINK if mounted else OFFLINE_SHRINK)) and not s.system
        nxt = self.next_segment(s)
        grow = bool(mode & (ONLINE_GROW if mounted else OFFLINE_GROW)) and nxt is not None and nxt.kind == "unalloc"
        return shrink, grow

    def next_segment(self, s):
        segs = s.disk.segments
        i = segs.index(s) if s in segs else -1
        return segs[i + 1] if 0 <= i < len(segs) - 1 else None

    def toolbar(self):
        return []

    # context menus

    def on_list_menu(self, event):
        ui.popup(self.vlist.tree, event, self.item_actions())

    def on_disk_menu(self, event):
        d = self.dlist.selected()
        if d:
            ui.popup(self.dlist.tree, event, self.disk_actions(d))

    def on_map_button(self, area, event):
        area.grab_focus()
        hit = next((obj for (x, y, w, h, obj) in self.rects if x <= event.x < x + w and y <= event.y < y + h), None)
        if hit is None:
            return False
        self.select(hit)
        if event.button == 3:
            ui.popup(area, event, self.disk_actions(hit) if isinstance(hit, Disk) else self.item_actions())
        elif event.type == Gdk.EventType._2BUTTON_PRESS:
            if isinstance(hit, Disk):
                self.disk_properties(hit)
            elif hit.kind != "unalloc":
                VolumeSheet(self, hit.children[0] if hit.children else hit)
        return True

    def on_map_key(self, area, event):
        segs = [s for d in self.disks for s in d.segments]
        if self.sel not in segs or event.keyval not in (Gdk.KEY_Left, Gdk.KEY_Right):
            return False
        i = segs.index(self.sel) + (1 if event.keyval == Gdk.KEY_Right else -1)
        if 0 <= i < len(segs):
            self.select(segs[i])
        return True

    # drawing

    def draw(self, area, cr):
        self.rects = []
        width = area.get_allocated_width()
        row_h, head_w, gap = 72, 118, 14
        y = 10
        for d in self.disks:
            x = 10
            # disk header
            cr.set_source_rgb(0.94, 0.94, 0.94)
            cr.rectangle(x, y, head_w, row_h)
            cr.fill()
            if self.sel is d:
                self.hatch(cr, x, y, head_w, row_h)
            cr.set_source_rgb(0.62, 0.62, 0.62)
            cr.set_line_width(1)
            cr.rectangle(x + 0.5, y + 0.5, head_w - 1, row_h - 1)
            cr.stroke()
            self.text(area, cr, f"<b>{GLib.markup_escape_text(d.title)}</b>", x + 8, y + 6, head_w - 16)
            lines = [d.kind, ui.size_text(d.size) if d.size else "", d.status]
            self.text(area, cr, "\n".join(GLib.markup_escape_text(l) for l in lines if l), x + 8, y + 24,
                      head_w - 16, small=True)
            self.rects.append((x, y, head_w, row_h, d))
            # segments
            x0 = x + head_w + 2
            avail = max(100, width - x0 - 10)
            segs = d.segments
            if not segs:
                cr.set_source_rgb(1, 1, 1)
                cr.rectangle(x0, y, avail, row_h)
                cr.fill()
                cr.set_source_rgb(0.62, 0.62, 0.62)
                cr.rectangle(x0 + 0.5, y + 0.5, avail - 1, row_h - 1)
                cr.stroke()
            for (sx, sw, s) in self.layout(segs, x0, avail):
                cr.set_source_rgb(1, 1, 1)
                cr.rectangle(sx, y, sw, row_h)
                cr.fill()
                if self.sel is s:
                    self.hatch(cr, sx, y, sw, row_h)
                cr.set_source_rgb(*COLORS[s.color])
                cr.rectangle(sx, y, sw, 9)
                cr.fill()
                cr.set_source_rgb(0.62, 0.62, 0.62)
                cr.rectangle(sx + 0.5, y + 0.5, sw - 1, row_h - 1)
                cr.stroke()
                if s.kind == "unalloc":
                    lines = [ui.size_text(s.size), "Unallocated"]
                    title = ""
                else:
                    t = s.children[0] if s.children else s
                    title = s.name
                    lines = [f"{ui.size_text(s.size)} {t.fs_name}".strip(), s.status]
                markup = (f"<b>{GLib.markup_escape_text(title)}</b>\n" if title else "") + \
                    "\n".join(GLib.markup_escape_text(l) for l in lines)
                self.text(area, cr, markup, sx + 6, y + 13, sw - 12, small=True)
                self.rects.append((sx, y, sw, row_h, s))
            y += row_h + gap
        # legend
        x = 10
        y += 4
        for key, label in LEGEND:
            cr.set_source_rgb(*COLORS[key])
            cr.rectangle(x, y + 3, 10, 10)
            cr.fill()
            w = self.text(area, cr, GLib.markup_escape_text(label), x + 14, y, 200, small=True)
            x += w + 34

    def layout(self, segs, x0, avail):
        """Proportional widths with a minimum so small partitions stay readable."""
        min_w = 96
        total = sum(max(s.size, 1) for s in segs) or 1
        widths = [max(min_w, avail * max(s.size, 1) / total) for s in segs]
        over = sum(widths) - avail
        if over > 0:
            big = [i for i, w in enumerate(widths) if w > min_w]
            spare = sum(widths[i] - min_w for i in big) or 1
            for i in big:
                widths[i] -= over * (widths[i] - min_w) / spare
        out, x = [], x0
        for s, w in zip(segs, widths):
            out.append((int(x), int(w) - 2, s))
            x += w
        return out

    def hatch(self, cr, x, y, w, h):
        cr.save()
        cr.rectangle(x, y, w, h)
        cr.clip()
        cr.set_source_rgba(0.55, 0.55, 0.55, 0.55)
        cr.set_line_width(1)
        i = -h
        while i < w:
            cr.move_to(x + i, y + h)
            cr.line_to(x + i + h, y)
            i += 5
        cr.stroke()
        cr.restore()

    def text(self, area, cr, markup, x, y, w, small=False):
        layout = area.create_pango_layout("")
        layout.set_markup(markup, -1)
        if small:
            desc = layout.get_context().get_font_description() or Pango.FontDescription()
            desc = desc.copy()
            desc.set_size(int(desc.get_size() * 0.9) or 9 * Pango.SCALE)
            layout.set_font_description(desc)
        layout.set_width(int(max(10, w) * Pango.SCALE))
        layout.set_ellipsize(Pango.EllipsizeMode.END)
        cr.set_source_rgb(0.1, 0.1, 0.1)
        cr.move_to(x, y)
        PangoCairo.show_layout(cr, layout)
        return layout.get_pixel_extents()[1].width

    # operations

    def busy(self, title):
        def done(ok, value):
            if not ok:
                ui.error(self.console.win, title, str(value))
            GLib.timeout_add(500, lambda: self.refresh() and False)
        return done

    def rescan(self):
        for d in self.disks:
            ui.call(block_proxy(d.kname), "Rescan", "(a{sv})", {}, done=lambda ok, v: None)
        GLib.timeout_add(1200, lambda: self.refresh() and False)

    def attach_vhd(self):
        dlg = Gtk.FileChooserNative.new("Attach Virtual Hard Disk", self.console.win, Gtk.FileChooserAction.OPEN,
                                        "_Attach", "_Cancel")
        f = Gtk.FileFilter()
        f.set_name("Disk images (*.img, *.iso, *.raw, *.vhd)")
        for p in ("*.img", "*.iso", "*.raw", "*.vhd", "*.vhdx", "*.bin"):
            f.add_pattern(p)
        dlg.add_filter(f)
        if dlg.run() == Gtk.ResponseType.ACCEPT:
            path = dlg.get_filename()
            ui.run(["udisksctl", "loop-setup", "-f", path],
                   lambda ok, o, e: (ok or ui.error(self.console.win, "Couldn\u2019t attach the disk", e.strip()),
                                     GLib.timeout_add(800, lambda: self.refresh() and False)))
        dlg.destroy()

    def detach(self, d):
        ui.call(block_proxy(d.kname, "Loop"), "Delete", "(a{sv})", {},
                done=self.busy("Couldn\u2019t detach the disk"))

    def drive_call(self, d, method):
        blk = block_proxy(d.kname)
        drive = ui.prop(blk, "Drive")
        if not drive or drive == "/":
            ui.error(self.console.win, d.title, "This disk can\u2019t be ejected.")
            return
        ui.call(ui.proxy(UD, drive, f"{UD}.Drive"), method, "(a{sv})", {},
                done=self.busy(f"Couldn\u2019t {'eject' if method == 'Eject' else 'power off'} the disk"))

    def initialize(self, d, style=None):
        if style:
            ui.confirm(self.console.win, f"Convert {d.title}",
                       f"{d.title} will use {'GPT' if style == 'gpt' else 'MBR'}. It has no partitions, so nothing is lost.",
                       "Convert", lambda: ui.call(block_proxy(d.kname), "Format", "(sa{sv})", style, opts(),
                                                  done=self.busy("Couldn\u2019t convert the disk")))
            return
        dlg = ui.Dialog(self.console.win, "Initialize Disk", "OK")
        dlg.add_text(f"You must initialize a disk before Disk Management can use it.\n\n{d.title} "
                     f"({ui.size_text(d.size)})")
        gpt = Gtk.RadioButton.new_with_label(None, "GPT (GUID Partition Table)")
        mbr = Gtk.RadioButton.new_with_label_from_widget(gpt, "MBR (Master Boot Record)")
        dlg.add_field("Use the following partition style for the selected disk:", gpt)
        dlg.body.pack_start(mbr, False, False, 0)
        dlg.add_text("Note: The GPT partition style is recommended for most computers. It is required for "
                     "disks larger than 2 TB and for UEFI-based computers.")

        def go():
            dlg.destroy()
            ui.call(block_proxy(d.kname), "Format", "(sa{sv})", "gpt" if gpt.get_active() else "dos", opts(),
                    done=self.busy("Couldn\u2019t initialize the disk"))
        dlg.on_primary(go)
        dlg.run_dialog()

    def format_choices(self):
        out = []
        for t, label in FORMATS:
            res = manager_sync("CanFormat", "(s)", t)
            if res is None or res[0]:
                out.append((t, label))
        return out

    def format(self, v):
        dlg = ui.Dialog(self.console.win, f"Format {v.name}", "OK")
        label = ui.wsui.entry(v.label if v.label != "EFI System Partition" else "", "New Volume", 320)
        dlg.add_field("Volume label:", label)
        fs = Gtk.ComboBoxText()
        for t, name in self.format_choices():
            fs.append(t, name)
        fs.set_active_id(v.fstype if v.fstype in dict(FORMATS) else "ext4")
        if fs.get_active() < 0:
            fs.set_active(0)
        dlg.add_field("File system:", fs)
        quick = Gtk.CheckButton(label="Perform a quick format", active=True)
        dlg.body.pack_start(quick, False, False, 0)

        def go():
            dlg.destroy()
            ui.confirm(self.console.win, f"Format {v.name}",
                       "Formatting this volume will erase all data on it. Back up any data you want to keep before "
                       "formatting. Do you want to continue?", "OK",
                       lambda: self.do_format(v, fs.get_active_id(), label.get_text().strip(), quick.get_active()))
        dlg.on_primary(go)
        dlg.run_dialog()

    def do_format(self, v, fstype, label, quick):
        o = opts(update_partition_type=True, tear_down=True)
        if label:
            o["label"] = GLib.Variant("s", label.upper()[:11] if fstype == "vfat" else label)
        if not quick:
            o["erase"] = GLib.Variant("s", "zero")
        blk = block_proxy(v.kname)

        def go(*_):
            ui.call(blk, "Format", "(sa{sv})", fstype, o, done=self.busy("Couldn\u2019t format the volume"))
        if v.mounts:
            ui.call(block_proxy(v.kname, "Filesystem"), "Unmount", "(a{sv})", {}, done=lambda ok, val: go()
                    if ok else ui.error(self.console.win, "The volume is in use", str(val)))
        else:
            go()

    def new_volume(self, gap):
        d = gap.disk
        dlg = ui.Dialog(self.console.win, "New Simple Volume", "Finish", width=480)
        max_mb = gap.size // MB
        dlg.add_text(f"Maximum disk space in MB: {max_mb}\nMinimum disk space in MB: 8")
        size = Gtk.SpinButton.new_with_range(8, max(8, max_mb), 1)
        size.set_value(max_mb)
        dlg.add_field("Simple volume size in MB:", size)
        fs = Gtk.ComboBoxText()
        for t, name in self.format_choices():
            fs.append(t, name)
        fs.set_active_id("ext4")
        dlg.add_field("File system:", fs)
        label = ui.wsui.entry("New Volume", "", 320)
        dlg.add_field("Volume label:", label)
        mount = Gtk.CheckButton(label="Mount the volume after formatting it", active=True)
        dlg.body.pack_start(mount, False, False, 0)

        def go():
            mb = int(size.get_value())
            fstype = fs.get_active_id() or "ext4"
            lab = label.get_text().strip()
            dlg.destroy()
            fo = opts(update_partition_type=True)
            if lab:
                fo["label"] = GLib.Variant("s", lab.upper()[:11] if fstype == "vfat" else lab)
            table = block_proxy(d.kname, "PartitionTable")

            def created(ok, value):
                if not ok:
                    ui.error(self.console.win, "Couldn\u2019t create the volume", str(value))
                elif mount.get_active() and value:
                    path = value[0] if isinstance(value, tuple) else value
                    fsp = ui.proxy(UD, path, f"{UD}.Filesystem")
                    GLib.timeout_add(800, lambda: ui.call(fsp, "Mount", "(a{sv})", {}, done=lambda *_: self.refresh())
                                     and False)
                GLib.timeout_add(600, lambda: self.refresh() and False)
            size_b = min(mb * MB, gap.size)
            ui.call(table, "CreatePartitionAndFormat", "(ttssa{sv}sa{sv})", gap.start, size_b, "", "",
                    {}, fstype, fo, done=created)
        dlg.on_primary(go)
        dlg.run_dialog()

    def resize(self, v, grow):
        nxt = self.next_segment(v)
        cur_mb = v.size // MB
        used = (v.fssize - v.avail) if v.fssize and v.avail is not None else v.size // 2
        dlg = ui.Dialog(self.console.win, "Extend Volume" if grow else f"Shrink {v.name}",
                        "Extend" if grow else "Shrink", width=460)
        if grow:
            room = (nxt.size // MB) if nxt else 0
            dlg.add_text(f"Total volume size in MB: {cur_mb}\nMaximum available space in MB: {room}")
            amount = Gtk.SpinButton.new_with_range(1, max(1, room), 1)
            amount.set_value(room)
            dlg.add_field("Select the amount of space in MB:", amount)
        else:
            room = max(0, (v.size - used) // MB - 64)
            dlg.add_text(f"Total size before shrink in MB: {cur_mb}\nSize of available shrink space in MB: {room}")
            amount = Gtk.SpinButton.new_with_range(1, max(1, room), 1)
            amount.set_value(room)
            dlg.add_field("Enter the amount of space to shrink in MB:", amount)

        def go():
            delta = int(amount.get_value()) * MB
            dlg.destroy()
            new = v.size + delta if grow else v.size - delta
            part = block_proxy(v.kname, "Partition")
            fsp = block_proxy(v.kname, "Filesystem")
            err = self.busy("Couldn\u2019t resize the volume")
            if grow:
                ui.call(part, "Resize", "(ta{sv})", new, {}, done=lambda ok, val: ui.call(
                    fsp, "Resize", "(ta{sv})", 0, {}, done=err) if ok else err(ok, val))
            else:
                ui.call(fsp, "Resize", "(ta{sv})", new, {}, done=lambda ok, val: ui.call(
                    part, "Resize", "(ta{sv})", new, {}, done=err) if ok else err(ok, val))
        dlg.on_primary(go)
        dlg.run_dialog()

    def paths(self, v):
        dlg = ui.Dialog(self.console.win, f"Change Drive Letter and Paths for {v.name}", None, "Close", width=480)
        dlg.add_text("Allow access to this volume by using the following mount points:")
        lst = ui.simple_list(v.mounts or [], 110)
        dlg.body.pack_start(lst, False, False, 0)
        row = Gtk.Box(spacing=8)
        fsp = block_proxy(v.kname, "Filesystem")

        def mount():
            dlg.destroy()
            ui.call(fsp, "Mount", "(a{sv})", {}, done=self.busy("Couldn\u2019t mount the volume"))

        def unmount():
            dlg.destroy()
            if v.system:
                ui.error(self.console.win, "This volume is in use",
                         "Windows can\u2019t remove the path of the system or boot volume.")
                return
            ui.call(fsp, "Unmount", "(a{sv})", {}, done=self.busy("Couldn\u2019t unmount the volume"))
        row.pack_start(ui.wsui.button("Add\u2026", mount, sensitive=not v.mounts), False, False, 0)
        row.pack_start(ui.wsui.button("Remove", unmount, sensitive=bool(v.mounts) and not v.system), False, False, 0)
        dlg.body.pack_start(row, False, False, 0)
        dlg.add_text("Add mounts the volume under /run/media for your account. Remove unmounts it.")
        dlg.run_dialog()

    def disk_properties(self, d):
        DiskSheet(self, d)

    def properties(self):
        s = self.sel
        if isinstance(s, Disk):
            DiskSheet(self, s)
        elif isinstance(s, Volume) and s.kind != "unalloc":
            VolumeSheet(self, s.children[0] if s.children else s)

    def delete(self, v=None):
        v = v or self.sel
        if not isinstance(v, Volume) or v.kind != "part" or v.system:
            return

        def go():
            part = block_proxy(v.kname, "Partition")

            def run(*_):
                ui.call(part, "Delete", "(a{sv})", opts(tear_down=True), done=self.busy("Couldn\u2019t delete the volume"))
            if v.mounts:
                ui.call(block_proxy(v.kname, "Filesystem"), "Unmount", "(a{sv})", {},
                        done=lambda ok, val: run() if ok else ui.error(self.console.win, "The volume is in use", str(val)))
            else:
                run()
        ui.confirm(self.console.win, "Delete simple volume",
                   "Deleting this volume will erase all data on it. Back up any data you want to keep before "
                   "deleting. Do you want to continue?", "Yes", go)

    # hot-plug

    def shown(self):
        if self.monitor is None:
            self.monitor = ui.run(["udevadm", "monitor", "--udev", "-s", "block"], lines=self.on_uevent)

    def hidden(self):
        if self.monitor is not None:
            self.monitor.force_exit()
            self.monitor = None

    def on_uevent(self, line):
        if not line.startswith("UDEV"):
            return
        if self.pending:
            GLib.source_remove(self.pending)
        self.pending = GLib.timeout_add(800, self.hotplug)

    def hotplug(self):
        self.pending = None
        self.refresh()
        return False


def usage_pie(used, total):
    area = Gtk.DrawingArea()
    area.set_size_request(120, 120)

    def draw(w, cr):
        cx, cy, r = 60, 60, 52
        frac = used / total if total else 0
        cr.set_source_rgb(0.75, 0.75, 0.75)
        cr.arc(cx, cy, r, 0, 6.2832)
        cr.fill()
        if frac > 0:
            cr.set_source_rgb(0.0, 0.47, 0.83)
            cr.move_to(cx, cy)
            cr.arc(cx, cy, r, -1.5708, -1.5708 + 6.2832 * frac)
            cr.close_path()
            cr.fill()
    area.connect("draw", draw)
    return area


def swatch(rgb):
    area = Gtk.DrawingArea()
    area.set_size_request(14, 14)
    area.set_valign(Gtk.Align.CENTER)
    area.connect("draw", lambda w, cr: (cr.set_source_rgb(*rgb), cr.rectangle(0, 0, 14, 14), cr.fill()))
    return area


class VolumeSheet:
    def __init__(self, view, v):
        self.view, self.v = view, v
        title = v.name
        self.win = ui.PropertySheet(view.console.win, f"{title} Properties", apply=self.apply, width=460, height=540)
        self.general(self.win.page("General"))
        self.tools(self.win.page("Tools"))
        self.hardware(self.win.page("Hardware"))
        self.details(self.win.page("Details"))
        self.win.present_sheet()

    def general(self, box):
        v = self.v
        self.label = ui.wsui.entry(v.label if v.fstype not in ("swap", "LVM2_member") else "", "", 320)
        self.label.connect("changed", self.win.changed)
        self.label.set_sensitive(bool(v.fstype) and v.fstype not in ("LVM2_member", "crypto_LUKS"))
        box.pack_start(ui.sheet_header(ui.pick_icon("drive-harddisk"), self.label), False, False, 0)
        box.pack_start(Gtk.Separator(), False, False, 0)
        kind = "Removable Disk" if v.disk.removable else ("CD Drive" if v.disk.rom else "Local Disk")
        box.pack_start(ui.kv_grid([("Type:", kind), ("File system:", v.fs_name or "RAW")]), False, False, 0)
        box.pack_start(Gtk.Separator(), False, False, 0)
        total = v.fssize or v.size
        if v.mounts and v.avail is not None:
            used = total - v.avail
            grid = Gtk.Grid(column_spacing=14, row_spacing=8)
            rows = [((0.0, 0.47, 0.83), "Used space:", used),
                    ((0.75, 0.75, 0.75), "Free space:", v.avail)]
            for i, (rgb, name, n) in enumerate(rows):
                grid.attach(swatch(rgb), 0, i, 1, 1)
                grid.attach(ui.label(name, "key"), 1, i, 1, 1)
                grid.attach(ui.label(f"{n:,} bytes", xalign=1.0), 2, i, 1, 1)
                grid.attach(ui.label(ui.size_text(n), xalign=1.0), 3, i, 1, 1)
            grid.attach(ui.label("Capacity:", "key"), 1, 2, 1, 1)
            grid.attach(ui.label(f"{total:,} bytes", xalign=1.0), 2, 2, 1, 1)
            grid.attach(ui.label(ui.size_text(total), xalign=1.0), 3, 2, 1, 1)
            box.pack_start(grid, False, False, 0)
            pie = usage_pie(used, total)
            pie.set_halign(Gtk.Align.CENTER)
            box.pack_start(pie, False, False, 0)
            box.pack_start(ui.label(f"Mounted at {', '.join(v.mounts)}", "caption", xalign=0.5), False, False, 0)
        else:
            box.pack_start(ui.kv_grid([("Capacity:", f"{v.size:,} bytes   {ui.size_text(v.size)}"),
                                       ("Status:", "Not mounted" if v.fstype not in ("swap", "LVM2_member") else v.status)]),
                           False, False, 0)

    def tools(self, box):
        v = self.v
        box.pack_start(ui.label("Error checking", "summary-section"), False, False, 0)
        box.pack_start(ui.label("This option will check the volume for file system errors. The volume must not be "
                                "mounted.", wrap=True), False, False, 0)
        check = ui.wsui.button("Check", self.check, sensitive=not v.mounts and bool(v.fstype))
        check.set_halign(Gtk.Align.END)
        box.pack_start(check, False, False, 0)
        box.pack_start(Gtk.Separator(), False, False, 4)
        box.pack_start(ui.label("Optimize and defragment drive", "summary-section"), False, False, 0)
        box.pack_start(ui.label("Optimizing an SSD tells it which blocks are free (TRIM), which keeps it fast.",
                                wrap=True), False, False, 0)
        opt = ui.wsui.button("Optimize", self.optimize, sensitive=bool(v.mounts))
        opt.set_halign(Gtk.Align.END)
        box.pack_start(opt, False, False, 0)

    def check(self):
        fsp = block_proxy(self.v.kname, "Filesystem")
        ui.call(fsp, "Check", "(a{sv})", {}, done=lambda ok, val: ui.error(
            self.win, "Error checking",
            ("No errors were found." if (val[0] if isinstance(val, tuple) else val) else
             "Errors were found on this volume. Repair them from a terminal with fsck.")
            if ok else str(val)))

    def optimize(self):
        ui.helper(["trim", self.v.mount], lambda ok, msg: ui.error(
            self.win, "Optimize Drives", msg or ("The drive was optimized." if ok else "The drive couldn\u2019t be optimized.")))

    def hardware(self, box):
        rows = [(d.model or d.title, "Disk drives" if not d.rom else "DVD/CD-ROM drives") for d in self.view.disks]
        box.pack_start(ui.label("All disk drives:", "caption"), False, False, 0)
        box.pack_start(ui.simple_list(rows, 160, headers=["Name", "Type"]), False, False, 0)
        d = self.v.disk
        box.pack_start(ui.label("Device Properties", "summary-section"), False, False, 0)
        box.pack_start(ui.kv_grid([("Manufacturer:", d.vendor or "(Standard disk drives)"),
                                   ("Location:", d.tran.upper() or "Unknown"),
                                   ("Device status:", "This device is working properly.")]), False, False, 0)

    def details(self, box):
        v = self.v
        box.pack_start(ui.kv_grid([
            ("Device:", v.path or None),
            ("Disk:", f"{v.disk.title} ({v.disk.path})"),
            ("Partition:", f"{v.partn}" if v.partn else None),
            ("Partition type:", v.parttype or None),
            ("File system:", f"{v.fs_name} {v.fsver}".strip() if v.fstype else None),
            ("UUID:", v.uuid or None),
            ("Offset:", f"{v.start:,} bytes" if v.kind == "part" else None),
            ("Mount points:", "\n".join(v.mounts) if v.mounts else "None"),
        ]), False, False, 0)

    def apply(self):
        new = self.label.get_text().strip()
        if new == self.v.label:
            return
        fsp = block_proxy(self.v.kname, "Filesystem")
        ui.call(fsp, "SetLabel", "(sa{sv})", new.upper()[:11] if self.v.fstype == "vfat" else new, {},
                done=self.view.busy("Couldn\u2019t rename the volume"))


class DiskSheet:
    def __init__(self, view, d):
        self.view, self.d = view, d
        name = (f"{d.vendor} {d.model}".strip() if d.vendor and d.vendor not in ("ATA",) else d.model) or d.title
        self.win = ui.PropertySheet(view.console.win, f"{name} Properties", width=460, height=540, buttons=False)
        box = self.win.page("General")
        box.pack_start(ui.sheet_header(ui.pick_icon("drive-optical" if d.rom else "drive-harddisk"), name), False, False, 0)
        box.pack_start(Gtk.Separator(), False, False, 0)
        box.pack_start(ui.kv_grid([("Device type:", "DVD/CD-ROM drives" if d.rom else "Disk drives"),
                                   ("Manufacturer:", "(Standard disk drives)"),
                                   ("Location:", d.tran.upper() or ("Virtual disk" if d.loop else "Unknown")),
                                   ("Serial number:", d.serial or None)]), False, False, 0)
        box.pack_start(ui.label("Device status", "caption"), False, False, 0)
        box.pack_start(ui.text_box("This device is working properly.", 70), False, False, 0)
        self.health(box)

        vbox = self.win.page("Volumes")
        vbox.pack_start(ui.kv_grid([("Disk:", d.title), ("Type:", d.kind), ("Status:", d.status),
                                    ("Partition style:", d.style or "Not initialized"),
                                    ("Capacity:", f"{d.size // MB:,} MB" if d.size else "No Media"),
                                    ("Unallocated space:", f"{d.unallocated() // MB:,} MB"),
                                    ("Reserved space:", "1 MB" if d.pttype else "0 MB")]), False, False, 0)
        vbox.pack_start(ui.label("Volumes:", "caption"), False, False, 0)
        rows = [(s.name, f"{s.size // MB:,} MB") for s in d.segments if s.kind != "unalloc"]
        vbox.pack_start(ui.simple_list(rows or [("(none)", "")], 160, headers=["Volume", "Capacity"]), False, False, 0)

        pbox = self.win.page("Policies")
        pbox.pack_start(ui.label("Write-caching policy", "summary-section"), False, False, 0)
        wc = ui.sysfs(f"/sys/class/block/{d.kname}/queue", "write_cache")
        pbox.pack_start(ui.label(f"Write caching on the device: {'enabled' if wc == 'write back' else 'disabled' if wc else 'unknown'}.\n\n"
                                 "Linux flushes the cache on shutdown and when a volume is unmounted, so removable "
                                 "disks can be unplugged once Eject finishes.", wrap=True), False, False, 0)
        self.win.present_sheet()

    def health(self, box):
        blk = block_proxy(self.d.kname)
        drive = ui.prop(blk, "Drive")
        if not drive or drive == "/":
            return
        ata = ui.proxy(UD, drive, f"{UD}.Drive.Ata")
        if not ata or ui.prop(ata, "SmartSupported") is None or not ui.prop(ata, "SmartSupported"):
            return
        temp = ui.prop(ata, "SmartTemperature") or 0
        hours = (ui.prop(ata, "SmartPowerOnSeconds") or 0) // 3600
        failing = ui.prop(ata, "SmartFailing")
        box.pack_start(ui.label("Health", "summary-section"), False, False, 0)
        box.pack_start(ui.kv_grid([
            ("SMART status:", "Failing \u2013 back up this disk now" if failing else "Healthy"),
            ("Temperature:", f"{temp - 273.15:.0f} \u00b0C" if temp else None),
            ("Powered on:", f"{hours:,} hours" if hours else None)]), False, False, 0)


def node():
    return Node(NAME, ICON, DiskView, key=KEY, description=DESCRIPTION)
