# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tobytes

import contextlib
import time
from collections.abc import Callable, Iterator
from io import BytesIO
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import TypeVar

import slint_testing
from editor_sync import EditorSync, current_editor_sync
from PIL import Image
from ui_locators import Window
from ui_reporting import capture_failure, current_report, replay_stage


def screenshot(window: Window) -> Image.Image:
    previous = b""

    def settled() -> Image.Image | None:
        nonlocal previous
        image = Image.open(
            BytesIO(window.grab_window_with_mime_type("image/bmp"))
        ).convert("RGB")
        data = image.tobytes()
        stable = data == previous
        previous = data
        return image if stable else None

    return wait_until(settled)


PALETTE_KINDS = ("Image", "Rectangle", "Text", "TouchArea")


def press_key(window: Window, key: str) -> None:
    window.dispatch_event(slint_testing.KeyPressedEvent(text=key))
    window.dispatch_event(slint_testing.KeyReleasedEvent(text=key))


def press_keys(window: Window, text: str) -> None:
    for key in text:
        press_key(window, key)


T = TypeVar("T")


def wait_until(probe: Callable[[], T | None], timeout: float = 5) -> T:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            result = probe()
        except slint_testing.RequestError:
            # An element the probe reads was replaced, e.g. by a preview update. Probe again; the
            # final attempt below lets the error through.
            result = None
        if result is not None:
            return result
        time.sleep(0.02)
    result = probe()
    assert result is not None
    return result


def first_window(
    application: slint_testing.Application,
) -> Window:
    window = application.first_window
    assert window is not None
    return Window(window)


ELEMENT_ROWS = {
    "Rectangle": "root-rectangle",
    "Text": "root-text",
    "Image": "root-image",
}


def outline_row(window: Window, label: str) -> slint_testing.Element:
    return window.get_by_role(
        slint_testing.AccessibleRole.ListItem, name=label
    ).resolve()


def outline_rows(window: Window) -> list[slint_testing.Element]:
    return (
        window.get_by_role(
            slint_testing.AccessibleRole.List, name="Current file outline"
        )
        .get_by_role(slint_testing.AccessibleRole.ListItem)
        .all()
    )


def select_outline_row(window: Window, row_label: str) -> slint_testing.Element:
    row = outline_row(window, row_label)
    row.invoke_accessible_default_action()
    return wait_until(lambda: row if row.accessible_item_selected else None)


def select_fixture_element(window: Window, element_type: str) -> None:
    select_outline_row(window, ELEMENT_ROWS[element_type])
    window.get_by_role(
        slint_testing.AccessibleRole.Region, name=f"Selected {element_type}"
    ).resolve()


@contextlib.contextmanager
def launch_editor(
    binary: Path,
    environment: dict[str, str],
    file: Path | None = None,
) -> Iterator[slint_testing.Application]:
    arguments = [str(binary)]
    if file is not None:
        arguments.append(str(file))
    with TemporaryDirectory(prefix="slint-editor-sync-") as directory:
        sync = EditorSync(Path(directory))
        token = current_editor_sync.set(sync)
        try:
            with slint_testing.Application(
                arguments,
                env=environment | {"SLINT_EDITOR_TEST_SYNC": directory},
                launch_timeout=20,
            ) as application:
                try:
                    yield application
                    report = current_report.get()
                    if report is not None and report.completed_stages == 0:
                        with replay_stage("completed"):
                            pass
                except Exception as error:
                    capture_failure(application, error)
                    raise

        finally:
            current_editor_sync.reset(token)


def file_row(window: Window, path: Path) -> slint_testing.Element:
    from canvas_interactions import center

    tree_locator = window.get_by_role(slint_testing.AccessibleRole.Tree, name="Files")
    tree = tree_locator.resolve()
    row = tree_locator.get_by_role(
        slint_testing.AccessibleRole.ListItem, name=str(path)
    )
    scroll_step = max(1, min(250, tree.size.height / 2))
    for delta in [0, 10000] + [-scroll_step] * 32:
        if delta:
            window.dispatch_event(
                slint_testing.PointerScrolledEvent(
                    center(tree), delta_x=0, delta_y=delta
                )
            )
        rows = row.all()
        if rows:
            return rows[0]
    return row.resolve()


def palette_row(window: Window, kind: str) -> slint_testing.Element:
    from canvas_interactions import center

    pane_locator = window.get_by_accessible_name("Element library")
    pane = pane_locator.resolve()
    row = pane_locator.get_by_role(slint_testing.AccessibleRole.ListItem, name=kind)
    top = pane.absolute_position.y
    bottom = top + pane.size.height
    position = slint_testing.LogicalPosition(x=center(pane).x, y=(top + bottom) / 2)
    step = max(1, (bottom - top) / 2)
    for delta in [0, 10000] + [-step] * 16:
        if delta:
            window.dispatch_event(
                slint_testing.PointerScrolledEvent(position, delta_x=0, delta_y=delta)
            )
        rows = row.all()
        if len(rows) == 1 and top < center(rows[0]).y < bottom:
            return rows[0]
    raise AssertionError(f"No visible palette row for {kind!r}")


def press_shortcut(window: Window, *keys: str) -> None:
    pressed = []
    try:
        for key in keys:
            window.dispatch_event(slint_testing.KeyPressedEvent(text=key))
            pressed.append(key)
    finally:
        for key in reversed(pressed):
            window.dispatch_event(slint_testing.KeyReleasedEvent(text=key))
