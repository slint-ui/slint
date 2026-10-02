# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from source_snapshot import SourceSnapshot
from ui_driver import (
    element,
    first_window,
    launch_editor,
    select_outline_row,
)


def test_resize_starts_outside_visible_handle(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        frame = element(window, "Selected Rectangle")
        initial_width, initial_height = frame.size.width, frame.size.height
        handle = element(window, "Rectangle resize bottom-right")
        position = center(handle)
        # Five pixels from the corner is outside the visible four-pixel half-width.
        start = slint_testing.LogicalPosition(x=position.x + 5, y=position.y + 5)
        end = slint_testing.LogicalPosition(x=start.x + 20, y=start.y + 16)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        assert frame.size.width == pytest.approx(initial_width + 20)
        assert frame.size.height == pytest.approx(initial_height + 16)
        snapshot.assert_unchanged_now()
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
