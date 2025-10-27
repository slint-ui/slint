# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import slint

try:
    from .counter import CounterWindow
except ImportError as exc:  # pragma: no cover - user-facing guidance
    raise SystemExit(
        "Generated bindings not found. Run `python generate.py` in the "
        "examples/counter directory first."
    ) from exc


class CounterApp(CounterWindow):
    @slint.callback
    def request_increase(self) -> None:
        self.counter += 1


def main() -> None:
    app = CounterApp()
    app.show()
    app.run()


if __name__ == "__main__":
    main()
