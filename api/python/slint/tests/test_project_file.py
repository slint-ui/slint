# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import logging
import pathlib
from collections.abc import Iterator
from typing import Any

import pytest

from slint import CompileError, load_file


@pytest.fixture
def no_diagnostics(caplog: pytest.LogCaptureFixture) -> Iterator[None]:
    caplog.set_level(logging.DEBUG, logger="slint")
    yield
    assert not [
        record for record in caplog.get_records("call") if record.name == "slint"
    ]


def project_directory(tmp_path: pathlib.Path, settings: dict[str, Any]) -> pathlib.Path:
    # resolve() so the directory matches the paths the compiler reports back.
    directory = tmp_path.resolve()
    (directory / "slint-project.json").write_text(
        json.dumps({"edition": "1.19", **settings})
    )
    return directory


def write_main(directory: pathlib.Path, source: str) -> pathlib.Path:
    main = directory / "main.slint"
    main.write_text(source)
    return main


@pytest.mark.parametrize("project_input", ["directory", "manifest"])
def test_include_paths_come_from_the_project_file(
    tmp_path: pathlib.Path,
    no_diagnostics: None,
    project_input: str,
) -> None:
    directory = project_directory(
        tmp_path, {"main": "main.slint", "include-paths": ["include"]}
    )
    (directory / "include").mkdir()
    (directory / "include" / "shared.slint").write_text("export component Shared { }")

    write_main(
        directory,
        """import { Shared } from "shared.slint";
           export component Main inherits Window { Shared { } }""",
    )

    module = load_file(
        directory if project_input == "directory" else directory / "slint-project.json"
    )
    assert module.Main is not None


@pytest.mark.parametrize("project_input", ["directory", "manifest"])
def test_library_paths_come_from_the_project_file(
    tmp_path: pathlib.Path, no_diagnostics: None, project_input: str
) -> None:
    directory = project_directory(
        tmp_path, {"main": "main.slint", "library-paths": {"widgets": "widgets.slint"}}
    )
    (directory / "widgets.slint").write_text("export component Widget { }")

    write_main(
        directory,
        """import { Widget } from "@widgets";
           export component Main inherits Window { Widget { } }""",
    )

    module = load_file(
        directory if project_input == "directory" else directory / "slint-project.json"
    )
    assert module.Main is not None


def test_an_invalid_project_file_is_reported(tmp_path: pathlib.Path) -> None:
    directory = tmp_path.resolve()
    (directory / "slint-project.json").write_text("{")
    write_main(directory, "export component Main inherits Window { }")

    with pytest.raises(CompileError) as error:
        load_file(directory)
    assert any(
        "slint-project.json" in diagnostic.message
        for diagnostic in error.value.diagnostics
    )


@pytest.mark.parametrize("project_input", ["directory", "manifest"])
def test_building_the_project_directory_builds_its_main_file(
    tmp_path: pathlib.Path,
    no_diagnostics: None,
    project_input: str,
) -> None:
    directory = project_directory(
        tmp_path, {"main": "ui/main.slint", "include-paths": ["include"]}
    )
    (directory / "include").mkdir()
    (directory / "include" / "shared.slint").write_text("export component Shared { }")
    (directory / "ui").mkdir()
    (directory / "ui" / "main.slint").write_text(
        """import { Shared } from "shared.slint";
           export component Main inherits Window { Shared { } }"""
    )

    module = load_file(
        directory if project_input == "directory" else directory / "slint-project.json"
    )
    instance = module.Main()
    assert instance is not None


def test_building_a_file_ignores_its_project_file(
    tmp_path: pathlib.Path, caplog: pytest.LogCaptureFixture
) -> None:
    directory = tmp_path.resolve()
    (directory / "slint-project.json").write_text("{")
    main = write_main(directory, "export component Main inherits Window { }")

    caplog.set_level(logging.DEBUG, logger="slint")
    module = load_file(main)
    assert module.Main is not None
    assert [record.levelno for record in caplog.records if record.name == "slint"] == [
        logging.INFO
    ]
