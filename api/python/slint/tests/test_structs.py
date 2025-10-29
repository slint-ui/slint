# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import importlib.util
import inspect
import sys
from pathlib import Path

import pytest

import slint.core as core
from slint.codegen.generator import generate_project
from slint.codegen.models import GenerationConfig


def test_builtin_enum_class_helper_not_available() -> None:
    assert not hasattr(core, "built_in_enum_class")
    with pytest.raises(AttributeError):
        getattr(core, "built_in_enum_class")


def test_builtin_struct_factory_not_available() -> None:
    assert not hasattr(core, "built_in_struct_factory")
    with pytest.raises(AttributeError):
        getattr(core, "built_in_struct_factory")


def test_user_structs_exported_and_builtin_hidden() -> None:
    source = inspect.cleandoc(
        """
        export struct Custom {
            value: int,
        }

        export enum CustomEnum {
            first,
            second,
        }

        export global Data {
            in-out property <Custom> custom;
        }

        export component Test inherits Window {
            in-out property <Custom> data <=> Data.custom;
            in-out property <CustomEnum> mode;
            callback pointer_event(event: PointerEvent);
            width: 100px;
            height: 100px;
            TouchArea { }
        }
        """
    )

    compiler = core.Compiler()
    result = compiler.build_from_source(source, Path(""))

    structs, enums = result.structs_and_enums
    assert set(structs.keys()) == {"Custom"}
    assert "PointerEvent" not in structs
    assert set(enums.keys()) == {"CustomEnum"}

    custom_struct_proto = structs["Custom"]
    assert hasattr(custom_struct_proto, "value")

    component = result.component("Test")
    assert component is not None
    instance = component.create()
    assert instance is not None

    instance.set_property("data", {"value": 99})
    data = instance.get_property("data")
    assert hasattr(data, "value")
    assert data.value == 99

    CustomEnum = enums["CustomEnum"]
    instance.set_property("mode", CustomEnum.second)
    assert instance.get_property("mode") == CustomEnum.second


@pytest.fixture
def generated_struct_module(tmp_path: Path):
    slint_file = Path(__file__).with_name("test-load-file-source.slint")
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
    spec = importlib.util.spec_from_file_location("generated_structs", module_path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules.pop(spec.name, None)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[arg-type]
    return module


def test_struct_accepts_keywords_only(generated_struct_module) -> None:
    MyData = generated_struct_module.MyData

    with pytest.raises(TypeError, match="keyword arguments only"):
        MyData("foo", 42)

    instance = MyData(name="foo", age=42)
    assert instance.name == "foo"
    assert instance.age == 42


def test_struct_rejects_unknown_keywords(generated_struct_module) -> None:
    MyData = generated_struct_module.MyData

    with pytest.raises(TypeError, match="unexpected keyword"):  # noqa: PT012
        MyData(name="foo", age=1, extra=True)
