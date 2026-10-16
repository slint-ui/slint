// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// The package refers to itself by name, so these go through the `exports` of package.json,
// like they do for an application that depends on slint-ui.

import { test, expect } from "vitest";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

test("require the package", () => {
    const slint = require("slint-ui");
    expect(typeof slint.loadSource).toBe("function");
    expect(typeof slint.ArrayModel).toBe("function");
});

test("import the package", async () => {
    const slint = await import("slint-ui");
    expect(typeof slint.loadSource).toBe("function");
});

test("internal files aren't exported", () => {
    expect(() => require("slint-ui/dist/index.js")).toThrow(
        /not defined by "exports"/,
    );
    expect(() => require("slint-ui/package.json")).toThrow(
        /not defined by "exports"/,
    );
});
