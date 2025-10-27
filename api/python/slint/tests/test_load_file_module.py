# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import importlib
from types import ModuleType
from typing import TYPE_CHECKING

import test_load_file_source as generated_module


def _module():
    if TYPE_CHECKING:
        return generated_module

    # Reload to ensure a fresh module for each call
    return importlib.reload(generated_module)


def test_codegen_module_exports() -> None:
    module = _module()

    expected_exports = {
        "App",
        "Diag",
        "MyDiag",
        "MyData",
        "Secret_Struct",
        "Public_Struct",
        "TestEnum",
    }
    assert expected_exports.issubset(set(module.__all__))

    assert module.MyDiag is module.Diag
    assert module.Public_Struct is module.Secret_Struct

    test_enum = module.TestEnum
    assert test_enum.Variant1.name == "Variant1"

    instance = module.App()
    del instance

    struct_instance = module.MyData()
    struct_instance.name = "Test"
    struct_instance.age = 42

    struct_instance = module.MyData(name="testing")
    assert struct_instance.name == "testing"


def test_generated_module_wrapper() -> None:
    module = _module()

    instance = module.App()

    assert instance.hello == "World"
    instance.hello = "Ok"
    assert instance.hello == "Ok"

    instance.say_hello = lambda x: "from here: " + x
    assert instance.say_hello("wohoo") == "from here: wohoo"

    assert instance.plus_one(42) == 43

    assert instance.MyGlobal.global_prop == "This is global"
    assert instance.MyGlobal.minus_one(100) == 99
    assert instance.SecondGlobal.second == "second"

    del instance


def test_constructor_kwargs() -> None:
    module = _module()

    def early_say_hello(arg: str) -> str:
        return "early:" + arg

    instance = module.App(hello="Set early", say_hello=early_say_hello)

    assert instance.hello == "Set early"
    assert instance.invoke_say_hello("test") == "early:test"

    del instance
