# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from slint import load_file
import slint
import pytest
from pathlib import Path


def base_dir() -> Path:
    origin = __spec__.origin
    assert origin is not None
    base_dir = Path(origin).parent
    assert base_dir is not None
    return base_dir


def test_callback_decorators(caplog: pytest.LogCaptureFixture) -> None:
    module = load_file(base_dir() / "test-load-file.slint", quiet=False)

    class SubClass(module.App):  # type: ignore
        @slint.callback()
        def say_hello_again(self, arg: str) -> str:
            return "say_hello_again:" + arg

        @slint.callback(name="say-hello")
        def renamed(self, arg: str) -> str:
            return "renamed:" + arg

        @slint.callback(global_name="MyGlobal", name="global-callback")
        def global_callback(self, arg: str) -> str:
            return "global:" + arg

    instance = SubClass()
    assert instance.invoke_say_hello("ok") == "renamed:ok"
    assert instance.invoke_say_hello_again("ok") == "say_hello_again:ok"
    assert instance.invoke_global_callback("ok") == "global:ok"
    del instance
