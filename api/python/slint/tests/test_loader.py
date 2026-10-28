# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import os
import pathlib
import sys

import pytest

from slint import CompileError, SlintAutoLoader, loader


def test_magic_import() -> None:
    instance = loader.test_load_file.App()
    del instance


def test_magic_import_path() -> None:
    oldsyspath = sys.path
    assert loader.printerdemo is None
    try:
        sys.path.append(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
        instance = loader.demos.printerdemo.ui.printerdemo.MainWindow()
        del instance
    finally:
        sys.path = oldsyspath


def write_project(directory: pathlib.Path) -> None:
    (directory / "slint-project.json").write_text(
        json.dumps(
            {
                "slint-edition": "1.19",
                "main": "main.slint",
                "include-paths": ["include"],
            }
        )
    )
    (directory / "include").mkdir()
    (directory / "include" / "shared.slint").write_text(
        'export global Shared { out property <string> text: "shared"; }'
    )
    (directory / "main.slint").write_text(
        """import { Shared } from "shared.slint";
           export component Main inherits Window { out property <string> text: Shared.text; }"""
    )


def test_slint_project_loads_the_project_file(tmp_path: pathlib.Path) -> None:
    (tmp_path / "ui").mkdir()
    write_project(tmp_path / "ui")

    instance = SlintAutoLoader(tmp_path).ui.slint_project.Main()
    assert instance.text == "shared"
    del instance


def test_slint_project_wins_over_a_slint_file(tmp_path: pathlib.Path) -> None:
    write_project(tmp_path)
    (tmp_path / "slint-project.slint").write_text("export component Other { }")

    assert hasattr(SlintAutoLoader(tmp_path).slint_project, "Main")


def test_slint_project_falls_back_to_a_slint_file(tmp_path: pathlib.Path) -> None:
    (tmp_path / "slint-project.slint").write_text("export component Other { }")

    assert hasattr(SlintAutoLoader(tmp_path).slint_project, "Other")


def test_a_file_next_to_the_project_file_ignores_it(tmp_path: pathlib.Path) -> None:
    write_project(tmp_path)

    with pytest.raises(CompileError):
        _ = SlintAutoLoader(tmp_path).main
