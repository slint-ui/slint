# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import re
import threading

import pytest

from slint_test import (
    Point,
    Session,
    StrictMatchError,
    UnsupportedCapability,
    expect,
    reporting,
    step,
)


def test_keyboard_pointer_and_capture(window):
    field = window.get_by_role("text-input", name="Name")
    field.fill("Alice")
    expect(field).to_have_value("Alice")
    field.clear()
    field.press_sequentially("Bob")
    expect(field).to_have_value("Bob")
    apply = window.get_by_role("button", name="Apply")
    apply.click()
    expect(window.get_by_accessible_name("Count")).to_have_value("1")
    expect(field).not_to_have_value("Alice")
    point = apply.center()
    window.pointer.move_to(point)
    window.pointer.press_at(point)
    window.pointer.release_at(point)
    expect(window.get_by_accessible_name("Count")).to_have_value("2")
    window.pointer.scroll(0, 1, at=point)
    window.pointer.exit()
    apply.click(force=True)
    expect(window.get_by_accessible_name("Count")).to_have_value("3")
    assert window.screenshot().startswith(b"\x89PNG")


def test_recreated_ancestor_and_regex(window):
    message = window.get_by_role("region", name="Dynamic").get_by_accessible_name(
        "Message"
    )
    expect(message).to_have_value("first")
    window.get_by_role("button", name=re.compile("^Replace$")).activate()
    expect(message).to_have_value("second")
    window.get_by_role("button", name="Replace").activate()
    expect(message).to_have_value("first")


def test_ambiguity_and_absence(window):
    duplicate = window.get_by_role("button", name="Duplicate")
    expect(duplicate).to_have_count(2)
    with pytest.raises(AssertionError, match="expected 1"):
        expect(duplicate).to_be_visible(timeout=30)
    with pytest.raises(StrictMatchError, match="matched 2"):
        duplicate.click()
    expect(duplicate.nth(0)).to_have_count(1)
    expect(duplicate.nth(0)).to_be_visible()
    duplicate.nth(0).wait_for()
    with pytest.raises(ValueError, match="nonnegative"):
        duplicate.nth(-1)
    expect(window.get_by_accessible_name("Missing")).to_have_count(0)
    expect(window.get_by_accessible_name("Missing")).to_be_hidden()
    window.get_by_accessible_name("Missing").wait_for(state="hidden")
    with pytest.raises(ValueError, match="state"):
        duplicate.wait_for(state="detached")
    with pytest.raises(AssertionError, match="last observed"):
        expect(window.get_by_accessible_name("Missing")).to_have_value(
            "anything", timeout=30
        )


def test_composition(window):
    region = window.get_by_role("region").filter(
        has=window.get_by_accessible_name("Message")
    )
    expect(region).to_have_count(1)
    expect(region.get_by_accessible_name("Message")).to_have_value("first")


def test_no_false_hit_test_claim(window):
    with pytest.raises(UnsupportedCapability, match="hit targets"):
        window.get_by_role("button", name="Apply").click(require_hit_target=True)
    expect(window.get_by_accessible_name("Count")).to_have_value("0")


def test_held_input_cleanup_and_outside_coordinates(window):
    with (
        pytest.raises(ValueError, match="deliberate"),
        window.pointer.drag_from(Point(10, 10)) as drag,
    ):
        drag.move_by(-2000, 2000)
        window.keyboard.down("Shift")
        raise ValueError("deliberate")
    assert not window.pointer.held
    assert not window.keyboard.held
    window.get_by_role("text-input", name="Name").fill("lowercase")
    expect(window.get_by_accessible_name("Name")).to_have_value("lowercase")


def test_nested_trace_and_observer_isolation(window, caplog):
    events = []
    with reporting(events.append), step("Application helper", layer="adapter"):
        window.get_by_role("button", name="Apply").activate()
        expect(window.get_by_accessible_name("Count")).to_have_value("1")
    starts = [e for e in events if e["kind"] == "action-start"]
    assert starts[1]["parent_id"] == starts[0]["action_id"]
    assertion = next(e for e in starts if e["layer"] == "assertion")
    assert assertion["parent_id"] == starts[0]["action_id"]
    assert starts[1]["source"]["line"] > 0

    def broken(_):
        raise RuntimeError("observer failure")

    with reporting(broken):
        window.get_by_role("button", name="Apply").activate()
    expect(window.get_by_accessible_name("Count")).to_have_value("2")
    assert "observer failure" in caplog.text


def test_sustained_failure_is_immediate():
    values = iter(["bad", "good"])
    with pytest.raises(AssertionError, match="bad"):
        expect.poll(lambda: next(values)).to_remain("good", for_ms=100)
    assert next(values) == "good"


def test_cancellation_in_wait():
    cancel = threading.Event()
    cancel.set()
    with pytest.raises(InterruptedError):
        Session(cancel=cancel).wait(lambda: False, bool)


@pytest.mark.parametrize("target", ["Disabled", "Read only"])
def test_readiness_rejects_disabled_or_read_only(window, target):
    locator = window.get_by_accessible_name(target)
    with pytest.raises(AssertionError):
        if target == "Disabled":
            locator.click(timeout=60)
        else:
            locator.fill("changed", timeout=60)
    expect(window.get_by_accessible_name("Count")).to_have_value("0")
    expect(window.get_by_accessible_name("Read only")).to_have_value("locked")


def test_covered_pointer_does_not_fall_back_to_accessibility(window):
    window.get_by_role("button", name="Covered").click()
    expect(window.get_by_accessible_name("Count")).to_have_value("0")
    assert not window.capabilities["hit_testing"]
    assert not window.capabilities["scroll_into_view"]
    expect(window.get_by_accessible_name("List row 0")).to_have_count(1)
    expect(window.get_by_accessible_name("List row 7")).to_have_count(0)


def test_popup_scoping(window):
    popup = window.get_by_role("region", name="Popup content")
    expect(popup).to_be_hidden()
    window.get_by_role("button", name="Popup").click()
    expect(popup.get_by_accessible_name("Popup message")).to_be_visible()


@pytest.mark.parametrize("duration", [float("inf"), float("nan"), -1])
def test_sustained_invalid_intervals(duration):
    with pytest.raises(ValueError):
        expect.poll(lambda: True).to_remain(True, for_ms=duration)


def test_drag_enter_failure_closes_reporting_scope(window, monkeypatch):
    events = []
    original = window.pointer.press

    def fail():
        original()
        raise ValueError("after pointer down")

    monkeypatch.setattr(window.pointer, "press", fail)
    with reporting(events.append):
        with (
            pytest.raises(ValueError, match="after pointer down"),
            window.pointer.drag_from(Point(5, 5)),
        ):
            pytest.fail("must not enter")
        with step("Next action"):
            pass
    assert not window.pointer.held
    assert events[-2]["parent_id"] is None
    assert any(e.get("title") == "Drag" and e.get("status") == "Failed" for e in events)


def test_one_deadline_covers_all_fill_input(window):
    field = window.get_by_role("text-input", name="Name")
    with pytest.raises((AssertionError, TimeoutError)):
        field.fill("a" * 10000, timeout=100)
    assert not window.keyboard.held
    assert field.read(lambda e: len(e.accessible_value)) < 10000


def test_nested_drag_rejection_preserves_held_pointer(window):
    with window.pointer.drag_from(Point(10, 10)) as outer:
        with (
            pytest.raises(RuntimeError, match="already held"),
            window.pointer.drag_from(Point(20, 20)),
        ):
            pytest.fail("Nested drag must fail")
        assert window.pointer.held
        assert window.pointer.position == Point(10, 10)
        outer.release()
    assert not window.pointer.held
