# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import os
import struct
import time
from pathlib import Path

import pytest

from slint_test import Session, expect, reporting, step
from slint_test.control import debugging, suspended_clock
from slint_test.inspection import capture


class Controller:
    def __init__(self):
        self.events = []

    def before(self, event):
        self.events.append(("before", event["title"]))
        with suspended_clock():
            time.sleep(0.04)

    def failed(self, event, error):
        self.events.append(("failed", str(error)))

    def after(self, event):
        self.events.append(("after", event["title"]))


def test_pauses_suspend_deadlines_and_restore_controller():
    control = Controller()
    session = Session()
    with debugging(control), session.operation(20), step("outer"), step("inner"):
        session.check()
    assert control.events == [
        ("before", "outer"),
        ("before", "inner"),
        ("after", "inner"),
        ("after", "outer"),
    ]
    with step("ordinary"):
        pass
    assert len(control.events) == 4


def test_failure_control_precedes_observer_and_preserves_exception():
    control = Controller()
    error = ValueError("original failure")
    order = []
    with (
        pytest.raises(ValueError) as raised,
        debugging(control),
        reporting(lambda e: order.append(e["kind"])),
        step("broken"),
    ):
        raise error
    assert raised.value is error
    assert control.events == [
        ("before", "broken"),
        ("failed", "original failure"),
        ("after", "broken"),
    ]
    assert order == ["action-start", "action-end"]


def test_native_pause_does_not_expire_transport_deadline(window):
    with debugging(Controller()):
        window.get_by_role("button", name="Apply").activate(timeout=30)
    expect(window.get_by_accessible_name("Count")).to_have_value("1")


def test_inspection_unique_locators_and_duplicate_names(app):
    snapshot, image = capture(app.raw)
    assert image.startswith(b"\x89PNG")
    assert snapshot["width"] == 480
    assert not snapshot["truncated"]
    apply = next(e for e in snapshot["elements"] if e["name"] == "Apply")
    assert "get_by_role" in apply["locator"]
    duplicate = [e for e in snapshot["elements"] if e["name"] == "Duplicate"]
    assert len(duplicate) == 2
    assert all(not e["locator"] for e in duplicate)
    truncated, _ = capture(app.raw, limit=1)
    assert truncated["truncated"]
    assert not truncated["elements"][0]["locator"]


def test_inspector_logical_bounds_at_double_scale():
    from slint_testing import slint_systest_pb2 as proto

    from slint_test import launch

    with launch(
        [
            os.environ["SLINT_FIXTURE_PYTHON"],
            str(Path(__file__).with_name("fixture_app.py")),
        ],
        env=os.environ
        | {
            "SLINT_BACKEND": "headless-skia",
            "SLINT_EMIT_DEBUG_INFO": "1",
        },
    ) as app:
        window = app.raw.first_window
        assert window is not None
        app.raw._send_request(
            proto.RequestToAUT(
                request_dispatch_window_event=proto.RequestDispatchWindowEvent(
                    window_handle=window.handle,
                    event=proto.WindowEvent(
                        scale_factor_changed=proto.ScaleFactorChangedEvent(
                            scale_factor=2
                        )
                    ),
                )
            )
        )
        assert window._get_props().scale_factor == 2
        snapshot, image = capture(app.raw)
        assert (snapshot["width"], snapshot["height"]) == (240, 360)
        assert struct.unpack(">II", image[16:24]) == (480, 720)


def test_optional_source_context_restores_parent(tmp_path):
    from slint_test import inspection_sources

    events = []
    outer, inner = tmp_path / "Outer.slint", tmp_path / "Inner.slint"
    with reporting(events.append), inspection_sources(outer), inspection_sources(inner):
        pass
    assert [e["sources"] for e in events] == [
        [str(outer)],
        [str(inner)],
        [str(outer)],
        [],
    ]
    assert not outer.exists()
