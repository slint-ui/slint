// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { runtimeRoot, runtimePin, verifyRuntime } from "./runtime-pin.mjs";
if (!existsSync(runtimeRoot)) {
    if (process.env.SLINT_REPO)
        throw Error(
            "SLINT_REPO does not exist; explicit checkouts are read-only.",
        );
    mkdirSync(dirname(runtimeRoot), { recursive: true });
    execFileSync(
        "git",
        [
            "clone",
            "--filter=blob:none",
            "--no-checkout",
            runtimePin.repository,
            runtimeRoot,
        ],
        { stdio: "inherit" },
    );
    execFileSync(
        "git",
        ["-C", runtimeRoot, "checkout", "--detach", runtimePin.revision],
        { stdio: "inherit" },
    );
}
verifyRuntime();
