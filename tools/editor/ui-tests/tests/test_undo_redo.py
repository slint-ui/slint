# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import logging
import math
import os
import sys
import time
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center, manual_drag, manual_radius_drag
from slint_testing import keys
from source_snapshot import SourceSnapshot
from test_canvas import radius_handle
from test_inspector import edit_field, wait_for_field
from ui_driver import (
    first_window,
    launch_editor,
    select_fixture_element,
    wait_until,
    window_element_with_label,
)

SOURCE = "UndoRedo.slint"
INITIAL = {"x": 80, "y": 80, "width": 180, "height": 120, "rotation": 0, "radius": 12}
CASES = [
    ("handle-move", {"x": 104, "y": 96}),
    ("handle-resize", {"width": 204, "height": 136}),
    ("handle-rotation", {"rotation": 15}),
    ("handle-radius", {"radius": 20}),
    ("inspector-x", {"x": 104}),
    ("inspector-y", {"y": 96}),
    ("inspector-width", {"width": 204}),
    ("inspector-height", {"height": 136}),
    ("inspector-rotation", {"rotation": 15}),
    ("inspector-radius", {"radius": 20}),
]
FIELDS = {
    "x": "Position X",
    "y": "Position Y",
    "width": "Width",
    "height": "Height",
    "rotation": "Rotation",
    "radius": "Corner radius",
}


def pause(case: str, stage: str) -> None:
    print(f"{case}: {stage}", flush=True)
    if os.environ.get("SLINT_UNDO_REDO_REPLAY") == "1":
        time.sleep(1)


def point(
    window: slint_testing.Window, label: str, angle: float = 0
) -> tuple[float, float]:
    element = window_element_with_label(window, label)
    p, size = element.absolute_position, element.size
    radians = math.radians(angle)
    return (
        p.x + size.width / 2 * math.cos(radians) - size.height / 2 * math.sin(radians),
        p.y + size.width / 2 * math.sin(radians) + size.height / 2 * math.cos(radians),
    )


def frame(window: slint_testing.Window) -> tuple[float, ...]:
    a = point(window, "Rectangle resize top-left")
    b = point(window, "Rectangle resize top-right")
    c = point(window, "Rectangle resize bottom-right")
    angle = math.degrees(math.atan2(b[1] - a[1], b[0] - a[0]))
    corrected_a = point(window, "Rectangle resize top-left", angle)
    corrected_c = point(window, "Rectangle resize bottom-right", angle)
    return (
        (corrected_a[0] + corrected_c[0]) / 2,
        (corrected_a[1] + corrected_c[1]) / 2,
        math.dist(a, b),
        math.dist(b, c),
        angle,
    )


def assert_visual(
    window: slint_testing.Window,
    values: dict[str, int],
    origin: tuple[float, float],
    check_radius: bool,
) -> None:
    expected = (
        origin[0] + values["x"] + values["width"] / 2,
        origin[1] + values["y"] + values["height"] / 2,
        values["width"],
        values["height"],
        values["rotation"],
    )
    try:
        wait_until(
            lambda: (
                True
                if all(abs(a - b) < 1.5 for a, b in zip(frame(window), expected))
                else None
            )
        )
    except AssertionError as error:
        raise AssertionError(
            f"Expected frame {expected}, got {frame(window)}"
        ) from error
    for name in ("x", "y", "width", "height"):
        wait_for_field(
            window,
            FIELDS[name],
            str(values[name]),
            slint_testing.AccessibleRole.TextInput,
        )
    if not check_radius:
        return
    radius_handle(window, "top-left")

    def radius_matches() -> bool | None:
        a = point(window, "Rectangle resize top-left", values["rotation"])
        r = point(window, "Rectangle radius top-left", values["rotation"])
        angle = math.radians(values["rotation"])
        dx, dy = r[0] - a[0], r[1] - a[1]
        local = (
            dx * math.cos(angle) + dy * math.sin(angle),
            -dx * math.sin(angle) + dy * math.cos(angle),
        )
        return (
            True if all(abs(v - (10 + values["radius"])) < 1.5 for v in local) else None
        )

    wait_until(radius_matches)


def shortcut(window: slint_testing.Window, redo: bool) -> None:
    modifier = keys.Control
    modifiers = [modifier] + ([keys.Shift] if redo and sys.platform != "win32" else [])
    for key in modifiers:
        window.dispatch_event(slint_testing.KeyPressedEvent(text=key))
    key = "y" if redo and sys.platform == "win32" else "z"
    try:
        window.dispatch_event(slint_testing.KeyPressedEvent(text=key))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=key))
    finally:
        for key in reversed(modifiers):
            window.dispatch_event(slint_testing.KeyReleasedEvent(text=key))


def edit(
    window: slint_testing.Window,
    case: str,
    changes: dict[str, int],
    snapshot: SourceSnapshot,
) -> None:
    if case.startswith("inspector-"):
        name, value = next(iter(changes.items()))
        edit_field(
            window, FIELDS[name], str(value), slint_testing.AccessibleRole.TextInput
        )
        return
    if case == "handle-radius":
        manual_radius_drag(window, radius_handle(window, "top-left"), 8, 8, snapshot)
        return
    if case != "handle-rotation":
        label = (
            "Rectangle move handle"
            if case == "handle-move"
            else "Rectangle resize bottom-right"
        )
        manual_drag(window, window_element_with_label(window, label), 24, 16, snapshot)
        return
    handle = window_element_with_label(window, "Rectangle rotate top-left")
    start = center(handle)
    cx, cy, *_ = frame(window)
    angle = math.radians(15)
    dx, dy = start.x - cx, start.y - cy
    end = slint_testing.LogicalPosition(
        x=cx + dx * math.cos(angle) - dy * math.sin(angle),
        y=cy + dx * math.sin(angle) + dy * math.cos(angle),
    )
    button = slint_testing.PointerEventButton.Left
    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
    window.dispatch_event(slint_testing.PointerMoveEvent(end))
    wait_until(
        lambda: (
            True
            if window_element_with_label(
                window, "Rotation angle", slint_testing.AccessibleRole.Text
            ).accessible_value
            == "15"
            else None
        )
    )
    snapshot.assert_unchanged_now()
    window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
    window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))


@pytest.mark.parametrize(
    "case,changes",
    [
        pytest.param(
            case,
            changes,
            id=case,
            marks=pytest.mark.skip(reason=reason) if reason else (),
        )
        for case, changes in CASES
        for reason in [
            {
                "handle-radius": "Requires a Rust corner-radius persistence fix",
                "inspector-rotation": "No Rectangle rotation property editor is available",
                "inspector-radius": "No Rectangle corner-radius property editor is available",
            }.get(case)
        ]
    ],
)
def test_rectangle_undo_redo(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    case: str,
    changes: dict[str, int],
) -> None:
    source = fixture_project / SOURCE
    baseline = source.read_bytes()
    expected = baseline
    for name, value in changes.items():
        prop, unit = {
            "rotation": ("transform-rotation", "deg"),
            "radius": ("border-radius", "px"),
        }.get(name, (name, "px"))
        old = f"        {prop}: {INITIAL[name]}{unit};".encode()
        assert expected.count(old) == 1
        expected = expected.replace(old, f"        {prop}: {value}{unit};".encode())
    snapshot = SourceSnapshot.capture(fixture_project)
    stage = "initial"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        try:
            select_fixture_element(window, "Rectangle")
            cx, cy, *_ = frame(window)
            origin = (
                cx - INITIAL["x"] - INITIAL["width"] / 2,
                cy - INITIAL["y"] - INITIAL["height"] / 2,
            )
            assert_visual(window, INITIAL, origin, case.endswith("radius"))
            snapshot.assert_unchanged_now()
            pause(case, stage)
            stage = "initial edit"
            edit(window, case, changes, snapshot)
            for stage, content, values in [
                ("edited", expected, INITIAL | changes),
                ("undo", baseline, INITIAL),
                ("redo", expected, INITIAL | changes),
            ]:
                if stage != "edited":
                    if case.startswith("inspector-"):
                        select_fixture_element(window, "Rectangle")
                    shortcut(window, redo=stage == "redo")
                snapshot.wait_for_exact(content, SOURCE)
                assert_visual(window, values, origin, case.endswith("radius"))
                pause(case, stage)
        except Exception as error:
            artifact = fixture_project.parent / f"{case}-{stage.replace(' ', '-')}.png"
            try:
                artifact.write_bytes(window.grab_window_as_png())
                print(f"Failure screenshot: {artifact}", flush=True)
            except Exception:
                logging.getLogger(__name__).exception("Screenshot unavailable")
            raise AssertionError(f"{case} failed at {stage}: {error}") from error
