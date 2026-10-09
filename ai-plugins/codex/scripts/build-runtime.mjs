// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { spawn } from "node:child_process";
import { execFileSync } from "node:child_process";
import { readFile, writeFile, mkdtemp, rm, copyFile, chmod } from "node:fs/promises";
import { publishRuntime } from "../runtime-assets.mjs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const repository = dirname(dirname(pluginRoot));
const destination = join(pluginRoot, "runtime");
const target = process.env.CARGO_TARGET_DIR || join(repository, "target");
const revision = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repository, encoding: "utf8" }).trim();
const trackedChanges = execFileSync("git", ["diff", "HEAD", "--", "Cargo.toml", "Cargo.lock", "api", "internal", "tools/lsp", "tools/editor", "tools/editor-mcp"], { cwd: repository, encoding: "utf8" });
if (trackedChanges) throw new Error("Commit Slint runtime source changes before building the plugin runtime.");

async function run(command, args, cwd = repository, extra = {}) {
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, stdio: "inherit", env: { ...process.env, CARGO_TARGET_DIR: target, ...extra } });
    child.on("error", reject);
    child.on("exit", code => code === 0 ? resolve() : reject(new Error(`${command} exited with ${code}`)));
  });
}

const runtime = await mkdtemp(join(pluginRoot, ".runtime-build-"));
try {
  await run("cargo", ["build", "--locked", "-p", "slint-lsp", "--bin", "slint-lsp", "--no-default-features", "--features", "backend-winit,renderer-software"]);
  await run("cargo", ["build", "--locked", "-p", "slint-editor-mcp", "--bin", "slint-editor-mcp"]);
  await run("wasm-pack", ["build", "--release", "--target", "web", "--no-opt", "--out-dir", join(runtime, "wasm"), "--", "--locked", "--features", "console_error_panic_hook"], join(repository, "api/wasm-interpreter"));
  const executables = ["slint-lsp", "slint-editor-mcp"].map(name => process.platform === "win32" ? `${name}.exe` : name);
  for (const executable of executables) {
    await copyFile(join(target, "debug", executable), join(runtime, executable));
    await chmod(join(runtime, executable), 0o755);
    if (process.platform !== "win32") await run("strip", ["-S", join(runtime, executable)]);
  }
  const executable = executables[0];
  const lspVersion = execFileSync(join(runtime, executable), ["--version"], { encoding: "utf8" }).trim();
  const wasmPackage = JSON.parse(await readFile(join(runtime, "wasm/package.json"), "utf8"));
  if (lspVersion !== `slint-lsp ${wasmPackage.version}`) throw new Error("LSP and Wasm versions differ.");
  const currentRevision = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repository, encoding: "utf8" }).trim();
  if (currentRevision !== revision) throw new Error("Repository revision changed during the build. Rebuild the runtime.");
  await writeFile(join(runtime, "runtime.json"), JSON.stringify({ version: wasmPackage.version, revision, platform: process.platform, architecture: process.arch }, null, 2) + "\n");
  await run(process.execPath, [join(pluginRoot, "scripts/build-preview.mjs")], repository, { SLINT_PLUGIN_RUNTIME_DIR: runtime });
  await publishRuntime(runtime, destination);
  console.log(`Built Slint ${wasmPackage.version} from ${revision}.`);
} finally { await rm(runtime, { recursive: true, force: true }); }
