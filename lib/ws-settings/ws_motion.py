"""Windows 11 motion for the shell's GTK 3 layer surfaces (Start, flyouts, Task View).

Timings and curves are WinUI's: panels rise into place from behind the taskbar on a
decelerate curve and drop away faster on an accelerate curve. Settings > Accessibility >
Visual effects > Animation effects (gtk-enable-animations) turns the motion off.
"""

import subprocess

import gi

gi.require_version("Gdk", "3.0")
gi.require_version("Gtk", "3.0")
from gi.repository import Gdk, Gtk  # noqa: E402

# WinUI ControlFaster/Fast/NormalAnimationDuration, in milliseconds.
FASTER = 83
FAST = 167
NORMAL = 250


def bezier(x1, y1, x2, y2):
    """A CSS cubic-bezier() easing as a function of progress 0..1."""
    def at(p1, p2, t):
        return 3 * (1 - t) ** 2 * t * p1 + 3 * (1 - t) * t * t * p2 + t ** 3

    def ease(x):
        if x <= 0:
            return 0.0
        if x >= 1:
            return 1.0
        lo, hi = 0.0, 1.0
        for _ in range(24):
            t = (lo + hi) / 2
            if at(x1, x2, t) < x:
                lo = t
            else:
                hi = t
        return at(y1, y2, (lo + hi) / 2)
    return ease


DECELERATE = bezier(0, 0, 0, 1)
ACCELERATE = bezier(0.7, 0, 1, 0.5)


def enabled():
    settings = Gtk.Settings.get_default()
    return settings is None or settings.props.gtk_enable_animations


_acrylic = None


def acrylic():
    """True when SwayFX blurs what is behind the shell, so panels can be translucent."""
    global _acrylic
    if _acrylic is None:
        try:
            _acrylic = subprocess.run(["rpm", "-q", "--quiet", "swayfx"]).returncode == 0
        except OSError:
            _acrylic = False
    return _acrylic


class Motion:
    """One animation at a time on a widget's frame clock. step(t) gets progress 0..1;
    done runs at the end, but not when the animation is replaced or stopped."""

    def __init__(self, widget):
        self.widget = widget
        self.tick_id = 0

    @property
    def running(self):
        return bool(self.tick_id)

    def run(self, duration, step, done=None):
        self.stop()
        if not enabled() or duration <= 0:
            step(1.0)
            if done:
                done()
            return
        start = [None]

        def tick(_widget, clock):
            now = clock.get_frame_time()
            if start[0] is None:
                start[0] = now
            t = min(1.0, (now - start[0]) / (duration * 1000))
            step(t)
            if t < 1.0:
                return True
            self.tick_id = 0
            if done:
                done()
            return False
        step(0.0)
        self.tick_id = self.widget.add_tick_callback(tick)

    def stop(self):
        if self.tick_id:
            self.widget.remove_tick_callback(self.tick_id)
            self.tick_id = 0


class Slide(Gtk.Bin):
    """Fills a layer window whose bottom edge is the taskbar, and moves its child down by
    a fraction of the distance that hides panel behind that edge. The window clips it."""

    def __init__(self, child, panel):
        super().__init__()
        self.panel = panel
        self.frac = 0.0
        self.applied = 0
        self.shown = False
        self.motion = Motion(self)
        self.add(child)

    def do_size_allocate(self, alloc):
        self.set_allocation(alloc)
        child = self.get_child()
        if not child or not child.get_visible():
            return
        offset = 0
        if self.frac > 0:
            try:
                _x, top = self.panel.translate_coordinates(self, 0, 0)
                rest = top - self.applied
            except (TypeError, ValueError):
                rest = 0
            offset = round(self.frac * max(0, alloc.height - rest))
        rect = Gdk.Rectangle()
        rect.x, rect.y, rect.width, rect.height = alloc.x, alloc.y + offset, alloc.width, alloc.height
        child.size_allocate(rect)
        self.applied = offset

    def place(self, frac, opacity):
        # Fade the panel, not this full-screen widget: GTK renders a translucent
        # widget through an offscreen group the size of its area on every frame.
        self.frac = frac
        self.panel.set_opacity(opacity)
        self.queue_allocate()

    def enter(self):
        """Rise into place: call after the window is shown."""
        if not self.motion.running:
            self.place(1.0, 0.0)
        f0, a0 = self.frac, self.panel.get_opacity()
        self.shown = True
        self.motion.run(NORMAL, lambda t: self.place(
            f0 * (1 - DECELERATE(t)), a0 + (1 - a0) * min(1.0, t * NORMAL / FASTER)))

    def leave(self, done):
        """Drop behind the taskbar, then done() (which hides the window)."""
        if not self.get_mapped() or not (self.shown or self.motion.running):
            self.motion.stop()
            self.shown = False
            done()
            return
        f0, a0 = self.frac, self.panel.get_opacity()
        self.shown = False
        self.motion.run(FAST, lambda t: self.place(
            f0 + (1 - f0) * ACCELERATE(t), a0 * (1 - t)), done)


class Fade:
    """Fades a widget in and out, as Task View and the desktop name do."""

    def __init__(self, widget):
        self.widget = widget
        self.shown = False
        self.motion = Motion(widget)

    def enter(self):
        if not self.motion.running:
            self.widget.set_opacity(0.0)
        a0 = self.widget.get_opacity()
        self.shown = True
        self.motion.run(FAST, lambda t: self.widget.set_opacity(a0 + (1 - a0) * DECELERATE(t)))

    def leave(self, done):
        if not self.widget.get_mapped() or not (self.shown or self.motion.running):
            self.motion.stop()
            self.shown = False
            done()
            return
        a0 = self.widget.get_opacity()
        self.shown = False
        self.motion.run(FAST, lambda t: self.widget.set_opacity(a0 * (1 - t)), done)
