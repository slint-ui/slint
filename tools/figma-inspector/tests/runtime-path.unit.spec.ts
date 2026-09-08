// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { execFile } from "node:child_process";
import {
    copyFile,
    mkdir,
    mkdtemp,
    realpath,
    rm,
    writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { expect, test } from "vitest";

const execFileAsync = promisify(execFile);
const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

test("resolves runtime paths in checkouts with URL-encoded characters independently of cwd", async () => {
    const temporaryRoot = await realpath(
        await mkdtemp(join(tmpdir(), "slint-runtime-path-")),
    );
    try {
        const checkout = join(temporaryRoot, "plugin space # % ü");
        const script = join(checkout, "scripts/runtime-pin.mjs");
        await mkdir(dirname(script), { recursive: true });
        await copyFile(join(projectRoot, "scripts/runtime-pin.mjs"), script);
        await writeFile(join(checkout, "runtime-pin.json"), "{}");
        const env = { ...process.env };
        delete env.SLINT_REPO;
        for (const override of [
            undefined,
            "../upstream space",
            join(temporaryRoot, "absolute upstream"),
        ]) {
            const { stdout } = await execFileAsync(
                process.execPath,
                [
                    "--input-type=module",
                    "--eval",
                    `const { runtimeRoot } = await import(${JSON.stringify(pathToFileURL(script).href)}); console.log(JSON.stringify(runtimeRoot));`,
                ],
                {
                    cwd: temporaryRoot,
                    env:
                        override === undefined
                            ? env
                            : { ...env, SLINT_REPO: override },
                },
            );
            expect(JSON.parse(stdout)).toBe(
                resolve(checkout, override ?? ".generated/slint-source"),
            );
        }
    } finally {
        await rm(temporaryRoot, { recursive: true, force: true });
    }
});
