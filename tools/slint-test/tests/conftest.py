# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import os
from pathlib import Path

import pytest

from slint_test import launch


@pytest.fixture
def app():
    python = os.environ.get("SLINT_FIXTURE_PYTHON")
    if not python:
        raise pytest.UsageError(
            "Set SLINT_FIXTURE_PYTHON to an interpreter with Slint testing support"
        )
    with launch(
        [python, str(Path(__file__).with_name("fixture_app.py"))],
        env=os.environ
        | {"SLINT_BACKEND": "headless-skia", "SLINT_EMIT_DEBUG_INFO": "1"},
    ) as application:
        yield application


@pytest.fixture
def window(app):
    return app.window()
