// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";

import { private_api } from "../dist/index.js";
import * as napi from "../rust-module.cjs";

test("get/set include paths", () => {
    const compiler = new private_api.ComponentCompiler();

    expect(compiler.includePaths.length).toBe(0);

    compiler.includePaths = ["path/one/", "path/two/", "path/three/"];

    expect(compiler.includePaths).toStrictEqual([
        "path/one/",
        "path/two/",
        "path/three/",
    ]);
});

test("get/set library paths", () => {
    const compiler = new private_api.ComponentCompiler();

    compiler.libraryPaths = {
        "libfile.slint": "third_party/libfoo/ui/lib.slint",
        libdir: "third_party/libbar/ui/",
    };

    expect(compiler.libraryPaths).toStrictEqual({
        "libfile.slint": "third_party/libfoo/ui/lib.slint",
        libdir: "third_party/libbar/ui/",
    });
});

test("get/set style", () => {
    const compiler = new private_api.ComponentCompiler();

    expect(compiler.style).toBeNull();

    compiler.style = "fluent";
    expect(compiler.style).toBe("fluent");
});

test("get/set build from source", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(`export component App {}`, "");
    expect(definition.App).not.toBeNull();
    expect(definition.App!.name).toBe("App");
});

test("constructor error ComponentDefinition and ComponentInstance", () => {
    let componentDefinitionError: any;
    try {
        new private_api.ComponentDefinition();
    } catch (error) {
        componentDefinitionError = error;
    }
    expect(componentDefinitionError).toBeDefined();
    expect(componentDefinitionError.message).toBe(
        "ComponentDefinition can only be created by using ComponentCompiler.",
    );

    let componentInstanceError: any;
    try {
        new private_api.ComponentInstance();
    } catch (error) {
        componentInstanceError = error;
    }
    expect(componentInstanceError).toBeDefined();
    expect(componentInstanceError.message).toBe(
        "ComponentInstance can only be created by using ComponentCompiler.",
    );
});

test("properties ComponentDefinition", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `export struct Struct {}
  export component App {
    in-out property <bool> bool-property;
    in-out property <brush> brush-property;
    in-out property <color> color-property;
    in-out property <float> float-property;
    in-out property <image> image-property;
    in-out property <int> int-property;
    in-out property <[string]> model-property;
    in-out property <string> string-property;
    in-out property <Struct> struct-property;
  }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const properties = definition.App!.properties;
    expect(properties.length).toBe(9);

    properties.sort((a, b) => {
        const nameA = a.name.toUpperCase(); // ignore upper and lowercase
        const nameB = b.name.toUpperCase(); // ignore upper and lowercase

        if (nameA < nameB) {
            return -1;
        }

        if (nameA > nameB) {
            return 1;
        }

        return 0;
    });

    expect(properties[0].name).toBe("bool-property");
    expect(properties[0].valueType).toBe(napi.ValueType.Bool);
    expect(properties[1].name).toBe("brush-property");
    expect(properties[1].valueType).toBe(napi.ValueType.Brush);
    expect(properties[2].name).toBe("color-property");
    expect(properties[2].valueType).toBe(napi.ValueType.Brush);
    expect(properties[3].name).toBe("float-property");
    expect(properties[3].valueType).toBe(napi.ValueType.Number);
    expect(properties[4].name).toBe("image-property");
    expect(properties[4].valueType).toBe(napi.ValueType.Image);
    expect(properties[5].name).toBe("int-property");
    expect(properties[5].valueType).toBe(napi.ValueType.Number);
    expect(properties[6].name).toBe("model-property");
    expect(properties[6].valueType).toBe(napi.ValueType.Model);
    expect(properties[7].name).toBe("string-property");
    expect(properties[7].valueType).toBe(napi.ValueType.String);
    expect(properties[8].name).toBe("struct-property");
    expect(properties[8].valueType).toBe(napi.ValueType.Struct);
});

test("callbacks ComponentDefinition", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    callback first-callback();
    callback second-callback();
  }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const callbacks = definition.App!.callbacks;
    expect(callbacks.length).toBe(2);

    callbacks.sort();

    expect(callbacks[0]).toBe("first-callback");
    expect(callbacks[1]).toBe("second-callback");
});

test("globalProperties ComponentDefinition", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `export struct Struct {}

  export global TestGlobal {
    in-out property <bool> bool-property;
    in-out property <brush> brush-property;
    in-out property <color> color-property;
    in-out property <float> float-property;
    in-out property <image> image-property;
    in-out property <int> int-property;
    in-out property <[string]> model-property;
    in-out property <string> string-property;
    in-out property <Struct> struct-property;
  }

  export component App {
  }`,
        "",
    );

    expect(definition.App).not.toBeNull();

    expect(definition.App!.globalProperties("NonExistent")).toBeNull();

    const properties = definition.App!.globalProperties("TestGlobal");
    expect(properties).not.toBeNull();

    expect(properties!.length).toBe(9);

    properties!.sort((a, b) => {
        const nameA = a.name.toUpperCase(); // ignore upper and lowercase
        const nameB = b.name.toUpperCase(); // ignore upper and lowercase

        if (nameA < nameB) {
            return -1;
        }

        if (nameA > nameB) {
            return 1;
        }

        return 0;
    });

    expect(properties![0].name).toBe("bool-property");
    expect(properties![0].valueType).toBe(napi.ValueType.Bool);
    expect(properties![1].name).toBe("brush-property");
    expect(properties![1].valueType).toBe(napi.ValueType.Brush);
    expect(properties![2].name).toBe("color-property");
    expect(properties![2].valueType).toBe(napi.ValueType.Brush);
    expect(properties![3].name).toBe("float-property");
    expect(properties![3].valueType).toBe(napi.ValueType.Number);
    expect(properties![4].name).toBe("image-property");
    expect(properties![4].valueType).toBe(napi.ValueType.Image);
    expect(properties![5].name).toBe("int-property");
    expect(properties![5].valueType).toBe(napi.ValueType.Number);
    expect(properties![6].name).toBe("model-property");
    expect(properties![6].valueType).toBe(napi.ValueType.Model);
    expect(properties![7].name).toBe("string-property");
    expect(properties![7].valueType).toBe(napi.ValueType.String);
    expect(properties![8].name).toBe("struct-property");
    expect(properties![8].valueType).toBe(napi.ValueType.Struct);
});

test("globalCallbacks ComponentDefinition", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export global TestGlobal {
    callback first-callback();
    callback second-callback();
  }
  export component App {
  }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    expect(definition.App!.globalCallbacks("NonExistent")).toBeNull();

    const callbacks = definition.App!.globalCallbacks("TestGlobal");
    expect(callbacks).not.toBeNull();
    expect(callbacks!.length).toBe(2);

    callbacks!.sort();

    expect(callbacks![0]).toBe("first-callback");
    expect(callbacks![1]).toBe("second-callback");
});

test("compiler diagnostics", () => {
    const compiler = new private_api.ComponentCompiler();
    expect(
        compiler.buildFromSource(
            `export component App {
    garbage
  }`,
            "testsource.slint",
        ),
    ).toStrictEqual({});

    const diags = compiler.diagnostics;
    expect(diags.length).toBe(1);
    expect(diags[0]).toStrictEqual({
        level: 0,
        message: "Parse error",
        lineNumber: 2,
        columnNumber: 12,
        fileName: "testsource.slint",
    });
});

test("non-existent properties and callbacks", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `

  export component App {
  }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    let prop_err: any;
    try {
        instance!.setProperty("non-existent", 42);
    } catch (error) {
        prop_err = error;
    }
    expect(prop_err).toBeDefined();
    expect(prop_err.code).toBe("GenericFailure");
    expect(prop_err.message).toBe(
        "Property non-existent not found in the component",
    );

    let callback_err: any;
    try {
        instance!.setCallback("non-existent-callback", () => {});
    } catch (error) {
        callback_err = error;
    }
    expect(callback_err).toBeDefined();
    expect(callback_err.code).toBe("GenericFailure");
    expect(callback_err.message).toBe(
        "Callback non-existent-callback not found in the component",
    );
});
