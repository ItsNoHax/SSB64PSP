#!/usr/bin/env python3
"""Send one synthetic key press to an X11 window.

This is the dependency-light fallback used by run-ppsspp.sh's stage audit on
hosts without xdotool. The caller is responsible for focusing the window.
"""

import sys
import time

from Xlib import X, XK, display
from Xlib.ext import xtest


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} WINDOW_ID KEY", file=sys.stderr)
        return 2

    window_id = int(sys.argv[1], 0)
    key_name = sys.argv[2]
    connection = display.Display()
    window = connection.create_resource_object("window", window_id)
    window.set_input_focus(X.RevertToParent, X.CurrentTime)
    connection.sync()

    keysym = XK.string_to_keysym(key_name)
    keycode = connection.keysym_to_keycode(keysym)
    if keycode == 0:
        print(f"unknown X11 key: {key_name}", file=sys.stderr)
        return 2

    xtest.fake_input(connection, X.KeyPress, keycode)
    connection.sync()
    time.sleep(0.05)
    xtest.fake_input(connection, X.KeyRelease, keycode)
    connection.sync()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
