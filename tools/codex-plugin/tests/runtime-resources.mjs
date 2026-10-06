// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { once } from "node:events";
import { mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

function connect() {
  const child = spawn(process.execPath, [fileURLToPath(new URL("../server.mjs", import.meta.url))]);
  const pending = new Map();
  let nextId = 0;
  let closed = false;
  createInterface({ input: child.stdout }).on("line", line => {
    const message = JSON.parse(line);
    pending.get(message.id)?.(message);
    pending.delete(message.id);
  });
  return {
    async call(method, params) {
      const id = ++nextId;
      const response = await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error(`MCP ${method} timed out`)), 15000);
        pending.set(id, message => { clearTimeout(timer); resolve(message); });
        child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
      });
      assert.equal(response.error, undefined, JSON.stringify(response.error));
      return response.result;
    },
    async close() {
      if (closed) return;
      closed = true;
      const exited = once(child, "exit");
      child.stdin.end();
      const [code] = await exited;
      assert.equal(code, 0);
    },
  };
}

test("cached preview resources survive a server restart without large HTML or loopback URLs", async () => {
  let client = connect();
  try {
    const listing = await client.call("resources/list");
    const preview = listing.resources.find(resource => resource.mimeType === "text/html;profile=mcp-app");
    const html = (await client.call("resources/read", { uri: preview.uri })).contents[0].text;
    assert(Buffer.byteLength(html) < 1024 * 1024);
    assert(!html.includes("127.0.0.1"));
    const metadata = JSON.parse(html.match(/<script id="slint-runtime" type="application\/json">(.*?)<\/script>/)[1]);
    await client.close();
    client = connect();
    const javascript = (await client.call("resources/read", { uri: metadata.javascriptUri })).contents[0].text;
    assert.equal(javascript, await readFile(new URL("../runtime/wasm/slint_wasm_interpreter.js", import.meta.url), "utf8"));
    const chunks = [];
    for (const uri of metadata.wasmChunkUris) {
      const resource = (await client.call("resources/read", { uri })).contents[0];
      const bytes = Buffer.from(resource.blob, "base64");
      assert(bytes.length <= 256 * 1024);
      chunks.push(bytes);
    }
    assert.deepEqual(gunzipSync(Buffer.concat(chunks)), await readFile(new URL("../runtime/wasm/slint_wasm_interpreter_bg.wasm", import.meta.url)));
  } finally {
    await client.close();
  }
});

test("file-backed edits retain identity and reject stale validation", async () => {
  const client = connect();
  const directory = await mkdtemp(join(tmpdir(), "slint-edit-cycle-"));
  const path = join(directory, "button.slint");
  try {
    let source = await readFile(new URL("../examples/button.slint", import.meta.url), "utf8");
    const edits = [text => text, text => text.replace("#dc2626", "#2563eb"), text => text.replace("click me!", "press me"), text => text.replace("200px", "224px"), text => text.replaceAll("\n", "\r\n")];
    for (let index = 0; index < edits.length; index++) {
      source = edits[index](source);
      await writeFile(path, source);
      const revision = index + 1;
      const validation = await client.call("tools/call", { name: "validate_slint", arguments: { path, revision } });
      assert.equal(validation.structuredContent.status, "valid");
      const rendered = await client.call("tools/call", { name: "render_slint", arguments: { path, revision, validatedSourceHash: validation.structuredContent.sourceHash } });
      assert.equal(rendered.isError, undefined);
      assert.equal(rendered.structuredContent.sourcePath, await realpath(path));
      assert.equal(rendered.structuredContent.revision, revision);
      assert.equal(rendered.structuredContent.source, source);
      assert.equal(rendered.structuredContent.sourceHash, validation.structuredContent.sourceHash);
    }
    const validation = await client.call("tools/call", { name: "validate_slint", arguments: { path, revision: 6 } });
    await writeFile(path, source.replace("press me", "changed"));
    const stale = await client.call("tools/call", { name: "render_slint", arguments: { path, revision: 6, validatedSourceHash: validation.structuredContent.sourceHash } });
    assert.equal(stale.isError, true);
    assert.match(stale.structuredContent.message, /changed after validation/);
  } finally {
    await client.close();
    await rm(directory, { recursive: true, force: true });
  }
});

test("red and blue source revisions pass the bundled LSP and render the exact validated bytes", async () => {
  const client = connect();
  const directory = await mkdtemp(join(tmpdir(), "slint-source-test-"));
  const path = join(directory, "button.slint");
  try {
    const example = await readFile(new URL("../examples/button.slint", import.meta.url), "utf8");
    for (const [revision, color] of [[1, "#dc2626"], [2, "#2563eb"]]) {
      const source = example.replace("background-color: #dc2626;", `background-color: ${color};`);
      await writeFile(path, source);
      const validation = await client.call("tools/call", { name: "validate_slint", arguments: { path, revision } });
      assert.equal(validation.isError, undefined);
      assert.equal(validation.structuredContent.status, "valid");
      assert.equal(validation.structuredContent.sourceHash, createHash("sha256").update(source).digest("hex"));
      const rendering = await client.call("tools/call", { name: "render_slint", arguments: { source, revision, width: 320, height: 160 } });
      assert.equal(rendering.isError, undefined);
      assert.equal(rendering.structuredContent.source, source);
      assert.equal(rendering.structuredContent.revision, revision);
      assert.equal(rendering.structuredContent.sourceHash, validation.structuredContent.sourceHash);
      assert.equal(rendering.structuredContent.runtimeRevision, validation.structuredContent.runtimeRevision);
    }
  } finally {
    await client.close();
    await rm(directory, { recursive: true, force: true });
  }
});

test("invalid Slint and render arguments return errors", async () => {
  const client = connect();
  const directory = await mkdtemp(join(tmpdir(), "slint-invalid-test-"));
  const path = join(directory, "invalid.slint");
  try {
    const { tools } = await client.call("tools/list");
    const statuses = tools.find(tool => tool.name === "validate_slint").outputSchema.properties.status.enum;
    assert.deepEqual(statuses, ["valid", "error", "failure"]);
    const source = "export component Broken inherits Window { width: banana; }";
    await writeFile(path, source);
    const validation = await client.call("tools/call", { name: "validate_slint", arguments: { path, revision: 1 } });
    assert.equal(validation.isError, true);
    assert.equal(validation.structuredContent.status, "error");
    assert(validation.structuredContent.diagnostics.some(diagnostic => diagnostic.severity === 1));
    const missing = await client.call("tools/call", { name: "validate_slint", arguments: { path: join(directory, "missing.slint"), revision: 1 } });
    assert.equal(missing.isError, true);
    assert.equal(missing.structuredContent.status, "failure");
    for (const arguments_ of [{ source, revision: 0 }, { source, revision: 1, width: 0 }]) {
      const rendering = await client.call("tools/call", { name: "render_slint", arguments: arguments_ });
      assert.equal(rendering.isError, true);
    }
  } finally {
    await client.close();
    await rm(directory, { recursive: true, force: true });
  }
});
