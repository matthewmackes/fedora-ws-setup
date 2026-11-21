# Device Manager: the hardware in this PC from the udev database and /sys, sorted into
# the Windows categories, with problem badges, property sheets and disable/enable.
import glob
import json
import os
import platform
import re
import socket
import subprocess
import threading

import cairo

import mmcui as ui
from mmcui import Action, Node
from gi.repository import Gdk, GdkPixbuf, GLib, Gtk, Pango

KEY = "devmgmt"
NAME = "Device Manager"
ICON = "computer"
DESCRIPTION = "View and update your hardware\u2019s settings and driver software"

STATE_FILE = "/etc/ws-mmc/disabled-devices"
PREFS = os.path.join(ui.wsui.CONFIG, "ws-mmc", "devmgmt.json")

CATEGORIES = {
    "Audio inputs and outputs": ("audio-speakers", "audio-card"),
    "Batteries": ("battery",),
    "Bluetooth": ("bluetooth",),
    "Cameras": ("camera-web",),
    "Computer": ("computer",),
    "Disk drives": ("drive-harddisk",),
    "Display adapters": ("video-display",),
    "DVD/CD-ROM drives": ("drive-optical",),
    "Firmware": ("application-x-firmware",),
    "Human Interface Devices": ("input-gaming",),
    "IEEE 1394 host controllers": ("network-wired",),
    "Keyboards": ("input-keyboard",),
    "Memory technology devices": ("media-flash",),
    "Mice and other pointing devices": ("input-mouse",),
    "Monitors": ("video-display",),
    "Network adapters": ("network-wired", "network-workgroup"),
    "Other devices": ("dialog-question",),
    "Ports (COM & LPT)": ("modem",),
    "Print queues": ("printer",),
    "Processors": ("cpu", "processor", "applications-system"),
    "Security devices": ("security-high",),
    "Sensors": ("sensors-temperature", "utilities-system-monitor"),
    "Software devices": ("application-x-executable",),
    "Sound, video and game controllers": ("audio-card",),
    "Storage controllers": ("drive-harddisk-system", "drive-harddisk"),
    "Storage volumes": ("drive-harddisk",),
    "System devices": ("applications-system", "computer"),
    "Universal Serial Bus controllers": ("drive-removable-media-usb", "media-removable"),
    "Universal Serial Bus devices": ("drive-removable-media-usb", "media-removable"),
}

# Categories whose devices can be disabled without taking the session down with them.
DISABLE_OK = {"Network adapters", "Bluetooth", "Cameras", "Sound, video and game controllers",
              "Universal Serial Bus devices", "Other devices", "Memory technology devices",
              "Human Interface Devices", "Keyboards", "Mice and other pointing devices",
              "Disk drives", "DVD/CD-ROM drives", "Ports (COM & LPT)"}

ACPI_NAMES = {
    "PNP0000": "Programmable interrupt controller", "PNP0100": "System timer",
    "PNP0103": "High precision event timer", "PNP0200": "Direct memory access controller",
    "PNP0800": "System speaker", "PNP0B00": "System CMOS/real time clock", "PNP0C01": "System board",
    "PNP0C02": "Motherboard resources", "PNP0C04": "Numeric data processor",
    "PNP0C09": "Embedded Controller", "PNP0C0B": "ACPI Fan", "PNP0C0C": "ACPI Power Button",
    "PNP0C0D": "ACPI Lid", "PNP0C0E": "ACPI Sleep Button",
    "PNP0C14": "Windows Management Interface for ACPI", "PNP0A08": "PCI Express Root Complex",
    "PNP0A03": "PCI Bus", "LNXPWRBN": "ACPI Fixed Feature Button", "LNXTHERM": "ACPI Thermal Zone",
    "ACPI000C": "ACPI Processor Aggregator", "ACPI000E": "ACPI Wake Alarm",
    "USBC000": "UCM-UCSI ACPI Device", "INT0800": "Intel 82802 Firmware Hub Device",
    "INT3400": "Intel Dynamic Platform and Thermal Framework Manager",
    "INT3403": "Intel Dynamic Platform and Thermal Framework Generic Participant",
    "INT33A1": "Intel Power Engine Plug-in", "LEN0268": "Lenovo PM Device",
    "AMDI0010": "AMD I2C Controller", "AMDI0030": "AMD GPIO Controller",
}

OTHER_NAMES = {0x01: "Mass Storage Controller", 0x02: "Network Controller", 0x03: "Video Controller",
               0x04: "Multimedia Controller", 0x07: "PCI Simple Communications Controller",
               0x08: "Base System Device", 0x0c: "Unknown device", 0x0d: "Network Controller",
               0x10: "PCI Encryption/Decryption Controller",
               0x11: "PCI Data Acquisition and Signal Processing Controller",
               0x12: "Processing Accelerator"}

STATUS = {0: "This device is working properly.",
          22: "This device is disabled. (Code 22)",
          28: "The drivers for this device are not installed. (Code 28)\n\n"
              "There are no compatible drivers for this device.\n\n"
              "To find a driver for this device, click Update Driver."}

VENDOR_SUFFIX = re.compile(
    r"(,|\s+(Inc|Corp|Corporation|Co|Ltd|Limited|LLC|GmbH|AG|SA|Technology|Technologies|Semiconductor|"
    r"Electronics|Group|International|Microelectronics|Systems|Incorporated)\b).*$", re.I)


def short_vendor(v):
    if not v:
        return ""
    if v.startswith("Advanced Micro Devices"):
        return "AMD"
    v = re.sub(r"\s*\[.*?\]", "", v)
    return VENDOR_SUFFIX.sub("", v).strip() or v


def decode(s):
    """udev's \\x20 escapes (ID_MODEL_ENC and friends)."""
    return re.sub(r"\\x([0-9a-fA-F]{2})", lambda m: chr(int(m.group(1), 16)), s or "").strip()


def category_icon(cat):
    return ui.pick_icon(*CATEGORIES.get(cat, ("application-x-executable",)))


def read(path, name, default=""):
    return ui.sysfs("/sys" + path if path.startswith("/devices") else path, name, default)


class Device:
    def __init__(self, name, category, path=None, manufacturer="", location="", ctl=None, bus=None,
                 hidden=False, problem=0, info=None, key=None, icon=None, parent=None):
        self.name, self.category, self.path = name, category, path
        self.manufacturer, self.location = manufacturer, location
        self.ctl, self.bus = ctl, bus          # the PCI or USB device that disable/enable acts on
        self.hidden, self.problem = hidden, problem
        self.info = info or {}                 # extra General-tab rows
        self.key = key or path or f"{category}:{name}"
        self.icon = icon or category_icon(category)
        self.parent = parent                   # connection-view parent path, when not in the path
        self.drv_path = None
        self.driver = ""
        self.module = ""
        self.rec = None


# --- Scanning ------------------------------------------------------------------------

def udev_db():
    try:
        text = subprocess.run(["udevadm", "info", "--export-db"], capture_output=True, text=True,
                              timeout=20).stdout
    except (OSError, subprocess.TimeoutExpired):
        return {}
    recs = {}
    for block in text.split("\n\n"):
        r = {"E": {}, "S": []}
        for line in block.splitlines():
            if len(line) < 3:
                continue
            t, v = line[0], line[3:]
            if t == "P":
                r["P"] = v
            elif t == "N":
                r["N"] = v
            elif t == "S":
                r["S"].append(v)
            elif t == "E":
                k, _, val = v.partition("=")
                r["E"][k] = val
        if "P" in r:
            recs[r["P"]] = r
    return recs


def disabled_records():
    out = {}
    for line in ui.read_file(STATE_FILE).splitlines():
        parts = line.split("\t")
        if len(parts) >= 5 and parts[0].startswith("/devices/"):
            out[parts[0]] = {"bus": parts[1], "driver": parts[2], "category": parts[3], "name": parts[4]}
    return out


def root_devices():
    """sysfs paths of the block devices that hold / , /boot and /home (through dm and md)."""
    paths = set()

    def add(name):
        real = os.path.realpath(f"/sys/class/block/{name}")
        if not os.path.exists(real) or real in paths:
            return
        paths.add(real[4:])
        for s in glob.glob(f"/sys/class/block/{name}/slaves/*"):
            add(os.path.basename(s))
    for mp in ("/", "/boot", "/home", "/usr", "/var"):
        src = ui.out(["findmnt", "-rno", "SOURCE", mp]).strip().split("[")[0]
        if src.startswith("/dev/"):
            add(os.path.basename(os.path.realpath(src)))
    return paths


class Scanner:
    def __init__(self):
        self.dmi_model = ui.sysfs("/sys/class/dmi/id", "product_version") or ""
        self.devices = []
        self.claimed = set()   # USB device paths that a function device already represents

    def run(self):
        self.recs = udev_db()
        self.disabled = disabled_records()
        self.roots = root_devices()
        for build in (self.computer, self.processors, self.firmware, self.pci, self.hdaudio,
                      self.network, self.bluetooth, self.cameras, self.inputs, self.hid, self.disks,
                      self.sound_usb, self.ports, self.power, self.tpm, self.monitors, self.sensors,
                      self.acpi, self.usb, self.audio_endpoints, self.printers):
            try:
                build()
            except Exception as e:  # one odd device must not hide all the others
                import traceback
                traceback.print_exception(e)
        self.apply_disabled()
        for d in self.devices:
            self.find_driver(d)
        return self.devices

    # helpers

    def add(self, dev):
        self.devices.append(dev)
        return dev

    def E(self, path, key, default=""):
        r = self.recs.get(path)
        return r["E"].get(key, default) if r else default

    def ancestor(self, path, pred):
        while path and path != "/devices":
            r = self.recs.get(path)
            if r and pred(r):
                return path
            path = os.path.dirname(path)
        return None

    def usb_dev(self, path):
        return self.ancestor(path, lambda r: r["E"].get("DEVTYPE") == "usb_device")

    def pci_dev(self, path):
        return self.ancestor(path, lambda r: r["E"].get("SUBSYSTEM") == "pci")

    def control(self, path):
        """The USB or PCI device to disable for a function device, and its bus."""
        u = self.usb_dev(path)
        if u and not os.path.basename(u).startswith("usb"):
            return u, "usb"
        p = self.pci_dev(path)
        return (p, "pci") if p else (None, None)

    def model_name(self, path, vendor_prefix=True):
        e = self.recs.get(path, {}).get("E", {})
        model = e.get("ID_MODEL_FROM_DATABASE") or decode(e.get("ID_MODEL_ENC")) or \
            e.get("ID_MODEL", "").replace("_", " ")
        vendor = e.get("ID_VENDOR_FROM_DATABASE") or decode(e.get("ID_VENDOR_ENC"))
        return self.clean(model, vendor if vendor_prefix else "")

    def clean(self, model, vendor="", marketing=False):
        m = (model or "").strip()
        if m.endswith(")") and "(" in m:
            head, _, sub = m[:-1].rpartition(" (")
            if head:
                m = head if sub == self.dmi_model or "ThinkPad" in sub or "/" in sub else sub
        bracket = re.search(r"\[([^\]]+)\]", m)
        if bracket:
            m = bracket.group(1) if marketing else re.sub(r"\s*\[[^\]]+\]", "", m)
        sv = short_vendor(vendor)
        if sv and m and not m.lower().startswith(sv.lower().split()[0]):
            m = f"{sv} {m}"
        return m.strip()

    def usb_location(self, path):
        name = os.path.basename(path).split(":")[0]
        if name.startswith("usb"):
            return f"USB bus {name[3:]}"
        bus, _, ports = name.partition("-")
        port = ports.split(".")[-1] if ports else "0"
        try:
            return f"Port_#{int(port):04d}.Hub_#{int(bus):04d}"
        except ValueError:
            return name

    # categories

    def computer(self):
        arch = {"x86_64": "x64", "aarch64": "ARM64", "i686": "x86"}.get(platform.machine(), platform.machine())
        dmi = "/sys/class/dmi/id"
        self.add(Device(f"ACPI {arch}-based PC", "Computer", "/devices/LNXSYSTM:00",
                        manufacturer="(Standard computers)", key="computer", info={
                            "System manufacturer": ui.sysfs(dmi, "sys_vendor") or None,
                            "System model": " ".join(filter(None, (ui.sysfs(dmi, "product_version"),
                                                                   ui.sysfs(dmi, "product_name")))) or None}))

    def processors(self):
        text = ui.read_file("/proc/cpuinfo")
        for block in text.strip().split("\n\n"):
            info = dict((k.strip(), v.strip()) for k, _, v in (l.partition(":") for l in block.splitlines()))
            if "processor" not in info:
                continue
            n = info["processor"]
            vendor = {"GenuineIntel": "Intel", "AuthenticAMD": "AMD"}.get(info.get("vendor_id"), info.get("vendor_id", ""))
            name = info.get("model name") or info.get("Hardware") or "Processor"
            d = self.add(Device(name, "Processors", f"/devices/system/cpu/cpu{n}", manufacturer=vendor,
                                location=f"Processor {n}", parent="/devices/LNXSYSTM:00"))
            d.driver = ui.sysfs(f"/sys/devices/system/cpu/cpu{n}/cpufreq", "scaling_driver")

    def firmware(self):
        dmi = "/sys/class/dmi/id"
        if not os.path.isdir(dmi):
            return
        d = self.add(Device("System Firmware", "Firmware", "/devices/virtual/dmi/id",
                            manufacturer=ui.sysfs(dmi, "bios_vendor"), key="firmware",
                            parent="/devices/LNXSYSTM:00", info={
                                "Version": ui.sysfs(dmi, "bios_version") or None,
                                "Release date": ui.sysfs(dmi, "bios_date") or None,
                                "Firmware type": "UEFI" if os.path.isdir("/sys/firmware/efi") else "BIOS (legacy)"}))
        d.hidden = False

    def pci(self):
        for path, r in self.recs.items():
            e = r["E"]
            if e.get("SUBSYSTEM") != "pci":
                continue
            try:
                cls = int(read(path, "class", "0"), 16)
            except ValueError:
                cls = 0
            base, sub = cls >> 16, (cls >> 8) & 0xff
            slot = os.path.basename(path)
            dom, bus, devfn = (slot.split(":") + ["", "", ""])[:3]
            dev, _, fn = devfn.partition(".")
            location = f"PCI bus {int(bus, 16)}, device {int(dev, 16)}, function {int(fn or '0', 16)}" \
                if bus and dev else slot
            vendor = e.get("ID_VENDOR_FROM_DATABASE", "")
            name = self.clean(e.get("ID_MODEL_FROM_DATABASE") or f"PCI Device {slot}", vendor,
                              marketing=base == 0x03)
            cat = pci_category(base, sub)
            problem = 0
            driver = e.get("DRIVER") or os.path.basename(os.path.realpath(f"/sys{path}/driver")) \
                if os.path.exists(f"/sys{path}/driver") else ""
            if not driver and base not in (0x05, 0x06, 0x0b):
                cat, problem = "Other devices", 28
                name = OTHER_NAMES.get(base, "PCI Device")
                if base == 0x0c and sub == 0x05:
                    name = "SM Bus Controller"
            if base == 0x04 and sub == 0x03:
                name = "High Definition Audio Controller"
            elif cat == "System devices":
                name += f" - {read(path, 'device', '0x')[2:].upper()}"
            if base == 0x07 and sub == 0x00:
                ttys = glob.glob(f"/sys{path}/tty/tty*") + glob.glob(f"/sys{path}/*/tty/tty*") + \
                    glob.glob(f"/sys{path}/*/*/tty/tty*")
                if ttys:
                    name += f" ({os.path.basename(ttys[0])})"
            info = {}
            nets = glob.glob(f"/sys{path}/net/*")
            if nets:
                info["Network connection"] = ", ".join(os.path.basename(n) for n in nets)
            self.add(Device(name, cat, path, manufacturer=vendor, location=location, ctl=path, bus="pci",
                            problem=problem, info=info))

    def hdaudio(self):
        for path, r in self.recs.items():
            if r["E"].get("SUBSYSTEM") != "hdaudio":
                continue
            vendor = read(path, "vendor_name")
            chip = read(path, "chip_name")
            if "hdmi" in r["E"].get("DRIVER", "") or "HDMI" in chip:
                name = f"{short_vendor(vendor) or 'Intel'} Display Audio"
            else:
                name = f"{vendor} High Definition Audio".strip()
            self.add(Device(name, "Sound, video and game controllers", path, manufacturer=vendor,
                            location="Internal High Definition Audio Bus",
                            info={"Codec": f"{vendor} {chip}".strip() or None}))

    def network(self):
        for path, r in self.recs.items():
            e = r["E"]
            if e.get("SUBSYSTEM") != "net":
                continue
            iface = os.path.basename(path)
            if iface == "lo":
                continue
            if path.startswith("/devices/virtual/"):
                kind = e.get("DEVTYPE") or ("bridge" if os.path.isdir(f"/sys{path}/bridge") else "virtual")
                self.add(Device(f"Virtual Network Adapter ({iface})", "Network adapters", path,
                                manufacturer="Linux", location=f"{kind} interface", hidden=True,
                                info={"Network connection": iface}))
                continue
            ctl, bus = self.control(path)
            if bus == "pci" and not self.usb_dev(path):
                continue  # the PCI function itself is the adapter
            if bus == "usb":
                self.claimed.add(ctl)
            name = self.model_name(ctl or path) or f"Network Adapter ({iface})"
            self.add(Device(name, "Network adapters", path,
                            manufacturer=self.E(ctl, "ID_VENDOR_FROM_DATABASE") or decode(e.get("ID_VENDOR_ENC")),
                            location=self.usb_location(ctl) if bus == "usb" else "",
                            ctl=ctl, bus=bus, info={"Network connection": iface}))

    def bluetooth(self):
        for path, r in self.recs.items():
            if r["E"].get("SUBSYSTEM") != "bluetooth" or r["E"].get("DEVTYPE") != "host":
                continue
            ctl, bus = self.control(path)
            if bus == "usb":
                self.claimed.add(ctl)
            name = self.model_name(ctl) if ctl else "Bluetooth Adapter"
            if "bluetooth" not in name.lower():
                name += " Bluetooth Adapter"
            self.add(Device(name, "Bluetooth", path, manufacturer=self.E(ctl, "ID_VENDOR_FROM_DATABASE"),
                            location=self.usb_location(ctl) if bus == "usb" else "", ctl=ctl, bus=bus,
                            info={"Adapter": os.path.basename(path)}))

    def cameras(self):
        seen = set()
        for path, r in sorted(self.recs.items()):
            if r["E"].get("SUBSYSTEM") != "video4linux":
                continue
            caps = read(path, "name")
            ctl, bus = self.control(path)
            owner = ctl or os.path.dirname(os.path.dirname(path))
            if owner in seen:
                continue
            seen.add(owner)
            if bus == "usb":
                self.claimed.add(ctl)
                name = read(ctl, "product") or self.model_name(ctl)
            else:
                name = caps or "Camera"
            self.add(Device(name, "Cameras", os.path.dirname(os.path.dirname(path)),
                            manufacturer=read(ctl, "manufacturer") if bus == "usb" else "",
                            location=self.usb_location(ctl) if bus == "usb" else "", ctl=ctl, bus=bus,
                            info={"Video device": ", ".join(sorted(
                                os.path.basename(p) for p in glob.glob(f"/sys{os.path.dirname(path)}/video*")))}))

    def inputs(self):
        for path, r in self.recs.items():
            e = r["E"]
            if e.get("SUBSYSTEM") != "input" or not re.search(r"/input\d+$", path):
                continue
            name = e.get("NAME", "").strip('"')
            if "/sound/card" in path or "/acpi.video_bus" in path or "LNXPWRBN" in path \
                    or re.search(r"/PNP0C0[CDE]:", path):
                continue
            ctl, bus = self.control(path)
            usb = bus == "usb"
            if usb:
                self.claimed.add(ctl)
            builtin = "/serio" in path or "/rmi4" in path or "/i2c-" in path
            if path.startswith("/devices/virtual/"):
                cat, label, hidden = "Software devices", name or "Virtual input device", True
            elif e.get("ID_INPUT_KEYBOARD") == "1":
                cat, hidden = "Keyboards", False
                label = "Standard PS/2 Keyboard" if "/serio" in path else "HID Keyboard Device"
            elif any(e.get(k) == "1" for k in ("ID_INPUT_TOUCHPAD", "ID_INPUT_MOUSE", "ID_INPUT_POINTINGSTICK",
                                               "ID_INPUT_TABLET")):
                cat, hidden = "Mice and other pointing devices", False
                label = name if builtin else "HID-compliant mouse"
            elif e.get("ID_INPUT_TOUCHSCREEN") == "1":
                cat, label, hidden = "Human Interface Devices", "HID-compliant touch screen", False
            elif e.get("ID_INPUT_JOYSTICK") == "1":
                cat, label, hidden = "Human Interface Devices", "HID-compliant game controller", False
            elif usb or "bluetooth" in path or "/uhid/" in path:
                cat, hidden = "Human Interface Devices", False
                label = "HID-compliant consumer control device" if "Consumer" in name else \
                    "HID-compliant system controller" if "System" in name else f"HID-compliant device"
            elif "pcspkr" in path:
                cat, label, hidden = "System devices", "System speaker", False
            else:
                cat, label, hidden = "System devices", name or "Input device", False
            self.add(Device(label, cat, path, manufacturer=self.E(ctl, "ID_VENDOR_FROM_DATABASE") if usb else
                            ("(Standard keyboards)" if cat == "Keyboards" else ""),
                            location=self.usb_location(ctl) if usb else (
                                "plugged into keyboard port" if "/serio" in path else ""),
                            ctl=ctl if usb else None, bus=bus if usb else None, hidden=hidden,
                            info={"Reported name": name or None}))

    def hid(self):
        for path, r in self.recs.items():
            e = r["E"]
            drv = e.get("DRIVER", "")
            if e.get("DEVTYPE") == "usb_interface" and drv == "usbhid":
                ctl = self.usb_dev(path)
                self.claimed.add(ctl)
                self.add(Device("USB Input Device", "Human Interface Devices", path,
                                manufacturer="(Standard system devices)", location=self.usb_location(ctl),
                                ctl=ctl, bus="usb"))
            elif drv.startswith("i2c_hid"):
                self.add(Device("I2C HID Device", "Human Interface Devices", path,
                                manufacturer="(Standard system devices)", location="I2C bus"))
            elif e.get("SUBSYSTEM") == "hid" and "/uhid/" in path:
                self.add(Device("Bluetooth Low Energy GATT compliant HID device", "Human Interface Devices",
                                path, info={"Reported name": e.get("HID_NAME")}))

    def disks(self):
        for path, r in self.recs.items():
            e = r["E"]
            if e.get("SUBSYSTEM") != "block":
                continue
            kname = os.path.basename(path)
            if re.match(r"(loop|ram|zram|dm-|md|nbd)", kname):
                continue
            ctl, bus = self.control(path)
            if e.get("DEVTYPE") == "partition":
                size = int(read(path, "size", "0") or 0) * 512
                self.add(Device(f"Volume ({kname})", "Storage volumes", path, manufacturer="Linux",
                                location=e.get("ID_FS_LABEL") or kname, hidden=True,
                                info={"Size": ui.size_text(size), "File system": e.get("ID_FS_TYPE") or None}))
                continue
            if e.get("DEVTYPE") != "disk":
                continue
            model = decode(e.get("ID_MODEL_ENC")) or e.get("ID_MODEL", "").replace("_", " ") or kname
            vendor = decode(e.get("ID_VENDOR_ENC")) or e.get("ID_VENDOR", "")
            cdrom = e.get("ID_CDROM") == "1" or kname.startswith("sr")
            usb = bus == "usb"
            if usb:
                self.claimed.add(ctl)
                name = f"{vendor} {model} USB Device".strip()
            elif kname.startswith("mmcblk"):
                name = f"{read(os.path.dirname(os.path.dirname(path)), 'type') or 'SD'} Card".replace("SD Card", "SDHC Card")
            else:
                name = model
            size = int(read(path, "size", "0") or 0) * 512
            on_root = any(rp.startswith(path) for rp in self.roots)
            self.add(Device(name, "DVD/CD-ROM drives" if cdrom else "Disk drives", path,
                            manufacturer="(Standard disk drives)",
                            location=self.usb_location(ctl) if usb else e.get("ID_PATH", ""),
                            ctl=None if on_root else (ctl if usb else None), bus=bus if usb else None,
                            info={"Capacity": ui.size_text(size) if size else "No media",
                                  "Device": f"/dev/{kname}", "Serial number": e.get("ID_SERIAL_SHORT") or None}))

    def sound_usb(self):
        for path, r in self.recs.items():
            if r["E"].get("SUBSYSTEM") != "sound" or not re.search(r"/card\d+$", path):
                continue
            ctl, bus = self.control(path)
            if bus != "usb":
                continue
            self.claimed.add(ctl)
            self.add(Device(read(ctl, "product") or self.model_name(ctl) or "USB Audio Device",
                            "Sound, video and game controllers", path, manufacturer=read(ctl, "manufacturer"),
                            location=self.usb_location(ctl), ctl=ctl, bus="usb"))

    def ports(self):
        for path, r in self.recs.items():
            if r["E"].get("SUBSYSTEM") != "tty" or path.startswith("/devices/virtual/"):
                continue
            tty = os.path.basename(path)
            ctl, bus = self.control(path)
            if bus == "usb":
                self.claimed.add(ctl)
                self.add(Device(f"USB Serial Device ({tty})", "Ports (COM & LPT)", path,
                                manufacturer=read(ctl, "manufacturer"), location=self.usb_location(ctl),
                                ctl=ctl, bus="usb"))
            elif "/pnp0/" in path or ("serial8250" in path and read(path, "type", "0") not in ("0", "")):
                self.add(Device(f"Communications Port ({tty})", "Ports (COM & LPT)", path,
                                manufacturer="(Standard port types)"))

    def power(self):
        for path, r in self.recs.items():
            if r["E"].get("SUBSYSTEM") != "power_supply":
                continue
            kind = read(path, "type")
            if kind == "Battery" and read(path, "scope") != "Device":
                full, design = read(path, "energy_full") or read(path, "charge_full"), \
                    read(path, "energy_full_design") or read(path, "charge_full_design")
                info = {"Battery": os.path.basename(path), "Model": read(path, "model_name") or None,
                        "Manufacturer": read(path, "manufacturer") or None,
                        "Charge": f"{read(path, 'capacity')}%" if read(path, "capacity") else None}
                if full and design and int(design):
                    info["Health"] = f"{100 * int(full) // int(design)}% of design capacity"
                self.add(Device("ACPI-Compliant Control Method Battery", "Batteries", path,
                                manufacturer="(Standard battery)", info=info))
            elif kind == "Mains":
                self.add(Device("AC Adapter", "Batteries", path, manufacturer="(Standard battery)",
                                info={"Plugged in": "Yes" if read(path, "online") == "1" else "No"}))

    def tpm(self):
        for path, r in self.recs.items():
            if r["E"].get("SUBSYSTEM") != "tpm":
                continue
            major = read(path, "tpm_version_major") or "2"
            parent = os.path.dirname(os.path.dirname(path))
            self.add(Device(f"Trusted Platform Module {major}.0", "Security devices", path,
                            manufacturer=self.E(parent, "ID_VENDOR_FROM_DATABASE") or "(Standard)",
                            location="ACPI"))

    def monitors(self):
        pnp = {}
        for line in ui.read_file("/usr/share/hwdata/pnp.ids").splitlines():
            code, _, name = line.partition("\t")
            pnp[code] = name
        for path, r in self.recs.items():
            if r["E"].get("DEVTYPE") != "drm_connector" or read(path, "status") != "connected":
                continue
            try:
                with open(f"/sys{path}/edid", "rb") as f:
                    edid = f.read()
            except OSError:
                edid = b""
            name, maker, serial = "Generic PnP Monitor", "(Standard monitor types)", None
            if len(edid) >= 128:
                v = (edid[8] << 8) | edid[9]
                code = "".join(chr(((v >> s) & 31) + 64) for s in (10, 5, 0))
                maker = pnp.get(code, code)
                for off in (54, 72, 90, 108):
                    blk = edid[off:off + 18]
                    if blk[:3] == b"\0\0\0" and blk[3] in (0xFC, 0xFF):
                        text = blk[5:].split(b"\n")[0].decode("latin-1").strip()
                        if blk[3] == 0xFC and text:
                            name = text
                        elif blk[3] == 0xFF:
                            serial = text
                if name == "Generic PnP Monitor" and maker:
                    name = f"Generic PnP Monitor ({short_vendor(maker)})"
            connector = os.path.basename(path).split("-", 1)[-1]
            self.add(Device(name, "Monitors", path, manufacturer=maker, location=f"Connector {connector}",
                            info={"Serial number": serial, "Connector": connector}))

    def sensors(self):
        for path, r in self.recs.items():
            if r["E"].get("SUBSYSTEM") == "iio" and re.search(r"iio:device\d+$", path):
                self.add(Device(read(path, "name") or "Sensor", "Sensors", path))

    def acpi(self):
        skip = {"LNXSYSTM", "LNXSYBUS", "LNXCPU", "LNXVIDEO", "LNXPOWER", "PNP0C0A", "ACPI0003",
                "MSFT0101", "PNP0303", "PNP0F13", "ACPI0007", "ACPI0010"}
        for path, r in self.recs.items():
            e = r["E"]
            if e.get("SUBSYSTEM") != "acpi" or not e.get("MODALIAS", "").startswith("acpi:"):
                continue
            ids = [i for i in e["MODALIAS"][5:].split(":") if i]
            if not ids or ids[0] in skip or any(i in skip for i in ids):
                continue
            name = next((ACPI_NAMES[i] for i in ids if i in ACPI_NAMES), None)
            if not name:
                continue
            status = read(path, "status")
            if status and status.isdigit() and not int(status) & 1:
                continue
            d = self.add(Device(name, "System devices", path, manufacturer="(Standard system devices)",
                                info={"ACPI ID": " ".join(ids)}))
            phys = f"/sys{path}/physical_node"
            if os.path.exists(phys):
                d.drv_path = os.path.realpath(phys)[4:]

    def usb(self):
        for path, r in self.recs.items():
            e = r["E"]
            if e.get("DEVTYPE") != "usb_device":
                continue
            name = os.path.basename(path)
            speed = read(path, "speed", "0")
            fast = speed.replace(".", "").isdigit() and float(speed) >= 5000
            cls = read(path, "bDeviceClass")
            loc = self.usb_location(path)
            ifaces = [p for p, rr in self.recs.items()
                      if os.path.dirname(p) == path and rr["E"].get("DEVTYPE") == "usb_interface"]
            if name.startswith("usb"):
                self.add(Device("USB Root Hub (USB 3.0)" if fast else "USB Root Hub",
                                "Universal Serial Bus controllers", path, manufacturer="(Standard USB HUBs)",
                                location=loc))
                continue
            if cls == "09":
                self.add(Device("Generic SuperSpeed USB Hub" if fast else "Generic USB Hub",
                                "Universal Serial Bus controllers", path, manufacturer="(Standard USB HUBs)",
                                location=loc, ctl=path, bus="usb"))
                continue
            for i in ifaces:
                if self.E(i, "DRIVER") in ("usb-storage", "uas"):
                    self.add(Device("USB Attached SCSI (UAS) Mass Storage Device" if self.E(i, "DRIVER") == "uas"
                                    else "USB Mass Storage Device", "Universal Serial Bus controllers", i,
                                    manufacturer="Compatible USB storage device", location=loc,
                                    ctl=path, bus="usb"))
            bound = [i for i in ifaces if self.E(i, "DRIVER")]
            product = read(path, "product") or self.model_name(path) or "Unknown USB Device"
            if len(ifaces) > 1:
                self.add(Device("USB Composite Device", "Universal Serial Bus controllers", path,
                                manufacturer="(Standard USB Host Controller)", location=loc, ctl=path, bus="usb",
                                info={"Reported name": product}))
            if path in self.claimed or any(self.E(i, "DRIVER") in ("usb-storage", "uas", "usbhid") for i in ifaces):
                continue
            if not bound:
                self.add(Device(product, "Other devices", path + ":fn", location=loc, ctl=path, bus="usb",
                                problem=28, manufacturer=read(path, "manufacturer"), parent=path))
            else:
                self.add(Device(product, "Universal Serial Bus devices", path + ":fn", location=loc,
                                manufacturer=read(path, "manufacturer"), ctl=path, bus="usb", parent=path))

    def audio_endpoints(self):
        env = dict(os.environ)
        env.setdefault("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")
        if not os.path.exists(os.path.join(env["XDG_RUNTIME_DIR"], "pulse", "native")):
            env["XDG_RUNTIME_DIR"] = f"/run/user/{os.getuid()}"
        by_id_path = {r["E"].get("ID_PATH"): p for p, r in self.recs.items()
                      if r["E"].get("SUBSYSTEM") in ("pci", "usb") and r["E"].get("ID_PATH")}
        for kind, label in (("sinks", "Speakers"), ("sources", "Microphone")):
            try:
                data = json.loads(subprocess.run(["pactl", "-f", "json", "list", kind], capture_output=True,
                                                 text=True, timeout=5, env=env).stdout or "[]")
            except (OSError, ValueError, subprocess.TimeoutExpired):
                return
            for s in data:
                props = s.get("properties", {})
                if s.get("name", "").endswith(".monitor") or props.get("device.class") == "monitor":
                    continue
                desc = s.get("description", s.get("name", ""))
                lab = "Headphones" if props.get("device.form_factor") == "headphone" else label
                if "HDMI" in desc or "DisplayPort" in desc:
                    lab = "Digital Display Audio"
                parent = by_id_path.get(props.get("device.bus_path")) or "/devices/LNXSYSTM:00"
                self.add(Device(f"{lab} ({desc})", "Audio inputs and outputs", None, key=f"audio:{s['name']}",
                                manufacturer=props.get("device.vendor.name") or "PipeWire", parent=parent,
                                location=props.get("device.bus_path", ""),
                                info={"Endpoint": s.get("name"), "State": s.get("state", "").title() or None}))

    def printers(self):
        if not os.path.exists("/usr/bin/lpstat"):
            return
        for line in ui.out(["lpstat", "-e"], timeout=3).splitlines():
            if line.strip():
                self.add(Device(line.strip(), "Print queues", None, key=f"printer:{line.strip()}",
                                manufacturer="CUPS", parent="/devices/LNXSYSTM:00"))

    def apply_disabled(self):
        present = {}
        for d in self.devices:
            if d.ctl:
                present.setdefault(d.ctl, []).append(d)
        for path, rec in self.disabled.items():
            devs = present.get(path)
            if devs:
                for d in devs:
                    if d.category not in ("Universal Serial Bus controllers",):
                        d.problem = 22
                if any(d.category == rec["category"] for d in devs):
                    continue
            self.add(Device(rec["name"], rec["category"], path, ctl=path, bus=rec["bus"], problem=22,
                            key=f"{path}:disabled"))
            if devs:
                for d in devs:
                    if d.category in ("Other devices", "Universal Serial Bus devices"):
                        self.devices.remove(d)

    def find_driver(self, d):
        if d.driver:
            return
        start = d.drv_path or (d.path.removesuffix(":fn") if d.path else None)
        if not start:
            return
        path = start
        while path and path not in ("/devices", "/"):
            link = f"/sys{path}/driver"
            if os.path.exists(link):
                d.drv_path = path
                d.driver = os.path.basename(os.path.realpath(link))
                mod = f"/sys{path}/driver/module"
                d.module = os.path.basename(os.path.realpath(mod)) if os.path.exists(mod) else ""
                d.rec = self.recs.get(path)
                return
            if d.category not in ("Disk drives", "DVD/CD-ROM drives", "Network adapters", "Bluetooth",
                                  "Cameras", "Keyboards", "Mice and other pointing devices",
                                  "Human Interface Devices", "Monitors", "Security devices",
                                  "Sound, video and game controllers", "Ports (COM & LPT)", "Storage volumes",
                                  "Batteries", "System devices", "Universal Serial Bus devices",
                                  "Software devices"):
                break
            path = os.path.dirname(path)
        d.drv_path = start
        d.rec = self.recs.get(start)


def pci_category(base, sub):
    if base == 0x01:
        return "Storage controllers"
    if base == 0x02:
        return "Network adapters"
    if base == 0x03:
        return "Display adapters"
    if base == 0x04:
        return "System devices" if sub == 0x03 else "Sound, video and game controllers"
    if base == 0x07 and sub == 0x00:
        return "Ports (COM & LPT)"
    if base == 0x08 and sub == 0x05:
        return "Memory technology devices"
    if base == 0x0c and sub == 0x03:
        return "Universal Serial Bus controllers"
    if base == 0x0c and sub == 0x00:
        return "IEEE 1394 host controllers"
    if base == 0x0d and sub == 0x11:
        return "Bluetooth"
    if base == 0x0d:
        return "Network adapters"
    if base == 0x10:
        return "Security devices"
    if base == 0xff:
        return "Memory technology devices"
    return "System devices"


_badges = {}


def badge(icon, problem=0, size=16):
    """The device icon, with Windows' yellow warning triangle for a problem or the grey
    down arrow for a disabled device in the bottom-right corner."""
    key = (icon, problem, size)
    if key in _badges:
        return _badges[key]
    theme = Gtk.IconTheme.get_default()
    info = theme.lookup_icon(icon, size, Gtk.IconLookupFlags.FORCE_SIZE)
    try:
        if info is None:
            raise GLib.Error("no icon")
        fg = Gdk.RGBA()
        fg.parse("#1a1a1a")
        base = info.load_symbolic(fg, None, None, None)[0] if info.is_symbolic() else info.load_icon()
    except GLib.Error:
        base = GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, True, 8, size, size)
        base.fill(0)
    if not problem:
        _badges[key] = base
        return base
    surf = cairo.ImageSurface(cairo.FORMAT_ARGB32, size, size)
    cr = cairo.Context(surf)
    Gdk.cairo_set_source_pixbuf(cr, base, 0, 0)
    cr.paint()
    s = size * 0.62
    k = size / 16
    x0, y0 = size - s, size - s
    if problem == 22:
        cr.arc(x0 + s / 2, y0 + s / 2, s / 2 - 0.5 * k, 0, 6.2832)
        cr.set_source_rgb(1, 1, 1)
        cr.fill_preserve()
        cr.set_source_rgb(0.33, 0.33, 0.33)
        cr.set_line_width(k)
        cr.stroke()
        cr.move_to(x0 + s * 0.5, y0 + s * 0.22)
        cr.line_to(x0 + s * 0.5, y0 + s * 0.74)
        cr.move_to(x0 + s * 0.27, y0 + s * 0.52)
        cr.line_to(x0 + s * 0.5, y0 + s * 0.76)
        cr.line_to(x0 + s * 0.73, y0 + s * 0.52)
        cr.set_line_width(1.3 * k)
        cr.stroke()
    else:
        cr.move_to(x0 + s / 2, y0)
        cr.line_to(x0 + s, y0 + s)
        cr.line_to(x0, y0 + s)
        cr.close_path()
        cr.set_source_rgb(0.99, 0.79, 0.07)
        cr.fill_preserve()
        cr.set_source_rgb(0.45, 0.33, 0)
        cr.set_line_width(0.8 * k)
        cr.stroke()
        cr.set_source_rgb(0, 0, 0)
        cr.rectangle(x0 + s / 2 - 0.6 * k, y0 + s * 0.32, 1.2 * k, s * 0.36)
        cr.rectangle(x0 + s / 2 - 0.6 * k, y0 + s * 0.76, 1.2 * k, 1.2 * k)
        cr.fill()
    pix = Gdk.pixbuf_get_from_surface(surf, 0, 0, size, size)
    _badges[key] = pix
    return pix


def instance_ids(d):
    """Windows-style hardware IDs, compatible IDs and the device instance path."""
    path = (d.drv_path or d.path or "").removesuffix(":fn")
    sysp = "/sys" + path
    hw, compat = [], []
    if d.bus == "pci" or (path and os.path.exists(sysp + "/vendor") and os.path.exists(sysp + "/class")
                          and "/pci" in path and re.search(r"\d{4}:\w\w:\w\w\.\w$", path)):
        g = lambda n: ui.sysfs(sysp, n, "0x0")[2:].upper()  # noqa: E731
        ven, dev, sv, sd, rev, cls = g("vendor"), g("device"), g("subsystem_vendor"), g("subsystem_device"), \
            g("revision")[-2:], g("class")
        hw = [f"PCI\\VEN_{ven}&DEV_{dev}&SUBSYS_{sd}{sv}&REV_{rev}", f"PCI\\VEN_{ven}&DEV_{dev}&SUBSYS_{sd}{sv}",
              f"PCI\\VEN_{ven}&DEV_{dev}&CC_{cls[-6:]}", f"PCI\\VEN_{ven}&DEV_{dev}&CC_{cls[-6:-2]}"]
        compat = [f"PCI\\VEN_{ven}&DEV_{dev}&REV_{rev}", f"PCI\\VEN_{ven}&DEV_{dev}", f"PCI\\VEN_{ven}&CC_{cls[-6:]}",
                  f"PCI\\CC_{cls[-6:]}", f"PCI\\CC_{cls[-6:-2]}"]
        inst = f"{hw[1]}\\{os.path.basename(path)}"
    else:
        usb = d.ctl if d.bus == "usb" else None
        if usb and os.path.exists(f"/sys{usb}/idVendor"):
            g = lambda n: ui.sysfs(f"/sys{usb}", n).upper()  # noqa: E731
            hw = [f"USB\\VID_{g('idVendor')}&PID_{g('idProduct')}&REV_{g('bcdDevice')}",
                  f"USB\\VID_{g('idVendor')}&PID_{g('idProduct')}"]
            compat = [f"USB\\Class_{g('bDeviceClass')}&SubClass_{g('bDeviceSubClass')}&Prot_{g('bDeviceProtocol')}",
                      f"USB\\Class_{g('bDeviceClass')}"]
        inst = path or d.key
    alias = ui.sysfs(sysp, "modalias") if path else ""
    if alias:
        compat.append(alias)
    return hw, compat, inst


def module_info(module):
    if not module:
        return {}
    info = {}
    for line in ui.out(["modinfo", module], timeout=5).splitlines():
        k, _, v = line.partition(":")
        v = v.strip()
        if k and v:
            info.setdefault(k.strip(), []).append(v)
    return info


# --- The results pane ----------------------------------------------------------------

class DeviceView(ui.View):
    def __init__(self, console, node):
        super().__init__(console, node)
        prefs = ui.json_file(PREFS, {})
        self.mode = prefs.get("mode", "type")
        self.show_hidden = prefs.get("hidden", False)
        self.devices = []
        self.monitor = None
        self.pending = None
        self.scanning = False
        self.expanded = None
        self.store = Gtk.TreeStore(GdkPixbuf.Pixbuf, str, object, bool)
        self.tree = Gtk.TreeView(model=self.store, headers_visible=False, enable_search=True,
                                 search_column=1, show_expanders=True, level_indentation=4)
        col = Gtk.TreeViewColumn()
        pix = Gtk.CellRendererPixbuf(stock_size=Gtk.IconSize.MENU)
        pix.set_padding(3, 3)
        cell = Gtk.CellRendererText(ellipsize=Pango.EllipsizeMode.END)
        col.pack_start(pix, False)
        col.pack_start(cell, True)
        col.add_attribute(pix, "pixbuf", 0)
        col.add_attribute(cell, "text", 1)
        col.set_cell_data_func(cell, lambda _c, c, m, it, _d: c.set_property(
            "foreground", "#8a8a8a" if m[it][3] else None))
        self.tree.append_column(col)
        self.tree.get_selection().connect("changed", lambda *_: self.changed())
        self.tree.connect("row-activated", self.on_activate)
        self.tree.connect("button-press-event", self.on_button)
        self.widget = ui.scrolled(self.tree)
        self.widget.set_policy(Gtk.PolicyType.AUTOMATIC, Gtk.PolicyType.AUTOMATIC)
        self.refresh()

    # data

    def refresh(self):
        if self.scanning:
            return
        self.scanning = True
        self.status("Scanning for devices\u2026")

        def work():
            try:
                devices = Scanner().run()
            except Exception as e:
                import traceback
                traceback.print_exception(e)
                devices = []
            GLib.idle_add(self.loaded, devices)
        threading.Thread(target=work, daemon=True).start()

    def loaded(self, devices):
        self.scanning = False
        self.devices = devices
        self.populate()
        self.status("")
        return False

    def visible(self):
        return [d for d in self.devices if self.show_hidden or not d.hidden]

    def populate(self):
        keep = self.selected_key()
        expanded = self.expanded_keys() if len(self.store) else None
        self.store.clear()
        host = socket.gethostname().split(".")[0].upper()
        root = self.store.append(None, [badge(ui.pick_icon("computer")), host, None, False])
        devs = self.visible()
        if self.mode == "connection":
            self.populate_connection(root, devs)
        elif self.mode == "driver":
            by = {}
            for d in devs:
                by.setdefault(d.module or d.driver or "(no driver)", []).append(d)
            for drv in sorted(by, key=str.lower):
                it = self.store.append(root, [badge(ui.pick_icon("application-x-firmware", "application-x-executable")),
                                              drv, ("driver", drv), False])
                for d in sorted(by[drv], key=lambda d: d.name.lower()):
                    self.store.append(it, [badge(d.icon, d.problem), d.name, d, d.hidden])
        else:
            by = {}
            for d in devs:
                by.setdefault(d.category, []).append(d)
            for cat in sorted(by, key=str.lower):
                it = self.store.append(root, [badge(category_icon(cat)), cat, ("category", cat),
                                              all(d.hidden for d in by[cat])])
                for d in sorted(by[cat], key=lambda d: d.name.lower()):
                    self.store.append(it, [badge(d.icon, d.problem), d.name, d, d.hidden])
        self.tree.expand_row(self.store.get_path(root), False)
        self.restore(expanded, keep)

    def populate_connection(self, root, devs):
        by_path = {}
        for d in devs:
            if d.path:
                by_path.setdefault(d.path.removesuffix(":fn"), d)

        def parent_of(d):
            p = d.parent if d.parent and d.parent != d.path else os.path.dirname((d.path or "").removesuffix(":fn"))
            if d.parent and d.parent in by_path and by_path[d.parent] is not d:
                return by_path[d.parent]
            while p and p not in ("/devices", "/"):
                if p in by_path and by_path[p] is not d:
                    return by_path[p]
                p = os.path.dirname(p)
            computer = by_path.get("/devices/LNXSYSTM:00")
            return computer if computer is not d else None
        children = {}
        for d in devs:
            children.setdefault(id(parent_of(d)), []).append(d)

        def add(parent_iter, parent_dev, depth=0):
            for d in sorted(children.get(id(parent_dev), []), key=lambda d: d.name.lower()):
                it = self.store.append(parent_iter, [badge(d.icon, d.problem), d.name, d, d.hidden])
                if depth < 40:
                    add(it, d, depth + 1)
        add(root, None)

    def node_key(self, obj):
        if obj is None:
            return "root"
        return obj.key if isinstance(obj, Device) else f"{obj[0]}:{obj[1]}"

    def selected_key(self):
        model, it = self.tree.get_selection().get_selected()
        return self.node_key(model[it][2]) if it else None

    def expanded_keys(self):
        keys = set()

        def walk(model, path, it):
            if self.tree.row_expanded(path):
                keys.add(self.node_key(model[it][2]))
        self.store.foreach(walk)
        return keys

    def restore(self, expanded, keep):
        def walk(model, path, it):
            obj = model[it][2]
            key = self.node_key(obj)
            if expanded is None:
                if isinstance(obj, tuple) and any(isinstance(c[2], Device) and c[2].problem for c in model[it].iterchildren()):
                    self.tree.expand_to_path(path)
            elif key in expanded:
                self.tree.expand_to_path(path)
            if key == keep:
                self.tree.expand_to_path(path)
                self.tree.get_selection().select_path(path)
                self.tree.scroll_to_cell(path, None, False, 0, 0)
        self.store.foreach(walk)
        if keep is None:
            self.tree.get_selection().select_path(Gtk.TreePath.new_first())

    # selection and commands

    def selected(self):
        model, it = self.tree.get_selection().get_selected()
        return model[it][2] if it else None

    def selected_device(self):
        obj = self.selected()
        return obj if isinstance(obj, Device) else None

    def selected_title(self):
        d = self.selected_device()
        return d.name if d else None

    def can_disable(self, d):
        return bool(d.ctl and d.bus in ("pci", "usb") and d.category in DISABLE_OK)

    def can_uninstall(self, d):
        return bool(d.ctl and d.bus in ("pci", "usb") and (d.category in DISABLE_OK or d.problem))

    def actions(self):
        return [Action("Scan for hardware changes", self.scan, ui.pick_icon("system-search-symbolic", "view-refresh-symbolic")),
                None, Action("View", children=self.view_actions())]

    def view_actions(self):
        return [Action("Devices by type", lambda: self.set_mode("type"), checked=self.mode == "type"),
                Action("Devices by connection", lambda: self.set_mode("connection"), checked=self.mode == "connection"),
                Action("Devices by driver", lambda: self.set_mode("driver"), checked=self.mode == "driver"),
                None,
                Action("Show hidden devices", self.toggle_hidden, checked=self.show_hidden)]

    def item_actions(self):
        d = self.selected_device()
        if not d:
            return []
        acts = [Action("Update driver", self.update_driver, ui.pick_icon("software-update-available-symbolic",
                                                                         "system-software-update"))]
        if self.can_disable(d):
            acts.append(Action("Enable device", lambda: self.enable(d), ui.pick_icon("media-playback-start-symbolic"))
                        if d.problem == 22 else
                        Action("Disable device", lambda: self.disable(d), ui.pick_icon("media-playback-stop-symbolic")))
        if self.can_uninstall(d):
            acts.append(Action("Uninstall device", lambda: self.uninstall(d), ui.pick_icon("edit-delete-symbolic")))
        return acts + [None, Action("Scan for hardware changes", self.scan), None,
                       Action("Properties", lambda: DeviceSheet(self, d), "document-properties-symbolic")]

    def toolbar(self):
        d = self.selected_device()
        tools = [Action("Scan", self.scan, ui.pick_icon("system-search-symbolic", "view-refresh-symbolic"),
                        tip="Scan for hardware changes")]
        if d:
            tools.append(Action("Update", self.update_driver, ui.pick_icon("software-update-available-symbolic",
                                                                           "system-software-update"),
                                tip="Update device driver"))
            if self.can_disable(d):
                tools.append(Action("Enable", lambda: self.enable(d), "media-playback-start-symbolic",
                                    tip="Enable device") if d.problem == 22 else
                             Action("Disable", lambda: self.disable(d), "media-playback-stop-symbolic",
                                    tip="Disable device"))
            if self.can_uninstall(d):
                tools.append(Action("Uninstall", lambda: self.uninstall(d), "edit-delete-symbolic",
                                    tip="Uninstall device"))
        return tools

    def set_mode(self, mode):
        self.mode = mode
        self.save_prefs()
        self.populate()

    def toggle_hidden(self):
        self.show_hidden = not self.show_hidden
        self.save_prefs()
        self.populate()

    def save_prefs(self):
        ui.write_file(PREFS, json.dumps({"mode": self.mode, "hidden": self.show_hidden}) + "\n")

    def on_activate(self, tree, path, _col):
        obj = self.store[path][2]
        if isinstance(obj, Device):
            DeviceSheet(self, obj)
        elif tree.row_expanded(path):
            tree.collapse_row(path)
        else:
            tree.expand_row(path, False)

    def on_button(self, tree, event):
        if event.button != 3:
            return False
        hit = tree.get_path_at_pos(int(event.x), int(event.y))
        if hit:
            tree.get_selection().select_path(hit[0])
        items = self.item_actions() or [Action("Scan for hardware changes", self.scan), None,
                                        Action("Properties", enabled=False)]
        ui.popup(tree, event, items)
        return True

    def update_driver(self):
        ui.spawn(["ws-settings", "update"])

    def scan(self):
        self.status("Scanning for hardware changes\u2026")
        ui.helper(["device-rescan"], lambda ok, msg: (self.refresh(), None if ok else
                                                      self.status(msg or "The scan didn\u2019t finish.")))

    def after(self, title):
        def done(ok, msg):
            if not ok and msg:
                ui.error(self.console.win, title, msg)
            GLib.timeout_add(600, lambda: self.refresh() and False)
        return done

    def disable(self, d):
        ui.confirm(self.console.win, d.name,
                   "Disabling this device will cause it to stop functioning.\n\nDo you really want to disable it?",
                   "Yes", lambda: ui.helper(["device-disable", d.ctl, d.bus, d.category, d.name],
                                            self.after("Couldn\u2019t disable the device")))

    def enable(self, d):
        ui.helper(["device-enable", d.ctl], self.after("Couldn\u2019t enable the device"))

    def uninstall(self, d):
        text = "Warning: You are about to uninstall this device from your system."
        if d.bus == "usb":
            text += "\n\nThe device is reconnected and its driver loaded again straight away."
        else:
            text += "\n\nUse Scan for hardware changes to install it again."
        ui.confirm(self.console.win, f"Uninstall Device \u2013 {d.name}", text, "Uninstall",
                   lambda: ui.helper(["device-remove", d.ctl], self.after("Couldn\u2019t uninstall the device")))

    # hot-plug

    def shown(self):
        if self.monitor is None:
            self.monitor = ui.run(["udevadm", "monitor", "--udev", "-s", "pci", "-s", "usb", "-s", "net",
                                   "-s", "block", "-s", "input", "-s", "bluetooth", "-s", "drm", "-s", "sound",
                                   "-s", "power_supply"], lines=self.on_uevent)

    def hidden(self):
        if self.monitor is not None:
            self.monitor.force_exit()
            self.monitor = None

    def on_uevent(self, line):
        if not line.startswith("UDEV"):
            return
        if self.pending:
            GLib.source_remove(self.pending)
        self.pending = GLib.timeout_add(700, self.hotplug)

    def hotplug(self):
        self.pending = None
        self.refresh()
        return False


# --- Property sheet ------------------------------------------------------------------

class DeviceSheet:
    def __init__(self, view, d):
        self.view, self.d = view, d
        self.power_changes = {}
        win = self.win = ui.PropertySheet(view.console.win, f"{d.name} Properties", apply=self.apply,
                                          width=480, height=560)
        self.general(win.page("General"))
        if d.drv_path or d.driver:
            self.driver_page(win.page("Driver"))
        self.details(win.page("Details"))
        if d.drv_path and d.rec:
            self.events(win.page("Events"))
        if d.bus == "pci" and d.ctl and os.path.exists(f"/sys{d.ctl}/resource"):
            self.resources(win.page("Resources"))
        if d.ctl and (os.path.exists(f"/sys{d.ctl}/power/wakeup") or
                      (d.bus == "usb" and os.path.exists(f"/sys{d.ctl}/power/control"))):
            self.power(win.page("Power Management"))
        win.present_sheet()

    def header(self, box):
        row = ui.sheet_header(self.d.icon, self.d.name)
        image = row.get_children()[0]
        image.set_from_pixbuf(badge(self.d.icon, self.d.problem, 32))
        box.pack_start(row, False, False, 0)
        box.pack_start(Gtk.Separator(), False, False, 0)

    def general(self, box):
        d = self.d
        self.header(box)
        rows = [("Device type:", d.category), ("Manufacturer:", d.manufacturer or "Unknown"),
                ("Location:", d.location or "Unknown")]
        rows += [(f"{k}:", v) for k, v in d.info.items()]
        box.pack_start(ui.kv_grid(rows), False, False, 0)
        box.pack_start(ui.label("Device status", "caption"), False, False, 0)
        status = STATUS.get(d.problem, STATUS[0])
        if d.problem == 0 and not d.driver and d.category not in ("Audio inputs and outputs", "Print queues",
                                                                  "Computer", "Firmware", "Processors"):
            status += "\n\nNo driver is needed for this device."
        box.pack_start(ui.text_box(status, 110), False, False, 0)

    def driver_page(self, box):
        d = self.d
        self.header(box)
        self.drv_grid = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box.pack_start(self.drv_grid, False, False, 0)
        self.drv_grid.pack_start(ui.label("Loading driver information\u2026", "caption"), False, False, 0)
        threading.Thread(target=lambda: GLib.idle_add(self.driver_loaded, module_info(d.module)),
                         daemon=True).start()
        buttons = [("Driver Details", "View details about the installed driver files.", self.driver_details, True),
                   ("Update Driver", "Update the driver for this device.", self.view.update_driver, True),
                   ("Roll Back Driver", "If the device fails after updating the driver, roll back to the "
                                        "previously installed driver.", None, False)]
        if self.view.can_disable(d):
            if d.problem == 22:
                buttons.append(("Enable Device", "Enable the device.", self.enable, True))
            else:
                buttons.append(("Disable Device", "Disable the device.", self.disable, True))
        if self.view.can_uninstall(d):
            buttons.append(("Uninstall Device", "Uninstall the device from the system (Advanced).",
                            self.uninstall, True))
        grid = Gtk.Grid(column_spacing=16, row_spacing=10)
        for i, (text, desc, cb, ok) in enumerate(buttons):
            b = ui.css(Gtk.Button(label=text, sensitive=ok), "btn")
            b.set_size_request(150, -1)
            b.set_valign(Gtk.Align.START)
            if cb:
                b.connect("clicked", lambda *_, c=cb: c())
            grid.attach(b, 0, i, 1, 1)
            lab = ui.label(desc, wrap=True)
            lab.set_hexpand(True)
            grid.attach(lab, 1, i, 1, 1)
        box.pack_start(grid, False, False, 8)

    def driver_loaded(self, info):
        d = self.d
        self.modinfo = info
        ui.clear(self.drv_grid)
        filename = (info.get("filename") or [""])[0]
        date = ""
        if filename and filename.startswith("/"):
            try:
                from datetime import datetime
                date = datetime.fromtimestamp(os.path.getmtime(filename)).strftime("%-m/%-d/%Y")
            except OSError:
                pass
        authors = info.get("author") or []
        vendor = short_vendor(d.manufacturer) if d.manufacturer and not d.manufacturer.startswith("(") else ""
        provider = vendor if vendor and any(vendor.lower() in a.lower() for a in authors) else \
            (re.split(r"[<,]", authors[0])[0].strip() if authors else "")
        if d.category == "Audio inputs and outputs":
            rows = [("Driver Provider:", "PipeWire"), ("Driver:", "PipeWire audio endpoint")]
        elif not d.driver:
            rows = [("Driver:", "No driver is loaded for this device.")]
        else:
            rows = [("Driver Provider:", provider or "The Linux kernel"),
                    ("Driver Date:", date or None),
                    ("Driver Version:", (info.get("version") or [None])[0] or platform.release()),
                    ("Digital Signer:", (info.get("signer") or ["Built into the kernel" if filename in
                                                                ("", "(builtin)") else "Not digitally signed"])[0]),
                    ("Driver:", d.driver + (f" (module {d.module})" if d.module and d.module != d.driver else ""))]
        self.drv_grid.pack_start(ui.kv_grid(rows), False, False, 0)
        self.drv_grid.show_all()
        return False

    def driver_details(self):
        info = getattr(self, "modinfo", {}) or {}
        files = [f for f in info.get("filename", []) if f.startswith("/")]
        for dep in ",".join(info.get("depends", [])).split(","):
            if dep:
                p = ui.out(["modinfo", "-n", dep]).strip()
                if p.startswith("/"):
                    files.append(p)
        files += [f"/usr/lib/firmware/{f}" for f in info.get("firmware", [])
                  if glob.glob(f"/usr/lib/firmware/{f}*")]
        sheet = ui.PropertySheet(self.win, "Driver File Details", width=480, height=440, buttons=False)
        box = sheet.page("Driver File Details")
        box.pack_start(ui.sheet_header(self.d.icon, self.d.name), False, False, 0)
        box.pack_start(ui.label("Driver files:", "caption"), False, False, 0)
        if not files:
            files = ["(built into the kernel)"] if self.d.driver else ["No driver files are required or have "
                                                                       "been loaded for this device."]
        box.pack_start(ui.simple_list(files, 150), True, True, 0)
        box.pack_start(ui.kv_grid([
            ("Provider:", (info.get("author") or ["The Linux kernel"])[0]),
            ("File version:", (info.get("version") or [platform.release()])[0]),
            ("Copyright:", (info.get("license") or [None])[0]),
            ("Digital Signer:", (info.get("signer") or [None])[0])]), False, False, 0)
        sheet.present_sheet()

    def details(self, box):
        d = self.d
        self.header(box)
        hw, compat, inst = instance_ids(d)
        rec = d.rec or {}
        props = rec.get("E", {}) if rec else {}
        path = d.path.removesuffix(":fn") if d.path else ""
        children = sorted(os.path.basename(p) for p in glob.glob(f"/sys{d.drv_path or path}/*")
                          if os.path.isdir(p) and os.path.exists(p + "/uevent")) if (d.drv_path or path) else []
        fields = [
            ("Device description", [d.name]),
            ("Hardware Ids", hw),
            ("Compatible Ids", compat),
            ("Class", [d.category]),
            ("Manufacturer", [d.manufacturer]),
            ("Driver", [d.driver] if d.driver else []),
            ("Driver module", [d.module] if d.module else []),
            ("Device instance path", [inst]),
            ("Physical device object name", ["/sys" + path] if path else []),
            ("Location paths", [props.get("ID_PATH")] if props.get("ID_PATH") else []),
            ("Location information", [d.location] if d.location else []),
            ("Bus reported device description", [x for x in (props.get("ID_MODEL_FROM_DATABASE"),
                                                             read(d.ctl, "product") if d.ctl else "") if x]),
            ("Parent", [os.path.dirname(d.drv_path or path)] if (d.drv_path or path) else []),
            ("Children", children),
            ("Device node", ["/dev/" + rec["N"]] + ["/dev/" + s for s in rec.get("S", [])] if rec.get("N") else []),
            ("Power data", [f"Runtime status: {read(d.drv_path or path, 'power/runtime_status')}",
                            f"Power control: {read(d.drv_path or path, 'power/control')}"]
             if path and os.path.exists(f"/sys{d.drv_path or path}/power/runtime_status") else []),
        ]
        fields = [(k, [x for x in v if x]) for k, v in fields if any(v)]
        fields += [(f"udev: {k}", [v]) for k, v in sorted(props.items())]
        box.pack_start(ui.label("Property", "caption"), False, False, 0)
        combo = Gtk.ComboBoxText()
        for k, _v in fields:
            combo.append_text(k)
        box.pack_start(combo, False, False, 0)
        box.pack_start(ui.label("Value", "caption"), False, False, 0)
        values = ui.simple_list([], 220)
        box.pack_start(values, True, True, 0)

        def show(*_):
            values.store.clear()
            i = combo.get_active()
            for v in fields[i][1] if i >= 0 else []:
                values.store.append([str(v)])
        combo.connect("changed", show)
        combo.set_active(0)

        def menu(tree, event):
            if event.button != 3:
                return False
            model, paths = tree.get_selection().get_selected_rows()
            text = "\n".join(model[p][0] for p in paths) or "\n".join(r[0] for r in model)
            ui.popup(tree, event, [Action("Copy", lambda: ui.clipboard(text)),
                                   Action("Select All", lambda: tree.get_selection().select_all())])
            return True
        values.tree.get_selection().set_mode(Gtk.SelectionMode.MULTIPLE)
        values.tree.connect("button-press-event", menu)

    def events(self, box):
        d = self.d
        self.header(box)
        box.pack_start(ui.label("Device events", "caption"), False, False, 0)
        lst = ui.simple_list([], 200, headers=["Timestamp", "Description"])
        box.pack_start(lst, True, True, 0)
        box.pack_start(ui.label("Information", "caption"), False, False, 0)
        info = ui.text_box("", 100)
        box.pack_start(info, False, False, 0)
        more = ui.css(Gtk.Button(label="View All Events\u2026"), "btn")
        more.set_halign(Gtk.Align.END)
        more.connect("clicked", lambda *_: ui.spawn(["ws-mmc", "eventvwr.msc"]))
        box.pack_start(more, False, False, 0)
        subsystem = d.rec["E"].get("SUBSYSTEM", "")
        match = f"_KERNEL_DEVICE=+{subsystem}:{os.path.basename(d.drv_path)}"
        rows = []

        def done(ok, out, _err):
            for line in out.splitlines():
                try:
                    e = json.loads(line)
                except ValueError:
                    continue
                msg = e.get("MESSAGE")
                if isinstance(msg, list):
                    msg = bytes(msg).decode("utf-8", "replace")
                rows.append((ui.when(int(e.get("__REALTIME_TIMESTAMP", "0"))), msg or ""))
            if d.driver:
                rows.insert(0, ("", f"Device configured ({d.driver})"))
            for r in rows[-200:]:
                lst.store.append(list(r))
            if rows:
                lst.tree.get_selection().select_path(Gtk.TreePath.new_first())
        lst.tree.get_selection().connect("changed", lambda s: info.view.get_buffer().set_text(
            (lambda m, it: f"{m[it][1]}" if it else "")(*s.get_selected())))
        ui.run(["journalctl", "-k", "-b", "-o", "json", "--no-pager", "-n", "200", match], done)

    def resources(self, box):
        d = self.d
        self.header(box)
        rows = []
        for line in ui.read_file(f"/sys{d.ctl}/resource").splitlines():
            parts = line.split()
            if len(parts) != 3:
                continue
            start, end, flags = (int(x, 16) for x in parts)
            if not start and not end:
                continue
            if flags & 0x100:
                rows.append(("I/O Range", f"{start:04X} - {end:04X}"))
            elif flags & 0x200:
                rows.append(("Memory Range", f"{start:016X} - {end:016X}"))
        msi = sorted(int(os.path.basename(p)) for p in glob.glob(f"/sys{d.ctl}/msi_irqs/*"))
        irq = read(d.ctl, "irq")
        for n in msi or ([int(irq)] if irq.isdigit() and int(irq) not in (0, 255) else []):
            rows.append(("IRQ", f"0x{n:08X} ({n})"))
        box.pack_start(ui.label("Resource settings:", "caption"), False, False, 0)
        box.pack_start(ui.simple_list(rows or [("(none)", "This device isn\u2019t using any resources.")], 220,
                                      headers=["Resource type", "Setting"]), True, True, 0)
        box.pack_start(ui.label("Conflicting device list:", "caption"), False, False, 0)
        box.pack_start(ui.text_box("No conflicts.", 60), False, False, 0)

    def power(self, box):
        d = self.d
        self.header(box)
        if d.bus == "usb" and os.path.exists(f"/sys{d.ctl}/power/control"):
            c = Gtk.CheckButton(label="Allow the computer to turn off this device to save power",
                                active=read(d.ctl, "power/control") == "auto")
            c.connect("toggled", lambda b: self.power_change("control", "auto" if b.get_active() else "on"))
            box.pack_start(c, False, False, 0)
        if os.path.exists(f"/sys{d.ctl}/power/wakeup"):
            w = Gtk.CheckButton(label="Allow this device to wake the computer",
                                active=read(d.ctl, "power/wakeup") == "enabled")
            w.connect("toggled", lambda b: self.power_change("wakeup", "enabled" if b.get_active() else "disabled"))
            box.pack_start(w, False, False, 0)
        box.pack_start(ui.label("Changes take effect now and last until the next restart.", "caption", wrap=True),
                       False, False, 0)

    def power_change(self, key, value):
        self.power_changes[key] = value
        self.win.changed()

    def apply(self):
        for key, value in self.power_changes.items():
            ui.helper(["device-power", self.d.ctl, key, value],
                      lambda ok, msg: ok or ui.error(self.view.console.win, "Couldn\u2019t change power settings", msg))
        self.power_changes = {}

    def disable(self):
        self.win.destroy()
        self.view.disable(self.d)

    def enable(self):
        self.win.destroy()
        self.view.enable(self.d)

    def uninstall(self):
        self.win.destroy()
        self.view.uninstall(self.d)


def node():
    return Node(NAME, ICON, DeviceView, key=KEY, description=DESCRIPTION)
