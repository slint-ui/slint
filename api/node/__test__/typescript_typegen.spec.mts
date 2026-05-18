// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";
import { execSync } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const ROOT = resolve(__dirname, "..", "..", "..");
const COMPILER = join(ROOT, "target", "debug", "slint-compiler");
const TSC = join(ROOT, "node_modules", ".bin", "tsc");

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
                paths: { "slint-ui": [join(__dirname, "..", "dist", "index.d.ts")] },
            },
        }),
    );

    return dir;
}

function tscCheck(dir: string, code: string): { success: boolean; output: string } {
    writeFileSync(join(dir, "check.ts"), code);
    try {
        execSync(`${TSC} --noEmit`, { cwd: dir, stdio: "pipe" });
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
