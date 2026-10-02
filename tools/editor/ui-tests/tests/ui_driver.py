# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tobytes

import contextlib
from collections.abc import Iterator
from io import BytesIO
from pathlib import Path
from tempfile import TemporaryDirectory

import slint_testing
from editor_sync import EditorSync, current_editor_sync
from PIL import Image
from slint_testing import wait_until
from ui_assertions import expect
from ui_reporting import capture_failure, current_report, replay_stage


def screenshot(window: slint_testing.Window) -> Image.Image:
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


def press_key(window: slint_testing.Window, key: str) -> None:
    window.dispatch_event(slint_testing.KeyPressedEvent(text=key))
    window.dispatch_event(slint_testing.KeyReleasedEvent(text=key))


def press_keys(window: slint_testing.Window, text: str) -> None:
    for key in text:
        press_key(window, key)


def first_window(
    application: slint_testing.Application,
) -> slint_testing.Window:
    window = application.first_window
    assert window is not None
    return window


def query(
    scope: slint_testing.Window | slint_testing.Element,
    name: str | None = None,
    *,
    role: slint_testing.AccessibleRole | None = None,
    id: str | None = None,
) -> slint_testing.ElementQuery:
    """The query that element() and elements() run, for assertions on its matches."""
    query = scope.query_descendants()
    if id is not None:
        query = query.match_id(id)
    if role is not None:
        query = query.match_accessible_role(role)
    if name is not None:
        query = query.match_accessible_label(name)
    return query


def element(
    scope: slint_testing.Window | slint_testing.Element,
    name: str | None = None,
    *,
    role: slint_testing.AccessibleRole | None = None,
    id: str | None = None,
    timeout: float = 5,
    tracking: bool = True,
) -> slint_testing.Element:
    """Waits until exactly one element below `scope` matches, and returns it.

    The element tracks the query: every property read or action runs the query again, so it
    follows the element when the UI replaces it, for example when the preview updates. Pass
    `tracking=False` for a handle to the instance that matches now, to keep reading it while the
    element is hidden from queries.
    """
    lookup = query(scope, name, role=role, id=id)
    if tracking:
        return lookup.tracking(timeout).find_one()
    return wait_until(lookup.find_one, timeout=timeout)


def elements(
    scope: slint_testing.Window | slint_testing.Element,
    name: str | None = None,
    *,
    role: slint_testing.AccessibleRole | None = None,
    id: str | None = None,
) -> list[slint_testing.Element]:
    """The elements below `scope` that match right now, without waiting."""
    return query(scope, name, role=role, id=id).find_all()


ELEMENT_ROWS = {
    "Rectangle": "root-rectangle",
    "Text": "root-text",
    "Image": "root-image",
}


def outline_row(window: slint_testing.Window, label: str) -> slint_testing.Element:
    return element(window, label, role=slint_testing.AccessibleRole.ListItem)


def outline_rows(window: slint_testing.Window) -> list[slint_testing.Element]:
    return elements(
        element(window, "Current file outline", role=slint_testing.AccessibleRole.List),
        role=slint_testing.AccessibleRole.ListItem,
    )


def select_outline_row(
    window: slint_testing.Window, row_label: str
) -> slint_testing.Element:
    row = outline_row(window, row_label)
    row.invoke_accessible_default_action()
    expect(row).to_be_selected()
    return row


def select_fixture_element(window: slint_testing.Window, element_type: str) -> None:
    select_outline_row(window, ELEMENT_ROWS[element_type])
    element(
        window, f"Selected {element_type}", role=slint_testing.AccessibleRole.Region
    )


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


def file_row(window: slint_testing.Window, path: Path) -> slint_testing.Element:
    from canvas_interactions import center

    tree = element(window, "Files", role=slint_testing.AccessibleRole.Tree)
    role = slint_testing.AccessibleRole.ListItem
    scroll_step = max(1, min(250, tree.size.height / 2))
    for delta in [0, 10000] + [-scroll_step] * 32:
        if delta:
            window.dispatch_event(
                slint_testing.PointerScrolledEvent(
                    center(tree), delta_x=0, delta_y=delta
                )
            )
        if elements(tree, str(path), role=role):
            break
    return element(tree, str(path), role=role)


def palette_row(window: slint_testing.Window, kind: str) -> slint_testing.Element:
    from canvas_interactions import center

    pane = element(window, "Element library")
    role = slint_testing.AccessibleRole.ListItem
    rect = pane.absolute_rect
    top = rect.y
    bottom = top + rect.height
    position = slint_testing.LogicalPosition(x=center(pane).x, y=(top + bottom) / 2)
    step = max(1, (bottom - top) / 2)
    for delta in [0, 10000] + [-step] * 16:
        if delta:
            window.dispatch_event(
                slint_testing.PointerScrolledEvent(position, delta_x=0, delta_y=delta)
            )
        rows = elements(pane, kind, role=role)
        if len(rows) == 1 and top < center(rows[0]).y < bottom:
            return element(pane, kind, role=role)
    raise AssertionError(f"No visible palette row for {kind!r}")


def press_shortcut(window: slint_testing.Window, *keys: str) -> None:
    pressed = []
    try:
        for key in keys:
            window.dispatch_event(slint_testing.KeyPressedEvent(text=key))
            pressed.append(key)
    finally:
        for key in reversed(pressed):
            window.dispatch_event(slint_testing.KeyReleasedEvent(text=key))
