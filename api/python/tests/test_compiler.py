# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
from slint import slint as native
from slint.slint import ValueType


def test_basic_compiler():
    compiler = native.Compiler()

    assert compiler.include_paths == []
    compiler.include_paths = ["testing"]
    assert compiler.include_paths == ["testing"]

    assert len(compiler.build_from_source("Garbage", "").component_names) == 0

    result = compiler.build_from_source("""
        export global TestGlobal {
            in property <string> theglobalprop;
            callback globallogic();
            public function globalfun() {}
        }

        export component Test {
            in property <string> strprop;
            in property <int> intprop;
            in property <float> floatprop;
            in property <bool> boolprop;
            in property <image> imgprop;
            in property <brush> brushprop;
            in property <color> colprop;
            in property <[string]> modelprop;

            callback test-callback();
            public function ff() {}
        }
    """, "")
    assert result.component_names == ["Test"]
    compdef = result.component("Test")

    assert compdef != None

    assert compdef.name == "Test"

    props = [(name, type) for name, type in compdef.properties.items()]
    assert props == [('boolprop', ValueType.Bool), ('brushprop', ValueType.Brush), ('colprop', ValueType.Brush), ('floatprop', ValueType.Number),
                     ('imgprop', ValueType.Image), ('intprop', ValueType.Number), ('modelprop', ValueType.Model), ('strprop', ValueType.String)]

    assert compdef.callbacks == ["test-callback"]
    assert compdef.functions == ["ff"]

    assert compdef.globals == ["TestGlobal"]

    assert compdef.global_properties("Garbage") == None
    assert [(name, type) for name, type in compdef.global_properties(
        "TestGlobal").items()] == [('theglobalprop', ValueType.String)]

    assert compdef.global_callbacks("Garbage") == None
    assert compdef.global_callbacks("TestGlobal") == ["globallogic"]

    assert compdef.global_functions("Garbage") == None
    assert compdef.global_functions("TestGlobal") == ["globalfun"]

    instance = compdef.create()
    assert instance != None


def test_compiler_build_from_path():
    compiler = native.Compiler()

    result = compiler.build_from_path("Nonexistent.slint")
    assert len(result.component_names) == 0

    diags = result.diagnostics
    assert len(diags) == 1

    assert diags[0].level == native.DiagnosticLevel.Error
    assert diags[0].message.startswith("Could not load Nonexistent.slint:")
