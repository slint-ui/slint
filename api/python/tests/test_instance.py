# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
from slint import slint as native
from slint.slint import ValueType, PyImage
import os

Color = native.PyColor
Brush = native.PyBrush


def test_property_access():
    compiler = native.Compiler()

    compdef = compiler.build_from_source("""
        export global TestGlobal {
            in property <string> theglobalprop: "Hey";
            callback globallogic();
        }

        export struct MyStruct {
            title: string,
            finished: bool,
            dash-prop: bool,
        }

        export component Test {
            in property <string> strprop: "Hello";
            in property <int> intprop: 42;
            in property <float> floatprop: 100;
            in property <bool> boolprop: true;
            in property <image> imgprop;
            in property <brush> brushprop: Colors.rgb(255, 0, 255);
            in property <color> colprop: Colors.rgb(0, 255, 0);
            in property <[string]> modelprop;
            in property <MyStruct> structprop: {
                title: "builtin",
                finished: true,
                dash-prop: true,
            };
            in property <image> imageprop: @image-url("../../../demos/printerdemo/ui/images/cat.jpg");

            callback test-callback();
        }
    """, os.path.join(os.path.dirname(__file__), "main.slint")).component("Test")
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

    structval = instance.get_property("structprop")
    assert isinstance(structval, native.PyStruct)
    assert structval.title == "builtin"
    assert structval.finished == True
    assert structval.dash_prop == True
    instance.set_property(
        "structprop", {'title': 'new', 'finished': False, 'dash_prop': False})
    structval = instance.get_property("structprop")
    assert structval.title == "new"
    assert structval.finished == False
    assert structval.dash_prop == False

    imageval = instance.get_property("imageprop")
    assert imageval.width == 320
    assert imageval.height == 480
    assert "cat.jpg" in imageval.path

    with pytest.raises(RuntimeError, match="The image cannot be loaded"):
        PyImage.load_from_path("non-existent.png")

    instance.set_property("imageprop", PyImage.load_from_path(os.path.join(
        os.path.dirname(__file__), "../../../examples/iot-dashboard/images/humidity.png")))
    imageval = instance.get_property("imageprop")
    assert imageval.size == (36, 36)
    assert "humidity.png" in imageval.path

    with pytest.raises(TypeError, match="'int' object cannot be converted to 'PyString'"):
        instance.set_property("structprop", {42: 'wrong'})

    brushval = instance.get_property("brushprop")
    assert str(brushval.color) == "argb(255, 255, 0, 255)"
    instance.set_property("brushprop", Brush(Color("rgb(128, 128, 128)")))
    brushval = instance.get_property("brushprop")
    assert str(brushval.color) == "argb(255, 128, 128, 128)"

    brushval = instance.get_property("colprop")
    assert str(brushval.color) == "argb(255, 0, 255, 0)"
    instance.set_property("colprop", Color("rgb(128, 128, 128)"))
    brushval = instance.get_property("colprop")
    assert str(brushval.color) == "argb(255, 128, 128, 128)"

    with pytest.raises(ValueError, match="no such property"):
        instance.set_global_property("nonexistent", "theglobalprop", 42)
    with pytest.raises(ValueError, match="no such property"):
        instance.set_global_property("TestGlobal", "nonexistent", 42)

    assert instance.get_global_property("TestGlobal", "theglobalprop") == "Hey"
    instance.set_global_property("TestGlobal", "theglobalprop", "Ok")
    assert instance.get_global_property("TestGlobal", "theglobalprop") == "Ok"


def test_callbacks():
    compiler = native.Compiler()

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
            callback void-callback();
        }
    """, "").component("Test")
    assert compdef != None

    instance = compdef.create()
    assert instance != None

    assert instance.invoke("test-callback", "foo") == "local foo"

    assert instance.invoke_global(
        "TestGlobal", "globallogic", "foo") == "global foo"

    with pytest.raises(RuntimeError, match="no such callback"):
        instance.set_callback("non-existent", lambda x: x)

    instance.set_callback("test-callback", lambda x: "python " + x)
    assert instance.invoke("test-callback", "foo") == "python foo"

    with pytest.raises(RuntimeError, match="no such callback"):
        instance.set_global_callback("TestGlobal", "non-existent", lambda x: x)

    instance.set_global_callback(
        "TestGlobal", "globallogic", lambda x: "python global " + x)
    assert instance.invoke_global(
        "TestGlobal", "globallogic", "foo") == "python global foo"

    instance.set_callback("void-callback", lambda: None)
    instance.invoke("void-callback")


if __name__ == "__main__":
    import slint
    module = slint.load_file(
        "../../demos/printerdemo/ui/printerdemo.slint")
    instance = module.MainWindow()
    instance.PrinterQueue.start_job = lambda title: print(
        f"new print job {title}")
    instance.run()
