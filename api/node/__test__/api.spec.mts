// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import test from 'ava'
import * as path from 'node:path';
import { fileURLToPath } from 'url';
import { setFlagsFromString } from 'v8';
import { runInNewContext } from 'vm';

setFlagsFromString('--expose_gc');
const gc = runInNewContext('gc');

import { loadFile, loadSource, CompileError } from '../index.js'

const dirname = path.dirname(fileURLToPath(import.meta.url));

// loadFile api
test('loadFile', (t) => {
    let demo = loadFile(path.join(dirname, "resources/test.slint")) as any;
    let test = new demo.Test();
    t.is(test.check, "Test");

    let errorPath = path.join(dirname, "resources/error.slint");

    const error = t.throws(() => {
        loadFile(errorPath)
    },
        { instanceOf: CompileError }
    );

    t.is(error?.message, "Could not compile " + errorPath);
    t.deepEqual(error?.diagnostics, [
        {
            columnNumber: 18,
            level: 0,
            lineNumber: 7,
            message: 'Missing type. The syntax to declare a property is `property <type> name;`. Only two way bindings can omit the type',
            fileName: errorPath
        },
        {
            columnNumber: 22,
            level: 0,
            lineNumber: 7,
            message: 'Syntax error: expected \';\'',
            fileName: errorPath
        },
        {
            columnNumber: 22,
            level: 0,
            lineNumber: 7,
            message: 'Parse error',
            fileName: errorPath
        },
    ]);
})

test('loadFile constructor parameters', (t) => {
    let demo = loadFile(path.join(dirname, "resources/test-constructor.slint")) as any;
    let hello = "";
    let test = new demo.Test({ say_hello: function () { hello = "hello"; }, check: "test" });

    test.say_hello();

    t.is(test.check, "test");
    t.is(hello, "hello");
})

test('loadFile component instances and modules are sealed', (t) => {
    "use strict";
    let demo = loadFile(path.join(dirname, "resources/test.slint")) as any;

    t.throws(() => {
        demo.no_such_property = 42;
    }, { instanceOf: TypeError });

    let test = new demo.Test();
    t.is(test.check, "Test");

    t.throws(() => {
        test.no_such_callback = () => { };
    }, { instanceOf: TypeError });
})


// loadSource api
test('loadSource', (t) => {
    const source = `export component Test {
        out property <string> check: "Test";
    }`
    const path = 'api.spec.ts';
    let demo = loadSource(source, path) as any;
    let test = new demo.Test();
    t.is(test.check, "Test");

    const errorSource = `export component Error {
        out property bool> check: "Test";
    }`

    const error = t.throws(() => {
        loadSource(errorSource, path)
    },
        { instanceOf: CompileError }
    );

    t.is(error?.message, "Could not compile " + path);
    // console.log(error?.diagnostics)
    t.deepEqual(error?.diagnostics, [
        {
            columnNumber: 22,
            level: 0,
            lineNumber: 2,
            message: 'Missing type. The syntax to declare a property is `property <type> name;`. Only two way bindings can omit the type',
            fileName: path
        },
        {
            columnNumber: 26,
            level: 0,
            lineNumber: 2,
            message: 'Syntax error: expected \';\'',
            fileName: path
        },
        {
            columnNumber: 26,
            level: 0,
            lineNumber: 2,
            message: 'Parse error',
            fileName: path
        },
    ]);
})

test('loadSource constructor parameters', (t) => {
    const source = `export component Test {
        callback say_hello();
        in-out property <string> check;
    }`
    const path = 'api.spec.ts';
    let demo = loadSource(source, path) as any;
    let hello = "";
    let test = new demo.Test({ say_hello: function () { hello = "hello"; }, check: "test" });

    test.say_hello();

    t.is(test.check, "test");
    t.is(hello, "hello");
})

test('loadSource component instances and modules are sealed', (t) => {
    "use strict";
    const source = `export component Test {
        out property <string> check: "Test";
    }`
    const path = 'api.spec.ts';
    let demo = loadSource(source, path) as any;

    t.throws(() => {
        demo.no_such_property = 42;
    }, { instanceOf: TypeError });

    let test = new demo.Test();
    t.is(test.check, "Test");

    t.throws(() => {
        test.no_such_callback = () => { };
    }, { instanceOf: TypeError });
})

test('callback closure cyclic references do not prevent GC', async (t) => {

    // Setup:
    // A component instance with a callback installed from JS:
    //    * The callback captures the surrounding environment, which
    //      includes an extra reference to the component instance itself
    //      --> a cyclic reference
    //    * Invoking the callback clears the reference in the outer captured
    //      environment.
    //
    // Note: WeakRef's deref is used to observe the GC. This means that we must
    // separate the test into different jobs with await, to permit for collection.
    // (See https://tc39.es/ecma262/multipage/managing-memory.html#sec-weak-ref.prototype.deref)

    let demo_module = loadFile(path.join(__dirname, "resources/test-gc.slint")) as any;
    let demo = new demo_module.Test();
    t.is(demo.check, "initial value");
    t.true(Object.hasOwn(demo, "say_hello"));
    let callback_invoked = false;
    let demo_weak = new WeakRef(demo);

    demo.say_hello = () => {
        demo = null;
        callback_invoked = true;
    };

    t.true(demo_weak.deref() !== undefined);

    // After the first GC, the instance should not have been collected because the
    // current environment's demo variable is a strong reference.
    await new Promise(resolve => setTimeout(resolve, 0));
    gc();

    t.true(demo_weak.deref() !== undefined);

    // Invoke the callback, to clear "demo"
    demo.say_hello();
    t.true(callback_invoked);
    t.true(demo === null);

    // After the this GC call, the instance should have been collected. Strong references
    // in Rust should not keep it alive.
    await new Promise(resolve => setTimeout(resolve, 0));
    gc();

    t.is(demo_weak.deref(), undefined, "The demo instance should have been collected and the weak ref should deref to undefined");
})
