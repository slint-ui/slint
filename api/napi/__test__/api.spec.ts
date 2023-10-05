// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import test from 'ava'
const path = require('node:path');

import { loadFile } from '../index'

test('loadFile', (t) => {
    let demo = loadFile(path.join(__dirname, "resources/test.slint"));
    let test = new demo.Test();
    t.is(test.check, "Test");

    t.throws(() => {
        loadFile("path/to/wrong/file.slint")
        },
        {
            message: "Could not load path/to/wrong/file.slint: No such file or directory (os error 2)"
        }
    );
})