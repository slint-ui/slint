# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import contextlib
import time
from collections.abc import Callable, Iterator
from pathlib import Path
from typing import TypeVar

import slint_testing

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
) -> slint_testing.Window:
    window = application.first_window
    assert window is not None
    return window


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


def element_with_label(
    root: slint_testing.Element,
    label: str,
    role: slint_testing.AccessibleRole | None = None,
    timeout: float = 5,
) -> slint_testing.Element:
    matches: list[slint_testing.Element] = []

    def unique_match() -> slint_testing.Element | None:
        nonlocal matches
        matches = elements_with_label(root, label, role)
        return matches[0] if len(matches) == 1 else None

    try:
        return wait_until(unique_match, timeout=timeout)
    except AssertionError as error:
        raise AssertionError(
            f"expected exactly one element labeled {label!r}, found {len(matches)}"
        ) from error


def window_element_with_label(
    window: slint_testing.Window,
    label: str,
    role: slint_testing.AccessibleRole | None = None,
    timeout: float = 5,
) -> slint_testing.Element:
    return element_with_label(window.root_element, label, role, timeout)


ELEMENT_ROWS = {
    "Rectangle": "root-rectangle",
    "Text": "root-text",
    "Image": "root-image",
}


def select_outline_row(
    window: slint_testing.Window, row_label: str
) -> slint_testing.Element:
    row = window_element_with_label(
        window, row_label, slint_testing.AccessibleRole.ListItem
    )
    row.invoke_accessible_default_action()
    return wait_until(lambda: row if row.accessible_item_selected else None)


def select_fixture_element(window: slint_testing.Window, element_type: str) -> None:
    select_outline_row(window, ELEMENT_ROWS[element_type])
    window_element_with_label(
        window,
        f"Selected {element_type}",
        slint_testing.AccessibleRole.Region,
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
    with slint_testing.Application(
        arguments, env=environment, launch_timeout=20
    ) as application:
        yield application
