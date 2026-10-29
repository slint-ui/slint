# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import pathlib
from typing import Any

from slint import slint as native


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


def messages(result: native.CompilationResult) -> list[str]:
    return [diagnostic.message for diagnostic in result.diagnostics]


def test_include_paths_come_from_the_project_file(
    tmp_path: pathlib.Path,
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

    result = native.Compiler().build_from_path(directory)
    assert messages(result) == []
    assert "Main" in result.component_names


def test_library_paths_come_from_the_project_file(tmp_path: pathlib.Path) -> None:
    directory = project_directory(
        tmp_path, {"main": "main.slint", "library-paths": {"widgets": "widgets.slint"}}
    )
    (directory / "widgets.slint").write_text("export component Widget { }")

    write_main(
        directory,
        """import { Widget } from "@widgets";
           export component Main inherits Window { Widget { } }""",
    )

    result = native.Compiler().build_from_path(directory)
    assert messages(result) == []
    assert "Main" in result.component_names


def test_an_invalid_project_file_is_reported(tmp_path: pathlib.Path) -> None:
    directory = tmp_path.resolve()
    (directory / "slint-project.json").write_text("{")
    write_main(directory, "export component Main inherits Window { }")

    result = native.Compiler().build_from_path(directory)
    assert any("slint-project.json" in message for message in messages(result))


def test_building_the_project_directory_builds_its_main_file(
    tmp_path: pathlib.Path,
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

    result = native.Compiler().build_from_path(directory)
    assert messages(result) == []
    assert "Main" in result.component_names


def test_building_the_project_file_is_an_error(tmp_path: pathlib.Path) -> None:
    directory = project_directory(tmp_path, {"main": "main.slint"})
    write_main(directory, "export component Main inherits Window { }")

    result = native.Compiler().build_from_path(directory / "slint-project.json")
    assert messages(result) == [
        f"A `slint-project.json` cannot be compiled directly; pass its directory instead ({directory})"
    ]


def test_building_a_file_ignores_its_project_file(tmp_path: pathlib.Path) -> None:
    directory = tmp_path.resolve()
    (directory / "slint-project.json").write_text("{")
    main = write_main(directory, "export component Main inherits Window { }")

    result = native.Compiler().build_from_path(main)
    assert messages(result) == []
