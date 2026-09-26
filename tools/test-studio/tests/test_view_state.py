# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import importlib.util
import queue
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

from state import record


class FakeService:
    def __init__(self, root):
        self.updates = queue.Queue()
        self.commands = []

    def submit(self, kind, **data):
        self.commands.append((kind, data))

    def stop(self):
        pass


@pytest.fixture
def studio(monkeypatch, tmp_path):
    timer = SimpleNamespace(start=lambda *args: None, stop=lambda: None)
    fake_slint = SimpleNamespace(
        ListModel=list, Timer=lambda: timer, TimerMode=SimpleNamespace(Repeated=0)
    )
    monkeypatch.setitem(sys.modules, "slint", fake_slint)
    spec = importlib.util.spec_from_file_location(
        "studio_view_test", Path(__file__).parents[1] / "app.py"
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    monkeypatch.setattr(module, "Service", FakeService)
    ui = SimpleNamespace(
        query="",
        marker_query="",
        outcome_index=0,
        dark=False,
        busy=False,
        run_diagnostics="",
        visible_editor=False,
    )
    types = SimpleNamespace(
        TestRow=SimpleNamespace, StepRow=SimpleNamespace, HistoryRow=SimpleNamespace
    )
    args = SimpleNamespace(
        data_dir=tmp_path,
        repo=None,
        test_python=None,
        editor_binary=None,
        dark=False,
        components=tmp_path,
    )
    view = module.Studio(ui, types, args)
    view.settings = {"projects": [], "retention": {"count": 20, "days": 30, "gib": 5}}
    view.project = module.default_project(tmp_path)
    yield view
    view.images.shutdown(wait=True)


def example():
    return record(
        {
            "id": "a",
            "title": "A",
            "path": "test_a.py:1",
            "source": "old source",
            "case": "",
            "suite": "A",
            "run_id": "previous",
        }
    )


def test_collection_failure_preserves_tree_and_marks_stale(studio):
    item = example()
    studio.current = {"a": item}
    studio.records = studio.current
    studio.service.updates.put(
        (
            "run",
            {
                "metadata": {"collect": True},
                "snapshot": {
                    "state": "finished",
                    "diagnostics": [{"kind": "error", "detail": "bad syntax"}],
                    "environment": {},
                    "exit_code": 2,
                    "cancelled": False,
                    "records": {},
                },
                "final": True,
            },
        )
    )
    studio.poll()
    assert studio.current["a"]["source"] == "old source"
    assert studio.ui.stale
    assert not studio.ui.busy


def test_refresh_keeps_capture_ownership(studio):
    item = example()
    item.update(
        status="Passed",
        steps=[
            {
                "title": "step",
                "screenshot": "capture.png",
                "duration": 0,
                "status": "Passed",
            }
        ],
    )
    studio.current = {"a": item}
    studio.records = studio.current
    collected = {**example(), "source": "new source", "run_id": "collection"}
    studio.service.updates.put(
        (
            "run",
            {
                "metadata": {"collect": True},
                "snapshot": {
                    "state": "finished",
                    "diagnostics": [],
                    "environment": {},
                    "exit_code": 0,
                    "cancelled": False,
                    "records": {"a": collected},
                },
                "final": True,
            },
        )
    )
    studio.poll()
    assert studio.current["a"]["run_id"] == "previous"
    assert studio.current["a"]["source"] == "new source"
    assert studio.current["a"]["status"] == "Passed"


def test_cli_overrides_are_not_saved(studio):
    original = dict(studio.project)
    studio.settings["projects"] = [original]
    studio.project = {**original, "binary": "/temporary/editor"}
    studio.session_override = True
    studio.persist()
    assert studio.settings["projects"][0]["binary"] == original["binary"]


def test_historical_results_do_not_replace_current_discovery(studio):
    studio.current = {"new": example()}
    studio.service.updates.put(
        (
            "loaded",
            {
                "metadata": {"id": "history", "state": "finished"},
                "snapshot": {
                    "records": {"old": example()},
                    "diagnostics": [],
                    "environment": {},
                },
            },
        )
    )
    studio.poll()
    assert list(studio.current) == ["new"]
    assert list(studio.records) == ["old"]
    assert studio.ui.historical
    assert studio.ui.source_code == "old source"


def test_updates_preserve_selected_capture(studio):
    item = example()
    item["steps"] = [
        {"title": str(i), "screenshot": "", "status": "Passed", "duration": 0}
        for i in range(3)
    ]
    studio.current = studio.records = {"a": item}
    studio.selected = "a"
    studio.selected_step = 0
    studio.update_list()
    assert studio.ui.selected_step == 0
    item["steps"].append(
        {"title": "new", "screenshot": "", "status": "Passed", "duration": 0}
    )
    studio.update_list()
    assert studio.ui.selected_step == 0


def test_restore_selection_survives_asynchronous_discovery(studio):
    project = {**studio.project, "view": {"selected": "a", "step": 0}}
    studio.activate(project)
    assert studio.restore_selection == "a"
    assert studio.settings["projects"][0]["view"]["selected"] == "a"
    studio.current = studio.records = {"a": example()}
    studio.update_list()
    assert studio.selected == "a"
    assert studio.selected_step == 0


@pytest.fixture
def captures(studio, monkeypatch):
    from PIL import Image

    jobs = []
    monkeypatch.setattr(studio.images, "submit", jobs.append)
    monkeypatch.setattr(
        sys.modules["slint"],
        "Image",
        SimpleNamespace(load_from_array=lambda pixels: bytes(pixels)),
        raising=False,
    )
    item = example()
    directory = studio.args.data_dir / "runs" / "previous"
    directory.mkdir(parents=True)
    for index, color in enumerate(("red", "green", "blue")):
        name = f"{index}.png"
        Image.new("RGBA", (2, 2), color).save(directory / name)
        item["steps"].append(
            {"title": name, "screenshot": name, "duration": 0, "status": "Passed"}
        )
    studio.records = {"a": item}
    studio.selected = "a"
    studio.metadata = {"id": "previous"}
    studio.select_step(0)
    jobs.pop()()
    studio.poll()
    return jobs


def test_capture_stays_visible_until_replacement_is_ready(studio, captures):
    previous = studio.ui.screenshot
    studio.select_step(1)
    studio.render_selection()
    assert studio.ui.has_screenshot
    assert studio.ui.screenshot == previous
    assert studio.ui.screenshot_caption == "Loading 1.png…"
    assert len(captures) == 1
    captures.pop()()
    studio.poll()
    assert studio.ui.has_screenshot
    assert studio.ui.screenshot != previous
    assert studio.ui.screenshot_caption == "1.png"


def test_rapid_capture_changes_ignore_obsolete_loads(studio, captures):
    previous = studio.ui.screenshot
    studio.select_step(1)
    studio.select_step(2)
    captures[0]()
    studio.poll()
    assert studio.ui.screenshot == previous
    assert studio.ui.has_screenshot
    captures[1]()
    studio.poll()
    assert studio.ui.screenshot != previous
    assert studio.ui.screenshot_caption == "2.png"


def test_return_to_displayed_capture_reuses_image(studio, captures):
    previous = studio.ui.screenshot
    studio.select_step(1)
    studio.select_step(0)
    assert studio.ui.has_screenshot
    assert studio.ui.screenshot_caption == "0.png"
    assert len(captures) == 1
    captures[0]()
    studio.poll()
    assert studio.ui.screenshot == previous
    assert studio.ui.screenshot_caption == "0.png"


def test_step_without_capture_invalidates_pending_load(studio, captures):
    studio.select_step(1)
    studio.records["a"]["steps"][2]["screenshot"] = ""
    studio.select_step(2)
    captures[0]()
    studio.poll()
    assert not studio.ui.has_screenshot
    assert studio.capture_key is None


def test_missing_capture_clears_previous_image_after_failure(studio, captures):
    studio.records["a"]["steps"][1]["screenshot"] = "missing.png"
    studio.select_step(1)
    assert studio.ui.has_screenshot
    captures[0]()
    studio.poll()
    assert not studio.ui.has_screenshot
    assert studio.ui.screenshot_caption.startswith("Cannot load capture:")


def test_action_selection_survives_completion_and_collapse(studio):
    item = example()
    item["steps"] = [
        {
            "action_id": "parent",
            "title": "Helper",
            "status": "Running",
            "duration": 0,
            "screenshot": "",
            "depth": 0,
        },
        {
            "action_id": "child",
            "parent_id": "parent",
            "title": "Fill",
            "status": "Running",
            "duration": 0,
            "screenshot": "",
            "depth": 1,
        },
    ]
    studio.records = {"a": item}
    studio.selected = "a"
    studio.select_step(1)
    item["steps"][1].update(status="Passed", duration=0.2)
    item["steps"].append(
        {
            "action_id": "next",
            "title": "Assert",
            "status": "Running",
            "duration": 0,
            "screenshot": "",
        }
    )
    studio.render_selection()
    assert studio.selected_step == 1
    assert studio.ui.selected_step == 1
    assert studio.ui.output.startswith("Action: Fill")
    studio.toggle_action(0)
    assert [row.row_index for row in studio.ui.steps] == [0, 2]
    assert studio.selected_step == 1
    studio.toggle_action(0)
    assert [row.row_index for row in studio.ui.steps] == [0, 1, 2]


def test_action_without_capture_reuses_previous_image(studio):
    item = example()
    item["steps"] = [
        {
            "title": "Launch",
            "status": "Passed",
            "duration": 0,
            "screenshot": "capture.png",
        },
        {
            "action_id": "fill",
            "title": "Fill",
            "status": "Passed",
            "duration": 0.1,
            "screenshot": "",
        },
    ]
    studio.records = {"a": item}
    studio.selected = "a"
    studio.metadata = {"id": "previous"}
    studio.loaded_capture_key = ("previous", "a", 1, "capture.png")
    studio.select_step(1)
    assert studio.ui.has_screenshot
    assert studio.ui.screenshot_caption == "Fill · last capture: Launch"
    assert studio.ui.output.startswith("Action: Fill")


def test_inspector_picks_overlaps_and_maps_logical_coordinates(studio):
    studio.inspection = {
        "width": 200,
        "height": 100,
        "elements": [
            {
                "index": 0,
                "role": "Region",
                "name": "Outer",
                "locator": "",
                "bounds": {"x": 0, "y": 0, "width": 200, "height": 100},
            },
            {
                "index": 1,
                "role": "Button",
                "name": "Inner",
                "locator": "",
                "bounds": {"x": 80, "y": 40, "width": 40, "height": 20},
            },
        ],
    }
    studio.pick_element(0.5, 0.5)
    assert studio.inspect_selected == 1
    assert studio.ui.highlight_x == 0.4
    assert studio.ui.highlight_width == 0.2
    studio.pick_element(0.5, 0.5)
    assert studio.inspect_selected == 0


def test_historical_pause_cannot_send_control(studio):
    studio.metadata = {"id": "old", "debug": True}
    studio.update_debug({"debug": {"paused": True, "pause_id": 9}}, live=False)
    assert not studio.ui.paused
    assert not studio.ui.debugging
    studio.ui.busy = False
    studio.debug_command("continue")


def test_stale_inspection_image_does_not_change_selection(studio):
    studio.inspect_key = ("new-run", "new.png")
    studio.inspect_selected = 7
    studio.apply_inspection({"key": ("old-run", "old.png"), "inspection": {}})
    assert studio.inspect_selected == 7


def test_stop_disables_control_even_when_old_paused_update_arrives(studio):
    studio.metadata = {"id": "run", "debug": True}
    studio.operation = "run"
    snapshot = {"state": "running", "debug": {"paused": True, "pause_id": 1}}
    studio.update_debug(snapshot, live=True)
    assert studio.ui.paused
    studio.stop()
    studio.update_debug(snapshot, live=True)
    assert not studio.ui.paused
    assert not studio.ui.debugging


def test_saving_discovery_settings_does_not_open_old_history(studio):
    studio.show_settings()
    studio.ui.config_paths = "/external/test_sample.py"
    studio.save_settings()
    assert studio.project["paths"] == ["/external/test_sample.py"]
    studio.ui.busy = False
    studio.service.updates.put(
        (
            "history",
            [
                {
                    "id": "old",
                    "collect": False,
                    "created": 0,
                    "state": "finished",
                    "selectors": ["old_test"],
                }
            ],
        )
    )
    studio.poll()
    assert not studio.pending_restore
    assert not studio.historical
    assert not any(kind == "load" for kind, _ in studio.service.commands)


def test_startup_restores_recent_history(studio):
    studio.activate(studio.project)
    studio.ui.busy = False
    studio.service.updates.put(
        (
            "history",
            [
                {
                    "id": "old",
                    "collect": False,
                    "created": 0,
                    "state": "finished",
                    "selectors": ["old_test"],
                }
            ],
        )
    )
    studio.poll()
    assert ("load", {"id": "old"}) in studio.service.commands
