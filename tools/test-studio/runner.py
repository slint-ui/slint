# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import os
import signal
import subprocess
import tempfile
import threading
from pathlib import Path

SUITES = [
    f"tests/test_{name}.py"
    for name in ("undo_redo", "startup", "selection", "navigation", "canvas_zoom")
]


class TestProcess:
    def __init__(
        self,
        repo: Path,
        python: Path,
        binary: Path,
        selectors: list[str],
        *,
        collect=False,
        visible=False,
    ):
        self.directory = Path(tempfile.mkdtemp(prefix="slint-test-studio-"))
        self.events_path = self.directory / "events.jsonl"
        self.log_path = self.directory / "pytest.log"
        self.events_path.touch()
        self.offset = 0
        self.finished = False
        self.cancelled = False
        self.collect = collect
        self.selectors = selectors
        self.log = self.log_path.open("w", encoding="utf-8")
        selector_path = self.directory / "selectors.json"
        selector_path.write_text(json.dumps(selectors))
        suite = repo / "tools/editor/ui-tests"
        env = os.environ.copy()
        for key in (
            "SLINT_MCP_PORT",
            "SLINT_TEST_SERVER",
            "PYTEST_ADDOPTS",
            "PYTEST_CURRENT_TEST",
            "PYTHONPATH",
        ):
            env.pop(key, None)
        env.update(
            SLINT_EDITOR_BINARY=str(binary),
            SLINT_EDITOR_UI_TEST_BACKEND="winit-skia" if visible else "headless-skia",
            SLINT_BACKEND="winit-skia" if visible else "headless-skia",
            PYTHONUNBUFFERED="1",
            PYTHONDONTWRITEBYTECODE="1",
        )
        command = [
            str(python),
            str(Path(__file__).with_name("bridge.py")),
            "--suite",
            str(suite),
            "--events",
            str(self.events_path),
            "--selectors",
            str(selector_path),
        ]
        if collect:
            command.append("--collect")
        try:
            self.process = subprocess.Popen(
                command,
                cwd=suite,
                env=env,
                stdout=self.log,
                stderr=subprocess.STDOUT,
                start_new_session=os.name != "nt",
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP
                if os.name == "nt"
                else 0,
            )
        except BaseException:
            self.log.close()
            raise

    def poll(self):
        events = []
        with self.events_path.open(encoding="utf-8") as stream:
            stream.seek(self.offset)
            while True:
                start = stream.tell()
                line = stream.readline()
                if not line or not line.endswith("\n"):
                    self.offset = start
                    break
                try:
                    events.append(json.loads(line))
                except json.JSONDecodeError:
                    events.append(
                        {"kind": "error", "detail": "Invalid event from pytest bridge"}
                    )
                self.offset = stream.tell()
        code = self.process.poll()
        if code is not None and not self.finished:
            # Drain once more after process exit so the last result cannot be lost.
            with self.events_path.open(encoding="utf-8") as stream:
                stream.seek(self.offset)
                for line in stream:
                    if line.strip():
                        try:
                            events.append(json.loads(line))
                        except json.JSONDecodeError:
                            events.append(
                                {
                                    "kind": "error",
                                    "detail": "Incomplete event from pytest bridge",
                                }
                            )
                self.offset = stream.tell()
            self.finished = True
            if os.name != "nt":
                try:
                    os.killpg(self.process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
            self.log.close()
            events.append(
                {"kind": "finished", "code": code, "cancelled": self.cancelled}
            )
        return events

    def stop(self):
        if self.process.poll() is not None:
            return
        self.cancelled = True
        if os.name == "nt":
            subprocess.run(
                ["taskkill", "/PID", str(self.process.pid), "/T", "/F"],
                capture_output=True,
                check=False,
            )
        else:
            try:
                os.killpg(self.process.pid, signal.SIGTERM)
            except ProcessLookupError:
                return

            def reap():
                try:
                    self.process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    try:
                        os.killpg(self.process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    self.process.wait()

            threading.Thread(target=reap, daemon=True).start()

    def close(self):
        self.stop()
        try:
            self.process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
        self.log.close()
