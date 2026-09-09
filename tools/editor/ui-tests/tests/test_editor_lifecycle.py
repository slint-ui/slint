# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from contextlib import contextmanager

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
from ui_driver import file_row, first_window, launch_editor, window_element_with_label


@pytest.fixture
def rectangle_editor(editor_binary, editor_environment, fixture_project):
    return open_rectangle(editor_binary, editor_environment, fixture_project)


@contextmanager
def open_rectangle(editor_binary, editor_environment, fixture_project):
    baseline = prepare(fixture_project)
    source = fixture_project / SOURCE
    with launch_editor(editor_binary, editor_environment, source) as app:
        window = first_window(app)
        sync = current_editor_sync.get()
        sync.wait_for_applied(source, baseline)
        select_element(window, "Rectangle")
        yield window, sync, source, baseline


def operation_state(action):
    return action.sync._request(mode="operation", operation=action.operation).data


@pytest.mark.parametrize("stage", ["publication", "factory"])
def test_publication_gate_holds_the_instance_and_supersedes_old_attempt(
    rectangle_editor, stage
):
    with rectangle_editor as (window, sync, source, baseline):
        older = baseline.replace(b"32deg", b"45deg")
        newest = baseline.replace(b"32deg", b"67deg")
        checkpoint = sync.checkpoint()
        with sync.gate(stage, source) as gate:
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


def test_edit_and_queued_undo_remain_pending_until_publication(rectangle_editor):
    with rectangle_editor as (window, sync, source, baseline):
        edited = baseline.replace(b"32deg", b"62deg")
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


def test_source_gate_preserves_overlap_without_blocking_ui(rectangle_editor):
    with rectangle_editor as (window, sync, source, baseline):
        changed = baseline.replace(b"32deg", b"77deg")
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


@pytest.mark.parametrize("move", [False, True])
@pytest.mark.parametrize("cancel", ["escape", "selection", "source"])
def test_sealed_gesture_stays_pending_until_cancellation(
    rectangle_editor, cancel, move
):
    with rectangle_editor as (window, sync, source, baseline):
        knob = window_element_with_label(window, "Rotation knob")
        position = point(knob, 32)
        with sync.action() as gesture:
            window.dispatch_event(
                slint_testing.PointerPressEvent(
                    position, slint_testing.PointerEventButton.Left
                )
            )
            if move:
                position = point(knob, 62)
                window.dispatch_event(slint_testing.PointerMoveEvent(position))
        assert source.read_bytes() == baseline
        state = operation_state(gesture)
        assert state["operation_state"]["sealed"]
        assert not state["settled"]
        assert "inspector gesture" in state["operation_state"]["pending"].values()
        with sync.action() as cancellation:
            if cancel == "escape":
                window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Escape))
                window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Escape))
            elif cancel == "selection":
                select_element(window, "Text")
            else:
                baseline = baseline.replace(b"32deg", b"17deg")
                source.write_bytes(baseline)
                sync.wait_for_applied(source, baseline)
        gesture.wait_for_settled(outcome="canceled")
        gesture.assert_no_source_writes()
        cancellation.assert_no_source_writes()
        with sync.action() as release:
            window.dispatch_event(
                slint_testing.PointerReleaseEvent(
                    position, slint_testing.PointerEventButton.Left
                )
            )
        release.assert_no_source_writes()
        assert source.read_bytes() == baseline


def test_write_then_undo_counts_both_writes_even_when_final_bytes_match(
    rectangle_editor,
):
    with rectangle_editor as (window, sync, source, baseline):
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


def test_same_content_and_return_to_original_have_new_observations(rectangle_editor):
    with rectangle_editor as (window, sync, source, baseline):
        changed = baseline.replace(b"32deg", b"42deg")
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


def test_unrelated_installation_cannot_release_pending_edit_history(rectangle_editor):
    with rectangle_editor as (window, sync, source, baseline):
        older = baseline.replace(b"32deg", b"45deg")
        edited = baseline.replace(b"32deg", b"62deg")
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


def wait_for_acknowledgment(sync, edit_id, checkpoint):
    return sync._request(
        mode="acknowledgment", edit=edit_id, after=checkpoint.cursor
    ).data["acknowledgment"]


@pytest.mark.parametrize("first", ["acknowledgment", "publication"])
def test_edit_waits_for_both_matching_milestones(rectangle_editor, first):
    with rectangle_editor as (window, sync, source, baseline):
        seeded = baseline.replace(b"32deg", b"42deg")
        with sync.action() as seed:
            edit_field(window, "Rotation", "42")
        seed.wait_for_settled(outcome="completed")
        sync.wait_for_applied(source, seeded)
        checkpoint = sync.checkpoint()
        with (
            sync.gate("acknowledgment", source) as acknowledgment,
            sync.gate("publication", source) as publication,
        ):
            with sync.action() as edit:
                edit_field(window, "Rotation", "62")
            edit_id = acknowledgment.wait_for_reached().data["gate_state"]["edit"]
            publication.wait_for_reached()
            window_element_with_label(window, "Rotation knob").single_click(
                slint_testing.PointerEventButton.Left
            )
            with sync.action() as undo:
                shortcut(window)
            assert (
                "queued history"
                in operation_state(undo)["operation_state"]["pending"].values()
            )
            if first == "acknowledgment":
                acknowledgment.release()
                assert wait_for_acknowledgment(sync, edit_id, checkpoint)["accepted"]
                remaining = "publication gate"
            else:
                publication.release()
                wait_for_field(window, "Rotation", "62")
                remaining = "acknowledgment gate"
            assert (
                remaining
                in operation_state(edit)["operation_state"]["pending"].values()
            )
            assert (
                "queued history"
                in operation_state(undo)["operation_state"]["pending"].values()
            )
            assert operation_state(undo)["operation_state"]["writes"] == 0
        edit.wait_for_settled(outcome="completed")
        result = undo.wait_for_settled(outcome="completed")
        assert result.data["operation_state"]["writes"] == 1
        sync.wait_for_applied(source, seeded)
        assert source.read_bytes() == seeded


def test_obsolete_acknowledgment_cannot_finish_newer_edit(rectangle_editor):
    with rectangle_editor as (window, sync, source, baseline):
        with sync.gate("acknowledgment", source) as acknowledgment:
            with sync.action() as obsolete:
                edit_field(window, "Rotation", "62")
            obsolete_id = acknowledgment.wait_for_reached().data["gate_state"]["edit"]
            wait_for_field(window, "Rotation", "62")
            checkpoint = sync.checkpoint()
            broken = b"export component Broken inherits Window { invalid syntax }"
            source.write_bytes(broken)
            sync.wait_for_processed(
                source, broken, after=checkpoint, outcome="compile_error"
            )
            source.write_bytes(baseline)
            sync.wait_for_applied(source, baseline)
            select_element(window, "Rectangle")
            with sync.gate("publication", source) as publication:
                with sync.action() as current:
                    edit_field(window, "Rotation", "77")
                publication.wait_for_reached()
                window_element_with_label(window, "Rotation knob").single_click(
                    slint_testing.PointerEventButton.Left
                )
                with sync.action() as undo:
                    shortcut(window)
                checkpoint = sync.checkpoint()
                acknowledgment.release()
                assert not wait_for_acknowledgment(sync, obsolete_id, checkpoint)[
                    "accepted"
                ]
                assert not operation_state(current)["settled"]
                assert (
                    "queued history"
                    in operation_state(undo)["operation_state"]["pending"].values()
                )
                assert operation_state(undo)["operation_state"]["writes"] == 0
            current.wait_for_settled(outcome="completed")
            result = undo.wait_for_settled(outcome="completed")
            assert result.data["operation_state"]["writes"] == 1
        obsolete.wait_for_settled(outcome="completed")
        sync.wait_for_applied(source, baseline)


@pytest.mark.parametrize("acknowledged", [False, True])
def test_retired_assigned_factory_resolves_edit_and_queued_history(
    rectangle_editor, acknowledged, fixture_project
):
    with rectangle_editor as (window, sync, source, baseline):
        edited = baseline.replace(b"32deg", b"62deg")
        with sync.action() as seed:
            edit_field(window, "Rotation", "42")
        seed.wait_for_settled(outcome="completed")
        checkpoint = sync.checkpoint()
        with (
            sync.gate("acknowledgment", source) as acknowledgment,
            sync.gate("factory", source) as factory,
        ):
            with sync.action() as edit:
                edit_field(window, "Rotation", "62")
            edit_id = acknowledgment.wait_for_reached().data["gate_state"]["edit"]
            attempt = factory.wait_for_reached().data["gate_state"]["attempt"]
            state = operation_state(edit)
            assert not state["settled"]
            assert "component factory" in state["operation_state"]["pending"].values()
            window_element_with_label(window, "Rotation knob").single_click(
                slint_testing.PointerEventButton.Left
            )
            with sync.action() as undo:
                shortcut(window)
            assert (
                "queued history"
                in operation_state(undo)["operation_state"]["pending"].values()
            )
            if acknowledged:
                acknowledgment.release()
                assert wait_for_acknowledgment(sync, edit_id, checkpoint)["accepted"]
            file_row(window, fixture_project / "assets").single_click(
                slint_testing.PointerEventButton.Left
            )
            file_row(window, fixture_project / "assets/checker.svg").single_click(
                slint_testing.PointerEventButton.Left
            )
            undo.wait_for_settled(outcome="canceled")
            undo.assert_no_source_writes()
            if not acknowledged:
                assert not operation_state(edit)["settled"]
                acknowledgment.release()
                assert wait_for_acknowledgment(sync, edit_id, checkpoint)["accepted"]
            result = edit.wait_for_settled(outcome="completed")
            assert result.data["operation_state"]["writes"] == 1
            assert source.read_bytes() == edited
            file_row(window, source).single_click(slint_testing.PointerEventButton.Left)
            sync.wait_for_applied(source, edited, after=checkpoint)
        retired = sync.wait_for_processed(
            source, edited, after=checkpoint, outcome="superseded"
        )
        assert retired.data["attempt"]["id"] == attempt
        sync.wait_for_applied(source, edited)
        select_element(window, "Rectangle")
        wait_for_field(window, "Rotation", "62")


@pytest.mark.parametrize(
    "fault,mutations,prefix",
    [
        ("before_open", 0, None),
        ("after_truncate", 1, 0),
        ({"after_bytes": 5}, 1, 5),
    ],
)
def test_write_failure_reports_mutation_and_reconciles_history(
    rectangle_editor, fault, mutations, prefix
):
    with rectangle_editor as (window, sync, source, baseline):
        seeded = baseline.replace(b"32deg", b"42deg")
        expected = seeded if prefix is None else seeded[:prefix]
        with sync.action() as seed:
            edit_field(window, "Rotation", "42")
        seed.wait_for_settled(outcome="completed")
        checkpoint = sync.checkpoint()
        sync._request(mode="write_fault", url=source, fault=fault)
        try:
            with sync.action() as edit:
                edit_field(window, "Rotation", "62")
            result = edit.wait_for_settled(outcome="failed")
        finally:
            sync._request(mode="clear_write_fault", url=source)
        state = result.data["operation_state"]
        assert state["accepted_edits"] == 1
        assert state["writes"] == 0
        assert state["mutations"] == mutations
        assert source.read_bytes() == expected
        if mutations:
            sync.wait_for_processed(
                source, expected, after=checkpoint, outcome="compile_error"
            )
        window_element_with_label(window, "Rotation knob").single_click(
            slint_testing.PointerEventButton.Left
        )
        with sync.action() as undo:
            shortcut(window)
        if mutations:
            undo.wait_for_settled(outcome="noop")
            undo.assert_no_source_writes()
            assert source.read_bytes() == expected
            source.write_bytes(seeded)
            sync.wait_for_applied(source, seeded)
        else:
            undo.wait_for_settled(outcome="completed")
            sync.wait_for_applied(source, baseline)
        select_element(window, "Rectangle")
        with sync.action() as next_edit:
            edit_field(window, "Rotation", "77")
        next_edit.wait_for_settled(outcome="completed")
        sync.wait_for_applied(source, baseline.replace(b"32deg", b"77deg"))


@pytest.mark.parametrize("acknowledged", [False, True])
@pytest.mark.parametrize("replacement", ["source", "component"])
@pytest.mark.parametrize("stage", ["factory", "publication"])
def test_newer_preview_resolves_pending_edit(
    rectangle_editor, acknowledged, replacement, stage, fixture_project
):
    with rectangle_editor as (window, sync, source, baseline):
        edited = baseline.replace(b"32deg", b"62deg")
        newer = baseline.replace(b"32deg", b"77deg")
        with sync.action() as seed:
            edit_field(window, "Rotation", "42")
        seed.wait_for_settled(outcome="completed")
        checkpoint = sync.checkpoint()
        with (
            sync.gate("acknowledgment", source) as ack,
            sync.gate(stage, source) as factory,
        ):
            with sync.action() as edit:
                edit_field(window, "Rotation", "62")
            edit_id = ack.wait_for_reached().data["gate_state"]["edit"]
            factory.wait_for_reached()
            window_element_with_label(window, "Rotation knob").single_click(
                slint_testing.PointerEventButton.Left
            )
            with sync.action() as undo:
                shortcut(window)
            assert (
                "queued history"
                in operation_state(undo)["operation_state"]["pending"].values()
            )
            if acknowledged:
                ack.release()
                assert wait_for_acknowledgment(sync, edit_id, checkpoint)["accepted"]
            if replacement == "source":
                source.write_bytes(newer)
                wait_for_field(window, "Rotation", "77")
            else:
                other = fixture_project / "Main.slint"
                file_row(window, other).single_click(
                    slint_testing.PointerEventButton.Left
                )
            if replacement == "source" and not acknowledged:
                ack.release()
            undo.wait_for_settled(outcome="canceled")
            undo.assert_no_source_writes()
            if not acknowledged:
                ack.release()
            factory.release()
            edit.wait_for_settled(outcome="completed")
            if replacement == "component":
                sync.wait_for_applied(other, other.read_bytes())
                file_row(window, source).single_click(
                    slint_testing.PointerEventButton.Left
                )
                newer = edited
            sync.wait_for_applied(source, newer)
        select_element(window, "Rectangle")
        with sync.action() as next_edit:
            edit_field(window, "Rotation", "88")
        next_edit.wait_for_settled(outcome="completed")
        window_element_with_label(window, "Rotation knob").single_click(
            slint_testing.PointerEventButton.Left
        )
        with sync.action() as next_undo:
            shortcut(window)
        result = next_undo.wait_for_settled(outcome="completed")
        assert result.data["operation_state"]["writes"] == 1
        sync.wait_for_applied(source, newer)


def test_image_surface_clears_mounted_preview_but_keeps_last_success(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project)
    source = fixture_project / SOURCE
    with launch_editor(editor_binary, editor_environment, source) as app:
        window = first_window(app)
        sync = current_editor_sync.get()
        sync.wait_for_applied(source, baseline)
        before = sync._request(mode="checkpoint").data
        assert before["mounted"]
        file_row(window, fixture_project / "assets").single_click(
            slint_testing.PointerEventButton.Left
        )
        file_row(window, fixture_project / "assets/checker.svg").single_click(
            slint_testing.PointerEventButton.Left
        )
        after = sync._request(mode="checkpoint").data
        assert not after["mounted"]
        assert after["installed"]["id"] == before["installed"]["id"]
        file_row(window, source).single_click(slint_testing.PointerEventButton.Left)
        sync.wait_for_applied(source, baseline)
        assert sync._request(mode="checkpoint").data["mounted"]


@pytest.mark.parametrize("acknowledged", [False, True])
def test_coalesced_edit_resolves_without_compiling_its_written_revision(
    rectangle_editor, acknowledged
):
    with rectangle_editor as (window, sync, source, baseline):
        newer = baseline.replace(b"32deg", b"77deg")
        checkpoint = sync.checkpoint()
        with (
            sync.gate("source", source) as processing,
            sync.gate("acknowledgment", source) as ack,
        ):
            with sync.action() as edit:
                edit_field(window, "Rotation", "62")
            processing.wait_for_reached()
            edit_id = ack.wait_for_reached().data["gate_state"]["edit"]
            if acknowledged:
                ack.release()
                assert wait_for_acknowledgment(sync, edit_id, checkpoint)["accepted"]
            source.write_bytes(newer)
            processing.release()
            wait_for_field(window, "Rotation", "77")
            ack.release()
            edit.wait_for_settled(outcome="completed")
        sync.wait_for_applied(source, newer)
        with sync.action() as next_edit:
            edit_field(window, "Rotation", "88")
        next_edit.wait_for_settled(outcome="completed")
        window_element_with_label(window, "Rotation knob").single_click(
            slint_testing.PointerEventButton.Left
        )
        with sync.action() as undo:
            shortcut(window)
        result = undo.wait_for_settled(outcome="completed")
        assert result.data["operation_state"]["writes"] == 1
        sync.wait_for_applied(source, newer)


def test_paused_factory_clears_mounted_identity(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project)
    source = fixture_project / SOURCE
    newer = baseline.replace(b"32deg", b"77deg")
    with launch_editor(editor_binary, editor_environment, source) as app:
        window = first_window(app)
        sync = current_editor_sync.get()
        sync.wait_for_applied(source, baseline)
        before = sync._request(mode="checkpoint").data
        with sync.gate("factory", source) as factory:
            source.write_bytes(newer)
            factory.wait_for_reached()
            window.grab_window_as_png()
            held = sync._request(mode="checkpoint").data
            assert not held["mounted"]
            assert held["installed"]["id"] == before["installed"]["id"]
        sync.wait_for_applied(source, newer)
        assert sync._request(mode="checkpoint").data["mounted"]
