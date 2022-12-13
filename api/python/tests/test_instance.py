# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import pytest
import slint
from slint import ValueType;

def test_property_access():
    compiler = slint.ComponentCompiler()

    compdef = compiler.build_from_source("""
        export global TestGlobal {
            in property <string> theglobalprop: "Hey";
            callback globallogic();
        }

        export component Test {
            in property <string> strprop: "Hello";
            in property <int> intprop: 42;
            in property <float> floatprop: 100;
            in property <bool> boolprop: true;
            in property <image> imgprop;
            in property <brush> brushprop;
            in property <color> colprop;
            in property <[string]> modelprop;

            callback test-callback();
        }
    """, "")
    assert compdef != None

    instance = compdef.create()
    assert instance != None

    with pytest.raises(ValueError, match="no such property"):
        instance.set_property("nonexistent", 42)

    assert instance.get_property("strprop") == "Hello"
    instance.set_property("strprop", "World")
    assert instance.get_property("strprop") == "World"
    with pytest.raises(ValueError, match="wrong type"):
        instance.set_property("strprop", 42)

    assert instance.get_property("intprop") == 42
    instance.set_property("intprop", 100)
    assert instance.get_property("intprop") == 100
    with pytest.raises(ValueError, match="wrong type"):
        instance.set_property("intprop", False)

    assert instance.get_property("floatprop") == 100
    instance.set_property("floatprop", 42)
    assert instance.get_property("floatprop") == 42
    with pytest.raises(ValueError, match="wrong type"):
        instance.set_property("floatprop", "Blah")

    assert instance.get_property("boolprop") == True
    instance.set_property("boolprop", False)
    assert instance.get_property("boolprop") == False
    with pytest.raises(ValueError, match="wrong type"):
        instance.set_property("boolprop", 0)

    with pytest.raises(ValueError, match="no such property"):
        instance.set_global_property("nonexistent", "theglobalprop", 42)
    with pytest.raises(ValueError, match="no such property"):
        instance.set_global_property("TestGlobal", "nonexistent", 42)

    assert instance.get_global_property("TestGlobal", "theglobalprop") == "Hey"
    instance.set_global_property("TestGlobal", "theglobalprop", "Ok")
    assert instance.get_global_property("TestGlobal", "theglobalprop") == "Ok"

def test_callbacks():
    compiler = slint.ComponentCompiler()

    compdef = compiler.build_from_source("""
        export global TestGlobal {
            callback globallogic(string) -> string;
            globallogic(value) => {
                return "global " + value;
            }
        }

        export component Test {
            callback test-callback(string) -> string;
            test-callback(value) => {
                return "local " + value;
            }
        }
    """, "")
    assert compdef != None

    instance = compdef.create()
    assert instance != None

    assert instance.invoke("test-callback", "foo") == "local foo"

    assert instance.invoke_global("TestGlobal", "globallogic", "foo") == "global foo"

    with pytest.raises(RuntimeError, match="no such callback"):
        instance.set_callback("non-existent", lambda x: x)

    instance.set_callback("test-callback", lambda x: "python " + x)
    assert instance.invoke("test-callback", "foo") == "python foo"

    with pytest.raises(RuntimeError, match="no such callback"):
        instance.set_global_callback("TestGlobal", "non-existent", lambda x: x)

    instance.set_global_callback("TestGlobal", "globallogic", lambda x: "python global " + x)
    assert instance.invoke_global("TestGlobal", "globallogic", "foo") == "python global foo"
