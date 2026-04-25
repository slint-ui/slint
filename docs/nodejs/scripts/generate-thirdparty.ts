// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const docsNodejsRoot = join(scriptsDir, "..");
const repoRoot = join(docsNodejsRoot, "..", "..");
const apiNode = join(repoRoot, "api", "node");
const outFile = join(docsNodejsRoot, "public", "thirdparty", "index.html");

mkdirSync(dirname(outFile), { recursive: true });

const result = spawnSync(
    "cargo",
    ["about", "generate", "thirdparty.hbs", "-o", outFile],
    { cwd: apiNode, stdio: "inherit" },
);

process.exit(result.status ?? 1);
