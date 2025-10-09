# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
from slint import load_file, CompileError
from pathlib import Path


def base_dir() -> Path:
    origin = __spec__.origin
    assert origin is not None
    base_dir = Path(origin).parent
    assert base_dir is not None
    return base_dir


def test_load_file(caplog: pytest.LogCaptureFixture) -> None:
    module = load_file(base_dir() / "test-load-file.slint", quiet=False)

    assert (
        "The property 'color' has been deprecated. Please use 'background' instead"
        in caplog.text
    )

    assert len(list(module.__dict__.keys())) == 7
    assert "App" in module.__dict__
    assert "Diag" in module.__dict__
    assert "MyDiag" in module.__dict__
    assert "MyData" in module.__dict__
    assert "Secret_Struct" in module.__dict__
    assert "Public_Struct" in module.__dict__
    assert "TestEnum" in module.__dict__
    instance = module.App()
    del instance
    instance = module.MyDiag()
    del instance

    struct_instance = module.MyData()
    struct_instance.name = "Test"
    struct_instance.age = 42

    struct_instance = module.MyData(name="testing")
    assert struct_instance.name == "testing"

    assert module.Public_Struct is module.Secret_Struct
    assert module.MyDiag is module.Diag


def test_load_file_fail() -> None:
    with pytest.raises(CompileError, match="Could not compile non-existent.slint"):
        load_file("non-existent.slint")


def test_compile_error() -> None:
    with pytest.raises(CompileError) as excinfo:
        load_file(base_dir() / "test-file-error.slint")
    err = excinfo.value
    diagnostics = err.diagnostics
    assert len(diagnostics) == 2
    assert diagnostics[0].message == "Unknown type 'garbage'"
    assert diagnostics[0].line_number == 6
    assert diagnostics[0].column_number == 15
    assert diagnostics[1].message == "Unknown element 'NewType'"
    assert diagnostics[1].line_number == 7
    assert diagnostics[1].column_number == 5
    path = base_dir() / "test-file-error.slint"
    assert str(err) == f"Could not compile {path}"
    assert len(err.__notes__) == 2
    assert err.__notes__[0] == f"{path}:6: Unknown type 'garbage'"
    assert err.__notes__[1] == f"{path}:7: Unknown element 'NewType'"


def test_load_file_wrapper() -> None:
    module = load_file(base_dir() / "test-load-file.slint", quiet=False)

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
    module = load_file(base_dir() / "test-load-file.slint", quiet=False)

    def early_say_hello(arg: str) -> str:
        return "early:" + arg

    instance = module.App(hello="Set early", say_hello=early_say_hello)

    assert instance.hello == "Set early"
    assert instance.invoke_say_hello("test") == "early:test"

    del instance
