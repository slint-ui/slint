# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from slint import slint as native
import weakref
import gc


def test_callback_gc():
    compiler = native.Compiler()

    compdef = compiler.build_from_source("""
        export component Test {
            out property <string> test-value: "Ok";
            callback test-callback(string) -> string;
        }
    """, "").component("Test")
    assert compdef != None

    instance = compdef.create()
    assert instance != None

    class Handler:
        def __init__(self, instance):
            self.instance = instance

        def python_callback(self, input):
            return input + instance.get_property("test-value")

    handler = Handler(instance)
    instance.set_callback(
        "test-callback", handler.python_callback)
    handler = None

    assert instance.invoke("test-callback", "World") == "WorldOk"

    wr = weakref.ref(instance)
    assert wr() is not None
    instance = None
    gc.collect()
    assert wr() is None
