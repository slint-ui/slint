# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tobytes

import contextlib
import subprocess
import sys
import time
from collections.abc import Callable, Iterator
from io import BytesIO
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import TypeVar

import slint_testing
from editor_sync import EditorSync, current_editor_sync
from PIL import Image
from slint_test import Locator, Window, expect
from ui_reporting import capture_failure, current_report, notify_observer, replay_stage


def screenshot(window: Window) -> Image.Image:
    previous = b""

    def settled() -> Image.Image | None:
        nonlocal previous
        image = Image.open(BytesIO(window.screenshot())).convert("RGB")
        data = image.tobytes()
        stable = data == previous
        previous = data
        return image if stable else None

    return wait_until(settled)


PALETTE_KINDS = ("Image", "Rectangle", "Text", "TouchArea")


T = TypeVar("T")


def wait_until(probe: Callable[[], T | None], timeout: float = 5) -> T:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = probe()
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


def elements_with_label(
    root: slint_testing.Element,
    label: str,
    role: slint_testing.AccessibleRole | None = None,
) -> list[slint_testing.Element]:
    query = root.query_descendants()
    if role is not None:
        query = query.match_accessible_role(role)
    return [
        element for element in query.find_all() if element.accessible_label == label
    ]


ELEMENT_ROWS = {
    "Rectangle": "root-rectangle",
    "Text": "root-text",
    "Image": "root-image",
}


def outline_row(window: Window, label: str) -> slint_testing.Element:
    return (
        window.get_by_role("list", name="Current file outline")
        .get_by_role("list-item", name=label)
        .resolve()
    )


def outline_rows(window: Window) -> list[slint_testing.Element]:
    return (
        window.get_by_role("list", name="Current file outline")
        .get_by_role("list-item")
        .all()
    )


def select_outline_row(window: Window, row_label: str) -> slint_testing.Element:
    row = window.get_by_role("list", name="Current file outline").get_by_role(
        "list-item", name=row_label
    )
    row.activate()
    expect(row).to_be_selected()
    return row.resolve()


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
            application = slint_testing.Application(
                arguments,
                env=environment | {"SLINT_EDITOR_TEST_SYNC": directory},
                launch_timeout=20,
            )
            try:
                application.__enter__()
            except BaseException:
                process = getattr(application, "process", None)
                if process is not None:
                    process.terminate()
                    try:
                        process.wait(timeout=2)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
                application.test_server_socket.close()
                raise
            notify_observer("application-ready", application=application)
            try:
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
                notify_observer(
                    "application-closing",
                    application=application,
                    returncode=application.process.poll(),
                    failed=sys.exc_info()[0] is not None,
                )
                try:
                    application.__exit__(*sys.exc_info())
                finally:
                    notify_observer(
                        "application-exit", returncode=application.process.poll()
                    )

        finally:
            current_editor_sync.reset(token)


def file_row(window: Window, path: Path) -> Locator:
    row = window.get_by_role("tree", name="Files").get_by_role(
        "list-item", name=str(path)
    )
    row.scroll_into_view()
    return row


def palette_row(window: Window, kind: str) -> Locator:
    from canvas_interactions import center

    pane_locator = window.get_by_accessible_name("Element library")
    pane = pane_locator.resolve()
    row = pane_locator.get_by_role("list-item", name=kind)
    top = pane.absolute_position.y
    bottom = top + pane.size.height
    position = slint_testing.LogicalPosition(x=center(pane).x, y=(top + bottom) / 2)
    step = max(1, (bottom - top) / 2)
    for delta in [0, 10000] + [-step] * 16:
        if delta:
            window.pointer.scroll(0, delta, at=position)
        matches = row.all()
        if len(matches) == 1 and top < center(matches[0]).y < bottom:
            return row
    raise AssertionError(f"No visible palette row for {kind!r}")
