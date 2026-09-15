// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export const runtimePin = JSON.parse(
    readFileSync(resolve(projectRoot, "runtime-pin.json"), "utf8"),
);
export const runtimeRoot = resolve(
    process.env.SLINT_REPO ??
        resolve(projectRoot, ".generated", "slint-source"),
);

export function verifyRuntime() {
    const git = (...args) =>
        execFileSync("git", ["-C", runtimeRoot, ...args], {
            encoding: "utf8",
        }).trim();
    const revision = git("rev-parse", "HEAD");
    const dirty = git("status", "--porcelain", "--untracked-files=all");
    if (revision !== runtimePin.revision || dirty !== "") {
        throw new Error(
            "The Slint runtime checkout must match runtime-pin.json and remain clean",
        );
    }
    return runtimeRoot;
}
