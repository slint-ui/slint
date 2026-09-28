# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from gradient_interactions import gesture
from source_snapshot import SourceSnapshot
from ui_driver import first_window, launch_editor, wait_until


@pytest.mark.parametrize("panel", ["files", "outline"])
def test_tree_indicators_scroll_without_losing_virtualization(
    editor_binary, editor_environment, tmp_path, panel
):
    file = tmp_path / "Main.slint"
    rows = "\n".join(
        f"row-{index} := Rectangle {{ width: 10px; height: 10px; }}"
        for index in range(150 if panel == "outline" else 1)
    )
    file.write_text(
        "export component Main inherits Window { width: 400px; height: 400px;"
        + rows
        + "}"
    )
    if panel == "files":
        for index in range(150):
            (tmp_path / f"File{index:03}.slint").write_text(
                "export component Example inherits Rectangle {}"
            )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        tree = window.get_by_role(
            "tree" if panel == "files" else "list",
            name="Files" if panel == "files" else "Current file outline",
        ).resolve()
        vertical = (
            tree.query_descendants()
            .match_id("EditorScrollIndicators::vertical")
            .match_descendants()
            .match_id("EditorScrollBar::thumb")
            .find_first()
        )
        assert vertical is not None

        def row_labels():
            return [
                row.accessible_label
                for row in tree.query_descendants()
                .match_accessible_role(slint_testing.AccessibleRole.ListItem)
                .find_all()
            ]

        before = wait_until(lambda: row_labels() or None)
        assert 0 < len(before) < 150
        assert vertical.computed_opacity == 0
        tree_size = tree.size
        window.pointer.scroll(0, -300, at=center(tree))
        wait_until(lambda: True if vertical.computed_opacity > 0.99 else None)
        wait_until(lambda: True if row_labels() != before else None)
        assert tree.size == tree_size

        wait_until(lambda: True if 0 < vertical.computed_opacity < 1 else None)
        before_drag = row_labels()
        start = center(vertical)
        end = slint_testing.LogicalPosition(x=start.x, y=start.y + 30)
        gesture(window, start, end)
        wait_until(lambda: True if row_labels() != before_drag else None)
        window.pointer.exit()
        assert 0 < len(row_labels()) < 150
        wait_until(lambda: True if vertical.computed_opacity == 0 else None)
        assert tree.size == tree_size
        original.assert_unchanged()
