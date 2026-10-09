# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import base64
import json
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
    pin = query(window, "Annotations for " + label)
    if pin.find_all():
        element(window, "Annotations for " + label).invoke_accessible_default_action()
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
        expect(query(window, "Resolve thread 1")).to_be_visible()
        pending.invoke_accessible_default_action()
        expect(pending).to_have_value("0")
        select_outline_row(window, "root-image")
        expect(element(window, "Annotation status 2")).to_have_value("Sent")
        screenshot(window).save(tmp_path / "canvas-annotation-sent.png")
        element(window, "Resolve thread 2").invoke_accessible_default_action()
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
                window, "Resolve thread " + identifier
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


@pytest.mark.skipif(os.name == "nt", reason="Fake CLI uses a POSIX shell")
def test_annotation_thread_agent_and_user_replies_save_send_and_persist(
    editor_binary, editor_environment, fixture_project, tmp_path
):
    source = fixture_project / "Main.slint"
    original = source.read_bytes()
    command = tmp_path / "codex-test"
    payload = tmp_path / "delivery-arguments"
    command.write_text("#!/bin/sh\nprintf 'Queue unavailable' >&2\nexit 1\n")
    command.chmod(0o755)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, original)
        window = first_window(editor)
        save_annotation(window, "root-rectangle", "Make the corners rounder.")
        call_editor_tool(
            editor_binary,
            fixture_project,
            "register_visual_editor_chat",
            provider="codex",
            threadId="annotation-test-chat",
            displayName="Refine corners",
            cliPath=str(command),
        )
        response = call_editor_tool(
            editor_binary,
            fixture_project,
            "reply_visual_editor_annotation",
            annotationId="1",
            provider="codex",
            text="Adjusted the radius. Does this size work?",
        )
        agent_id = response["structuredContent"]["messageId"]
        expect(element(window, "Reply message " + agent_id)).to_have_value(
            "Adjusted the radius. Does this size work?"
        )
        pending = element(window, "Send pending annotations")
        expect(pending).to_have_value("1")
        reply = element(window, "Reply to thread 1")
        assert (
            reply.absolute_position.y
            > element(window, "Reply message " + agent_id).absolute_position.y
        )
        reply.invoke_accessible_default_action()
        element(window, "Reply text").accessible_value = "A little smaller, please."
        save = element(window, "Save reply")
        assert abs(save.absolute_position.x - reply.absolute_position.x) < 1
        save.invoke_accessible_default_action()
        expect(pending).to_have_value("2")
        user_reply = element(window, "Reply message 3")
        expect(user_reply).to_have_value("A little smaller, please.")
        send = element(window, "Send reply 3")
        assert abs(send.absolute_position.x - reply.absolute_position.x) < 1
        assert reply.absolute_position.y > send.absolute_position.y
        send.invoke_accessible_default_action()
        expect(element(window, "Annotation send error")).to_have_value(
            "Queue unavailable"
        )
        expect(pending).to_have_value("2")
        expect(element(window, "Reply status 3")).to_have_value("Saved · not sent")
        command.write_text(f"#!/bin/sh\nprintf '%s\\n' \"$@\" > '{payload}'\n")
        send.invoke_accessible_default_action()
        expect(pending).to_have_value("1")
        expect(element(window, "Reply status 3")).to_have_value("Sent")
        expect(query(window, "Send reply 3")).to_be_hidden()
        arguments = payload.read_text().split("--message\n", 1)[1]
        delivered = json.loads(arguments.split(":\n", 1)[1])
        assert delivered[0]["id"] == "1"
        assert delivered[0]["pendingMessageIds"] == ["3"]
        assert [message["author"] for message in delivered[0]["conversation"]] == [
            "user",
            "codex",
            "user",
        ]
        screenshot(window).save(tmp_path / "canvas-annotation-thread.png")
        for note in (
            "The smaller radius is applied. I kept the outline and preserved the image alignment. Does the spacing also need adjustment?",
            "The screenshot shows the current canvas. The remaining question is whether you want the button spacing changed too.",
        ):
            call_editor_tool(
                editor_binary,
                fixture_project,
                "reply_visual_editor_annotation",
                annotationId="1",
                provider="codex",
                text=note,
            )
        expect(element(window, "Reply message 5")).to_have_value(note)
        reply.invoke_accessible_default_action()
        element(window, "Reply text").accessible_value = "Keep the spacing as it is."
        element(window, "Save reply").invoke_accessible_default_action()
        expect(pending).to_have_value("2")
        latest_send = element(window, "Send reply 6")
        viewport = element(window, "Canvas viewport").absolute_rect
        for action in (latest_send, reply):
            assert action.absolute_position.y >= viewport.y
            assert (
                action.absolute_position.y + action.size.height
                <= viewport.y + viewport.height
            )
        assert abs(latest_send.absolute_position.x - reply.absolute_position.x) < 1
        screenshot(window).save(tmp_path / "canvas-annotation-thread-scroll.png")
        latest_send.invoke_accessible_default_action()
        expect(pending).to_have_value("1")
        assert source.read_bytes() == original

    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, original)
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        expect(element(window, "Reply message 5")).to_have_value(note)
        expect(element(window, "Reply status 6")).to_have_value("Sent")
        pending = element(window, "Send pending annotations")
        expect(pending).to_have_value("1")
        pending.invoke_accessible_default_action()
        expect(pending).to_have_value("0")
        window.dispatch_event(
            slint_testing.PointerScrolledEvent(
                center(element(window, "Element annotations")), delta_x=0, delta_y=1000
            )
        )
        expect(query(window, "Resolve thread 1")).to_be_visible()
        element(window, "Resolve thread 1").invoke_accessible_default_action()
        expect(query(window, "Reply message 3")).to_be_hidden()
        expect(query(window, "Annotations for root-rectangle")).to_be_hidden()


def test_mcp_resolves_whole_thread_and_preserves_other_annotations(
    editor_binary, editor_environment, fixture_project
):
    source = fixture_project / "Main.slint"
    original = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, original)
        window = first_window(editor)
        save_annotation(window, "root-rectangle", "Round the corners.")
        save_annotation(window, "root-image", "Keep the image aligned.")
        select_outline_row(window, "root-rectangle")
        call_editor_tool(
            editor_binary,
            fixture_project,
            "reply_visual_editor_annotation",
            annotationId="1",
            provider="codex",
            text="The corners are rounded.",
        )
        element(window, "Reply to thread 1").invoke_accessible_default_action()
        element(window, "Reply text").accessible_value = "That works."
        element(window, "Save reply").invoke_accessible_default_action()
        pending = element(window, "Send pending annotations")
        expect(pending).to_have_value("3")
        call_editor_tool(
            editor_binary,
            fixture_project,
            "resolve_visual_editor_annotation",
            annotationId="1",
        )
        expect(query(window, "Annotations for root-rectangle")).to_be_hidden()
        expect(query(window, "Reply message 4")).to_be_hidden()
        expect(query(window, "Annotations for root-image")).to_be_visible()
        expect(pending).to_have_value("1")
        assert source.read_bytes() == original

    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, original)
        window = first_window(editor)
        expect(query(window, "Annotations for root-rectangle")).to_be_hidden()
        select_outline_row(window, "root-image")
        expect(element(window, "Annotation message 2")).to_have_value(
            "Keep the image aligned."
        )
        expect(element(window, "Send pending annotations")).to_have_value("1")
