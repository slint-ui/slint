# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Action control runs on the pytest thread, sharing its application connection."""

import importlib
import json
import time
from pathlib import Path


class Debugger:
    def __init__(self, plugin):
        self.plugin = plugin
        self.path = plugin.artifacts / "commands.jsonl"
        self.offset = 0
        self.sequence = 0
        self.pause_next = True
        self.stepping = None
        self.pause_before = ""
        self.failure = None
        self.serial = 0
        self.paused = False

    def commands(self):
        if not self.path.exists():
            return
        with self.path.open() as stream:
            stream.seek(self.offset)
            for _ in range(64):
                start = stream.tell()
                line = stream.readline()
                if not line or not line.endswith("\n"):
                    stream.seek(start)
                    break
                self.offset = stream.tell()
                try:
                    command = json.loads(line)
                    if (
                        command.get("version") != 1
                        or command.get("run_id") != self.plugin.artifacts.name
                        or command.get("sequence") != self.sequence + 1
                    ):
                        raise ValueError("Invalid debugger command envelope")
                    self.sequence = command["sequence"]
                    yield command
                except (ValueError, TypeError, AttributeError) as error:
                    self.plugin.emit("warning", detail=str(error))
            self.offset = stream.tell()

    def before(self, event):
        for command in self.commands():
            if command["kind"] == "pause":
                self.pause_next = True
            elif command["kind"] == "breakpoint":
                self.pause_before = command.get("title", "").casefold().strip()
        if self.plugin.application is None:
            return
        if self.pause_next or (
            self.pause_before and self.pause_before in event["title"].casefold()
        ):
            self.pause(event, "Before action")

    def failed(self, event, error):
        if self.plugin.application is not None and self.failure is not error:
            self.failure = error
            self.pause(event, "Action failed", str(error))

    def after(self, event):
        if event["action_id"] == self.stepping:
            self.stepping = None
            self.pause_next = True

    def pause(self, event, reason, error=""):
        control = importlib.import_module("slint_test.control")
        with control.suspended_clock():
            self.serial += 1
            self.paused = True
            self.pause_next = False
            self.plugin.emit(
                "debug-state",
                paused=True,
                pause_id=self.serial,
                reason=reason,
                action=event,
                error=error,
            )
            self.inspect()
            try:
                while True:
                    for command in self.commands():
                        kind = command["kind"]
                        if kind == "breakpoint":
                            self.pause_before = (
                                command.get("title", "").casefold().strip()
                            )
                        elif (
                            kind == "inspect" and command.get("pause_id") == self.serial
                        ):
                            self.inspect()
                        elif (
                            kind in ("continue", "step", "into")
                            and command.get("pause_id") == self.serial
                        ):
                            self.stepping = (
                                event["action_id"] if kind == "step" else None
                            )
                            self.pause_next = kind == "into"
                            return
                    process = self.plugin.application.process
                    if process.poll() is not None:
                        self.plugin.emit(
                            "warning",
                            detail=f"Application exited while paused: {process.returncode}",
                        )
                        return
                    time.sleep(0.03)
            finally:
                self.paused = False
                self.plugin.emit("debug-state", paused=False, pause_id=self.serial)

    def inspect(self):
        started = time.monotonic()
        try:
            inspector = importlib.import_module("slint_test.inspection")
            snapshot, png = inspector.capture(self.plugin.application)
            documents = []
            for filename in self.plugin.inspection_sources[:8]:
                path = Path(filename)
                try:
                    with path.open("rb") as stream:
                        content = stream.read(65537)
                    documents.append(
                        {
                            "path": str(path.resolve()),
                            "name": path.name,
                            "text": content[:65536].decode(errors="replace"),
                            "truncated": len(content) > 65536,
                        }
                    )
                except OSError as error:
                    documents.append(
                        {
                            "path": str(path.resolve()),
                            "name": path.name,
                            "text": f"Source unavailable: {error}",
                            "truncated": False,
                        }
                    )
            snapshot["sources"] = documents
            path = f"inspect-{self.serial:04d}-{time.time_ns()}.png"
            (self.plugin.artifacts / path).write_bytes(png)
            self.plugin.emit(
                "inspection",
                pause_id=self.serial,
                screenshot=path,
                capture_duration=time.monotonic() - started,
                **snapshot,
            )
        except Exception as error:  # noqa: BLE001
            self.plugin.emit("warning", detail=f"Inspection unavailable: {error}")
