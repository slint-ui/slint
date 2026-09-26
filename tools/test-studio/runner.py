# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from events import Writer, read


def environment(binary, backend):
    env = os.environ.copy()
    for key in (
        "SLINT_MCP_PORT",
        "SLINT_TEST_SERVER",
        "PYTEST_ADDOPTS",
        "PYTEST_CURRENT_TEST",
        "PYTHONPATH",
        "SLINT_SCALE_FACTOR",
    ):
        env.pop(key, None)
    env.update(
        SLINT_EDITOR_BINARY=str(binary),
        SLINT_EDITOR_UI_TEST_BACKEND=backend,
        SLINT_BACKEND=backend,
        PYTHONUNBUFFERED="1",
        PYTHONDONTWRITEBYTECODE="1",
        SLINT_EMIT_DEBUG_INFO="1",
        SLINT_ENABLE_EXPERIMENTAL_FEATURES="1",
    )
    return env


class Command:
    def __init__(self, args, *, cwd, env, output):
        self.process = subprocess.Popen(
            [
                sys.executable,
                str(Path(__file__).with_name("supervisor.py")),
                *map(str, args),
            ],
            cwd=cwd,
            env=env,
            stdin=subprocess.PIPE,
            stdout=output,
            stderr=subprocess.STDOUT,
        )

    def stop(self):
        if self.process.stdin and not self.process.stdin.closed:
            self.process.stdin.close()

    def close(self):
        self.stop()
        self.process.wait(timeout=5)


def checked_command(args, *, cwd, env, directory, name, cancel, timeout):
    path = directory / f"{name}.log"
    with path.open("w") as stream:
        command = Command(args, cwd=cwd, env=env, output=stream)
        deadline = time.monotonic() + timeout
        try:
            while command.process.poll() is None:
                if cancel.wait(0.03):
                    raise InterruptedError("Operation cancelled")
                if time.monotonic() >= deadline:
                    raise TimeoutError(
                        f"{name} exceeded {timeout:g} seconds; see {path.name}"
                    )
        finally:
            command.close()
    output = path.read_text(errors="replace")
    if command.process.returncode:
        raise RuntimeError(f"{name} failed ({command.process.returncode}):\n{output}")
    return output


def log_tail(path, limit=100000):
    with path.open("rb") as stream:
        stream.seek(0, 2)
        stream.seek(max(0, stream.tell() - limit))
        return stream.read().decode(errors="replace")


class TestProcess:
    def __init__(
        self,
        repo,
        python,
        binary,
        selectors,
        *,
        collect=False,
        debug=False,
        visible=False,
        directory=None,
    ):
        self.directory = Path(
            directory or tempfile.mkdtemp(prefix="slint-test-studio-")
        )
        self.events_path = self.directory / "events.jsonl"
        self.log_path = self.directory / "pytest.log"
        self.events_path.touch(exist_ok=True)
        self.offset = 0
        self.sequence = 0
        self.finished = False
        self.cancelled = False
        self.collect = collect
        self.selectors = list(selectors)
        self.log = self.log_path.open("w")
        selector_path = self.directory / "selectors.json"
        selector_path.write_text(json.dumps(self.selectors))
        suite = Path(repo) / "tools/editor/ui-tests"
        args = [
            python,
            Path(__file__).with_name("bridge.py"),
            "--suite",
            suite,
            "--events",
            self.events_path,
            "--selectors",
            selector_path,
        ]
        if collect:
            args.append("--collect")
        env = environment(binary, "winit-skia" if visible else "headless-skia")
        env["SLINT_STUDIO_DEBUG"] = "1" if debug else "0"
        try:
            self.command = Command(
                args,
                cwd=suite,
                env=env,
                output=self.log,
            )
            self.process = self.command.process
        except BaseException:
            self.log.close()
            raise

    def _read(self, final=False):
        events, self.offset, self.sequence, warnings = read(
            self.events_path,
            offset=self.offset,
            sequence=self.sequence,
            final=final,
        )
        events.extend(
            {"kind": "error", "category": "protocol", "detail": warning}
            for warning in warnings
        )
        return events

    def poll(self):
        if self.finished:
            return []
        events = self._read()
        code = self.process.poll()
        if code is not None:
            events += self._read(final=True)
            self.finished = True
            self.command.close()
            self.log.close()
            if code not in (0, 1, 5) and not self.cancelled:
                events.append(
                    Writer(self.events_path).emit(
                        "error",
                        category="runner",
                        detail=f"Pytest exited with code {code}.\n"
                        + log_tail(self.log_path),
                    )
                )
            events.append(
                Writer(self.events_path).emit(
                    "finished", code=code, cancelled=self.cancelled
                )
            )
        return events

    def stop(self):
        if not self.finished:
            self.cancelled = True
            self.command.stop()

    def close(self):
        self.stop()
        self.command.close()
        self.log.close()
