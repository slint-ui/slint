// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmod, copyFile, cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const pluginRoot = fileURLToPath(new URL("../../", import.meta.url));
const bridgeExecutable = process.platform === "win32" ? "slint-editor-mcp.exe" : "slint-editor-mcp";
const lspExecutable = process.platform === "win32" ? "slint-lsp.exe" : "slint-lsp";

function run(command, argumentsList, options = {}) {
  const result = spawnSync(command, argumentsList, { encoding: "utf8", timeout: 15000, ...options });
  assert.ifError(result.error);
  return result;
}

test("the relocated editor bridge exposes all tools without preview assets", async () => {
  const directory = await mkdtemp(join(tmpdir(), "slint editor bridge "));
  const plugin = join(directory, "installed plugin");
  try {
    await mkdir(join(plugin, "runtime"), { recursive: true });
    await copyFile(join(pluginRoot, "codex/editor-mcp.mjs"), join(plugin, "editor-mcp.mjs"));
    const executable = join(plugin, "runtime", bridgeExecutable);
    await copyFile(join(pluginRoot, "codex/runtime", bridgeExecutable), executable);
    await chmod(executable, 0o755);
    const requests = [
      { id: 1, method: "initialize" },
      { id: 2, method: "tools/list" },
      { id: 3, method: "tools/call", params: { name: "discover_visual_editors", arguments: { workingDirectory: "relative" } } },
    ].map(request => JSON.stringify({ jsonrpc: "2.0", ...request })).join("\n") + "\n";
    const result = run(process.execPath, [join(plugin, "editor-mcp.mjs")], { cwd: directory, input: requests });
    assert.equal(result.status, 0, result.stderr);
    const responses = result.stdout.trim().split("\n").map(line => JSON.parse(line));
    assert.equal(responses[0].result.serverInfo.name, "slint-editor-mcp");
    assert.deepEqual(responses[0].result.capabilities, { tools: {} });
    assert.deepEqual(responses[1].result.tools.map(tool => tool.name).sort(), [
      "discover_visual_editors", "register_visual_editor_chat", "reply_visual_editor_annotation",
      "resolve_visual_editor_annotation", "screenshot_visual_editor_canvas",
    ]);
    assert.equal(responses[2].result.isError, true);
    await rm(executable);
    const missing = run(process.execPath, [join(plugin, "editor-mcp.mjs")], { cwd: directory });
    assert.equal(missing.status, 1);
    assert.equal(missing.stdout, "");
    assert.match(missing.stderr, /Build the plugin runtime or install a platform package/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("the package bundles the bridge, configuration, and skill with executable permissions", async () => {
  const directory = await mkdtemp(join(tmpdir(), "slint-plugin-package-"));
  const plugin = join(directory, "plugin");
  const runtime = join(plugin, "codex/runtime");
  const archive = join(directory, "Slint.zip");
  try {
    await cp(pluginRoot, plugin, {
      recursive: true, dereference: true,
      filter: path => !["runtime", "node_modules"].includes(basename(path)) && !basename(path).startsWith(".runtime-build-"),
    });
    await mkdir(join(runtime, "wasm"), { recursive: true });
    await writeFile(join(runtime, "runtime.json"), JSON.stringify({ version: "1.19.0", revision: "test", platform: process.platform, architecture: process.arch }));
    for (const [path, contents] of [[lspExecutable, "test executable"], ["preview.html", "<!doctype html>"], ["slint.svg", "<svg/>"], ["wasm/slint_wasm_interpreter_bg.wasm", "test wasm"]]) {
      await writeFile(join(runtime, path), contents);
    }
    await copyFile(join(pluginRoot, "codex/runtime", bridgeExecutable), join(runtime, bridgeExecutable));
    for (const executable of [lspExecutable, bridgeExecutable]) await chmod(join(runtime, executable), 0o755);
    const script = join(plugin, "codex/scripts/package-plugin.mjs");
    const packaged = run(process.execPath, [script, archive]);
    assert.equal(packaged.status, 0, packaged.stderr);
    const inspected = run(process.env.SLINT_PYTHON_BIN || "python3", ["-c", "import json,os,sys,zipfile\nwith zipfile.ZipFile(sys.argv[1]) as archive:\n print(json.dumps({entry.filename: (entry.external_attr >> 16) & 0o777 for entry in archive.infolist()}))\n for entry in archive.infolist():\n  path=archive.extract(entry,sys.argv[2])\n  os.chmod(path,(entry.external_attr >> 16) & 0o777)", archive, join(directory, "extracted")]);
    assert.equal(inspected.status, 0, inspected.stderr);
    const entries = JSON.parse(inspected.stdout);
    const root = "slint/plugins/slint/";
    for (const executable of [lspExecutable, bridgeExecutable]) assert.equal(entries[`${root}codex/runtime/${executable}`], 0o755);
    for (const path of ["codex/editor-mcp.mjs", "codex/mcp.json", "skills/visual-editor-comments/SKILL.md"]) assert.equal(entries[root + path], 0o644);
    assert(!Object.keys(entries).some(path => path.includes("/tests/") || path.includes("node_modules")));
    const installed = join(directory, "extracted", root);
    const manifest = JSON.parse(await readFile(join(installed, ".codex-plugin/plugin.json"), "utf8"));
    const configuration = JSON.parse(await readFile(join(installed, manifest.mcpServers), "utf8"));
    assert.deepEqual(Object.keys(configuration.mcpServers).sort(), ["slint", "slint-docs", "slint-visual-editor"]);
    const bridge = configuration.mcpServers["slint-visual-editor"];
    const launched = run(bridge.command, bridge.args, { cwd: installed, input: '{"jsonrpc":"2.0","id":1,"method":"tools/list"}\n' });
    assert.equal(launched.status, 0, launched.stderr);
    assert.equal(JSON.parse(launched.stdout).result.tools.length, 5);
    await rm(join(runtime, bridgeExecutable));
    const incomplete = run(process.execPath, [script, join(directory, "incomplete.zip")]);
    assert.notEqual(incomplete.status, 0);
    assert.match(incomplete.stderr, /slint-editor-mcp/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});
