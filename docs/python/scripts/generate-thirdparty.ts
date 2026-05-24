// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const docsPythonRoot = join(scriptsDir, "..");
const repoRoot = join(docsPythonRoot, "..", "..");
const apiPython = join(repoRoot, "api", "python", "slint");
const outFile = join(docsPythonRoot, "public", "thirdparty", "index.html");

mkdirSync(dirname(outFile), { recursive: true });

const result = spawnSync(
    "cargo",
    ["about", "generate", "thirdparty.hbs", "-o", outFile],
    { cwd: apiPython, stdio: "inherit" },
);

process.exit(result.status ?? 1);
