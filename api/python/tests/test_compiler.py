# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

import pytest
from slint import slint as native
from slint.slint import ValueType;

def test_basic_compiler():
    compiler = native.ComponentCompiler()

    assert compiler.include_paths == []
    compiler.include_paths = ["testing"]
    assert compiler.include_paths == ["testing"]

    assert compiler.build_from_source("Garbage", "") == None

    compdef = compiler.build_from_source("""
        export global TestGlobal {
            in property <string> theglobalprop;
            callback globallogic();
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
        }
    """, "")
    assert compdef != None

    assert compdef.name == "Test"

    props = [(name, type) for name, type in compdef.properties.items()]
    assert props == [('boolprop', ValueType.Bool), ('brushprop', ValueType.Brush), ('colprop', ValueType.Brush), ('floatprop', ValueType.Number), ('imgprop', ValueType.Image), ('intprop', ValueType.Number), ('modelprop', ValueType.Model), ('strprop', ValueType.String)]

    assert compdef.callbacks == ["test-callback"]

    assert compdef.globals == ["TestGlobal"]

    assert compdef.global_properties("Garbage") == None
    assert [(name, type) for name, type in compdef.global_properties("TestGlobal").items()] == [('theglobalprop', ValueType.String)]

    assert compdef.global_callbacks("Garbage") == None
    assert compdef.global_callbacks("TestGlobal") == ["globallogic"]

    instance = compdef.create()
    assert instance != None

def test_compiler_build_from_path():
    compiler = native.ComponentCompiler()

    assert len(compiler.diagnostics) == 0

    assert compiler.build_from_path("Nonexistent.slint") == None
    diags = compiler.diagnostics
    assert len(diags) == 1

    assert diags[0].level == native.DiagnosticLevel.Error
    assert diags[0].message.startswith("Could not load Nonexistent.slint:")
