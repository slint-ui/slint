// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const result = spawnSync(
    "wasm-pack",
    [
        "build",
        fileURLToPath(
            new URL("../../../api/wasm-interpreter", import.meta.url),
        ),
        "--target",
        "nodejs",
        "--out-dir",
        fileURLToPath(new URL("checker", import.meta.url)),
        ...process.argv.slice(2),
        "--",
        "--no-default-features",
        "--features",
        "headless,console_error_panic_hook",
    ],
    { stdio: "inherit" },
);
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
