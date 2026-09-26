# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import argparse
import copy
import json
import os
import queue
import signal
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import TypedDict

import slint
from PIL import Image

from service import PRIMER_REVISION, Service
from state import matching, rerun_selection, tree_rows
from storage import data_root
from syntax import highlight


class Project(TypedDict):
    repo: str
    python: str
    binary: str
    paths: list[str]
    backend: str
    view: dict


def default_project(repo) -> Project:
    repo = Path(repo).expanduser().absolute()
    return {
        "repo": str(repo),
        "python": str(
            repo
            / "tools/editor/ui-tests/.venv"
            / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        ),
        "binary": str(
            repo
            / "target/debug"
            / ("slint-editor.exe" if os.name == "nt" else "slint-editor")
        ),
        "paths": ["tests"],
        "backend": "headless-skia",
        "view": {},
    }


class Studio:
    def __init__(self, ui, types, args):
        self.ui, self.types, self.args = ui, types, args
        self.service = Service(args.data_dir)
        self.images = ThreadPoolExecutor(max_workers=1)
        self.image_updates = queue.Queue()
        self.current = {}
        self.records = {}
        self.run_records = {}
        self.metadata = None
        self.history = []
        self.selected = ""
        self.selected_step = -1
        self.restore_selection = ""
        self.restore_step = -1
        self.capture_key = None
        self.loaded_capture_key = None
        self.session_override = any((args.repo, args.test_python, args.editor_binary))
        self.project: Project | None = None
        self.settings = {}
        self.collapsed = set()
        self.rows = []
        self.visible = []
        self.historical = False
        self.operation = None
        self.pending_restore = False
        ui.highlight_source = lambda source, dark: slint.StyledText.from_markdown(
            highlight(source, dark)
        )
        ui.select_test = self.select
        ui.toggle_group = self.toggle
        ui.navigate_tree = self.navigate
        ui.expand_selected = self.expand_selected
        ui.search = lambda _: self.update_list()
        ui.filter_changed = self.update_list
        ui.run_selected = self.run_selected
        ui.run_visible = lambda: self.run(list(self.visible))
        ui.rerun_failed = self.rerun
        ui.stop = self.stop
        ui.refresh = self.discover
        ui.select_step = self.select_step
        ui.open_artifacts = self.open_artifacts
        ui.show_settings = self.show_settings
        ui.save_settings = self.save_settings
        ui.choose_project = self.choose_project
        ui.show_history = self.show_history
        ui.open_run = self.open_run
        ui.pin_run = lambda run_id: self.change_run("pin", run_id)
        ui.delete_run = lambda run_id: self.change_run("delete", run_id)
        ui.show_current = self.show_current
        self.timer = slint.Timer()
        self.timer.start(
            slint.TimerMode.Repeated, timedelta(milliseconds=60), self.poll
        )
        self.service.submit("boot", components=str(args.components))

    def persist(self):
        if self.project is None:
            return
        self.project["view"] = {
            "selected": self.restore_selection or self.selected,
            "step": self.restore_step if self.restore_selection else self.selected_step,
            "query": self.ui.query,
            "marker": self.ui.marker_query,
            "outcome": self.ui.outcome_index,
            "collapsed": sorted(self.collapsed),
            "dark": self.ui.dark,
        }
        if not self.session_override:
            self.settings["projects"] = [copy.deepcopy(self.project)] + [
                p
                for p in self.settings.get("projects", [])
                if p["repo"] != self.project["repo"]
            ][:9]
        self.service.submit("settings", settings=self.settings)
        self.ui.recent_projects = slint.ListModel(
            [p["repo"] for p in self.settings["projects"]]
        )

    def activate(self, project):
        self.project = copy.deepcopy(project)
        self.current, self.records, self.run_records = {}, {}, {}
        self.metadata = None
        view = project.get("view", {})
        self.restore_selection = view.get("selected", "")
        self.restore_step = view.get("step", -1)
        self.selected = self.restore_selection
        self.selected_step = self.restore_step
        self.clear_capture()
        self.collapsed = set(view.get("collapsed", []))
        self.ui.query = view.get("query", "")
        self.ui.marker_query = view.get("marker", "")
        self.ui.outcome_index = max(0, min(7, int(view.get("outcome", 0))))
        self.ui.dark = self.args.dark or view.get("dark", self.ui.dark)
        self.ui.workspace_name = project["repo"]
        self.ui.visible_editor = project["backend"] == "winit-skia"
        self.ui.historical = False
        self.historical = False
        self.pending_restore = True
        self.update_list()
        self.discover()

    def choose_project(self, index):
        if not self.ui.busy and 0 <= index < len(self.settings.get("projects", [])):
            project = copy.deepcopy(self.settings["projects"][index])
            self.persist()
            self.session_override = False
            self.activate(project)

    def discover(self):
        if self.project and not self.ui.busy:
            self.start(self.project["paths"], collect=True)

    def start(self, selectors, collect=False):
        self.persist()
        self.ui.busy = True
        self.ui.summary = (
            "Discovering tests…" if collect else "Validating test environment…"
        )
        self.operation = "collect" if collect else "run"
        self.ui.run_diagnostics = ""
        self.service.submit(
            "run",
            project=self.project,
            selectors=selectors,
            collect=collect,
            retention=self.settings["retention"],
            items={n: self.current[n] for n in selectors if n in self.current}
            if not collect
            else {},
        )

    def run(self, selectors):
        if self.ui.busy or not selectors:
            return
        available = [n for n in selectors if n in self.current]
        missing = [n for n in selectors if n not in self.current]
        if missing:
            self.ui.run_diagnostics = "Unavailable in current discovery:\n" + "\n".join(
                missing
            )
            self.ui.active_tab = 2
            self.ui.summary = "Refresh discovery or select available tests."
            return
        self.show_current()
        self.run_records = {}
        self.selected = available[0]
        self.ui.active_tab = 0
        self.start(available)

    def run_selected(self):
        row = next((r for r in self.rows if r["id"] == self.selected), None)
        if row:
            self.run(row["members"])

    def rerun(self):
        available, missing = rerun_selection(self.run_records, self.current)
        if missing:
            self.ui.run_diagnostics = (
                "Failed tests unavailable in current discovery:\n" + "\n".join(missing)
            )
            self.ui.active_tab = 2
            self.ui.summary = (
                "Some failures cannot be rerun; select available tests to continue."
            )
        elif available:
            self.run(available)

    def stop(self):
        self.service.stop()
        self.ui.summary = "Stopping and cleaning up application processes…"

    def update_list(self):
        outcomes = [
            "All",
            "Failures",
            "Passed",
            "Skipped",
            "Expected failure",
            "Unexpected pass",
            "Cancelled",
            "Not run",
        ]
        self.visible = matching(
            self.records,
            self.ui.query,
            self.ui.marker_query,
            outcomes[self.ui.outcome_index],
        )
        self.rows = tree_rows(self.records, self.visible, self.collapsed)
        self.ui.tests = slint.ListModel(
            [
                self.types.TestRow(
                    id=r["id"],
                    title=r["title"],
                    detail=f"{len(r['members'])} tests · {r['status']}"
                    if r["group"]
                    else r["status"],
                    status=r["status"],
                    depth=r["depth"],
                    group=r["group"],
                    expanded=r["expanded"],
                )
                for r in self.rows
            ]
        )
        self.ui.shown_count = len(self.visible)
        self.ui.suite_summary = f"{len({r['suite'] for r in self.records.values()})} files · {len(self.records)} tests"
        counts = {
            s: sum(r["status"] == s for r in self.run_records.values())
            for s in ("Passed", "Failed", "Error", "Crashed", "Skipped")
        }
        self.ui.results_summary = f"{counts['Passed']} passed · {counts['Failed']} failed · {counts['Error'] + counts['Crashed']} errors · {counts['Skipped']} skipped"
        self.ui.ready = bool(self.current)
        self.ui.can_rerun = bool(
            rerun_selection(self.run_records, self.current)[0]
            or rerun_selection(self.run_records, self.current)[1]
        )
        row_ids = {r["id"] for r in self.rows}
        if self.restore_selection in row_ids:
            self.selected = self.restore_selection
            self.selected_step = self.restore_step
            self.restore_selection = ""
        if self.selected not in row_ids:
            self.selected_step = -1
            self.capture_key = None
            self.selected = self.rows[0]["id"] if self.rows else ""
        self.render_selection()

    def toggle(self, nodeid):
        if nodeid in self.collapsed:
            self.collapsed.remove(nodeid)
        else:
            self.collapsed.add(nodeid)
        self.update_list()

    def expand_selected(self, expanded):
        row = next((r for r in self.rows if r["id"] == self.selected), None)
        if row and row["group"] and row["expanded"] != expanded:
            self.toggle(row["id"])

    def navigate(self, direction):
        ids = [r["id"] for r in self.rows]
        if ids:
            index = ids.index(self.selected) if self.selected in ids else 0
            index = max(0, min(len(ids) - 1, index + direction))
            self.select(ids[index])
            self.ui.reveal_y = sum(48 if r["group"] else 58 for r in self.rows[:index])
            self.ui.reveal_height = 48 if self.rows[index]["group"] else 58
            self.ui.reveal_serial += 1

    def select(self, nodeid):
        self.restore_selection = ""
        if self.selected != nodeid:
            self.selected_step = -1
            self.capture_key = None
        self.selected = nodeid
        self.render_selection()

    def render_selection(self):
        ui = self.ui
        ui.selected_id = self.selected
        item = self.records.get(self.selected)
        row = next((r for r in self.rows if r["id"] == self.selected), None)
        ui.steps = slint.ListModel([])
        ui.elapsed = ""
        ui.has_artifacts = bool(self.metadata)
        if item is None:
            self.clear_capture()
            ui.selected_title = row["title"] if row else "No tests selected"
            ui.selected_path = (
                f"{len(row['members'])} matching tests"
                if row
                else "Choose a project or adjust your filters."
            )
            ui.selected_status = row["status"] if row else "Not run"
            ui.source_code = "Select an individual test to inspect its source."
            ui.output = ""
            ui.screenshot_caption = "Select a test to inspect its captures."
            return
        ui.selected_title, ui.selected_path = item["title"], item["path"]
        ui.selected_status = item["status"]
        ui.source_code = item["source"]
        ui.output = item["output"]
        ui.elapsed = f"{item['duration']:.2f}s" if item["duration"] else ""
        ui.steps = slint.ListModel(
            [
                self.types.StepRow(
                    title=s["title"],
                    detail=f"{s['duration']:.2f}s · {s['status']}"
                    if s["duration"]
                    else s["status"],
                    status=s["status"],
                )
                for s in item["steps"]
            ]
        )
        index = (
            self.selected_step if self.selected_step >= 0 else len(item["steps"]) - 1
        )
        self.select_step(index, automatic=True)

    def clear_capture(self):
        self.capture_key = None
        self.loaded_capture_key = None
        self.ui.has_screenshot = False

    def select_step(self, index, automatic=False):
        if not automatic:
            self.selected_step = index
        self.ui.selected_step = index
        item = self.records.get(self.selected)
        if not item or not 0 <= index < len(item["steps"]):
            self.clear_capture()
            self.ui.screenshot_caption = "No capture yet"
            return
        step = item["steps"][index]
        caption = step.get("warning") or step["title"]
        if not step["screenshot"] or not self.metadata:
            self.clear_capture()
            self.ui.screenshot_caption = caption
            return
        run_id = item.get("run_id", self.metadata["id"])
        key = (run_id, self.selected, index, step["screenshot"])
        if self.loaded_capture_key == key:
            self.capture_key = key
            self.ui.has_screenshot = True
            self.ui.screenshot_caption = caption
            return
        if self.capture_key == key:
            return
        if self.loaded_capture_key and self.loaded_capture_key[:2] != key[:2]:
            self.clear_capture()
        self.capture_key = key
        directory = self.args.data_dir / "runs" / run_id
        path = (directory / step["screenshot"]).resolve()
        if not path.is_relative_to(directory.resolve()):
            self.clear_capture()
            self.ui.screenshot_caption = "Invalid capture path"
            return
        self.ui.screenshot_caption = f"Loading {step['title']}…"

        def load():
            try:
                with Image.open(path) as image:
                    image = image.convert("RGBA")
                    result = {
                        "key": key,
                        "caption": caption,
                        "pixels": image.tobytes(),
                        "width": image.width,
                        "height": image.height,
                    }
            except (OSError, ValueError) as error:
                result = {"key": key, "error": str(error)}
            self.image_updates.put(result)

        self.images.submit(load)

    def show_current(self):
        self.historical = False
        self.ui.historical = False
        self.records = self.current
        self.update_list()

    def show_settings(self):
        if self.ui.busy or self.project is None:
            return
        for field in ("repo", "python", "binary"):
            setattr(self.ui, f"config_{field}", self.project[field])
        self.ui.config_paths = "\n".join(self.project["paths"])
        self.ui.config_backend = 1 if self.project["backend"] == "winit-skia" else 0
        for field in ("count", "days", "gib"):
            setattr(self.ui, f"config_{field}", str(self.settings["retention"][field]))
        self.ui.settings_error = ""
        self.ui.settings_open = True

    def save_settings(self):
        if self.ui.busy:
            return
        try:
            policy = {
                k: float(getattr(self.ui, f"config_{k}"))
                for k in ("count", "days", "gib")
            }
            if (
                any(not 0 < v < 1000000 for v in policy.values())
                or not policy["count"].is_integer()
            ):
                raise ValueError(
                    "Retention values must be positive; run count must be an integer."
                )
            project = default_project(self.ui.config_repo)
            project.update(
                python=str(Path(self.ui.config_python).expanduser().absolute()),
                binary=str(Path(self.ui.config_binary).expanduser().absolute()),
                paths=[
                    p.strip() for p in self.ui.config_paths.splitlines() if p.strip()
                ],
                backend="winit-skia" if self.ui.config_backend else "headless-skia",
            )
            if not project["paths"]:
                raise ValueError("Enter at least one discovery path.")
            self.settings["retention"] = policy
            self.persist()
            self.ui.settings_open = False
            self.session_override = False
            self.activate(project)
        except ValueError as error:
            self.ui.settings_error = str(error)

    def change_run(self, kind, run_id):
        if self.project is not None and not self.ui.busy:
            self.service.submit(kind, id=run_id, repo=self.project["repo"])

    def show_history(self):
        if not self.ui.busy and self.project is not None:
            self.ui.history_open = True
            self.service.submit("history", repo=self.project["repo"])

    def open_run(self, run_id):
        if not self.ui.busy:
            self.ui.history_open = False
            self.service.submit("load", id=run_id)

    def open_artifacts(self):
        if self.metadata:
            run_id = self.records.get(self.selected, {}).get(
                "run_id", self.metadata["id"]
            )
            path = str(self.args.data_dir / "runs" / run_id)
            if sys.platform == "darwin":
                subprocess.Popen(["open", path])
            elif os.name == "nt":
                os.startfile(path)
            else:
                subprocess.Popen(["xdg-open", path])

    def poll(self):
        for _ in range(16):
            try:
                kind, data = self.service.updates.get_nowait()
            except queue.Empty:
                break
            if kind == "boot":
                self.settings = data["settings"]
                legacy = self.settings.pop("legacy", {})
                projects = self.settings.get("projects", [])
                project = (
                    copy.deepcopy(projects[0])
                    if projects
                    else default_project(
                        legacy.get("repo", Path(__file__).resolve().parents[2])
                    )
                )
                if not projects:
                    if "test_python" in legacy:
                        project["python"] = legacy["test_python"]
                    if "editor_binary" in legacy:
                        project["binary"] = legacy["editor_binary"]
                if not projects:
                    self.settings["projects"] = [copy.deepcopy(project)]
                if self.args.repo:
                    project = default_project(self.args.repo)
                for key, value in (
                    ("python", self.args.test_python),
                    ("binary", self.args.editor_binary),
                ):
                    if value:
                        project[key] = str(value.absolute())
                self.activate(project)
            elif kind == "components":
                self.ui.custom_components = (
                    data["modified"] or data["revision"] != data["expected_revision"]
                )
            elif kind == "run":
                metadata, snapshot = data["metadata"], data["snapshot"]
                collect = metadata["collect"]
                self.ui.summary = snapshot["state"].capitalize() + "…"
                diagnostics = "\n\n".join(e["detail"] for e in snapshot["diagnostics"])
                self.ui.run_diagnostics = diagnostics
                self.ui.environment_info = json.dumps(snapshot["environment"], indent=2)
                if not collect:
                    self.metadata = metadata
                    self.run_records = snapshot["records"]
                    self.current.update(self.run_records)
                    self.records = self.current
                    self.update_list()
                if data["final"]:
                    self.ui.busy = False
                    self.operation = None
                    if collect:
                        if (
                            snapshot["exit_code"] in (0, 5)
                            and not snapshot["cancelled"]
                            and not any(
                                e["kind"] == "error" for e in snapshot["diagnostics"]
                            )
                        ):
                            self.current = {
                                n: {
                                    **self.current.get(n, r),
                                    **{
                                        k: v
                                        for k, v in r.items()
                                        if k
                                        not in (
                                            "steps",
                                            "output",
                                            "status",
                                            "duration",
                                            "run_id",
                                        )
                                    },
                                }
                                for n, r in snapshot["records"].items()
                            }
                            self.ui.stale = False
                            self.ui.summary = (
                                f"{len(self.current)} tests discovered"
                                if self.current
                                else "No tests found"
                            )
                        else:
                            self.ui.stale = True
                            self.ui.summary = (
                                "Discovery stopped or failed; previous tree is stale"
                            )
                            self.ui.active_tab = 2
                        if not self.historical:
                            self.records = self.current
                    else:
                        self.ui.summary = (
                            "Run stopped"
                            if snapshot["cancelled"]
                            else f"Run finished · pytest exit {snapshot['exit_code']}"
                        )
                        if diagnostics:
                            self.ui.active_tab = 2
                    self.update_list()
            elif kind == "history":
                self.history = [r for r in data if not r["collect"]]
                self.ui.run_history = slint.ListModel(
                    [
                        self.types.HistoryRow(
                            id=r["id"],
                            title=datetime.fromtimestamp(r["created"], UTC)
                            .astimezone()
                            .strftime("%d %b · %H:%M:%S"),
                            detail=f"{r['state']} · {len(r['selectors'])} selected",
                            pinned=r.get("pinned", False),
                        )
                        for r in self.history
                    ]
                )
                if self.pending_restore and not self.ui.busy:
                    self.pending_restore = False
                    if self.history:
                        self.open_run(self.history[0]["id"])
            elif kind == "loaded":
                self.metadata = data["metadata"]
                self.run_records = data["snapshot"]["records"]
                self.ui.environment_info = json.dumps(
                    data["snapshot"]["environment"], indent=2
                )
                self.records = self.run_records
                self.historical = True
                self.ui.historical = True
                self.ui.run_diagnostics = "\n\n".join(
                    e["detail"] for e in data["snapshot"]["diagnostics"]
                )
                self.ui.summary = f"Historical run · {self.metadata['id'][:8]} · {self.metadata['state']}"
                self.capture_key = None
                self.update_list()
            elif kind == "fatal":
                self.ui.busy = True
                self.ui.summary = data
                self.ui.run_diagnostics = data
                self.ui.selected_title = "Studio storage unavailable"
                self.ui.active_tab = 2
            elif kind in ("error", "notice") and data:
                self.ui.summary = data.splitlines()[-1]
                self.ui.run_diagnostics += "\n" + data
                if kind == "error":
                    self.ui.active_tab = 2
                    if self.operation:
                        self.ui.busy = False
                        self.operation = None
        for _ in range(8):
            try:
                data = self.image_updates.get_nowait()
            except queue.Empty:
                break
            if data["key"] == self.capture_key:
                if "error" in data:
                    self.clear_capture()
                    self.ui.screenshot_caption = "Cannot load capture: " + data["error"]
                else:
                    array = memoryview(data["pixels"]).cast(
                        "B", shape=(data["height"], data["width"], 4)
                    )
                    self.ui.screenshot = slint.Image.load_from_array(array)
                    self.loaded_capture_key = data["key"]
                    self.ui.has_screenshot = True
                    self.ui.screenshot_caption = data["caption"]

    def close(self):
        self.timer.stop()
        self.persist()
        self.service.close()
        self.images.shutdown(wait=False, cancel_futures=True)


def main():
    parser = argparse.ArgumentParser(
        description="Run Visual Editor tests in a native Slint desktop app"
    )
    parser.add_argument("--repo", type=Path)
    parser.add_argument("--test-python", type=Path)
    parser.add_argument("--editor-binary", type=Path)
    parser.add_argument(
        "--components",
        type=Path,
        default=Path(
            os.environ.get(
                "SLINT_PRIMER_DIR",
                str(Path.home() / "slint/github-app/packages/primer-slint"),
            )
        ),
    )
    parser.add_argument("--data-dir", type=Path, default=data_root())
    parser.add_argument("--dark", action="store_true")
    args = parser.parse_args()
    args.data_dir = args.data_dir.expanduser().absolute()
    components = args.components.resolve()
    if not (components / "primer.slint").is_file():
        parser.error(
            f"Primer library not found: {components}. Expected github-app revision {PRIMER_REVISION}"
        )
    types = slint.load_file(
        Path(__file__).parent / "ui/main.slint",
        library_paths={"primer": components / "primer.slint"},
    )
    ui = types.TestStudio(dark=args.dark)
    studio = Studio(ui, types, args)
    signal.signal(signal.SIGTERM, lambda *_: slint.quit_event_loop())
    signal.signal(signal.SIGINT, lambda *_: slint.quit_event_loop())
    try:
        ui.run()
    finally:
        studio.close()


if __name__ == "__main__":
    main()
