// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore greting -- deliberate typo to test that tsc rejects unknown properties

import { test, expect } from "vitest";
import { execSync } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const ROOT = resolve(__dirname, "..", "..", "..");
const COMPILER = join(ROOT, "target", "debug", "slint-compiler");
// Run tsc through node rather than the node_modules/.bin wrapper,
// which is not directly executable on Windows.
const TSC = join(ROOT, "node_modules", "typescript", "lib", "tsc.js");

function setupTypeCheckDir(): string {
    const dir = mkdtempSync(join(tmpdir(), "slint-ts-test-"));

    writeFileSync(
        join(dir, "app.slint"),
        `export component App inherits Window {
    in-out property <string> greeting;
    in-out property <int> counter;
    callback clicked();
}`,
    );

    execSync(`${COMPILER} -f typescript app.slint -o app.slint.d.ts`, {
        cwd: dir,
    });

    writeFileSync(
        join(dir, "tsconfig.json"),
        JSON.stringify({
            compilerOptions: {
                module: "esnext",
                moduleResolution: "bundler",
                strict: true,
                noEmit: true,
                skipLibCheck: true,
                paths: {
                    "slint-ui": [join(__dirname, "..", "dist", "index.d.ts")],
                },
            },
        }),
    );

    return dir;
}

function tscCheck(
    dir: string,
    code: string,
): { success: boolean; output: string } {
    writeFileSync(join(dir, "check.ts"), code);
    try {
        execSync(`node "${TSC}" --noEmit`, { cwd: dir, stdio: "pipe" });
        return { success: true, output: "" };
    } catch (e: any) {
        const stdout = e.stdout?.toString() ?? "";
        const stderr = e.stderr?.toString() ?? "";
        return { success: false, output: stdout + stderr };
    }
}

test("correct property names pass type checking", () => {
    const dir = setupTypeCheckDir();
    try {
        const result = tscCheck(
            dir,
            `import { App } from "./app.slint";
declare const app: App;
app.greeting = "hello";
app.counter = 42;
`,
        );
        expect(result.success).toBe(true);
    } finally {
        rmSync(dir, { recursive: true });
    }
}, 120_000);

test("typo in property name fails type checking", () => {
    const dir = setupTypeCheckDir();
    try {
        const result = tscCheck(
            dir,
            `import { App } from "./app.slint";
declare const app: App;
app.greting = "hello";
`,
        );
        expect(result.success).toBe(false);
        expect(result.output).toContain("greting");
    } finally {
        rmSync(dir, { recursive: true });
    }
}, 120_000);

test("wrong type assignment fails type checking", () => {
    const dir = setupTypeCheckDir();
    try {
        const result = tscCheck(
            dir,
            `import { App } from "./app.slint";
declare const app: App;
app.counter = "not a number";
`,
        );
        expect(result.success).toBe(false);
    } finally {
        rmSync(dir, { recursive: true });
    }
}, 120_000);

// Full module mode: `-o app.slint.ts` emits a self-contained module with a
// loadFile() wrapper instead of ambient declarations.

const MODULE_MODE_SLINT_SOURCE = `export struct Item { name: string, checked: bool }
export enum Mode { light, dark }
export component App inherits Window {
    in-out property <string> greeting;
    in-out property <int> counter;
    callback clicked();
}`;

function setupModuleModeDir(parent: string): string {
    const dir = mkdtempSync(join(parent, "slint-ts-module-"));
    writeFileSync(join(dir, "app.slint"), MODULE_MODE_SLINT_SOURCE);
    execSync(`${COMPILER} -f typescript app.slint -o app.slint.ts`, {
        cwd: dir,
    });
    return dir;
}

test("full module mode passes type checking", () => {
    const dir = setupModuleModeDir(tmpdir());
    try {
        writeFileSync(
            join(dir, "tsconfig.json"),
            JSON.stringify({
                compilerOptions: {
                    module: "esnext",
                    moduleResolution: "bundler",
                    // The generated module uses `new URL(..., import.meta.url)`
                    target: "es2022",
                    lib: ["es2022", "dom"],
                    strict: true,
                    noEmit: true,
                    skipLibCheck: true,
                    paths: {
                        "slint-ui": [
                            join(__dirname, "..", "dist", "index.d.ts"),
                        ],
                    },
                },
            }),
        );
        const result = tscCheck(
            dir,
            `import { App, Item, Mode } from "./app.slint";
const app = new App({ greeting: "hello" });
app.counter = 42;
const item: Item = Item({ name: "milk", checked: false });
const mode: Mode = Mode.dark;
console.log(app, item, mode);
`,
        );
        expect(result.success).toBe(true);
        expect(result.output).toBe("");
    } finally {
        rmSync(dir, { recursive: true });
    }
}, 120_000);

test("full module mode loads and instantiates at runtime", async () => {
    // Generate inside the package so the module's `import "slint-ui"` resolves.
    const dir = setupModuleModeDir(__dirname);
    try {
        const mod = await import(pathToFileURL(join(dir, "app.slint.ts")).href);

        const app = new mod.App({ greeting: "hello" });
        expect(app.greeting).toBe("hello");
        app.counter = 42;
        expect(app.counter).toBe(42);

        const item = mod.Item({ name: "milk" });
        expect(item.name).toBe("milk");
        expect(item.checked).toBe(false);

        expect(mod.Mode.dark).toBe("dark");
    } finally {
        rmSync(dir, { recursive: true });
    }
}, 120_000);
