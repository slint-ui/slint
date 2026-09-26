# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import math
import os
import shutil
import sys
import time
import uuid
from pathlib import Path


def data_root():
    if sys.platform == "darwin":
        return Path.home() / "Library/Application Support/Slint Test Studio"
    if os.name == "nt":
        return Path(os.environ.get("LOCALAPPDATA", Path.home())) / "Slint Test Studio"
    return (
        Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share"))
        / "slint-test-studio"
    )


def atomic_json(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(".tmp")
    with temp.open("w") as stream:
        json.dump(data, stream, indent=2)
        stream.flush()
        os.fsync(stream.fileno())
    temp.replace(path)


class Store:
    def __init__(self, root):
        self.root = Path(root)
        self.runs = self.root / "runs"
        self.runs.mkdir(parents=True, exist_ok=True)
        self.warnings = []

    def acquire(self):
        handle = (self.root / "studio.lock").open("a+")
        try:
            if os.name == "nt":
                import msvcrt

                handle.seek(0)
                if not handle.read(1):
                    handle.write("0")
                    handle.flush()
                handle.seek(0)
                msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl

                fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            handle.close()
            raise RuntimeError(
                "Another Studio instance is using this data directory. Close it or choose --data-dir."
            ) from error
        return handle

    def settings(self, legacy=None):
        path = self.root / "settings.json"
        if path.exists():
            try:
                saved = json.loads(path.read_text())
                if not isinstance(saved, dict) or saved.get("version") != 1:
                    raise ValueError("Unsupported settings")
                projects = saved.get("projects")
                policy = saved.get("retention", {})
                if not isinstance(projects, list) or not all(
                    isinstance(p, dict)
                    and all(
                        isinstance(p.get(k), str)
                        for k in ("repo", "python", "binary", "backend")
                    )
                    and isinstance(p.get("paths"), list)
                    and p["paths"]
                    and all(isinstance(v, str) for v in p["paths"])
                    and isinstance(p.get("view", {}), dict)
                    for p in projects
                ):
                    raise ValueError("Malformed projects")
                if not all(
                    isinstance(policy.get(k), (int, float))
                    and math.isfinite(policy[k])
                    and policy[k] > 0
                    for k in ("count", "days", "gib")
                ):
                    raise ValueError("Malformed retention settings")
                return saved
            except (OSError, ValueError, TypeError, AttributeError):
                self.warnings.append("Settings could not be read; defaults restored.")
        if path.exists():
            path.replace(self.root / f"settings-invalid-{time.time_ns()}.json")
        result = {
            "version": 1,
            "projects": [],
            "retention": {"count": 20, "days": 30, "gib": 5},
        }
        if legacy and legacy.exists():
            try:
                result["legacy"] = json.loads(legacy.read_text())
            except (OSError, ValueError):
                self.warnings.append("Legacy local.json could not be imported.")
        self.save_settings(result)
        return result

    def save_settings(self, settings):
        atomic_json(self.root / "settings.json", settings)

    def create(self, project, selectors, collect=False):
        run_id = uuid.uuid4().hex
        directory = self.runs / run_id
        directory.mkdir()
        metadata = {
            "version": 1,
            "id": run_id,
            "created": time.time(),
            "project": project,
            "selectors": selectors,
            "collect": collect,
            "state": "collecting" if collect else "preflight",
            "pinned": False,
        }
        self.save(directory, metadata)
        return directory, metadata

    def owned(self, directory):
        directory = Path(directory)
        if directory.is_symlink() or directory.parent.resolve() != self.runs.resolve():
            raise ValueError("Not a Studio-owned run directory")
        if len(directory.name) != 32 or any(
            c not in "0123456789abcdef" for c in directory.name
        ):
            raise ValueError("Invalid run directory")
        return directory

    def save(self, directory, metadata):
        atomic_json(self.owned(directory) / "run.json", metadata)

    def history(self, project=None):
        result = []
        for path in self.runs.iterdir():
            try:
                self.owned(path)
                data = json.loads((path / "run.json").read_text())
                if (
                    data.get("version") != 1
                    or data.get("id") != path.name
                    or not isinstance(data.get("created"), (int, float))
                    or not isinstance(data.get("project", {}).get("repo"), str)
                    or not isinstance(data.get("selectors"), list)
                    or not isinstance(data.get("collect"), bool)
                    or not isinstance(data.get("state"), str)
                ):
                    raise ValueError("Unsupported or malformed run metadata")
                if project is None or data["project"]["repo"] == project:
                    result.append(data)
            except (OSError, ValueError, KeyError, AttributeError, TypeError) as error:
                self.warnings.append(f"Cannot read run {path.name}: {error}")
        return sorted(result, key=lambda r: r["created"], reverse=True)

    def recover(self):
        for item in self.history():
            if item["state"] not in ("finished", "Interrupted"):
                item["state"] = "Interrupted"
                self.save(self.runs / item["id"], item)

    def load(self, run_id):
        from state import RunState

        directory = self.owned(self.runs / run_id)
        metadata = json.loads((directory / "run.json").read_text())
        state = RunState()
        path = directory / "events.jsonl"
        sequence = 0
        if path.exists():
            for line in path.read_text().splitlines():
                try:
                    event = json.loads(line)
                    if (
                        event.get("version") != 1
                        or event.get("run_id") != run_id
                        or event.get("sequence") != sequence + 1
                    ):
                        raise ValueError("Unsupported event envelope")
                    sequence = event["sequence"]
                    state.apply(event)
                except (ValueError, KeyError, TypeError) as error:
                    state.diagnostics.append(
                        {"kind": "warning", "detail": f"Damaged history event: {error}"}
                    )
        if metadata["state"] == "Interrupted":
            for item in state.records.values():
                if item["status"] in ("Running", "Queued"):
                    item["status"] = (
                        "Interrupted" if item["status"] == "Running" else "Not run"
                    )
            state.state = "Interrupted"
        return metadata, state

    def delete(self, run_id):
        directory = self.owned(self.runs / run_id)
        metadata = json.loads((directory / "run.json").read_text())
        if metadata["state"] not in ("finished", "Interrupted") or metadata.get(
            "pinned"
        ):
            raise ValueError("Active and pinned runs cannot be deleted")
        shutil.rmtree(directory)

    def prune(self, policy):
        history = self.history()
        sizes = {}
        for item in history:
            directory = self.runs / item["id"]
            sizes[item["id"]] = sum(
                p.stat().st_size
                for p in directory.rglob("*")
                if p.is_file() and not p.is_symlink()
            )
        total = sum(sizes.values())
        removable = [
            r
            for r in reversed(history)
            if r["state"] in ("finished", "Interrupted") and not r.get("pinned")
        ]
        completed = sum(r["state"] in ("finished", "Interrupted") for r in history)
        limit = policy["gib"] * 1024**3
        for item in removable:
            if (
                completed > policy["count"]
                or total > limit
                or time.time() - item["created"] > policy["days"] * 86400
            ):
                self.delete(item["id"])
                total -= sizes[item["id"]]
                completed -= 1
        return "Protected runs exceed the storage target." if total > limit else ""
