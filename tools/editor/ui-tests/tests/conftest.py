# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore addoption nodeid

import functools
import math
import os
import shutil
from collections.abc import Callable, Iterator
from pathlib import Path
from typing import TypeVar

import pytest
import slint_testing
from editor_sync import settle
from ui_reporting import TestReport, current_report

T = TypeVar("T")

UI_TEST_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = UI_TEST_ROOT.parents[2]
FIXTURE_PROJECT = UI_TEST_ROOT / "fixtures" / "editor-project"
DEFAULT_EDITOR_BINARY = REPOSITORY_ROOT / "target" / "debug" / "slint-editor"


@pytest.fixture
def editor_binary() -> Path:
    binary = Path(os.environ.get("SLINT_EDITOR_BINARY", DEFAULT_EDITOR_BINARY))
    assert binary.is_file(), f"Editor binary not found at {binary}"
    return binary


@pytest.fixture
def editor_environment(tmp_path_factory: pytest.TempPathFactory) -> dict[str, str]:
    settings_directory = tmp_path_factory.mktemp("editor-settings")
    environment = os.environ.copy()
    environment.pop("SLINT_SCALE_FACTOR", None)
    environment.update(
        {
            "SLINT_BACKEND": environment.get(
                "SLINT_EDITOR_UI_TEST_BACKEND", "headless-skia"
            ),
            "SLINT_EMIT_DEBUG_INFO": "1",
            "SLINT_ENABLE_EXPERIMENTAL_FEATURES": "1",
            "HOME": str(settings_directory / "home"),
            "XDG_CONFIG_HOME": str(settings_directory / "config"),
        }
    )
    return environment


@pytest.fixture
def fixture_project(tmp_path: Path) -> Path:
    destination = tmp_path / "editor-project"
    shutil.copytree(FIXTURE_PROJECT, destination)
    return destination


@pytest.fixture
def radial_scene(tmp_path: Path) -> Path:
    path = tmp_path / "RadialGradientScene.slint"
    path.write_text("""export component RadialGradientScene inherits Window {
    width: 400px;
    height: 400px;
    fill := Rectangle {
        width: 200px;
        height: 200px;
        background: @radial-gradient(circle, #7e3b66 0%, #264052 45%, #568fb8 100%);
    }
}
""")
    return path


def replay_pause_seconds(value: str) -> float:
    seconds = float(value)
    if not math.isfinite(seconds) or seconds < 0:
        raise ValueError("Replay pause must be a finite, nonnegative number")
    return seconds


def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(
        "--replay-pause",
        type=replay_pause_seconds,
        default=0,
        help="Pause for this many seconds after replay stages; use -s to see stage names",
    )


@pytest.fixture(autouse=True, scope="session")
def settle_before_interactions() -> Iterator[None]:
    with pytest.MonkeyPatch.context() as patch:
        for cls, name in (
            (slint_testing.Window, "dispatch_event"),
            (slint_testing.Element, "invoke_accessible_default_action"),
            (slint_testing.Element, "single_click"),
            (slint_testing.Element, "double_click"),
        ):
            patch.setattr(cls, name, settled(getattr(cls, name)))
        for name in ("absolute_position", "absolute_rect", "size"):
            getter = getattr(slint_testing.Element, name).fget
            patch.setattr(slint_testing.Element, name, property(settled(getter)))
        yield


def settled(function: Callable[..., T]) -> Callable[..., T]:
    @functools.wraps(function)
    def wrapper(*args, **kwargs) -> T:
        settle()
        return function(*args, **kwargs)

    return wrapper


@pytest.fixture(autouse=True)
def ui_test_report(request: pytest.FixtureRequest, tmp_path: Path) -> Iterator[None]:
    token = current_report.set(
        TestReport(
            request.node.nodeid,
            tmp_path / "screenshots",
            request.config.getoption("--replay-pause"),
        )
    )
    try:
        yield
    finally:
        current_report.reset(token)
