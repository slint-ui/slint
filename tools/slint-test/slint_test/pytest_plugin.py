# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from collections.abc import Iterator

import pytest

from .application import Application, launch
from .core import Window


def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(
        "--slint-command", nargs="+", help="Command for the generic Slint application"
    )


@pytest.fixture
def app_factory():
    return launch


@pytest.fixture
def app(request: pytest.FixtureRequest) -> Iterator[Application]:
    command = request.config.getoption("--slint-command")
    if not command:
        raise pytest.UsageError("Provide --slint-command or override the app fixture")
    with launch(command) as application:
        yield application


@pytest.fixture
def window(app: Application) -> Window:
    return app.window()
