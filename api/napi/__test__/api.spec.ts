// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import test from 'ava'
const path = require('node:path');

import { loadFile, CompilerError, Diagnostic } from '../index'

test('loadFile', (t) => {
    let demo = loadFile(path.join(__dirname, "resources/test.slint"));
    let test = new demo.Test();
    t.is(test.check, "Test");

    let errorPath = path.join(__dirname, "resources/error.slint");

    const error = t.throws(() => {
        loadFile(errorPath)
        },
        {instanceOf: CompilerError}
    );

    t.is(error?.message, "Could not compile " + errorPath);
    t.deepEqual(error?.diagnostics, [
        {
            column: 18,
            level: 0,
            lineNumber: 5,
            message: 'Missing type. The syntax to declare a property is `property <type> name;`. Only two way bindings can omit the type',
            sourceFile: errorPath
        },
        {
            column: 22,
            level: 0,
            lineNumber: 5,
            message: 'Syntax error: expected \';\'',
            sourceFile: errorPath
        },
        {
            column: 22,
            level: 0,
            lineNumber: 5,
            message: 'Parse error',
            sourceFile: errorPath
        },
    ]);
})