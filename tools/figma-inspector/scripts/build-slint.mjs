// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { execFileSync } from "node:child_process";
import { rmSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runtimePin, runtimeRoot, verifyRuntime } from "./runtime-pin.mjs";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const outputDir = resolve(projectRoot, ".generated", "slint-wasm");
const release = process.argv[2] !== "--dev";

verifyRuntime();
rmSync(outputDir, { recursive: true, force: true });
execFileSync(
    "wasm-pack",
    [
        "build",
        release ? "--release" : "--dev",
        "--target",
        "web",
        "--out-dir",
        outputDir,
        resolve(runtimeRoot, "api", "wasm-interpreter"),
        "--",
        "--locked",
        "--features",
        "console_error_panic_hook",
    ],
    {
        env: {
            ...process.env,
            CARGO_TARGET_DIR: resolve(
                projectRoot,
                "../../target/figma-inspector-wasm",
            ),
        },
        stdio: "inherit",
    },
);
verifyRuntime();
execFileSync(
    process.execPath,
    [resolve(projectRoot, "scripts/build-font-capabilities.mjs")],
    { stdio: "inherit" },
);
console.log(
    `Built Slint ${runtimePin.version} interpreter at ${runtimePin.revision}`,
);
