# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import os
import pathlib
from typing import Any

from slint import slint as native


def project_directory(tmp_path: pathlib.Path, settings: dict[str, Any]) -> pathlib.Path:
    # resolve() so the directory matches the paths the compiler reports back.
    directory = tmp_path.resolve()
    (directory / "slint.project.json").write_text(json.dumps(settings))
    return directory


def write_main(directory: pathlib.Path, source: str) -> pathlib.Path:
    main = directory / "main.slint"
    main.write_text(source)
    return main


def messages(result: native.CompilationResult) -> list[str]:
    return [diagnostic.message for diagnostic in result.diagnostics]


def test_include_directories_come_from_the_project_file(
    tmp_path: pathlib.Path,
) -> None:
    directory = project_directory(tmp_path, {"include-directories": ["include"]})
    (directory / "include").mkdir()
    (directory / "include" / "shared.slint").write_text("export component Shared { }")

    main = write_main(
        directory,
        """import { Shared } from "shared.slint";
           export component Main inherits Window { Shared { } }""",
    )

    result = native.Compiler().build_from_path(main)
    assert messages(result) == []
    assert "Main" in result.component_names


def test_library_paths_come_from_the_project_file(tmp_path: pathlib.Path) -> None:
    directory = project_directory(tmp_path, {"library-paths": {"widgets": "widgets.slint"}})
    (directory / "widgets.slint").write_text("export component Widget { }")

    main = write_main(
        directory,
        """import { Widget } from "@widgets";
           export component Main inherits Window { Widget { } }""",
    )

    result = native.Compiler().build_from_path(main)
    assert messages(result) == []
    assert "Main" in result.component_names


def test_the_project_file_style_reaches_the_compiler(tmp_path: pathlib.Path) -> None:
    directory = project_directory(tmp_path, {"style": "no-such-style"})
    main = write_main(directory, "export component Main inherits Window { }")

    # A style name the compiler rejects shows which style it actually used.
    result = native.Compiler().build_from_path(main)
    assert any("no-such-style" in message for message in messages(result))


def test_an_explicit_style_wins_over_the_project_file(tmp_path: pathlib.Path) -> None:
    directory = project_directory(tmp_path, {"style": "no-such-style"})
    main = write_main(directory, "export component Main inherits Window { }")

    compiler = native.Compiler()
    compiler.style = "fluent"
    result = compiler.build_from_path(main)
    assert messages(result) == []


def test_an_invalid_project_file_is_reported(tmp_path: pathlib.Path) -> None:
    directory = tmp_path.resolve()
    (directory / "slint.project.json").write_text("{")
    main = write_main(directory, "export component Main inherits Window { }")

    result = native.Compiler().build_from_path(main)
    assert any("slint.project.json" in message for message in messages(result))


def test_no_project_file_keeps_the_defaults(tmp_path: pathlib.Path) -> None:
    main = write_main(tmp_path.resolve(), "export component Main inherits Window { }")

    result = native.Compiler().build_from_path(main)
    assert messages(result) == []
    assert "Main" in result.component_names


def test_the_style_set_through_the_api_is_still_reported(
    tmp_path: pathlib.Path,
) -> None:
    # The getter reflects what was set, whether or not a project file exists.
    os.environ.pop("SLINT_STYLE", None)
    compiler = native.Compiler()
    compiler.style = "material"
    assert compiler.style == "material"
