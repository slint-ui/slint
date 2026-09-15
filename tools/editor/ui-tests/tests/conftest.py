# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore addoption nodeid

import math
import os
import shutil
from collections.abc import Iterator
from pathlib import Path

import pytest
from ui_reporting import TestReport, current_report

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
def editor_environment() -> dict[str, str]:
    environment = os.environ.copy()
    environment.pop("SLINT_SCALE_FACTOR", None)
    environment.update(
        {
            "SLINT_BACKEND": environment.get(
                "SLINT_EDITOR_UI_TEST_BACKEND", "headless-skia"
            ),
            "SLINT_EMIT_DEBUG_INFO": "1",
            "SLINT_ENABLE_EXPERIMENTAL_FEATURES": "1",
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
