# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from slint import slint as native
import weakref
import gc
import typing
from pathlib import Path


def test_callback_gc() -> None:
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
        export component Test {
            out property <string> test-value: "Ok";
            callback test-callback(string) -> string;
        }
    """,
        Path(""),
    ).component("Test")
    assert compdef is not None

    instance: native.ComponentInstance | None = compdef.create()
    assert instance is not None

    class Handler:
        def __init__(self, instance: native.ComponentInstance) -> None:
            self.instance = instance

        def python_callback(self, input: str) -> str:
            return input + typing.cast(str, self.instance.get_property("test-value"))

    handler: Handler | None = Handler(instance)
    assert handler is not None
    instance.set_callback("test-callback", handler.python_callback)
    handler = None

    assert instance.invoke("test-callback", "World") == "WorldOk"

    wr = weakref.ref(instance)
    assert wr() is not None
    instance = None
    gc.collect()
    assert wr() is None
