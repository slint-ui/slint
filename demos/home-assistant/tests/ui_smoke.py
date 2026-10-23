# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
# cSpell: ignore SIGSTOP SIGCONT

"""Exercise the real Slint window and save 1024 × 600 native captures.

Build home-assistant with SLINT_EMIT_DEBUG_INFO=1 and --features ui-testing, then run
with a Python environment containing slint_testing:
    python tests/ui_smoke.py /path/to/home-assistant
"""

import signal
import sys
import time
from pathlib import Path

import slint_testing as st

OUTPUT = Path(__file__).resolve().parents[1] / "captures"
OUTPUT.mkdir(exist_ok=True)
BINARY = str(Path(sys.argv[1]).resolve())


def wait_until(condition, timeout=3):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if condition():
            return
        time.sleep(0.04)
    raise AssertionError("UI condition did not settle")


def elements(window, label, role=st.AccessibleRole.Button):
    query = window.root_element.query_descendants().match_accessible_role(role)
    return [
        e
        for e in query.find_all()
        if e.accessible_label == label and e.computed_opacity > 0.95
    ]


def button(window, label):
    matches = elements(window, label, st.AccessibleRole.Button)
    assert len(matches) == 1, (label, len(matches))
    return matches[0]


def pointer(window, event, x, y):
    if event is st.PointerMoveEvent:
        window.dispatch_event(event(st.LogicalPosition(x, y)))
    else:
        window.dispatch_event(
            event(st.LogicalPosition(x, y), st.PointerEventButton.Left)
        )


def key(window, text):
    window.dispatch_event(st.KeyPressedEvent(text))
    window.dispatch_event(st.KeyReleasedEvent(text))


def save(window, name):
    (OUTPUT / f"{name}.png").write_bytes(window.grab_window_as_png())


with st.Application([BINARY, "--no-tour"], env={"SLINT_SCALE_FACTOR": "1"}) as app:
    window = app.first_window
    assert window.size.width == 1024 and window.size.height == 600
    save(window, "lock")

    # A short hold, and a hold dragged outside the ring, must both cancel.
    pointer(window, st.PointerPressEvent, 512, 432)
    time.sleep(0.3)
    pointer(window, st.PointerReleaseEvent, 512, 432)
    time.sleep(1)
    assert elements(window, "Hold to unlock")
    pointer(window, st.PointerPressEvent, 512, 432)
    time.sleep(0.3)
    pointer(window, st.PointerMoveEvent, 740, 432)
    time.sleep(1.2)
    pointer(window, st.PointerReleaseEvent, 740, 432)
    assert elements(window, "Hold to unlock")

    # A delayed event loop must not extend the 1.2-second hold. Suspend only
    # this test's child process; always resume it even if the test is interrupted.
    pointer(window, st.PointerPressEvent, 512, 432)
    time.sleep(0.2)
    if hasattr(signal, "SIGSTOP"):
        try:
            app.process.send_signal(signal.SIGSTOP)
            time.sleep(0.65)
        finally:
            app.process.send_signal(signal.SIGCONT)
    else:
        time.sleep(0.65)
    time.sleep(0.5)
    pointer(window, st.PointerReleaseEvent, 512, 432)
    wait_until(lambda: not elements(window, "Hold to unlock"))
    save(window, "living")

    def dial():
        return elements(window, "Target temperature", st.AccessibleRole.Slider)[0]

    assert dial().accessible_value == "21.5"
    button(window, "Increase temperature").single_click(st.PointerEventButton.Left)
    assert dial().accessible_value == "22.0"
    light = elements(window, "Ceiling", st.AccessibleRole.Checkbox)[0]
    assert light.accessible_checked
    light.single_click(st.PointerEventButton.Left)
    assert not light.accessible_checked

    # Shared cards must travel between columns, and room settings must persist.
    before = dial().absolute_position.x
    button(window, "Kitchen").single_click(st.PointerEventButton.Left)
    time.sleep(0.15)
    middle = dial().absolute_position.x
    assert before < middle < 440, (before, middle)
    save(window, "transition")
    wait_until(lambda: abs(dial().absolute_position.x - 440) < 0.1)
    assert dial().accessible_value == "20.0"
    time.sleep(0.3)
    save(window, "kitchen")
    button(window, "Relax").single_click(st.PointerEventButton.Left)
    assert button(window, "Relax").accessible_checked

    for room, capture in [
        ("Bedroom", "bedroom"),
        ("Office", "office"),
        ("Garden", "garden"),
    ]:
        button(window, room).single_click(st.PointerEventButton.Left)
        time.sleep(0.85)
        save(window, capture)
    button(window, "Talk").single_click(st.PointerEventButton.Left)
    assert button(window, "End talk")
    button(window, "End talk").single_click(st.PointerEventButton.Left)

    button(window, "Living Room").single_click(st.PointerEventButton.Left)
    time.sleep(0.85)
    assert dial().accessible_value == "22.0"
    assert not elements(window, "Ceiling", st.AccessibleRole.Checkbox)[
        0
    ].accessible_checked
    button(window, "Kitchen").single_click(st.PointerEventButton.Left)
    time.sleep(0.85)
    assert button(window, "Relax").accessible_checked
    button(window, "Living Room").single_click(st.PointerEventButton.Left)
    time.sleep(0.85)

    # The dead zone below the dial clamps to the nearest end, never wraps.
    window.drag_and_drop(st.LogicalPosition(194, 406), st.LogicalPosition(231, 432))
    assert dial().accessible_value == "16.0", dial().accessible_value
    window.drag_and_drop(st.LogicalPosition(342, 406), st.LogicalPosition(305, 432))
    assert dial().accessible_value == "28.0", dial().accessible_value
    button(window, "Increase temperature").single_click(st.PointerEventButton.Left)
    assert dial().accessible_value == "28.0"

    button(window, "Pause music").single_click(st.PointerEventButton.Left)
    assert button(window, "Play music")
    window.drag_and_drop(st.LogicalPosition(500, 537), st.LogicalPosition(718, 537))
    assert elements(window, "2:02", st.AccessibleRole.Text)
    time.sleep(0.3)
    assert elements(window, "2:02", st.AccessibleRole.Text)
    button(window, "Play music").single_click(st.PointerEventButton.Left)
    assert button(window, "Pause music")
    button(window, "Next track").single_click(st.PointerEventButton.Left)
    assert elements(window, "0:00", st.AccessibleRole.Text)

    # Card whitespace also supports room swipes.
    window.drag_and_drop(st.LogicalPosition(650, 100), st.LogicalPosition(540, 100))
    time.sleep(0.85)
    assert dial().accessible_value == "20.0"

    # Rapid selection must settle on the last room without stale card contents.
    for name in ["Garden", "Bedroom", "Office", "Kitchen", "Living Room"]:
        button(window, name).single_click(st.PointerEventButton.Left)
    time.sleep(0.9)
    assert dial().accessible_value == "28.0"
    assert elements(window, "Front door", st.AccessibleRole.Text)

    key(window, "d")
    time.sleep(0.5)
    save(window, "diagnostics")
    assert elements(window, "SLINT RENDERER", st.AccessibleRole.Text)
    key(window, "d")
    # Repeat pointer unlocks after locking again, including small movements while
    # held and releasing after the button has disappeared behind the doors.
    for attempt in range(3):
        button(window, "Lock home").single_click(st.PointerEventButton.Left)
        time.sleep(1)
        pointer(window, st.PointerPressEvent, 512, 432)
        for offset in [4, -3, 5, 0, -4]:
            time.sleep(0.26)
            pointer(window, st.PointerMoveEvent, 512 + offset, 432)
        wait_until(lambda: not elements(window, "Hold to unlock"))
        pointer(window, st.PointerReleaseEvent, 512, 432)

    button(window, "Lock home").single_click(st.PointerEventButton.Left)
    time.sleep(1)
    assert elements(window, "Hold to unlock")
    # Keyboard users can also hold Space to unlock.
    window.dispatch_event(st.KeyPressedEvent(" "))
    time.sleep(1.4)
    window.dispatch_event(st.KeyReleasedEvent(" "))
    wait_until(lambda: not elements(window, "Hold to unlock"))
    key(window, "5")
    time.sleep(0.85)
    assert elements(window, "WEATHER", st.AccessibleRole.Text)
    print(
        "PASS: hold/cancel, all rooms, animated layout, per-room state, dial limits, media, talk, swipe, rapid navigation, diagnostics and keyboard",
        flush=True,
    )

# The idle demonstration must unlock and visit rooms without user input.
with st.Application([BINARY], env={"SLINT_SCALE_FACTOR": "1"}) as app:
    window = app.first_window
    wait_until(lambda: not elements(window, "Hold to unlock"), timeout=8)
    visited = set()
    relocked = False
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline:
        for item in (
            window.root_element.query_descendants()
            .match_accessible_role(st.AccessibleRole.Text)
            .find_all()
        ):
            name = item.accessible_label
            if (
                name in ["Living Room", "Kitchen", "Bedroom", "Office", "Garden"]
                and item.computed_opacity > 0.95
            ):
                visited.add(name)
        if len(visited) == 5 and elements(window, "Hold to unlock"):
            relocked = True
            break
        time.sleep(0.2)
    assert len(visited) == 5, visited
    assert relocked, "Tour did not return to the lock screen"
    print(
        "PASS: automatic unlock, full five-room idle tour and automatic locking",
        flush=True,
    )
