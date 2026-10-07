// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";
import { snapshotProject } from "../project.mjs";

const child = spawn(process.argv[2] || "codex", ["app-server"]);
const pending = new Map();
let nextId = 0;
createInterface({ input: child.stdout }).on("line", line => {
  const message = JSON.parse(line);
  if (message.id === undefined) return;
  pending.get(message.id)?.(message);
  pending.delete(message.id);
});
child.on("error", error => {
  for (const request of pending.values()) request({ error: { message: error.message } });
});
function call(method, params) {
  return new Promise((resolve, reject) => {
    const id = ++nextId;
    const timer = setTimeout(() => reject(new Error(`${method} timed out`)), 25000);
    pending.set(id, message => {
      clearTimeout(timer);
      if (message.error) reject(new Error(JSON.stringify(message.error)));
      else resolve(message.result);
    });
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
}

try {
  await call("initialize", { clientInfo: { name: "slint-resource-test", version: "1" }, capabilities: { experimentalApi: true } });
  child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "initialized" }) + "\n");
  const inventory = await call("mcpServerStatus/list", { limit: 100, detail: "full" });
  const server = inventory.data.find(entry => entry.pluginId === "slint@slint" && entry.tools?.render_slint);
  assert(server, "Install and enable Slint from Slint before running this test.");
  assert.equal(server.toolsError, null);
  const preview = server.resources.find(resource => resource.mimeType === "text/html;profile=mcp-app");
  assert(preview, "The installed plugin has no preview resource.");
  const read = uri => call("mcpServer/resource/read", { server: server.name, uri });
  const html = (await read(preview.uri)).contents[0].text;
  assert(Buffer.byteLength(html) < 1024 * 1024);
  assert(!html.includes("127.0.0.1"));
  const metadata = JSON.parse(html.match(/<script id="slint-runtime" type="application\/json">(.*?)<\/script>/)[1]);
  assert.equal(metadata.javascriptUri, undefined);
  const chunks = [];
  for (const uri of metadata.wasmChunkUris) {
    const bytes = Buffer.from((await read(uri)).contents[0].blob, "base64");
    assert(bytes.length <= 256 * 1024);
    chunks.push(bytes);
  }
  const wasm = gunzipSync(Buffer.concat(chunks));
  assert.equal(createHash("sha256").update(wasm).digest("hex"), metadata.wasmHash);
  const project = await snapshotProject(fileURLToPath(new URL("../examples/button.slint", import.meta.url)), undefined, await readFile(new URL("../components/slint-button.slint", import.meta.url), "utf8"));
  for (const file of Object.values(project.files)) {
    const bytes = [];
    for (const uri of file.uris) bytes.push(Buffer.from((await read(uri)).contents[0].blob, "base64"));
    assert.equal(createHash("sha256").update(Buffer.concat(bytes)).digest("hex"), file.hash);
  }
  console.log(JSON.stringify({ status: "passed", server: server.name, preview: preview.uri, htmlBytes: Buffer.byteLength(html), runtimeVersion: metadata.version, runtimeReads: chunks.length, projectFiles: Object.keys(project.files).length }));
} finally {
  child.kill();
}
