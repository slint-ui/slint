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
        path.join(directory, "slint-project.json"),
        JSON.stringify({ edition: "1.19", ...settings }),
    );
    return directory;
}

function writeMain(directory: string, source: string): string {
    const main = path.join(directory, "main.slint");
    fs.writeFileSync(main, source);
    return main;
}

test.each(["directory", "manifest"])(
    "include paths come from the project file (%s)",
    (projectInput) => {
        const directory = projectDirectory({
            main: "main.slint",
            "include-paths": ["include"],
        });
        fs.mkdirSync(path.join(directory, "include"));
        fs.writeFileSync(
            path.join(directory, "include", "shared.slint"),
            "export component Shared { }",
        );

        writeMain(
            directory,
            `import { Shared } from "shared.slint";
             export component Main inherits Window { Shared { } }`,
        );

        const input = projectInput === "directory"
            ? directory
            : path.join(directory, "slint-project.json");
        expect(() => loadFile(input)).not.toThrow();
    },
);

test.each(["directory", "manifest"])(
    "library paths come from the project file (%s)",
    (projectInput) => {
        const directory = projectDirectory({
            main: "main.slint",
            "library-paths": { widgets: "widgets.slint" },
        });
        fs.writeFileSync(
            path.join(directory, "widgets.slint"),
            "export component Widget { }",
        );

        writeMain(
            directory,
            `import { Widget } from "@widgets";
             export component Main inherits Window { Widget { } }`,
        );

        const input = projectInput === "directory"
            ? directory
            : path.join(directory, "slint-project.json");
        expect(() => loadFile(input)).not.toThrow();
    },
);

test("a project file that isn't valid json is reported", () => {
    const directory = fs.realpathSync(
        fs.mkdtempSync(path.join(os.tmpdir(), "slint-project-file-")),
    );
    fs.writeFileSync(path.join(directory, "slint-project.json"), "{");
    writeMain(directory, "export component Main inherits Window { }");

    let error: CompileError | undefined;
    try {
        loadFile(directory);
    } catch (caughtError) {
        error = caughtError as CompileError;
    }
    expect(error).toBeDefined();
    expect(
        error!.diagnostics.some((diagnostic) =>
            diagnostic.message.includes("slint-project.json"),
        ),
    ).toBe(true);
});

test.each(["directory", "manifest"])(
    "loading the project directory loads its main file (%s)",
    (projectInput) => {
        const directory = projectDirectory({
            main: "ui/main.slint",
            "include-paths": ["include"],
        });
        fs.mkdirSync(path.join(directory, "ui"));
        fs.mkdirSync(path.join(directory, "include"));
        fs.writeFileSync(
            path.join(directory, "include", "shared.slint"),
            "export component Shared { }",
        );
        fs.writeFileSync(
            path.join(directory, "ui", "main.slint"),
            `import { Shared } from "shared.slint";
             export component Main inherits Window { Shared { } }`,
        );

        const input = projectInput === "directory"
            ? directory
            : path.join(directory, "slint-project.json");
        const ui = loadFile(input) as any;
        expect(ui.Main).toBeDefined();
    },
);

test("loading a slint file ignores its project file", () => {
    const directory = projectDirectory({ main: "elsewhere.slint" });
    fs.writeFileSync(path.join(directory, "slint-project.json"), "{");
    const main = writeMain(
        directory,
        "export component Main inherits Window { }",
    );

    expect(() => loadFile(main)).not.toThrow();
});
