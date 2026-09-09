# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
import sys
from contextlib import nullcontext
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import (
    OrientedFrame,
    manual_drag,
    manual_radius_drag,
    manual_rotation_drag,
    oriented_selection_frame,
    radius_handle,
    rotated_handle_center,
    rotation_delta,
)
from editor_sync import current_editor_sync
from inspector_interactions import FIELDS, edit_field, wait_for_field
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    first_window,
    launch_editor,
    press_shortcut,
    select_fixture_element,
    wait_until,
    window_element_with_label,
)
from ui_reporting import replay_stage

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
    actual: OrientedFrame | None = None

    def matches() -> bool | None:
        nonlocal actual
        actual = oriented_selection_frame(window, "Rectangle")
        return (
            True
            if actual is not None
            and all(abs(a - b) < 1.5 for a, b in zip(actual, expected))
            else None
        )

    try:
        wait_until(matches)
    except AssertionError as error:
        raise AssertionError(
            f"Expected frame {expected}, last observed {actual}"
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
        a = rotated_handle_center(
            window_element_with_label(window, "Rectangle resize top-left"),
            values["rotation"],
        )
        r = rotated_handle_center(
            window_element_with_label(window, "Rectangle radius top-left"),
            values["rotation"],
        )
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
    modifiers = [keys.Control] + (
        [keys.Shift] if redo and sys.platform != "win32" else []
    )
    key = "y" if redo and sys.platform == "win32" else "z"
    press_shortcut(window, *modifiers, key)


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
    target_angle = changes["rotation"]
    dx, dy = rotation_delta(window, handle, target_angle, kind="Rectangle")
    manual_rotation_drag(
        window, handle, dx, dy, snapshot, kind="Rectangle", target_angle=target_angle
    )


SKIPS = {
    "handle-radius": "Requires a Rust corner-radius persistence fix",
    "inspector-rotation": "No Rectangle rotation property editor is available",
    "inspector-radius": "No Rectangle corner-radius property editor is available",
}


@pytest.mark.parametrize(
    "case,changes",
    [
        pytest.param(
            case,
            changes,
            id=case,
            marks=pytest.mark.skip(reason=SKIPS[case]) if case in SKIPS else (),
        )
        for case, changes in CASES
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
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        with replay_stage("initial"):
            select_fixture_element(window, "Rectangle")
            cx, cy, *_ = wait_until(
                lambda: oriented_selection_frame(window, "Rectangle")
            )
            origin = (
                cx - INITIAL["x"] - INITIAL["width"] / 2,
                cy - INITIAL["y"] - INITIAL["height"] / 2,
            )
            assert_visual(window, INITIAL, origin, case.endswith("radius"))
            snapshot.assert_unchanged_now()
        with replay_stage("initial edit"):
            edit(window, case, changes, snapshot)
            snapshot.wait_for_applied(expected, SOURCE)
            assert_visual(window, INITIAL | changes, origin, case.endswith("radius"))
        for name, content, values in [
            ("undo", baseline, INITIAL),
            ("redo", expected, INITIAL | changes),
        ]:
            with replay_stage(name):
                if case.startswith("inspector-"):
                    select_fixture_element(window, "Rectangle")
                shortcut(window, redo=name == "redo")
                snapshot.wait_for_applied(content, SOURCE)
                assert_visual(window, values, origin, case.endswith("radius"))


@pytest.mark.parametrize("wait_for_reload", [False, True])
def test_redo_after_external_edit_preserves_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    wait_for_reload: bool,
) -> None:
    source = fixture_project / SOURCE
    baseline = source.read_bytes()
    edited = baseline.replace(b"x: 80px;", b"x: 104px;", 1)
    external = baseline.replace(b"x: 80px;", b"x: 900px;", 1)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        cx, cy, *_ = wait_until(lambda: oriented_selection_frame(window, "Rectangle"))
        origin = (
            cx - INITIAL["x"] - INITIAL["width"] / 2,
            cy - INITIAL["y"] - INITIAL["height"] / 2,
        )
        edit_field(window, FIELDS["x"], "104", slint_testing.AccessibleRole.TextInput)
        snapshot.wait_for_applied(edited, SOURCE)
        assert_visual(window, INITIAL | {"x": 104}, origin, False)
        select_fixture_element(window, "Rectangle")
        shortcut(window, redo=False)
        snapshot.wait_for_applied(baseline, SOURCE)
        wait_for_field(
            window, FIELDS["x"], "80", slint_testing.AccessibleRole.TextInput
        )
        select_fixture_element(window, "Rectangle")
        sync = current_editor_sync.get()
        scope = nullcontext() if wait_for_reload else sync.gate("source", source)
        with scope as gate:
            source.write_bytes(external)
            snapshot_after_external = SourceSnapshot.capture(fixture_project)
            if gate is not None:
                gate.wait_for_reached()
            else:
                sync.wait_for_applied(source, external)
                wait_for_field(
                    window, FIELDS["x"], "900", slint_testing.AccessibleRole.TextInput
                )
            with sync.action() as redo:
                shortcut(window, redo=True)
            redo.wait_for_settled(outcome="noop" if wait_for_reload else "rejected")
            redo.assert_no_source_writes()
        snapshot.wait_for_applied(external, SOURCE)
        snapshot_after_external.assert_unchanged_now()
