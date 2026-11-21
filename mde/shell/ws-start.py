#!/usr/bin/env python3
"""Thin client for the Start menu hosted inside ws-taskbar.

Start lives in the taskbar flyout stack. This binary only forwards commands to
ws-taskbar.sock so Super, Search, and scripts keep working.
"""
import os
import socket
import sys

RUNTIME = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
SOCK = os.path.join(RUNTIME, "ws-taskbar.sock")
ARGS = {
    "": "start-toggle",
    "--show": "start-show",
    "--search": "start-show",
    "--hide": "start-hide",
    "--settings": "start-settings",
    "--daemon": None,
}


def send(cmd):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
    try:
        s.sendto(cmd.encode(), SOCK)
        return True
    except OSError:
        return False
    finally:
        s.close()


def main():
    arg = sys.argv[1] if len(sys.argv) > 1 else ""
    if arg in ("-h", "--help"):
        print("usage: ws-start [--show|--search|--hide|--settings|--daemon]")
        return 0
    if arg not in ARGS and arg:
        print("usage: ws-start [--show|--search|--hide|--settings|--daemon]", file=sys.stderr)
        return 2
    cmd = ARGS.get(arg, "start-toggle")
    if cmd is None:
        # Compat: Start is owned by ws-taskbar; nothing to daemonize.
        return 0
    if not send(cmd):
        print("ws-taskbar is not running", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
