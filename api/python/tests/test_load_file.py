# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

import pytest
from slint import load_file, CompileError
import os


def test_load_file(caplog):
    module = load_file(os.path.join(os.path.dirname(
        __spec__.origin), "test_load_file.slint"), quiet=False)

    assert "The property 'color' has been deprecated. Please use 'background' instead" in caplog.text

    assert list(module.__dict__.keys()) == ["App"]
    instance = module.App()
    del instance


def test_load_file_fail():
    with pytest.raises(CompileError, match="Could not compile non-existent.slint"):
        load_file("non-existent.slint")


def test_load_file_wrapper():
    module = load_file(os.path.join(os.path.dirname(
        __spec__.origin), "test_load_file.slint"), quiet=False)

    instance = module.App()

    assert instance.hello == "World"
    instance.hello = "Ok"
    assert instance.hello == "Ok"

    instance.say_hello = lambda x: "from here: " + x
    assert instance.say_hello("wohoo") == "from here: wohoo"

    assert instance.MyGlobal.global_prop == "This is global"

    del instance


def test_constructor_kwargs():
    module = load_file(os.path.join(os.path.dirname(
        __spec__.origin), "test_load_file.slint"), quiet=False)

    def early_say_hello(arg):
        return "early:" + arg

    instance = module.App(hello="Set early", say_hello=early_say_hello)

    assert instance.hello == "Set early"
    assert instance.invoke_say_hello("test") == "early:test"

    del instance
