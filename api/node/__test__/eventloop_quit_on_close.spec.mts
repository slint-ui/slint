// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";

import { runEventLoop, quitEventLoop, private_api } from "../dist/index.js";
import { processEvents, ProcessEventsResult } from "../binding.cjs";

test("quit event loop on last window closed with callback", {
    concurrent: false,
}, async () => {
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

    const instance = definition.App!.create() as any;
    expect(instance).not.toBeNull();

    // The second run used to return at once on Linux, see #4468.
    for (const run of [1, 2]) {
        let hidden = false;
        instance.window().show();
        await runEventLoop(() => {
            setTimeout(() => {
                instance.window().hide();
                hidden = true;
            }, 2);
        });
        expect(hidden, `run ${run}`).toBe(true);
    }
});

function createApp(): any {
    const compiler = new private_api.ComponentCompiler();
    return compiler
        .buildFromSource(`export component App inherits Window {}`, "")
        .App!.create();
}

test("a quit requested before a run doesn't end it", {
    concurrent: false,
}, () => {
    const instance = createApp();

    // Hiding the last window requests a quit while no event loop runs.
    instance.window().show();
    instance.window().hide();
    // The polling fallback of runEventLoop() used to end at its first pump.
    expect(processEvents()).toBe(ProcessEventsResult.Continue);
});

test("quit event loop before a window is hidden", {
    concurrent: false,
}, async () => {
    const instance = createApp();

    // Hiding the window between the runs used to end the second run at once.
    for (const run of [1, 2]) {
        let quit = false;
        instance.window().show();
        await runEventLoop(() => {
            setTimeout(() => {
                quit = true;
                quitEventLoop();
            }, 100);
        });
        instance.window().hide();
        expect(quit, `run ${run}`).toBe(true);
    }
});
