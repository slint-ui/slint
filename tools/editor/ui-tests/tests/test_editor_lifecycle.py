# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
import slint_testing
from editor_sync import current_editor_sync
from slint_testing import keys
from test_inspector_transform import (
    SOURCE,
    edit_field,
    point,
    prepare,
    select_element,
    shortcut,
    wait_for_field,
)
from ui_driver import first_window, launch_editor, window_element_with_label


def operation_state(action):
    return action.sync._request(mode="operation", operation=action.operation).data


def test_publication_gate_holds_the_instance_and_supersedes_old_attempt(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project)
    source = fixture_project / SOURCE
    older = baseline.replace(b"32deg", b"45deg")
    newest = baseline.replace(b"32deg", b"67deg")
    with launch_editor(editor_binary, editor_environment, source) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        sync = current_editor_sync.get()
        sync.wait_for_applied(source, baseline)
        checkpoint = sync.checkpoint()
        with sync.gate("publication", source) as gate:
            source.write_bytes(older)
            held = gate.wait_for_reached().data["gate_state"]["attempt"]
            sync.wait_for_processed(source, older, after=checkpoint, outcome="compiled")
            wait_for_field(window, "Rotation", "32")
            source.write_bytes(newest)
            sync.wait_for_applied(source, newest, after=checkpoint)
            wait_for_field(window, "Rotation", "67")
        result = sync.wait_for_processed(
            source, older, after=checkpoint, outcome="superseded"
        )
        assert result.data["attempt"]["id"] == held
        sync.wait_for_applied(source, newest)
        wait_for_field(window, "Rotation", "67")


def test_edit_and_queued_undo_remain_pending_until_publication(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project)
    source = fixture_project / SOURCE
    edited = baseline.replace(b"32deg", b"62deg")
    with launch_editor(editor_binary, editor_environment, source) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        sync = current_editor_sync.get()
        sync.wait_for_applied(source, baseline)
        with sync.gate("publication", source) as gate:
            with sync.action() as edit:
                edit_field(window, "Rotation", "62")
            gate.wait_for_reached()
            assert source.read_bytes() == edited
            state = operation_state(edit)
            assert not state["settled"]
            assert state["operation_state"]["writes"] == 1
            # Document undo requires focus outside the numeric text field.
            knob = window_element_with_label(window, "Rotation knob")
            knob.single_click(slint_testing.PointerEventButton.Left)
            with sync.action() as undo:
                shortcut(window)
            state = operation_state(undo)
            assert not state["settled"]
            assert "queued history" in state["operation_state"]["pending"].values()
            assert state["operation_state"]["writes"] == 0
            assert source.read_bytes() == edited
        edit.wait_for_settled(outcome="completed")
        result = undo.wait_for_settled(outcome="completed")
        assert result.data["operation_state"]["accepted_edits"] == 1
        assert result.data["operation_state"]["writes"] == 1
        sync.wait_for_applied(source, baseline)
        wait_for_field(window, "Rotation", "32")


def test_source_gate_preserves_overlap_without_blocking_ui(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project)
    source = fixture_project / SOURCE
    changed = baseline.replace(b"32deg", b"77deg")
    with launch_editor(editor_binary, editor_environment, source) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        sync = current_editor_sync.get()
        sync.wait_for_applied(source, baseline)
        with sync.gate("source", source) as gate:
            source.write_bytes(changed)
            gate.wait_for_reached()
            wait_for_field(window, "Rotation", "32")
            with sync.action() as noop:
                pass
            noop.wait_for_settled(outcome="noop")
            noop.assert_no_source_writes()
            with pytest.raises(AssertionError, match="expected.*canceled.*noop"):
                noop.wait_for_settled(outcome="canceled")
        sync.wait_for_applied(source, changed)
        wait_for_field(window, "Rotation", "77")


@pytest.mark.parametrize("cancel", ["escape", "selection", "source"])
def test_active_gesture_has_terminal_cancellation_and_release_cannot_write(
    editor_binary, editor_environment, fixture_project, cancel
):
    baseline = prepare(fixture_project)
    source = fixture_project / SOURCE
    with launch_editor(editor_binary, editor_environment, source) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        sync = current_editor_sync.get()
        sync.wait_for_applied(source, baseline)
        knob = window_element_with_label(window, "Rotation knob")
        start, end = point(knob, 32), point(knob, 62)
        with sync.action() as gesture:
            window.dispatch_event(
                slint_testing.PointerPressEvent(
                    start, slint_testing.PointerEventButton.Left
                )
            )
            window.dispatch_event(slint_testing.PointerMoveEvent(end))
        assert not operation_state(gesture)["settled"]
        assert source.read_bytes() == baseline
        if cancel == "source":
            baseline = baseline.replace(b"32deg", b"17deg")
            source.write_bytes(baseline)
            sync.wait_for_applied(source, baseline)
        with sync.action() as release:
            if cancel == "escape":
                window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Escape))
                window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Escape))
            elif cancel == "selection":
                select_element(window, "Text")
            window.dispatch_event(
                slint_testing.PointerReleaseEvent(
                    end, slint_testing.PointerEventButton.Left
                )
            )
        gesture.wait_for_settled(outcome="canceled")
        gesture.assert_no_source_writes()
        release.assert_no_source_writes()
        assert source.read_bytes() == baseline


def test_pointer_down_without_movement_is_canceled_by_escape(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project)
    source = fixture_project / SOURCE
    with launch_editor(editor_binary, editor_environment, source) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        sync = current_editor_sync.get()
        sync.wait_for_applied(source, baseline)
        knob = window_element_with_label(window, "Rotation knob")
        position = point(knob, 32)
        with sync.action() as gesture:
            window.dispatch_event(
                slint_testing.PointerPressEvent(
                    position, slint_testing.PointerEventButton.Left
                )
            )
            state = operation_state(gesture)
            assert not state["settled"]
            window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Escape))
            window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Escape))
            window.dispatch_event(
                slint_testing.PointerReleaseEvent(
                    position, slint_testing.PointerEventButton.Left
                )
            )
        gesture.wait_for_settled(outcome="canceled")
        gesture.assert_no_source_writes()
        assert source.read_bytes() == baseline


def test_write_then_undo_counts_both_writes_even_when_final_bytes_match(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project)
    source = fixture_project / SOURCE
    with launch_editor(editor_binary, editor_environment, source) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        sync = current_editor_sync.get()
        sync.wait_for_applied(source, baseline)
        with sync.action() as edit_and_undo:
            edit_field(window, "Rotation", "42")
            sync.wait_for_applied(source, baseline.replace(b"32deg", b"42deg"))
            window_element_with_label(window, "Rotation knob").single_click(
                slint_testing.PointerEventButton.Left
            )
            shortcut(window)
        result = edit_and_undo.wait_for_settled(outcome="completed")
        assert source.read_bytes() == baseline
        assert result.data["operation_state"]["writes"] == 2
        assert result.data["operation_state"]["accepted_edits"] == 2
        with pytest.raises(AssertionError, match="wrote source"):
            edit_and_undo.assert_no_source_writes()


def test_same_content_and_return_to_original_have_new_observations(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project)
    source = fixture_project / SOURCE
    changed = baseline.replace(b"32deg", b"42deg")
    with launch_editor(editor_binary, editor_environment, source) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        sync = current_editor_sync.get()
        sync.wait_for_applied(source, baseline)
        checkpoint = sync.checkpoint()
        with sync.gate("source", source) as gate:
            source.write_bytes(baseline)
            gate.wait_for_reached()
        same = sync.wait_for_observed(source, baseline, after=checkpoint)
        assert same.data["observation"]["cursor"] > checkpoint.cursor
        checkpoint = sync.checkpoint()
        source.write_bytes(changed)
        sync.wait_for_applied(source, changed, after=checkpoint)
        checkpoint = sync.checkpoint()
        source.write_bytes(baseline)
        sync.wait_for_processed(source, baseline, after=checkpoint, outcome="compiled")
        sync.wait_for_applied(source, baseline, after=checkpoint)
        wait_for_field(window, "Rotation", "32")


def test_unrelated_installation_cannot_release_pending_edit_history(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project)
    source = fixture_project / SOURCE
    older = baseline.replace(b"32deg", b"45deg")
    edited = baseline.replace(b"32deg", b"62deg")
    with launch_editor(editor_binary, editor_environment, source) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        sync = current_editor_sync.get()
        sync.wait_for_applied(source, baseline)
        with sync.gate("publication", source) as publication:
            source.write_bytes(older)
            publication.wait_for_reached()
            with sync.gate("source", source) as processing:
                with sync.action() as edit:
                    edit_field(window, "Rotation", "62")
                processing.wait_for_reached()
                assert source.read_bytes() == edited
                publication.release()
                wait_for_field(window, "Rotation", "45")
                window_element_with_label(window, "Rotation knob").single_click(
                    slint_testing.PointerEventButton.Left
                )
                with sync.action() as undo:
                    shortcut(window)
                state = operation_state(undo)
                assert "queued history" in state["operation_state"]["pending"].values()
                assert state["operation_state"]["writes"] == 0
        edit.wait_for_settled(outcome="completed")
        undo.wait_for_settled(outcome="completed")
        sync.wait_for_applied(source, baseline)
        assert source.read_bytes() == baseline
