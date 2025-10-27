# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from typing import Any

import pytest
from slint import ListModel, core
from slint.codegen.generator import generate_project
from slint.codegen.models import GenerationConfig
from slint.core import TextHorizontalAlignment, TextVerticalAlignment


def _slint_source() -> Path:
    return Path(__file__).with_name("test-load-file-source.slint")


@pytest.fixture
def generated_module(tmp_path: Path) -> Any:
    slint_file = _slint_source()
    output_dir = tmp_path / "generated"
    config = GenerationConfig(
        include_paths=[slint_file.parent],
        library_paths={},
        style=None,
        translation_domain=None,
        quiet=True,
    )
    generate_project(inputs=[slint_file], output_dir=output_dir, config=config)

    module_path = output_dir / "test_load_file_source.py"
    assert module_path.exists()

    spec = importlib.util.spec_from_file_location("generated_test_load_file", module_path)
    assert spec and spec.loader

    sys.modules.pop(spec.name, None)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[arg-type]
    return module


def test_enums(generated_module: Any) -> None:
    module = generated_module

    TestEnum = module.TestEnum

    assert TestEnum.Variant1.name == "Variant1"
    assert TestEnum.Variant1.value == "Variant1"
    assert TestEnum.Variant2.name == "Variant2"
    assert TestEnum.Variant2.value == "Variant2"
    with pytest.raises(
        AttributeError, match="type object 'TestEnum' has no attribute 'Variant3'"
    ):
        TestEnum.Variant3

    instance = module.App()
    assert instance.enum_property == TestEnum.Variant2
    assert instance.enum_property.__class__ is TestEnum
    instance.enum_property = TestEnum.Variant1
    assert instance.enum_property == TestEnum.Variant1
    assert instance.enum_property.__class__ is TestEnum

    model_with_enums = instance.model_with_enums
    assert len(model_with_enums) == 2
    assert model_with_enums[0] == TestEnum.Variant2
    assert model_with_enums[1] == TestEnum.Variant1
    assert model_with_enums[0].__class__ is TestEnum
    model_with_enums = None  # allow GC to drop reference

    instance.model_with_enums = ListModel([TestEnum.Variant1, TestEnum.Variant2])
    assert len(instance.model_with_enums) == 2
    assert instance.model_with_enums[0] == TestEnum.Variant1
    assert instance.model_with_enums[1] == TestEnum.Variant2
    assert instance.model_with_enums[0].__class__ is TestEnum
    del instance


def test_builtin_enums_exposed() -> None:
    assert TextHorizontalAlignment.left.name == "left"
    assert TextVerticalAlignment.top.name == "top"
    assert TextHorizontalAlignment.left != TextHorizontalAlignment.right


def test_builtin_enum_property_roundtrip() -> None:
    compiler = core.Compiler()
    comp = compiler.build_from_source(
        """
        export component Test {
            in-out property <TextHorizontalAlignment> horizontal: TextHorizontalAlignment.left;
            in-out property <TextVerticalAlignment> vertical: TextVerticalAlignment.top;
            Text {
                horizontal-alignment: root.horizontal;
                vertical-alignment: root.vertical;
            }
        }
        """,
        Path(""),
    ).component("Test")

    assert comp is not None
    instance = comp.create()
    assert instance is not None

    assert instance.get_property("horizontal") == TextHorizontalAlignment.left
    assert instance.get_property("vertical") == TextVerticalAlignment.top

    instance.set_property("horizontal", TextHorizontalAlignment.right)
    instance.set_property("vertical", TextVerticalAlignment.bottom)

    assert instance.get_property("horizontal") == TextHorizontalAlignment.right
    assert instance.get_property("vertical") == TextVerticalAlignment.bottom


def test_builtin_enum_keyword_variants_have_safe_names() -> None:
    keyword_enums = (
        core.AccessibleRole,
        core.DialogButtonRole,
    )

    for enum_cls in keyword_enums:
        members = enum_cls.__members__
        assert "none" in members
        assert members["none"].value == "none"
