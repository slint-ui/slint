# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import os
import sys
import time

import slint_testing


def main():
    app = slint_testing.Application(
        [sys.argv[1]], env=os.environ.copy(), launch_timeout=15
    )
    try:
        with app:
            app.aut_connection.settimeout(3)
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                windows = app.windows
                if windows:
                    break
                time.sleep(0.02)
            else:
                raise RuntimeError("Editor connected without a window")
            window = windows[0]
            try:
                debug_id = window.root_element.id
            except IndexError:
                debug_id = ""
            if not debug_id:
                raise RuntimeError(
                    "Editor element IDs are unavailable; rebuild with SLINT_EMIT_DEBUG_INFO=1."
                )
            elements = window.root_element.query_descendants().find_all()
            if not elements:
                raise RuntimeError("Editor element inspection returned no elements")
            if not window.grab_window_as_png().startswith(b"\x89PNG"):
                raise RuntimeError("Editor screenshot was not a PNG")
            print("Connected; window, element inspection, and screenshot verified.")
    finally:
        app.test_server_socket.close()


if __name__ == "__main__":
    main()
