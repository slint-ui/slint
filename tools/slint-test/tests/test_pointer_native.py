# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import os

import pytest

from slint_test import expect, launch
from slint_test.diagnostics import WaitTimeout


@pytest.fixture
def native_window():
    viewer = os.environ.get("SLINT_POINTER_FIXTURE")
    if not viewer:
        raise pytest.UsageError(
            "Set SLINT_POINTER_FIXTURE to the native pointer_fixture example"
        )
    with launch(
        [viewer],
        env=os.environ
        | {"SLINT_BACKEND": "headless-skia", "SLINT_EMIT_DEBUG_INFO": "1"},
    ) as app:
        window = app.window()
        assert window.capabilities["hit_testing"]
        yield window


def test_read_only_query_and_covered_timeout(native_window):
    w = native_window
    target = w.get_by_role("button", name="Target")
    result = target.pointer_target()
    assert result["status"] == "covered"
    assert "Cover" in result["detail"]
    expect(w.get_by_accessible_name("Hover")).to_have_value("no")
    with pytest.raises(WaitTimeout, match="Cover"):
        target.click(timeout=200)
    expect(w.get_by_accessible_name("Count")).to_have_value("0")
    expect(w.get_by_accessible_name("Hover")).to_have_value("no")


def test_waits_until_cover_disappears(native_window):
    w = native_window
    w.get_by_role("button", name="Uncover soon").click()
    w.get_by_role("button", name="Target").click()
    expect(w.get_by_accessible_name("Count")).to_have_value("1")


@pytest.mark.parametrize(
    "name,count", [("Far", "10"), ("Nested", "20"), ("Rotated", "30")]
)
def test_scrolling_and_transformed_clicks(native_window, name, count):
    w = native_window
    target = w.get_by_role("button", name=name)
    before = target.pointer_target()
    assert before["status"] == ("ready" if name == "Rotated" else "clipped")
    target.click()
    expect(w.get_by_accessible_name("Count")).to_have_value(count)
    assert target.pointer_target()["status"] == "ready"


def test_fixed_clip_cannot_be_scrolled(native_window):
    target = native_window.get_by_role("button", name="Clipped")
    assert not target.pointer_target()["scrollable"]
    with pytest.raises(WaitTimeout, match="viewport"):
        target.click(timeout=200)


def test_popup_scope_and_real_text_input(native_window):
    w = native_window
    w.get_by_role("text-input", name="Name").fill("typed")
    expect(w.get_by_role("text-input", name="Name")).to_have_value("typed")
    w.get_by_role("button", name="Popup").click()
    assert (
        w.get_by_role("button", name="Rotated").pointer_target()["status"] == "covered"
    )
    w.get_by_role("button", name="Popup action").click()
    expect(w.get_by_accessible_name("Count")).to_have_value("40")


def test_hover_callback_cannot_redirect_click(native_window):
    w = native_window
    target = w.get_by_role("button", name="Target")
    w.get_by_role("button", name="Uncover soon").click()
    expect.poll(lambda: target.pointer_target()["status"]).to_equal("ready")
    w.get_by_role("button", name="Cover on hover").activate()
    with pytest.raises(WaitTimeout, match="Cover"):
        target.click(timeout=300)
    expect(w.get_by_accessible_name("Count")).to_have_value("0")


def test_unknown_policy_fails_without_input(native_window):
    from slint_test import UnsupportedCapability

    target = native_window.get_by_role("button", name="Custom policy")
    assert target.pointer_target()["status"] == "unsupported"
    with pytest.raises(UnsupportedCapability, match="policy"):
        target.click()
    expect(native_window.get_by_accessible_name("Count")).to_have_value("0")


def test_cancel_while_waiting_for_cover(native_window):
    import threading

    target = native_window.get_by_role("button", name="Target")
    cancel = threading.Timer(0.1, native_window.session.cancel.set)
    cancel.start()
    try:
        with pytest.raises(InterruptedError):
            target.click()
    finally:
        cancel.join()
        native_window.session.cancel.clear()
    expect(native_window.get_by_accessible_name("Count")).to_have_value("0")


def test_explicit_scroll_reveals_without_clicking(native_window):
    w = native_window
    target = w.get_by_role("button", name="Far")
    assert target.count() == 1
    assert target.pointer_target()["status"] == "clipped"
    target.scroll_into_view()
    assert target.pointer_target()["status"] == "ready"
    expect(w.get_by_accessible_name("Count")).to_have_value("0")
    with pytest.raises(WaitTimeout, match="viewport"):
        w.get_by_role("button", name="Clipped").scroll_into_view(timeout=200)
