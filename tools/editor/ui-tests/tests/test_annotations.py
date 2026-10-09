# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import base64
import os
from io import BytesIO

import pytest
import slint_testing
from canvas_interactions import center, center_canvas_selection, zoom_canvas
from editor_mcp import call_editor_tool
from editor_sync import wait_for_source
from PIL import Image, ImageChops
from source_snapshot import wait_for_source_change
from ui_assertions import expect
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_key,
    query,
    screenshot,
    select_outline_row,
)


def save_annotation(window, label, text):
    select_outline_row(window, label)
    if query(window, "Element annotations").find_all():
        element(window, "Add annotation").invoke_accessible_default_action()
    else:
        element(window, "Add annotation to " + label).invoke_accessible_default_action()
    element(window, "Annotation text").accessible_value = text
    element(window, "Save annotation").invoke_accessible_default_action()


@pytest.mark.skipif(os.name == "nt", reason="Fake CLI uses a POSIX shell")
def test_send_failure_and_retry_keep_annotations_until_resolved(
    editor_binary, editor_environment, fixture_project, tmp_path
):
    source = fixture_project / "Main.slint"
    original = source.read_bytes()
    command = tmp_path / "codex-test"
    command.write_text(
        "#!/bin/sh\nprintf '%s\\n' 'Chat destination unavailable' >&2\nexit 1\n"
    )
    command.chmod(0o755)

    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, original)
        window = first_window(editor)
        for label, text in (
            ("root-rectangle", "Increase the corner radius."),
            ("root-image", "Keep the image aligned with the button."),
        ):
            save_annotation(window, label, text)
        call_editor_tool(
            editor_binary,
            fixture_project,
            "register_visual_editor_chat",
            provider="codex",
            threadId="annotation-test-chat",
            displayName="Refine gallery controls",
            cliPath=str(command),
        )
        expect(element(window, "Annotation destination")).to_have_value(
            "Refine gallery controls · Registered"
        )
        pending = element(window, "Send pending annotations")
        expect(pending).to_have_value("2")
        expect(pending).to_be_enabled()
        send = element(window, "Send annotation 2")
        send.invoke_accessible_default_action()
        failure = element(window, "Annotation send error")
        expect.poll(lambda: failure.accessible_value).to_equal(
            "Chat destination unavailable"
        )
        expect(pending).to_have_value("2")
        expect(send).to_be_enabled()
        expect(element(window, "Annotation status 2")).to_have_value("Saved · not sent")
        screenshot(window).save(tmp_path / "canvas-annotation-send-error.png")
        select_outline_row(window, "root-rectangle")
        expect(query(window, "Annotation send error")).to_be_visible()
        element(
            window, "Dismiss annotation send error"
        ).invoke_accessible_default_action()
        expect(query(window, "Annotation send error")).to_be_hidden()

        command.write_text("#!/bin/sh\nexit 0\n")
        element(window, "Send annotation 1").invoke_accessible_default_action()
        expect(pending).to_have_value("1")
        expect(element(window, "Annotation status 1")).to_have_value("Sent")
        expect(query(window, "Send annotation 1")).to_be_hidden()
        expect(query(window, "Resolve annotation 1")).to_be_visible()
        pending.invoke_accessible_default_action()
        expect(pending).to_have_value("0")
        select_outline_row(window, "root-image")
        expect(element(window, "Annotation status 2")).to_have_value("Sent")
        screenshot(window).save(tmp_path / "canvas-annotation-sent.png")
        element(window, "Resolve annotation 2").invoke_accessible_default_action()
        expect(query(window, "Annotations for root-image")).to_be_hidden()
        expect(query(window, "Annotations for root-rectangle")).to_be_visible()
        assert source.read_bytes() == original

    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, original)
        window = first_window(editor)
        expect(element(window, "Annotation destination")).to_have_value(
            "Refine gallery controls · Registered"
        )
        expect(element(window, "Send pending annotations")).to_have_value("0")
        select_outline_row(window, "root-rectangle")
        expect(element(window, "Annotation status 1")).to_have_value("Sent")
        expect(query(window, "Annotations for root-image")).to_be_hidden()


@pytest.mark.parametrize("deletion", ["editor", "source", "file"])
def test_deletion_removes_pending_annotations(
    editor_binary, editor_environment, fixture_project, deletion
):
    source = fixture_project / "Main.slint"
    original = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, original)
        window = first_window(editor)
        for label, text in (
            ("root-rectangle", "Deleted item"),
            ("root-image", "Surviving item"),
        ):
            save_annotation(window, label, text)
        pending = element(window, "Send pending annotations")
        expect(pending).to_have_value("2")
        if deletion == "file":
            source.unlink()
            expected = "0"
        else:
            if deletion == "editor":
                select_outline_row(window, "root-rectangle")
                press_key(window, slint_testing.keys.Delete)
                wait_for_source_change(source, original)
            else:
                start = original.index(b"    root-rectangle := Rectangle {")
                end = original.index(b"    root-text := Text {")
                updated = original[:start] + original[end:]
                source.write_bytes(updated)
                wait_for_source(source, updated)
            expected = "1"
            expect(query(window, "Annotations for root-rectangle")).to_be_hidden()
            expect(query(window, "Annotations for root-image")).to_be_visible()
        expect(pending).to_have_value(expected)


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
            save_annotation(window, "root-rectangle", text)
        pending = element(window, "Send pending annotations")
        expect(pending).to_have_value("2")
        expect(pending).to_be_enabled(False)
        expect(element(window, "Annotation destination")).to_have_value(
            "No chat registered"
        )
        for identifier in ("1", "2"):
            expect(element(window, "Annotation status " + identifier)).to_have_value(
                "Saved · not sent"
            )
            expect(element(window, "Send annotation " + identifier)).to_be_enabled(
                False
            )
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
        expect(pending).to_have_value("0")
        assert source.read_bytes() == updated


def test_mcp_canvas_screenshot_matches_visible_viewport(
    editor_binary, editor_environment, fixture_project, tmp_path
):
    source = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        save_annotation(window, "root-rectangle", "Keep these rounded corners.")
        zoom_canvas(window, 150)
        center_canvas_selection(window)
        expect(query(window, "Element annotations")).to_be_visible()
        full = screenshot(window)
        response = call_editor_tool(
            editor_binary, fixture_project, "screenshot_visual_editor_canvas"
        )
        content = response["content"]
        assert len(content) == 1
        assert content[0]["type"] == "image"
        assert content[0]["mimeType"] == "image/png"
        canvas = Image.open(BytesIO(base64.b64decode(content[0]["data"]))).convert(
            "RGB"
        )
        viewport = element(window, "Canvas viewport").absolute_rect
        scale = full.width / window.size.width
        bounds = tuple(
            round(value * scale)
            for value in (
                viewport.x,
                viewport.y,
                viewport.x + viewport.width,
                viewport.y + viewport.height,
            )
        )
        expected = full.crop(bounds)
        assert canvas.size == expected.size
        assert ImageChops.difference(canvas, expected).getbbox() is None
        canvas.save(tmp_path / "canvas-mcp-screenshot.png")
        full.save(tmp_path / "canvas-mcp-full-window.png")
