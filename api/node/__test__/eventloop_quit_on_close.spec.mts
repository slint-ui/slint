// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Separate file because hiding the last window sets a sticky exit flag
// inside winit that can't be cleared via pump_app_events, poisoning
// any subsequent process_events call in the same process.

import { test, expect } from "vitest";

import { runEventLoop, private_api } from "../dist/index.js";

test.sequential("quit event loop on last window closed with callback", async () => {
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

    instance.window().show();
    await runEventLoop(() => {
        setTimeout(() => {
            instance.window().hide();
        }, 2);
    });
});
