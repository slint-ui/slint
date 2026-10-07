# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from ui_assertions import expect
from ui_driver import (
    element,
    first_window,
    launch_editor,
    query,
    screenshot,
    select_outline_row,
)


def test_canvas_annotations_survive_deselection_and_resolve(
    editor_binary, editor_environment, fixture_project, tmp_path
):
    source = fixture_project / "Main.slint"
    original = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, original)
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        pin = element(window, "Add annotation to root-rectangle")
        position = center(pin)
        for event in (
            slint_testing.PointerPressEvent(
                position, slint_testing.PointerEventButton.Left
            ),
            slint_testing.PointerReleaseEvent(
                position, slint_testing.PointerEventButton.Left
            ),
        ):
            window.dispatch_event(event)
        expect(query(window, "Element annotations")).to_be_visible()
        element(window, "Cancel").invoke_accessible_default_action()
        expect(query(window, "Element annotations")).to_be_hidden()

        for text in (
            "Increase the corner radius.",
            "Keep the field aligned with the button.",
        ):
            if query(window, "Element annotations").find_all():
                element(window, "Add annotation").invoke_accessible_default_action()
            else:
                element(
                    window, "Add annotation to root-rectangle"
                ).invoke_accessible_default_action()
            element(window, "Annotation text").accessible_value = text
            element(window, "Add annotation").invoke_accessible_default_action()
        pin = element(window, "Annotations for root-rectangle")
        expect(pin).to_have_value("2")
        expect(query(window, "Add annotation to root-rectangle")).to_be_hidden()
        screenshot(window).save(tmp_path / "canvas-annotations.png")

        select_outline_row(window, "root-image")
        expect(query(window, "Element annotations")).to_be_hidden()
        expect(query(window, "Annotations for root-rectangle")).to_be_visible()
        element(
            window, "Annotations for root-rectangle"
        ).invoke_accessible_default_action()
        expect(query(window, "Element annotations")).to_be_visible()
        element(window, "Collapse annotations").invoke_accessible_default_action()
        expect(query(window, "Element annotations")).to_be_hidden()

        before = element(window, "Annotations for root-rectangle").absolute_position.x
        updated = original.replace(b"x: 40px;", b"x: 70px;", 1)
        source.write_bytes(updated)
        wait_for_source(source, updated)
        expect.poll(
            lambda: (
                element(window, "Annotations for root-rectangle").absolute_position.x
            )
        ).to_equal(pytest.approx(before + 30))

        element(
            window, "Annotations for root-rectangle"
        ).invoke_accessible_default_action()
        for identifier in ("1", "2"):
            element(
                window, "Resolve annotation " + identifier
            ).invoke_accessible_default_action()
        expect(query(window, "Annotations for root-rectangle")).to_be_hidden()
        expect(query(window, "Add annotation to root-rectangle")).to_be_visible()
        assert source.read_bytes() == updated
