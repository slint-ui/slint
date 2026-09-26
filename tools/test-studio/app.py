# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import argparse
import json
import os
import subprocess
import sys
from datetime import timedelta
from pathlib import Path

import slint
from runner import SUITES, TestProcess


class Studio:
    def __init__(self, ui, types, repo, python, binary):
        self.ui, self.types = ui, types
        self.repo, self.python, self.binary = repo, python, binary
        self.records = {}
        self.visible = []
        self.selected = ""
        self.process = None
        self.query = ""
        self.failures_only = False
        self.runs = []
        self.discovered = {}
        ui.workspace_name = repo.name
        ui.select_test = self.select
        ui.search = self.search
        ui.filter_failures = self.filter_failures
        ui.run_selected = lambda: self.run([self.selected] if self.selected else [])
        ui.run_visible = lambda: self.run(list(self.visible))
        ui.stop = self.stop
        ui.refresh = self.discover
        ui.select_step = self.select_step
        ui.open_artifacts = self.open_artifacts
        self.timer = slint.Timer()
        self.timer.start(
            slint.TimerMode.Repeated, timedelta(milliseconds=60), self.poll
        )
        self.discover()

    def discover(self):
        if self.process is not None:
            return
        if not self.python.is_file():
            self.ui.summary = f"Test Python not found: {self.python}"
            self.ui.selected_title = "Set up the test environment"
            return
        self.discovered = {}
        self.start(SUITES, collect=True)

    def start(self, selectors, collect=False):
        try:
            self.process = TestProcess(
                self.repo,
                self.python,
                self.binary,
                selectors,
                collect=collect,
                visible=self.ui.visible_editor,
            )
        except (OSError, ValueError) as error:
            self.ui.summary = str(error)
            self.ui.output = str(error)
            self.ui.active_tab = 2
            for nodeid in selectors:
                if nodeid in self.records:
                    self.records[nodeid].update(status="Error", output=str(error))
            self.update_list()
            return
        self.runs.append(self.process)
        self.ui.busy = True
        self.ui.summary = (
            "Discovering Visual Editor tests…"
            if collect
            else f"Running {len(selectors)} test(s)…"
        )

    def run(self, selectors):
        if self.process is not None or not selectors:
            return
        if not self.binary.is_file():
            self.ui.summary = f"Editor binary not found: {self.binary}"
            return
        for nodeid in selectors:
            self.records[nodeid].update(
                status="Queued", steps=[], output="", duration=0, artifact_dir=""
            )
        self.ui.active_tab = 0
        self.select(selectors[0])
        self.update_list()
        self.start(selectors)

    def stop(self):
        if self.process:
            self.process.stop()
            self.ui.summary = "Stopping the test process and its editor…"

    def search(self, query):
        self.query = query.casefold().strip()
        self.update_list()

    def filter_failures(self, enabled):
        self.failures_only = enabled
        self.update_list()

    def update_list(self):
        self.visible = [
            nodeid
            for nodeid, record in self.records.items()
            if (not self.failures_only or record["status"] == "Failed")
            and self.query
            in f"{nodeid} {record['title']} {record['suite']} {record['case']}".casefold()
        ]
        self.ui.tests = slint.ListModel(
            [
                self.types.TestRow(
                    id=nodeid,
                    title=self.records[nodeid]["title"],
                    detail=" · ".join(
                        filter(
                            None,
                            [
                                self.records[nodeid]["suite"],
                                self.records[nodeid]["case"],
                                self.records[nodeid]["status"],
                            ],
                        )
                    ),
                    status=self.records[nodeid]["status"],
                )
                for nodeid in self.visible
            ]
        )
        counts = {
            status: sum(r["status"] == status for r in self.records.values())
            for status in ("Passed", "Failed", "Skipped")
        }
        self.ui.results_summary = f"{counts['Passed']} passed · {counts['Failed']} failed · {counts['Skipped']} skipped"

    def select(self, nodeid):
        if nodeid not in self.records:
            return
        self.selected = nodeid
        record = self.records[nodeid]
        self.ui.selected_id = nodeid
        self.ui.selected_title = record["title"]
        self.ui.selected_path = record["path"] + (
            f"  ·  {record['case']}" if record["case"] else ""
        )
        self.ui.selected_status = record["status"]
        self.ui.source_code = record["source"]
        self.ui.output = record["output"]
        self.ui.elapsed = f"{record['duration']:.2f}s" if record["duration"] else ""
        self.ui.steps = slint.ListModel(
            [
                self.types.StepRow(
                    title=step["title"].capitalize(),
                    detail=f"{step['duration']:.2f}s"
                    if step["duration"]
                    else step["status"],
                    status=step["status"],
                )
                for step in record["steps"]
            ]
        )
        self.select_step(len(record["steps"]) - 1)

    def select_step(self, index):
        record = self.records.get(self.selected)
        self.ui.selected_step = index
        self.ui.has_screenshot = False
        self.ui.screenshot_caption = "No capture yet"
        if record is None or not 0 <= index < len(record["steps"]):
            return
        step = record["steps"][index]
        self.ui.screenshot_caption = step["title"].capitalize()
        if step["screenshot"]:
            try:
                self.ui.screenshot = slint.Image.load_from_path(step["screenshot"])
                self.ui.has_screenshot = True
            except (OSError, RuntimeError, ValueError) as error:
                self.ui.screenshot_caption = f"Cannot load capture: {error}"
        elif step.get("warning"):
            self.ui.screenshot_caption = step["warning"]

    def poll(self):
        process = self.process
        if process is None:
            return
        changed = False
        selected_changed = False
        for event in process.poll():
            kind = event["kind"]
            nodeid = event.get("nodeid", "")
            if kind == "collected":
                record = {
                    **event,
                    "status": "Not run",
                    "steps": [],
                    "output": "",
                    "duration": 0,
                    "artifact_dir": "",
                }
                if process.collect:
                    self.discovered[event["id"]] = {
                        **self.records.get(event["id"], record),
                        **event,
                    }
            elif kind == "test-start":
                self.records[nodeid]["status"] = "Running"
                self.records[nodeid]["artifact_dir"] = str(process.directory)
                self.ui.summary = f"Running: {self.records[nodeid]['title']}"
                changed = True
            elif kind == "stage-start":
                self.ui.summary = f"Test stage: {event['title']}"
            elif kind == "step":
                self.records[nodeid]["steps"].append(event)
                selected_changed |= nodeid == self.selected
            elif kind == "report":
                record = self.records[nodeid]
                if event["detail"]:
                    record["output"] += event["detail"] + "\n"
                if event["output"]:
                    record["output"] += event["output"] + "\n"
                selected_changed |= nodeid == self.selected
            elif kind == "test-end":
                record = self.records[nodeid]
                record.update(status=event["status"], duration=event["duration"])
                record["output"] += (
                    f"\n{event['status']} in {event['duration']:.2f}s\nArtifacts: {process.directory}\n"
                )
                changed = True
                selected_changed |= nodeid == self.selected
            elif kind == "error":
                self.ui.summary = (
                    event["detail"].splitlines()[-1]
                    if event["detail"]
                    else "Test collection failed"
                )
                self.ui.output = event["detail"]
                self.ui.active_tab = 2
            elif kind == "finished":
                self.ui.busy = False
                if process.collect and event["code"] == 0:
                    self.records = self.discovered
                    if self.selected not in self.records:
                        self.selected = ""
                self.ui.ready = bool(self.records) and self.binary.is_file()
                for record in self.records.values():
                    if record["status"] in ("Running", "Queued"):
                        record["status"] = (
                            "Cancelled" if event["cancelled"] else "Error"
                        )
                        record["output"] += process.log_path.read_text(errors="replace")
                if event["cancelled"]:
                    self.ui.summary = "Run stopped"
                elif event["code"] not in (0, 1):
                    self.ui.summary = (
                        f"Pytest exited with code {event['code']}. See Output."
                    )
                    failure_log = process.log_path.read_text(errors="replace")
                    self.ui.output = failure_log
                    if self.selected in self.records:
                        self.records[self.selected]["output"] += failure_log
                    self.ui.active_tab = 2
                elif process.collect:
                    self.ui.summary = (
                        f"{len(self.records)} tests discovered · ready to run"
                        if self.ui.ready
                        else f"Set --editor-binary to a built Visual Editor: {self.binary}"
                    )
                else:
                    self.ui.summary = (
                        f"Run finished · artifacts in {process.directory.name}"
                    )
                self.process = None
                changed = True
                selected_changed = bool(self.selected)
        if changed:
            self.update_list()
        if not self.selected and self.records:
            preferred = next(
                (
                    n
                    for n in self.records
                    if n.endswith("test_rectangle_undo_redo[handle-move]")
                ),
                next(iter(self.records)),
            )
            self.select(preferred)
        elif selected_changed or changed and self.selected:
            self.select(self.selected)

    def open_artifacts(self):
        directory = self.records.get(self.selected, {}).get("artifact_dir")
        if directory:
            if sys.platform == "darwin":
                subprocess.Popen(["open", directory])
            elif os.name == "nt":
                os.startfile(directory)
            else:
                subprocess.Popen(["xdg-open", directory])

    def close(self):
        self.timer.stop()
        for process in self.runs:
            process.close()


def main():
    config_path = Path(__file__).with_name("local.json")
    config = json.loads(config_path.read_text()) if config_path.exists() else {}
    parser = argparse.ArgumentParser(
        description="Run Visual Editor tests in a native Slint desktop app"
    )
    parser.add_argument(
        "--repo",
        type=Path,
        default=Path(config.get("repo", Path(__file__).resolve().parents[2])),
    )
    parser.add_argument("--test-python", type=Path, default=config.get("test_python"))
    parser.add_argument(
        "--editor-binary", type=Path, default=config.get("editor_binary")
    )
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
    parser.add_argument("--dark", action="store_true")
    args = parser.parse_args()
    repo = args.repo.resolve()
    python = args.test_python or repo / "tools/editor/ui-tests/.venv" / (
        "Scripts/python.exe" if os.name == "nt" else "bin/python"
    )
    binary = args.editor_binary or Path(
        os.environ.get(
            "SLINT_EDITOR_BINARY",
            str(
                repo
                / "target/debug"
                / ("slint-editor.exe" if os.name == "nt" else "slint-editor")
            ),
        )
    )
    components = args.components.resolve()
    if not (components / "primer.slint").is_file():
        parser.error(
            f"Primer library not found at {components}; pass --components or SLINT_PRIMER_DIR"
        )
    ui_path = Path(__file__).parent / "ui/main.slint"
    types = slint.load_file(
        ui_path, library_paths={"primer": components / "primer.slint"}
    )
    ui = types.TestStudio(dark=args.dark)
    studio = Studio(ui, types, repo, python.absolute(), binary.absolute())
    try:
        ui.run()
    finally:
        studio.close()


if __name__ == "__main__":
    main()
