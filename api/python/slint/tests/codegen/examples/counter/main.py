from __future__ import annotations

import slint

try:
    from .generated.counter import CounterWindow
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
