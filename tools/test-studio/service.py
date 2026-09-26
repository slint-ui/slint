# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import copy
import json
import queue
import subprocess
import threading
import time
from pathlib import Path

from events import Writer
from preflight import check_environment, preflight
from runner import TestProcess
from state import RunState
from storage import Store

PRIMER_REVISION = "17ef19d5582858f1c7a91dd677dbf537941e2b1a"


class Service:
    def __init__(self, root):
        self.root = root
        self.commands = queue.Queue()
        self.updates = queue.Queue()
        self.cancel = threading.Event()
        self.cache = {}
        self.component_info = {}
        self.thread = threading.Thread(target=self.work, daemon=True)
        self.thread.start()

    def submit(self, kind, **data):
        if kind == "run":
            self.cancel.clear()
        self.commands.put((kind, copy.deepcopy(data)))

    def stop(self):
        self.cancel.set()

    def close(self):
        self.stop()
        self.commands.put(("close", {}))
        self.thread.join(timeout=6)

    def work(self):
        try:
            store = Store(self.root)
            with store.acquire():
                self.dispatch(store)
        except Exception as error:  # noqa: BLE001
            self.updates.put(("fatal", str(error)))

    def dispatch(self, store):
        while True:
            kind, data = self.commands.get()
            if kind == "close":
                return
            try:
                if kind == "boot":
                    store.recover()
                    settings = store.settings(Path(__file__).with_name("local.json"))
                    self.updates.put(("boot", {"settings": settings}))
                    components = data.get("components")
                    if components:
                        result = subprocess.run(
                            ["git", "-C", components, "rev-parse", "HEAD"],
                            capture_output=True,
                            text=True,
                            timeout=5,
                            check=False,
                        )
                        dirty = subprocess.run(
                            [
                                "git",
                                "-C",
                                components,
                                "status",
                                "--porcelain",
                                "--",
                                ".",
                            ],
                            capture_output=True,
                            text=True,
                            timeout=5,
                            check=False,
                        )
                        expected = PRIMER_REVISION
                        self.component_info = {
                            "path": components,
                            "expected_revision": expected,
                            "revision": result.stdout.strip(),
                            "modified": bool(dirty.stdout.strip()),
                        }
                        self.updates.put(("components", self.component_info))
                        if result.stdout.strip() != expected or dirty.stdout.strip():
                            self.updates.put(
                                (
                                    "notice",
                                    f"Primer local override: {components}; expected revision {expected[:12]}",
                                )
                            )
                elif kind == "settings":
                    store.save_settings(data["settings"])
                    warning = store.prune(data["settings"]["retention"])
                    self.updates.put(("notice", warning))
                elif kind == "run":
                    self.execute(store, **data)
                elif kind == "history":
                    self.updates.put(("history", store.history(data["repo"])))
                elif kind == "load":
                    metadata, state = store.load(data["id"])
                    self.updates.put(
                        ("loaded", {"metadata": metadata, "snapshot": state.snapshot()})
                    )
                elif kind in ("pin", "delete"):
                    directory = store.runs / data["id"]
                    metadata = json.loads(
                        (store.owned(directory) / "run.json").read_text()
                    )
                    if kind == "pin":
                        metadata["pinned"] = not metadata.get("pinned", False)
                        store.save(directory, metadata)
                    else:
                        store.delete(data["id"])
                    self.updates.put(("history", store.history(data["repo"])))
                for warning in store.warnings:
                    self.updates.put(("notice", warning))
                store.warnings.clear()
            except Exception as error:  # noqa: BLE001
                self.updates.put(("error", str(error)))

    def execute(self, store, project, selectors, collect, retention, items=None):
        directory, metadata = store.create(project, selectors, collect)
        state = RunState(state=metadata["state"])
        process = None
        writer = Writer(directory / "events.jsonl")

        def publish(final=False):
            if not final and self.updates.qsize() >= 2:
                return
            changed = metadata["state"] != state.state
            metadata["state"] = state.state
            if changed:
                store.save(directory, metadata)
            if final:
                metadata["exit_code"] = state.exit_code
                metadata["environment"] = state.environment
                store.save(directory, metadata)
            self.updates.put(
                (
                    "run",
                    {
                        "metadata": copy.deepcopy(metadata),
                        "snapshot": state.snapshot(),
                        "final": final,
                    },
                )
            )

        def emit(kind, **data):
            event = writer.emit(kind, **data)
            state.apply(event)

        try:
            emit("state", state=state.state)
            for item in (items or {}).values():
                emit(
                    "collected",
                    **{
                        k: v
                        for k, v in item.items()
                        if k
                        in (
                            "id",
                            "title",
                            "suite",
                            "case",
                            "source",
                            "path",
                            "groups",
                            "markers",
                        )
                    },
                )
            publish()
            info = check_environment(project, directory, self.cancel)
            info["studio_components"] = self.component_info
            if not collect:
                info = preflight(project, directory, self.cancel, self.cache, info)
            emit("environment", data=info)
            emit("state", state="collecting" if collect else "running")
            process = TestProcess(
                Path(project["repo"]),
                Path(project["python"]),
                Path(project["binary"]),
                selectors,
                collect=collect,
                visible=project["backend"] == "winit-skia",
                directory=directory,
            )
            last = 0
            while not process.finished:
                if self.cancel.is_set():
                    process.stop()
                    state.state = "stopping"
                for event in process.poll():
                    state.apply(event)
                if time.monotonic() - last > 0.12 or process.finished:
                    publish()
                    last = time.monotonic()
                if not process.finished:
                    time.sleep(0.03)
        except InterruptedError:
            if process:
                process.close()
                writer = Writer(directory / "events.jsonl")
            emit("finished", code=130, cancelled=True)
        except Exception as error:  # noqa: BLE001
            if process:
                process.close()
                writer = Writer(directory / "events.jsonl")
            emit(
                "error",
                category="preflight" if state.state == "preflight" else "runner",
                detail=str(error),
            )
            emit("finished", code=3, cancelled=self.cancel.is_set())
        finally:
            if process:
                process.close()
            publish(final=True)
            warning = store.prune(retention)
            if warning:
                self.updates.put(("notice", warning))
            self.updates.put(("history", store.history(project["repo"])))
