// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import test from 'ava'
import * as path from 'node:path';
import { fileURLToPath } from 'url';

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
            lineNumber: 5,
            message: 'Missing type. The syntax to declare a property is `property <type> name;`. Only two way bindings can omit the type',
            fileName: errorPath
        },
        {
            columnNumber: 22,
            level: 0,
            lineNumber: 5,
            message: 'Syntax error: expected \';\'',
            fileName: errorPath
        },
        {
            columnNumber: 22,
            level: 0,
            lineNumber: 5,
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
