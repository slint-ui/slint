# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import ast
import importlib
import keyword
import sys
import types
import typing
from collections.abc import Iterator
from pathlib import Path

import pytest


def _read_all_symbols(path: Path) -> list[str]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    for node in tree.body:
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name) and target.id == "__all__":
                    return typing.cast(list[str], ast.literal_eval(node.value))
    raise AssertionError(f"Missing __all__ in {path}")


def _collect_generated_imports(path: Path) -> list[str]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    for node in tree.body:
        if (
            isinstance(node, ast.ImportFrom)
            and node.level == 1
            and node.module == "_generated"
        ):
            return [alias.name for alias in node.names]
    return []


@pytest.fixture()
def emitters_modules(
    monkeypatch: pytest.MonkeyPatch,
) -> Iterator[tuple[types.ModuleType, types.ModuleType]]:
    root = Path(__file__).resolve().parents[2]

    slint_pkg = types.ModuleType("slint")
    slint_pkg.__path__ = [str(root / "slint")]
    monkeypatch.setitem(sys.modules, "slint", slint_pkg)

    codegen_pkg = types.ModuleType("slint.codegen")
    codegen_pkg.__path__ = [str(root / "slint" / "codegen")]
    monkeypatch.setitem(sys.modules, "slint.codegen", codegen_pkg)

    api_module = types.ModuleType("slint.api")

    def _normalize_prop(name: str) -> str:
        ident = name.replace("-", "_")
        if ident and ident[0].isdigit():
            ident = f"_{ident}"
        if keyword.iskeyword(ident):
            ident = f"{ident}_"
        return ident

    api_module._normalize_prop = _normalize_prop  # type: ignore[attr-defined]
    monkeypatch.setitem(sys.modules, "slint.api", api_module)

    models = importlib.import_module("slint.codegen.models")
    emitters = importlib.import_module("slint.codegen.emitters")

    module_names = ["slint.codegen.emitters", "slint.codegen.models"]

    try:
        yield emitters, models
    finally:
        for name in module_names:
            sys.modules.pop(name, None)


def test_write_package_emitters(
    tmp_path: Path, emitters_modules: tuple[types.ModuleType, types.ModuleType]
) -> None:
    emitters, models = emitters_modules

    artifacts = models.ModuleArtifacts(
        components=[
            models.ComponentMeta(
                name="AppWindow",
                py_name="AppWindow",
                properties=[],
                callbacks=[],
                functions=[],
                globals=[],
            ),
            models.ComponentMeta(
                name="OtherComponent",
                py_name="OtherComponent",
                properties=[],
                callbacks=[],
                functions=[],
                globals=[],
            ),
        ],
        structs=[
            models.StructMeta(
                name="Config",
                py_name="ConfigStruct",
                fields=[],
                is_builtin=False,
            )
        ],
        enums=[
            models.EnumMeta(
                name="Choice",
                py_name="ChoiceEnum",
                values=[
                    models.EnumValueMeta(
                        name="First",
                        py_name="First",
                        value="first",
                    )
                ],
                is_builtin=False,
                is_used=True,
            ),
            models.EnumMeta(
                name="BuiltinEnum",
                py_name="BuiltinEnum",
                values=[],
                is_builtin=True,
                is_used=True,
            ),
        ],
        named_exports=[
            ("AppWindow", "WindowAlias"),
            ("Config", "ConfigAlias"),
            ("BuiltinEnum", "BuiltinAlias"),
            ("OtherComponent", "other-component"),
            ("Unknown", "UnknownAlias"),
            ("Choice", "ChoiceAlias"),
        ],
        resource_paths=[],
    )

    package_dir = tmp_path / "generated"
    package_dir.mkdir()

    emitters.write_package_init(
        package_dir / "__init__.py",
        source_relative="ui/app.slint",
        artifacts=artifacts,
    )
    emitters.write_package_init_stub(package_dir / "__init__.pyi", artifacts=artifacts)
    emitters.write_package_enums(
        package_dir / "enums.py",
        source_relative="ui/app.slint",
        artifacts=artifacts,
    )
    emitters.write_package_enums_stub(package_dir / "enums.pyi", artifacts=artifacts)
    emitters.write_package_structs(
        package_dir / "structs.py",
        source_relative="ui/app.slint",
        artifacts=artifacts,
    )
    emitters.write_package_structs_stub(
        package_dir / "structs.pyi", artifacts=artifacts
    )

    init_py = (package_dir / "__init__.py").read_text(encoding="utf-8")
    assert init_py.startswith("# Generated by slint.codegen from ui/app.slint")
    assert "from . import enums, structs" in init_py
    assert "BuiltinAlias" not in init_py
    assert "UnknownAlias" not in init_py
    assert "WindowAlias = AppWindow" in init_py
    assert "ConfigAlias = ConfigStruct" in init_py
    assert "other_component = OtherComponent" in init_py
    assert "ChoiceAlias = ChoiceEnum" in init_py
    assert _collect_generated_imports(package_dir / "__init__.py") == [
        "AppWindow",
        "OtherComponent",
        "ConfigStruct",
        "ChoiceEnum",
    ]
    assert _read_all_symbols(package_dir / "__init__.py") == [
        "AppWindow",
        "OtherComponent",
        "ConfigStruct",
        "ChoiceEnum",
        "WindowAlias",
        "ConfigAlias",
        "other_component",
        "ChoiceAlias",
        "enums",
        "structs",
    ]

    init_pyi = (package_dir / "__init__.pyi").read_text(encoding="utf-8")
    assert init_pyi.startswith("from __future__ import annotations")
    assert "# Generated by" not in init_pyi
    assert "BuiltinAlias" not in init_pyi
    assert "UnknownAlias" not in init_pyi
    assert _collect_generated_imports(package_dir / "__init__.pyi") == [
        "AppWindow",
        "OtherComponent",
        "ConfigStruct",
        "ChoiceEnum",
    ]
    assert _read_all_symbols(package_dir / "__init__.pyi") == [
        "AppWindow",
        "OtherComponent",
        "ConfigStruct",
        "ChoiceEnum",
        "WindowAlias",
        "ConfigAlias",
        "other_component",
        "ChoiceAlias",
        "enums",
        "structs",
    ]

    enums_py = (package_dir / "enums.py").read_text(encoding="utf-8")
    assert enums_py.startswith("# Generated by slint.codegen from ui/app.slint")
    assert "BuiltinAlias" not in enums_py
    assert "ChoiceAlias = ChoiceEnum" in enums_py
    assert _collect_generated_imports(package_dir / "enums.py") == ["ChoiceEnum"]
    assert _read_all_symbols(package_dir / "enums.py") == [
        "ChoiceEnum",
        "ChoiceAlias",
    ]

    enums_pyi = (package_dir / "enums.pyi").read_text(encoding="utf-8")
    assert enums_pyi.startswith("from __future__ import annotations")
    assert "# Generated by" not in enums_pyi
    assert "BuiltinAlias" not in enums_pyi
    assert _collect_generated_imports(package_dir / "enums.pyi") == ["ChoiceEnum"]
    assert _read_all_symbols(package_dir / "enums.pyi") == [
        "ChoiceEnum",
        "ChoiceAlias",
    ]

    structs_py = (package_dir / "structs.py").read_text(encoding="utf-8")
    assert structs_py.startswith("# Generated by slint.codegen from ui/app.slint")
    assert "ConfigAlias = ConfigStruct" in structs_py
    assert _collect_generated_imports(package_dir / "structs.py") == ["ConfigStruct"]
    assert _read_all_symbols(package_dir / "structs.py") == [
        "ConfigStruct",
        "ConfigAlias",
    ]

    structs_pyi = (package_dir / "structs.pyi").read_text(encoding="utf-8")
    assert structs_pyi.startswith("from __future__ import annotations")
    assert "# Generated by" not in structs_pyi
    assert _collect_generated_imports(package_dir / "structs.pyi") == ["ConfigStruct"]
    assert _read_all_symbols(package_dir / "structs.pyi") == [
        "ConfigStruct",
        "ConfigAlias",
    ]
