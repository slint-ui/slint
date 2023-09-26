// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import test from 'ava'

import { ComponentCompiler, ComponentDefinition, ComponentInstance, Window, ValueType,
  Point, Size } from '../index'

test('get/set include paths', (t) => {
  let compiler = new ComponentCompiler;

  t.is(compiler.includePaths.length, 0);

  compiler.includePaths = ["path/one/", "path/two/", "path/three/"];

  t.deepEqual(compiler.includePaths, ["path/one/", "path/two/", "path/three/"]);
})

test('get/set style', (t) => {
  let compiler = new ComponentCompiler;

  t.is(compiler.style, null);

  compiler.style = "fluent";
  t.is(compiler.style, "fluent");
})

test('get/set build from source', (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`export component App {}`, "");
  t.not(definition, null);
  t.is(definition!.name, "App");
})

test('constructor error ComponentDefinition and ComponentInstance', (t) => {
  const componentDefinitionError = t.throws(() => { new ComponentDefinition });
  t.is(componentDefinitionError?.message, "ComponentDefinition can only be created by using ComponentCompiler.");

  const componentInstanceError = t.throws(() => { new ComponentInstance });
  t.is(componentInstanceError?.message, "ComponentInstance can only be created by using ComponentCompiler.");
})

test('properties ComponentDefinition', (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`export struct Struct {}
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
  }`, "");
  t.not(definition, null);

  let properties = definition!.properties;
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
  })

  t.is(properties[0].name, "bool-property");
  t.is(properties[0].valueType, ValueType.Bool);
  t.is(properties[1].name, "brush-property");
  t.is(properties[1].valueType, ValueType.Brush);
  t.is(properties[2].name, "color-property");
  t.is(properties[2].valueType, ValueType.Brush);
  t.is(properties[3].name, "float-property");
  t.is(properties[3].valueType, ValueType.Number);
  t.is(properties[4].name, "image-property");
  t.is(properties[4].valueType, ValueType.Image);
  t.is(properties[5].name, "int-property");
  t.is(properties[5].valueType, ValueType.Number);
  t.is(properties[6].name, "model-property");
  t.is(properties[6].valueType, ValueType.Model);
  t.is(properties[7].name, "string-property");
  t.is(properties[7].valueType, ValueType.String);
  t.is(properties[8].name, "struct-property");
  t.is(properties[8].valueType, ValueType.Struct);
})

test('callbacks ComponentDefinition', (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`
  export component App {
    callback first-callback();
    callback second-callback();
  }`, "");
  t.not(definition, null);

  let callbacks = definition!.callbacks;
  t.is(callbacks.length, 2);

  callbacks.sort();

  t.is(callbacks[0], "first-callback");
  t.is(callbacks[1], "second-callback");
})

test('globalProperties ComponentDefinition', (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`export struct Struct {}

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
  }`, "");

  t.not(definition, null);

  t.is(definition!.globalProperties("NonExistent"), null);

  let properties = definition!.globalProperties("TestGlobal");
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
  })

  t.is(properties![0].name, "bool-property");
  t.is(properties![0].valueType, ValueType.Bool);
  t.is(properties![1].name, "brush-property");
  t.is(properties![1].valueType, ValueType.Brush);
  t.is(properties![2].name, "color-property");
  t.is(properties![2].valueType, ValueType.Brush);
  t.is(properties![3].name, "float-property");
  t.is(properties![3].valueType, ValueType.Number);
  t.is(properties![4].name, "image-property");
  t.is(properties![4].valueType, ValueType.Image);
  t.is(properties![5].name, "int-property");
  t.is(properties![5].valueType, ValueType.Number);
  t.is(properties![6].name, "model-property");
  t.is(properties![6].valueType, ValueType.Model);
  t.is(properties![7].name, "string-property");
  t.is(properties![7].valueType, ValueType.String);
  t.is(properties![8].name, "struct-property");
  t.is(properties![8].valueType, ValueType.Struct);
})

test('globalCallbacks ComponentDefinition', (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`
  export global TestGlobal {
    callback first-callback();
    callback second-callback();
  }
  export component App {
  }`, "");
  t.not(definition, null);

  t.is(definition!.globalCallbacks("NonExistent"), null);

  let callbacks = definition!.globalCallbacks("TestGlobal");
  t.not(callbacks, null);
  t.is(callbacks!.length, 2);

  callbacks!.sort();

  t.is(callbacks![0], "first-callback");
  t.is(callbacks![1], "second-callback");
})

test('compiler diagnostics', (t) => {
  let compiler = new ComponentCompiler;
  t.is(compiler.buildFromSource(`export component App {
    garbage
  }`, "testsource.slint"), null);

  const diags = compiler.diagnostics;
  t.is(diags.length, 1);
  t.deepEqual(diags[0], {
    level: 0,
    message: 'Parse error',
    lineNumber: 2,
    column: 12,
    sourceFile: 'testsource.slint'
  });
})

test('non-existent properties and callbacks', (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`

  export component App {
  }`, "");
  t.not(definition, null);

  let instance = definition!.create();
  t.not(instance, null);

  const prop_err = t.throws(() => {
    instance!.setProperty("non-existent", 42);
  });
  t.is(prop_err!.code, 'GenericFailure');
  t.is(prop_err!.message, 'Property non-existent not found in the component');

  const callback_err = t.throws(() => {
    instance!.setCallback("non-existent-callback", () => { });
  });
  t.is(callback_err!.code, 'GenericFailure');
  t.is(callback_err!.message, 'Callback non-existent-callback not found in the component');
})

test('Window constructor', (t) => {
  t.throws(() => {
     new Window()
    },
      {
        code: "GenericFailure",
        message: "Window can only be created by using a Component."
      }
    );
})

test('Window show / hide', (t) => {
  let compiler = new ComponentCompiler;
  let definition = compiler.buildFromSource(`

  export component App inherits Window {
    width: 300px;
    height: 300px;
  }`, "");
  t.not(definition, null);

  let instance = definition!.create();
  t.not(instance, null);

  let window = instance!.window();
  t.is(window.isVisible(), false);
  window.show();
  t.is(window.isVisible(), true);
  window.hide();
  t.is(window.isVisible(), false);
})