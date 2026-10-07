// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { constants } from "node:fs";
import { access, readFile, rename, rm, stat } from "node:fs/promises";
import { join } from "node:path";

export async function loadRuntime(root) {
  try {
    const metadata = JSON.parse(await readFile(join(root, "runtime.json"), "utf8"));
    if (!["version", "revision", "platform", "architecture"].every(key => typeof metadata[key] === "string" && metadata[key].length)) return null;
    if (metadata.platform !== process.platform || metadata.architecture !== process.arch) return null;
    const executable = join(root, process.platform === "win32" ? "slint-lsp.exe" : "slint-lsp");
    const executableInfo = await stat(executable);
    if (!executableInfo.isFile() || !executableInfo.size) return null;
    await access(executable, constants.X_OK);
    const [javascript, wasm, html, icon] = await Promise.all([
      readFile(join(root, "wasm/slint_wasm_interpreter.js")),
      readFile(join(root, "wasm/slint_wasm_interpreter_bg.wasm")),
      readFile(join(root, "preview.html"), "utf8"),
      readFile(join(root, "slint.svg")),
    ]);
    if (!javascript.length || !wasm.length || !html.length || !icon.length || Buffer.byteLength(html) >= 1024 * 1024) return null;
    return { metadata, javascript, wasm, html, icon };
  } catch { return null; }
}

export async function publishRuntime(staging, destination) {
  if (!await loadRuntime(staging)) throw new Error("The staged Slint runtime is incomplete.");
  const previous = destination + ".previous-" + process.pid;
  let moved = false;
  try { await rename(destination, previous); moved = true; }
  catch (error) { if (error.code !== "ENOENT") throw error; }
  try { await rename(staging, destination); }
  catch (error) {
    if (moved) await rename(previous, destination);
    throw error;
  }
  if (moved) await rm(previous, { recursive: true, force: true });
}
