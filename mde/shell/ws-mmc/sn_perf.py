# Performance Monitor: a live graph of processor, memory, disk and network.
import os
import time

import cairo

import mmcui as ui
from mmcui import Action, Node
from gi.repository import GLib, Gtk, Pango, PangoCairo

KEY = "perfmon"
NAME = "Performance Monitor"
ICON = "utilities-system-monitor"
DESCRIPTION = "See how this computer is using its processor, memory, disks and network"

SAMPLES = 60
SERIES = [
    ("cpu", "Processor", (0.00, 0.47, 0.83)),
    ("mem", "Memory", (0.85, 0.45, 0.13)),
    ("disk", "Disk", (0.18, 0.62, 0.30)),
    ("net", "Network", (0.49, 0.30, 0.76)),
]


def read_cpu():
    for line in open("/proc/stat"):
        if line.startswith("cpu "):
            parts = [int(x) for x in line.split()[1:]]
            idle = parts[3] + (parts[4] if len(parts) > 4 else 0)
            return idle, sum(parts)
    return 0, 1


def read_mem():
    info = {}
    for line in open("/proc/meminfo"):
        k, _, v = line.partition(":")
        info[k] = int(v.strip().split()[0])
    total = info.get("MemTotal", 1)
    avail = info.get("MemAvailable", info.get("MemFree", 0))
    return total, total - avail


def read_disk():
    """Bytes transferred on whole disks (sda, nvme0n1), skipping partitions."""
    total = 0
    for line in open("/proc/diskstats"):
        p = line.split()
        if len(p) < 14:
            continue
        name = p[2]
        whole = (name.startswith(("sd", "vd", "hd")) and name[-1:].isalpha()) or (
            name.startswith("nvme") and "p" not in name) or (
            name.startswith("mmcblk") and "p" not in name)
        if whole:
            total += int(p[5]) + int(p[9])
    return total * 512


def read_net():
    rx = tx = 0
    for line in open("/proc/net/dev"):
        if ":" not in line:
            continue
        iface, _, rest = line.partition(":")
        if iface.strip() == "lo":
            continue
        parts = rest.split()
        rx += int(parts[0])
        tx += int(parts[8])
    return rx + tx


class PerfView(ui.View):
    def __init__(self, console, node):
        super().__init__(console, node)
        self.enabled = {k: True for k, _l, _c in SERIES}
        self.samples = {k: [] for k, _l, _c in SERIES}
        self.prev = None
        self.timer = None
        self.current = {}
        self.area = Gtk.DrawingArea(vexpand=True, hexpand=True)
        self.area.connect("draw", self.draw)
        legend = Gtk.Box(spacing=16)
        legend.set_margin_start(12)
        legend.set_margin_bottom(8)
        for key, label, rgb in SERIES:
            b = Gtk.CheckButton(label=label, active=True)
            b.connect("toggled", lambda w, k=key: self.toggle(k, w.get_active()))
            sw = Gtk.DrawingArea()
            sw.set_size_request(14, 14)
            sw.set_valign(Gtk.Align.CENTER)
            sw.connect("draw", lambda _w, cr, c=rgb: (cr.set_source_rgb(*c), cr.rectangle(1, 1, 12, 12), cr.fill()))
            legend.pack_start(sw, False, False, 0)
            legend.pack_start(b, False, False, 0)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box.pack_start(self.area, True, True, 0)
        box.pack_start(legend, False, False, 0)
        self.widget = box

    def toggle(self, key, on):
        self.enabled[key] = on
        self.area.queue_draw()

    def shown(self):
        self.prev = self.sample()
        if self.timer is None:
            self.timer = GLib.timeout_add(1000, self.tick)

    def hidden(self):
        if self.timer is not None:
            GLib.source_remove(self.timer)
            self.timer = None

    def sample(self):
        try:
            return {"t": time.monotonic(), "cpu": read_cpu(), "mem": read_mem(),
                    "disk": read_disk(), "net": read_net()}
        except OSError:
            return None

    def tick(self):
        cur = self.sample()
        prev = self.prev
        self.prev = cur
        if not cur or not prev:
            return True
        dt = max(0.2, cur["t"] - prev["t"])
        idle = cur["cpu"][0] - prev["cpu"][0]
        total = cur["cpu"][1] - prev["cpu"][1]
        cpu = 100 * (1 - idle / total) if total else 0
        mem = 100 * cur["mem"][1] / cur["mem"][0] if cur["mem"][0] else 0
        disk = (cur["disk"] - prev["disk"]) / dt / (1024 * 1024)
        net = (cur["net"] - prev["net"]) / dt / (1024 * 1024)
        self.current = {"cpu": max(0, cpu), "mem": mem, "disk": max(0, disk), "net": max(0, net)}
        for k, v in self.current.items():
            self.samples[k].append(v)
            del self.samples[k][:-SAMPLES]
        self.area.queue_draw()
        bits = [f"{label} {self.text(k)}" for k, label, _c in SERIES if self.enabled[k]]
        self.status("   ".join(bits))
        return True

    def text(self, key):
        v = self.current.get(key, 0)
        if key in ("cpu", "mem"):
            return f"{v:.0f}%"
        return f"{v:.1f} MB/s"

    def scale(self, key):
        # Processor and memory are a percentage. Disk and network are MB/s, and
        # the top of the graph is 100 MB/s so a quiet disk stays near the bottom.
        return 100

    def draw(self, area, cr):
        w, h = area.get_allocated_width(), area.get_allocated_height()
        cr.set_source_rgb(1, 1, 1)
        cr.paint()
        left, right, top, bottom = 48, 12, 16, 24
        gw, gh = max(10, w - left - right), max(10, h - top - bottom)
        cr.set_source_rgba(0, 0, 0, 0.08)
        cr.set_line_width(1)
        for i in range(5):
            y = top + gh * i / 4
            cr.move_to(left, y + 0.5)
            cr.line_to(left + gw, y + 0.5)
        cr.stroke()
        cr.set_source_rgb(0.4, 0.4, 0.4)
        layout = area.create_pango_layout("100")
        layout.set_font_description(Pango.FontDescription("9"))
        cr.move_to(4, top - 6)
        PangoCairo.show_layout(cr, layout)
        layout.set_text("0")
        cr.move_to(4, top + gh - 12)
        PangoCairo.show_layout(cr, layout)
        for key, _label, rgb in SERIES:
            if not self.enabled[key]:
                continue
            data = self.samples[key]
            if len(data) < 2:
                continue
            scale = self.scale(key)
            cr.set_source_rgb(*rgb)
            cr.set_line_width(1.6)
            started = False
            span = max(len(data) - 1, 1)
            for i, v in enumerate(data):
                x = left + gw * i / span
                y = top + gh * (1 - max(0, min(v, scale)) / scale)
                if not started:
                    cr.move_to(x, y)
                    started = True
                else:
                    cr.line_to(x, y)
            cr.stroke()
        return False

    def actions(self):
        return [Action("Clear", self.clear, "edit-clear-symbolic")]

    def clear(self):
        for k in self.samples:
            self.samples[k].clear()
        self.area.queue_draw()


def node():
    return Node(NAME, ICON, PerfView, key=KEY, description=DESCRIPTION)
