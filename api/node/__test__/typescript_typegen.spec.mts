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
    in-out property <color> tint;
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

test("a color property takes every form the runtime accepts", () => {
    const dir = setupTypeCheckDir();
    try {
        const result = tscCheck(
            dir,
            `import * as slint from "slint-ui";
import { App } from "./app.slint";
declare const app: App;
app.tint = "#ff0000";
app.tint = { red: 255, green: 0, blue: 0 };
app.tint = app.tint;
const brush: slint.Brush = app.tint as slint.Brush;
console.log(brush.color);
`,
        );
        expect(result.success).toBe(true);
        expect(result.output).toBe("");
    } finally {
        rmSync(dir, { recursive: true });
    }
}, 120_000);

test("reading a color property does not promise the channels", () => {
    // Reading one gives a Brush, so the channels are behind `.color` and reaching
    // for them directly used to type check and be undefined at run time.
    const dir = setupTypeCheckDir();
    try {
        const result = tscCheck(
            dir,
            `import { App } from "./app.slint";
declare const app: App;
console.log(app.tint.red);
`,
        );
        expect(result.success).toBe(false);
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

test("the loader hook exports every declared and re-exported name", () => {
    // Created inside the package so the generated module's `import "slint-ui"`
    // resolves.
    const dir = mkdtempSync(join(__dirname, "slint-ts-loader-"));
    try {
        writeFileSync(
            join(dir, "shared.slint"),
            `export struct Shared { value: int }
export component Widget inherits Rectangle {}`,
        );
        writeFileSync(
            join(dir, "base.slint"),
            `export * from "shared.slint";
export enum Mode { light, dark }`,
        );
        writeFileSync(
            join(dir, "app.slint"),
            `import { Widget } from "shared.slint";
export * from "base.slint";
struct Local { flag: bool }
// The comment and the trailing comma are what a hand-written parser trips over.
export {
    Local as Renamed, // the local struct
}
export global Settings { in-out property <int> volume; }
export component App inherits Window { Widget {} }`,
        );
        writeFileSync(
            join(dir, "check.mjs"),
            `import { App, Renamed, Shared, Mode } from "./app.slint";
if (!new App()) throw new Error("declared component missing");
if (Renamed({ flag: true }).flag !== true) throw new Error("aliased struct missing");
if (Shared({ value: 3 }).value !== 3) throw new Error("star-exported struct missing");
if (Mode.dark !== "dark") throw new Error("star-exported enum missing");
import * as everything from "./app.slint";
if ("Settings" in everything) throw new Error("a global is not a module export");
if (new App().Settings.volume !== 0) throw new Error("global not reachable on the instance");
`,
        );

        // The hook only takes effect in a process started with it, so the
        // check runs in its own node, the way an application would.
        execSync(
            `node --import ${JSON.stringify(pathToFileURL(join(__dirname, "..", "register.mjs")).href)} check.mjs`,
            { cwd: dir, stdio: "pipe" },
        );
    } finally {
        rmSync(dir, { recursive: true });
    }
}, 120_000);

const ENUM_SLINT = `export enum Mood { happy, very-happy }
export component App inherits Window {
    in-out property <Mood> mood: Mood.very-happy;
    in-out property <ColorScheme> scheme;
    in-out property <TextWrap> wrap: word-wrap;
    t := Text { wrap: root.wrap; }
}`;

test("a declared enum takes a plain string and the generated value object", () => {
    const dir = mkdtempSync(join(tmpdir(), "slint-ts-enum-"));
    try {
        writeFileSync(join(dir, "app.slint"), ENUM_SLINT);
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
                        "slint-ui": [
                            join(__dirname, "..", "dist", "index.d.ts"),
                        ],
                    },
                },
            }),
        );

        expect(
            tscCheck(
                dir,
                `import * as slint from "slint-ui";
import { App, Mood } from "./app.slint";
declare const app: App;
app.mood = Mood.very_happy;
app.mood = "happy";
const mood: Mood = app.mood;
// A public built-in enum comes from slint.language, by either spelling.
app.scheme = slint.language.ColorScheme.Dark;
app.scheme = "dark";
console.log(mood);
`,
            ).success,
        ).toBe(true);

        expect(
            tscCheck(
                dir,
                `import { App } from "./app.slint";\ndeclare const app: App;\napp.mood = "cheerful";\n`,
            ).success,
        ).toBe(false);

        // A built-in enum that slint.language doesn't carry is not public API in any
        // language binding, so the property is typed void, as in Rust and Python.
        expect(
            tscCheck(
                dir,
                `import { TextWrap } from "./app.slint";\nconsole.log(TextWrap);\n`,
            ).success,
        ).toBe(false);

        expect(
            tscCheck(
                dir,
                `import { App } from "./app.slint";\ndeclare const app: App;\napp.wrap = "no-wrap";\n`,
            ).success,
        ).toBe(false);
    } finally {
        rmSync(dir, { recursive: true });
    }
}, 120_000);

test("enum properties round-trip at run time", () => {
    // Created inside the package so the generated module's `import "slint-ui"` resolves.
    const dir = mkdtempSync(join(__dirname, "slint-ts-enum-"));
    try {
        writeFileSync(join(dir, "app.slint"), ENUM_SLINT);
        writeFileSync(
            join(dir, "check.mjs"),
            `import { App, Mood } from "./app.slint";
const app = new App();
if (app.mood !== "very-happy") throw new Error("read gives the string the type promises");
if (app.mood !== Mood.very_happy) throw new Error("the value object disagrees with the property");
app.mood = "happy";
if (app.mood !== Mood.happy) throw new Error("a plain string is not accepted");
`,
        );

        // The hook only takes effect in a process started with it, so the
        // check runs in its own node, the way an application would.
        execSync(
            `node --import ${JSON.stringify(pathToFileURL(join(__dirname, "..", "register.mjs")).href)} check.mjs`,
            { cwd: dir, stdio: "pipe" },
        );
    } finally {
        rmSync(dir, { recursive: true });
    }
}, 120_000);
