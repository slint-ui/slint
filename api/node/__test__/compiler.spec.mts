// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import test from "ava";

import { private_api } from "../dist/index.js";

test("get/set include paths", (t) => {
    const compiler = new private_api.ComponentCompiler();

    t.is(compiler.includePaths.length, 0);

    compiler.includePaths = ["path/one/", "path/two/", "path/three/"];

    t.deepEqual(compiler.includePaths, [
        "path/one/",
        "path/two/",
        "path/three/",
    ]);
});

test("get/set library paths", (t) => {
    const compiler = new private_api.ComponentCompiler();

    compiler.libraryPaths = {
        "libfile.slint": "third_party/libfoo/ui/lib.slint",
        libdir: "third_party/libbar/ui/",
    };

    t.deepEqual(compiler.libraryPaths, {
        "libfile.slint": "third_party/libfoo/ui/lib.slint",
        libdir: "third_party/libbar/ui/",
    });
});

test("get/set style", (t) => {
    const compiler = new private_api.ComponentCompiler();

    t.is(compiler.style, null);

    compiler.style = "fluent";
    t.is(compiler.style, "fluent");
});

test("get/set build from source", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(`export component App {}`, "");
    t.not(definition.App, null);
    t.is(definition.App!.name, "App");
});

test("constructor error ComponentDefinition and ComponentInstance", (t) => {
    const componentDefinitionError = t.throws(() => {
        new private_api.ComponentDefinition();
    });
    t.is(
        componentDefinitionError?.message,
        "ComponentDefinition can only be created by using ComponentCompiler.",
    );

    const componentInstanceError = t.throws(() => {
        new private_api.ComponentInstance();
    });
    t.is(
        componentInstanceError?.message,
        "ComponentInstance can only be created by using ComponentCompiler.",
    );
});

test("properties ComponentDefinition", (t) => {
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
    t.not(definition.App, null);

    const properties = definition.App!.properties;
    t.is(properties.length, 9);

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

    t.is(properties[0].name, "bool-property");
    t.is(properties[0].valueType, private_api.ValueType.Bool);
    t.is(properties[1].name, "brush-property");
    t.is(properties[1].valueType, private_api.ValueType.Brush);
    t.is(properties[2].name, "color-property");
    t.is(properties[2].valueType, private_api.ValueType.Brush);
    t.is(properties[3].name, "float-property");
    t.is(properties[3].valueType, private_api.ValueType.Number);
    t.is(properties[4].name, "image-property");
    t.is(properties[4].valueType, private_api.ValueType.Image);
    t.is(properties[5].name, "int-property");
    t.is(properties[5].valueType, private_api.ValueType.Number);
    t.is(properties[6].name, "model-property");
    t.is(properties[6].valueType, private_api.ValueType.Model);
    t.is(properties[7].name, "string-property");
    t.is(properties[7].valueType, private_api.ValueType.String);
    t.is(properties[8].name, "struct-property");
    t.is(properties[8].valueType, private_api.ValueType.Struct);
});

test("callbacks ComponentDefinition", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
  export component App {
    callback first-callback();
    callback second-callback();
  }`,
        "",
    );
    t.not(definition.App, null);

    const callbacks = definition.App!.callbacks;
    t.is(callbacks.length, 2);

    callbacks.sort();

    t.is(callbacks[0], "first-callback");
    t.is(callbacks[1], "second-callback");
});

test("globalProperties ComponentDefinition", (t) => {
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

    t.not(definition.App, null);

    t.is(definition.App!.globalProperties("NonExistent"), null);

    const properties = definition.App!.globalProperties("TestGlobal");
    t.not(properties, null);

    t.is(properties!.length, 9);

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

    t.is(properties![0].name, "bool-property");
    t.is(properties![0].valueType, private_api.ValueType.Bool);
    t.is(properties![1].name, "brush-property");
    t.is(properties![1].valueType, private_api.ValueType.Brush);
    t.is(properties![2].name, "color-property");
    t.is(properties![2].valueType, private_api.ValueType.Brush);
    t.is(properties![3].name, "float-property");
    t.is(properties![3].valueType, private_api.ValueType.Number);
    t.is(properties![4].name, "image-property");
    t.is(properties![4].valueType, private_api.ValueType.Image);
    t.is(properties![5].name, "int-property");
    t.is(properties![5].valueType, private_api.ValueType.Number);
    t.is(properties![6].name, "model-property");
    t.is(properties![6].valueType, private_api.ValueType.Model);
    t.is(properties![7].name, "string-property");
    t.is(properties![7].valueType, private_api.ValueType.String);
    t.is(properties![8].name, "struct-property");
    t.is(properties![8].valueType, private_api.ValueType.Struct);
});

test("globalCallbacks ComponentDefinition", (t) => {
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
    t.not(definition.App, null);

    t.is(definition.App!.globalCallbacks("NonExistent"), null);

    const callbacks = definition.App!.globalCallbacks("TestGlobal");
    t.not(callbacks, null);
    t.is(callbacks!.length, 2);

    callbacks!.sort();

    t.is(callbacks![0], "first-callback");
    t.is(callbacks![1], "second-callback");
});

test("compiler diagnostics", (t) => {
    const compiler = new private_api.ComponentCompiler();
    t.deepEqual(
        compiler.buildFromSource(
            `export component App {
    garbage
  }`,
            "testsource.slint",
        ),
        {},
    );

    const diags = compiler.diagnostics;
    t.is(diags.length, 1);
    t.deepEqual(diags[0], {
        level: 0,
        message: "Parse error",
        lineNumber: 2,
        columnNumber: 12,
        fileName: "testsource.slint",
    });
});

test("non-existent properties and callbacks", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `

  export component App {
  }`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    const prop_err = t.throws(() => {
        instance!.setProperty("non-existent", 42);
    }) as any;
    t.is(prop_err!.code, "GenericFailure");
    t.is(prop_err!.message, "Property non-existent not found in the component");

    const callback_err = t.throws(() => {
        instance!.setCallback("non-existent-callback", () => {});
    }) as any;
    t.is(callback_err!.code, "GenericFailure");
    t.is(
        callback_err!.message,
        "Callback non-existent-callback not found in the component",
    );
});
