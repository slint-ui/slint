# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import contextlib
import hashlib
import json
import shutil
import time
from collections.abc import Callable, Iterator
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import TypeVar

import slint_testing
from editor_sync import EditorSync, current_editor_sync
from ui_reporting import capture_failure, current_report, replay_stage

PALETTE_KINDS = ("Image", "Rectangle", "Text", "TouchArea")


def press_key(window: slint_testing.Window, key: str) -> None:
    window.dispatch_event(slint_testing.KeyPressedEvent(text=key))
    window.dispatch_event(slint_testing.KeyReleasedEvent(text=key))


def press_keys(window: slint_testing.Window, text: str) -> None:
    for key in text:
        press_key(window, key)


T = TypeVar("T")


def wait_until(
    probe: Callable[[], T | None], timeout: float = 5, *, deadline: float | None = None
) -> T:
    deadline = time.monotonic() + timeout if deadline is None else deadline
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
    return wait_until(lambda: application.first_window, timeout=20)


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


def find_element_with_label(
    root: slint_testing.Element,
    label: str,
    role: slint_testing.AccessibleRole | None = None,
) -> slint_testing.Element | None:
    matches = elements_with_label(root, label, role)
    return matches[0] if len(matches) == 1 else None


def find_window_element_with_label(
    window: slint_testing.Window,
    label: str,
    role: slint_testing.AccessibleRole | None = None,
) -> slint_testing.Element | None:
    return find_element_with_label(window.root_element, label, role)


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
    return wait_until(
        lambda: (
            current
            if (
                current := find_window_element_with_label(
                    window, row_label, slint_testing.AccessibleRole.ListItem
                )
            )
            is not None
            and current.accessible_item_selected
            else None
        )
    )


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
    with TemporaryDirectory(prefix="slint-editor-sync-") as directory:
        run_directory = Path(directory)
        binary_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
        (run_directory / "client-info.json").write_text(
            json.dumps(
                {
                    "binary": str(binary),
                    "sha256": binary_hash,
                    "protocol": 3,
                }
            )
        )
        sync = EditorSync(run_directory)
        token = current_editor_sync.set(sync)
        try:
            environment = dict(environment)
            environment.pop("SLINT_LIVE_PREVIEW", None)
            with slint_testing.Application(
                arguments,
                env=environment
                | {
                    "SLINT_EDITOR_TEST_SYNC": directory,
                    "SLINT_EDITOR_TEST_CONFIG_DIR": environment.get(
                        "SLINT_EDITOR_TEST_CONFIG_DIR", str(Path(directory) / "config")
                    ),
                },
                launch_timeout=20,
            ) as application:
                sync.process = application.process
                try:
                    handshake = sync._request(mode="handshake", timeout=20)
                    info = json.loads((run_directory / "client-info.json").read_text())
                    info.update(
                        {
                            "session": sync.session,
                            "protocol": 3,
                            "build": handshake.data["build"],
                        }
                    )
                    (run_directory / "client-info.json").write_text(json.dumps(info))
                    yield application
                    report = current_report.get()
                    if report is not None and report.completed_stages == 0:
                        with replay_stage("completed"):
                            pass
                except Exception as error:
                    capture_failure(application, error)
                    report = current_report.get()
                    if report is not None:
                        artifact_dir = report.artifacts / "editor-sync"
                        shutil.copytree(run_directory, artifact_dir, dirs_exist_ok=True)
                        error.add_note(f"Editor sync trace: {artifact_dir}")
                    raise

        finally:
            current_editor_sync.reset(token)


def file_row(window: slint_testing.Window, path: Path) -> slint_testing.Element:
    from canvas_interactions import center

    tree = window_element_with_label(window, "Files", slint_testing.AccessibleRole.Tree)
    scroll_step = max(1, min(250, tree.size.height / 2))
    for delta in [0, 10000] + [-scroll_step] * 32:
        if delta:
            window.dispatch_event(
                slint_testing.PointerScrolledEvent(
                    center(tree), delta_x=0, delta_y=delta
                )
            )
        rows = elements_with_label(
            tree, str(path), slint_testing.AccessibleRole.ListItem
        )
        if rows:
            return rows[0]
    return window_element_with_label(
        window, str(path), slint_testing.AccessibleRole.ListItem
    )


def press_shortcut(window: slint_testing.Window, *keys: str) -> None:
    pressed = []
    try:
        for key in keys:
            window.dispatch_event(slint_testing.KeyPressedEvent(text=key))
            pressed.append(key)
    finally:
        for key in reversed(pressed):
            window.dispatch_event(slint_testing.KeyReleasedEvent(text=key))
