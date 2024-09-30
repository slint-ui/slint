// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import test from "ava";

import { private_api, Window } from "../dist/index.js";

test("Window constructor", (t) => {
    t.throws(
        () => {
            new private_api.Window();
        },
        {
            code: "GenericFailure",
            message: "Window can only be created by using a Component.",
        },
    );
});

test("Window show / hide", (t) => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `

    export component App inherits Window {
        width: 300px;
        height: 300px;
    }`,
        "",
    );
    t.not(definition.App, null);

    const instance = definition.App!.create();
    t.not(instance, null);

    const window = instance!.window();
    t.is(window.visible, false);
    window.show();
    t.is(window.visible, true);
    window.hide();
    t.is(window.visible, false);
});
