# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import os
import shutil
from pathlib import Path

import pytest

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
