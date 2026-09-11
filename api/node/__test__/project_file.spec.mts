// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

import { loadFile, type CompileError, private_api } from "../dist/index.js";

private_api.initTesting();

// realpath so the directory matches the paths the compiler reports back.
function projectDirectory(settings: object): string {
    const directory = fs.realpathSync(
        fs.mkdtempSync(path.join(os.tmpdir(), "slint-project-file-")),
    );
    fs.writeFileSync(
        path.join(directory, "slint.project.json"),
        JSON.stringify(settings),
    );
    return directory;
}

function writeMain(directory: string, source: string): string {
    const main = path.join(directory, "main.slint");
    fs.writeFileSync(main, source);
    return main;
}

test("include directories come from the project file", () => {
    const directory = projectDirectory({ "include-directories": ["include"] });
    fs.mkdirSync(path.join(directory, "include"));
    fs.writeFileSync(
        path.join(directory, "include", "shared.slint"),
        "export component Shared { }",
    );

    const main = writeMain(
        directory,
        `import { Shared } from "shared.slint";
         export component Main inherits Window { Shared { } }`,
    );

    // Without the project file the import doesn't resolve.
    expect(() => loadFile(main)).not.toThrow();
});

test("library paths come from the project file", () => {
    const directory = projectDirectory({
        "library-paths": { widgets: "widgets.slint" },
    });
    fs.writeFileSync(
        path.join(directory, "widgets.slint"),
        "export component Widget { }",
    );

    const main = writeMain(
        directory,
        `import { Widget } from "@widgets";
         export component Main inherits Window { Widget { } }`,
    );

    expect(() => loadFile(main)).not.toThrow();
});

test("the project file style reaches the compiler", () => {
    const directory = projectDirectory({ style: "no-such-style" });
    const main = writeMain(
        directory,
        "export component Main inherits Window { }",
    );

    // A style name the compiler rejects shows which style it actually used.
    let error: CompileError | undefined;
    try {
        loadFile(main);
    } catch (e) {
        error = e as CompileError;
    }
    expect(error).toBeDefined();
    expect(
        error!.diagnostics.some((d) => d.message.includes("no-such-style")),
    ).toBe(true);
});

test("a style passed to loadFile wins over the project file", () => {
    const directory = projectDirectory({ style: "no-such-style" });
    const main = writeMain(
        directory,
        "export component Main inherits Window { }",
    );

    expect(() => loadFile(main, { style: "fluent" })).not.toThrow();
});

test("a project file that isn't valid json is reported", () => {
    const directory = fs.realpathSync(
        fs.mkdtempSync(path.join(os.tmpdir(), "slint-project-file-")),
    );
    fs.writeFileSync(path.join(directory, "slint.project.json"), "{");
    const main = writeMain(
        directory,
        "export component Main inherits Window { }",
    );

    let error: CompileError | undefined;
    try {
        loadFile(main);
    } catch (e) {
        error = e as CompileError;
    }
    expect(error).toBeDefined();
    expect(
        error!.diagnostics.some((d) =>
            d.message.includes("slint.project.json"),
        ),
    ).toBe(true);
});
