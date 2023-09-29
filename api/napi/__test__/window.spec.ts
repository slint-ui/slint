// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import test from 'ava'

import { ComponentCompiler, Window } from '../index'

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
    t.is(window.isVisible, false);
    window.show();
    t.is(window.isVisible, true);
    window.hide();
    t.is(window.isVisible, false);
})