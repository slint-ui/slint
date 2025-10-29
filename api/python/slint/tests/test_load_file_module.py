# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

from importlib import import_module, reload
from types import ModuleType


def _module() -> ModuleType:
    """
    Return a fresh instance of the generated module for each call.

    Using a dynamic import keeps mypy from requiring type information for the
    generated module, while runtime callers still get the reloaded module.
    """
    module = import_module("test_load_file_source")
    return reload(module)


def test_codegen_module_exports() -> None:
    module: ModuleType = _module()

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
    module: ModuleType = _module()

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
    module: ModuleType = _module()

    def early_say_hello(arg: str) -> str:
        return "early:" + arg

    instance = module.App(hello="Set early", say_hello=early_say_hello)

    assert instance.hello == "Set early"
    assert instance.invoke_say_hello("test") == "early:test"

    del instance
