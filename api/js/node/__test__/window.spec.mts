// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";

import { private_api, Window } from "../dist/index.js";

test("Window constructor", () => {
    let thrownError: any;
    try {
        new private_api.Window();
    } catch (error) {
        thrownError = error;
    }
    expect(thrownError).toBeDefined();
    expect(thrownError.code).toBe("GenericFailure");
    expect(thrownError.message).toBe(
        "Window can only be created by using a Component.",
    );
});

test("Window show / hide", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `

    export component App inherits Window {
        width: 300px;
        height: 300px;
    }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    const window = instance!.window();
    expect(window.visible).toBe(false);
    window.show();
    expect(window.visible).toBe(true);
    window.hide();
    expect(window.visible).toBe(false);
});
