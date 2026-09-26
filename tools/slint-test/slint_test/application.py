# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import contextlib
import os
import subprocess
import sys
from collections.abc import Iterator, Sequence
from pathlib import Path
from tempfile import TemporaryDirectory

import slint_testing as low

from .core import (
    BoundApplication,
    Session,
    StrictMatchError,
    UnsupportedCapability,
    Window,
)
from .reporting import emit, step


class Application:
    def __init__(self, raw: low.Application):
        self.raw = raw
        self.session = Session(process=raw.process)
        self.transport = BoundApplication(raw, self.session)
        self._windows: list[Window] = []

    def window(self, *, index: int | None = None, timeout: float = 5000) -> Window:
        def find():
            windows = self.transport.windows
            if index is None and len(windows) > 1:
                raise StrictMatchError(
                    f"Expected one window, found {len(windows)}; select an index"
                )
            chosen = 0 if index is None else index
            if chosen < 0:
                raise ValueError("Window index must be nonnegative")
            return windows[chosen] if chosen < len(windows) else None

        raw = self.session.wait(
            find,
            lambda value: value is not None,
            timeout=timeout,
            description="Application window",
        )
        assert raw is not None
        window = Window(raw, session=self.session)
        self._windows.append(window)
        return window


@contextlib.contextmanager
def launch(
    args: Sequence[str], *, env: dict[str, str] | None = None, timeout: float = 20000
) -> Iterator[Application]:
    if os.name == "nt":
        raise UnsupportedCapability(
            "Application process ownership currently requires POSIX"
        )
    with TemporaryDirectory(prefix="slint-test-") as data:
        environment = os.environ.copy() if env is None else env.copy()
        environment.update(XDG_CONFIG_HOME=data, XDG_DATA_HOME=data)
        command = [
            sys.executable,
            str(Path(__file__).with_name("_launch.py")),
            "--parent",
            str(os.getpid()),
            *args,
        ]
        raw = low.Application(command, env=environment, launch_timeout=timeout / 1000)
        app = None
        failed = True
        try:
            with step("Launch application", layer="generic", command=list(args)):
                raw.__enter__()
                raw.aut_connection.settimeout(5)
                app = Application(raw)
                emit({"kind": "application-ready", "application": raw})
            yield app
            failed = False
        finally:
            if app is not None:
                for window in app._windows:
                    with contextlib.suppress(Exception):
                        window.cleanup_input()
                emit(
                    {
                        "kind": "application-closing",
                        "application": raw,
                        "failed": failed,
                        "returncode": raw.process.poll(),
                    }
                )
            for name in ("aut_connection", "test_server_socket"):
                connection = getattr(raw, name, None)
                if connection is not None:
                    connection.close()
            process = getattr(raw, "process", None)
            if process is not None:
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                emit({"kind": "application-exit", "returncode": process.returncode})
