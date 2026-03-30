// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import Testing
import SlintInterpreter

// MARK: - Shared helpers

private func compile(_ source: String) -> SlintComponentDefinition? {
    let compiler = SlintCompiler()
    return compiler.buildFromSource(source, path: "test.slint")
}

// MARK: - SlintValue tests

@Suite("SlintValue")
struct SlintValueTests {

    // MARK: Constructors

    @Test func voidValue() {
        let v = SlintValue()
        #expect(v.valueType == .void)
    }

    @Test func numberValue() {
        let v = SlintValue(3.14)
        #expect(v.valueType == .number)
        #expect(v.asDouble == 3.14)
    }

    @Test func boolTrueValue() {
        let v = SlintValue(true)
        #expect(v.valueType == .bool)
        #expect(v.asBool == true)
    }

    @Test func boolFalseValue() {
        let v = SlintValue(false)
        #expect(v.asBool == false)
    }

    @Test func stringValue() {
        let v = SlintValue("hello")
        #expect(v.valueType == .string)
        #expect(v.asString == "hello")
    }

    @Test func integerLiteralCoercion() {
        let v: SlintValue = 42
        #expect(v.asDouble == 42.0)
    }

    @Test func floatLiteralCoercion() {
        let v: SlintValue = 1.5
        #expect(v.asDouble == 1.5)
    }

    @Test func boolLiteralCoercion() {
        let v: SlintValue = true
        #expect(v.asBool == true)
    }

    @Test func stringLiteralCoercion() {
        let v: SlintValue = "world"
        #expect(v.asString == "world")
    }

    // MARK: Wrong-type extractors return nil

    @Test func numberExtractorOnBool() {
        let v = SlintValue(true)
        #expect(v.asDouble == nil)
    }

    @Test func stringExtractorOnNumber() {
        let v = SlintValue(1.0)
        #expect(v.asString == nil)
    }

    @Test func boolExtractorOnString() {
        let v = SlintValue("yes")
        #expect(v.asBool == nil)
    }

    // MARK: Clone

    @Test func cloneProducesEqualValue() {
        let original = SlintValue(99.0)
        let copy = original.clone()
        #expect(copy.asDouble == 99.0)
    }
}

// MARK: - SlintStruct tests

@Suite("SlintStruct")
struct SlintStructTests {

    @Test func emptyStructHasZeroFields() {
        let s = SlintStruct()
        #expect(s.fieldCount == 0)
    }

    @Test func setAndGetField() {
        let s = SlintStruct()
        s.setField("count", value: SlintValue(5.0))
        #expect(s.getField("count")?.asDouble == 5.0)
    }

    @Test func getMissingFieldReturnsNil() {
        let s = SlintStruct()
        #expect(s.getField("missing") == nil)
    }

    @Test func fieldCountUpdates() {
        let s = SlintStruct()
        s.setField("a", value: SlintValue(1.0))
        s.setField("b", value: SlintValue(2.0))
        #expect(s.fieldCount == 2)
    }

    @Test func fieldNamesContainSetKeys() {
        let s = SlintStruct()
        s.setField("x", value: SlintValue(0.0))
        s.setField("y", value: SlintValue(0.0))
        let names = s.fieldNames
        #expect(names.contains("x"))
        #expect(names.contains("y"))
    }

    @Test func subscriptGet() {
        let s = SlintStruct()
        s.setField("active", value: SlintValue(true))
        #expect(s["active"]?.asBool == true)
    }

    @Test func subscriptSet() {
        let s = SlintStruct()
        s["label"] = SlintValue("hi")
        #expect(s["label"]?.asString == "hi")
    }

    @Test func cloneIsIndependent() {
        let s = SlintStruct()
        s.setField("n", value: SlintValue(1.0))
        let copy = s.clone()
        copy.setField("n", value: SlintValue(99.0))
        #expect(s.getField("n")?.asDouble == 1.0)
    }
}

// MARK: - SlintCompiler tests

@Suite("SlintCompiler")
struct SlintCompilerTests {

    @Test func compileMinimalComponent() {
        let compiler = SlintCompiler()
        let def = compiler.buildFromSource(
            "export component Minimal inherits Window {}",
            path: "test.slint"
        )
        #expect(def != nil)
        #expect(!compiler.hasErrors)
        #expect(compiler.diagnostics.isEmpty)
    }

    @Test func compileInvalidSourceReturnsNil() {
        let compiler = SlintCompiler()
        let def = compiler.buildFromSource("not valid slint !!!", path: "bad.slint")
        #expect(def == nil)
        #expect(compiler.hasErrors)
    }

    @Test func diagnosticsContainMessage() {
        let compiler = SlintCompiler()
        _ = compiler.buildFromSource("export component Bad { unknown-property: 0; }", path: "x.slint")
        #expect(!compiler.diagnostics.isEmpty)
    }

    @Test func compiledDefinitionName() {
        let def = compile("export component Hello inherits Rectangle {}")!
        #expect(def.name == "Hello")
    }

    @Test func compiledDefinitionProperties() {
        let source = """
        export component Counter inherits Rectangle {
            in-out property <int> count: 0;
        }
        """
        let def = compile(source)!
        #expect(def.propertiesCount >= 1)
        let names = def.properties.map(\.name)
        #expect(names.contains("count"))
    }

    @Test func compiledDefinitionCallbacks() {
        let source = """
        export component Btn inherits Rectangle {
            callback clicked();
        }
        """
        let def = compile(source)!
        #expect(def.callbacksCount >= 1)
        #expect(def.callbackNames.contains("clicked"))
    }
}

// MARK: - SlintComponentInstance tests

@Suite("SlintComponentInstance")
struct SlintComponentInstanceTests {

    @Test @MainActor func createInstance() {
        let def = compile("export component T inherits Rectangle {}")!
        let inst = def.createInstance()
        #expect(inst != nil)
    }

    @Test @MainActor func getIntProperty() {
        let source = """
        export component T inherits Rectangle {
            in-out property <int> value: 42;
        }
        """
        let inst = compile(source)!.createInstance()!
        let val = inst.getProperty("value")
        #expect(val?.asDouble == 42.0)
    }

    @Test @MainActor func setAndGetProperty() {
        let source = """
        export component T inherits Rectangle {
            in-out property <int> counter: 0;
        }
        """
        let inst = compile(source)!.createInstance()!
        #expect(inst.setProperty("counter", value: SlintValue(7.0)))
        #expect(inst.getProperty("counter")?.asDouble == 7.0)
    }

    @Test @MainActor func getMissingPropertyReturnsNil() {
        let inst = compile("export component T inherits Rectangle {}")!.createInstance()!
        #expect(inst.getProperty("does_not_exist") == nil)
    }

    @Test @MainActor func setOutputPropertyFails() {
        let source = """
        export component T inherits Rectangle {
            out property <int> readonly: 5;
        }
        """
        let inst = compile(source)!.createInstance()!
        // Setting an output property should fail
        #expect(!inst.setProperty("readonly", value: SlintValue(99.0)))
    }

    @Test @MainActor func getBoolProperty() {
        let source = """
        export component T inherits Rectangle {
            in-out property <bool> flag: true;
        }
        """
        let inst = compile(source)!.createInstance()!
        #expect(inst.getProperty("flag")?.asBool == true)
    }

    @Test @MainActor func getStringProperty() {
        let source = """
        export component T inherits Rectangle {
            in-out property <string> title: "hello";
        }
        """
        let inst = compile(source)!.createInstance()!
        #expect(inst.getProperty("title")?.asString == "hello")
    }

    @Test @MainActor func invokeCallback() {
        let source = """
        export component T inherits Rectangle {
            in-out property <int> result: 0;
            callback compute() -> int;
            compute => { return 99; }
        }
        """
        let inst = compile(source)!.createInstance()!
        let retVal = inst.invoke("compute")
        #expect(retVal?.asDouble == 99.0)
    }
}
