// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const executable = process.platform === "win32" ? "slint-editor-mcp.exe" : "slint-editor-mcp";
const child = spawn(fileURLToPath(new URL(`runtime/${executable}`, import.meta.url)), { stdio: "inherit" });
child.on("error", error => {
  console.error(`Could not start the Slint Visual Editor bridge: ${error.message}. Build the plugin runtime or install a platform package.`);
  process.exitCode = 1;
});
child.on("exit", code => { process.exitCode = code ?? 1; });
for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => child.kill(signal));
