// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import test from 'ava'
const path = require('node:path');

import { loadFile, CompileError } from '../index'

test('loadFile', (t) => {
    let demo = loadFile(path.join(__dirname, "resources/test.slint")) as any;
    let test = new demo.Test();
    t.is(test.check, "Test");

    let errorPath = path.join(__dirname, "resources/error.slint");

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

test('constructor parameters', (t) => {
    let demo = loadFile(path.join(__dirname, "resources/test-constructor.slint")) as any;
    let hello = "";
    let test = new demo.Test({ say_hello: function () { hello = "hello"; }, check: "test" });

    test.say_hello();

    t.is(test.check, "test");
    t.is(hello, "hello");
})
