// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { once } from "node:events";
import { chmod, copyFile, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

function connect(path = fileURLToPath(new URL("../server.mjs", import.meta.url)), env = {}) {
  const child = spawn(process.execPath, [path], { env: { ...process.env, ...env } });
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
    assert.equal(metadata.javascriptUri, undefined);
    assert(!listing.resources.some(resource => resource.mimeType === "text/javascript"));
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
      const rendered = await client.call("tools/call", { name: "render_slint", arguments: { path, revision, validatedProjectHash: validation.structuredContent.projectHash } });
      assert.equal(rendered.isError, undefined);
      assert.equal(rendered.structuredContent.sourcePath, await realpath(path));
      assert.equal(rendered.structuredContent.revision, revision);
      assert.equal(rendered._meta.preview.source, source);
      assert.equal(rendered.structuredContent.status, "submitted");
      assert.equal(rendered.structuredContent.source, undefined);
      assert.equal(rendered.structuredContent.project, undefined);
      assert.equal(rendered._meta.preview.project.id, rendered.structuredContent.projectHash);
      assert.deepEqual(rendered.content, [{ type: "text", text: "Slint preview submitted." }]);
      assert.equal(rendered.structuredContent.sourceHash, validation.structuredContent.sourceHash);
      assert.equal(rendered.structuredContent.sourceHash, createHash("sha256").update(source).digest("hex"));
      assert.equal(rendered.structuredContent.runtimeRevision, validation.structuredContent.runtimeRevision);
      const query = { previewId: rendered.structuredContent.previewId, revision, sourceHash: rendered.structuredContent.sourceHash };
      assert.equal((await client.call("tools/call", { name: "get_preview_screenshot", arguments: query })).structuredContent.status, "pending");
      const png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a9BkAAAAASUVORK5CYII=";
      const stored = await client.call("tools/call", { name: "publish_preview_capture", arguments: { ...query, captureToken: rendered._meta.captureToken, captureId: rendered._meta.captureId, data: png, capturedAt: Date.now() } });
      assert.equal(stored.structuredContent.status, "ready");
      const screenshot = await client.call("tools/call", { name: "get_preview_screenshot", arguments: query });
      assert.equal(screenshot.content[0].type, "image");
      assert.equal(screenshot.content[0].mimeType, "image/png");
      assert.equal(screenshot.content[0].data, png);
      assert.equal(screenshot.structuredContent.sourceHash, query.sourceHash);
      if (index === 0) {
        const uri = `slint://capture/${query.previewId}/${rendered._meta.captureId}/next`;
        const waiting = client.call("resources/read", { uri });
        const requester = connect();
        let requested;
        try { requested = await requester.call("tools/call", { name: "get_preview_screenshot", arguments: { ...query, fresh: true } }); }
        finally { await requester.close(); }
        assert.equal(requested.structuredContent.status, "pending");
        const captureId = requested.structuredContent.captureId;
        assert.equal(JSON.parse((await waiting).contents[0].text).captureId, captureId);
        assert.equal((await client.call("tools/call", { name: "get_preview_screenshot", arguments: { ...query, captureId: rendered._meta.captureId } })).isError, true);
        await client.call("tools/call", { name: "publish_preview_capture", arguments: { ...query, captureToken: rendered._meta.captureToken, captureId, data: png, capturedAt: Date.now() } });
        const fresh = await client.call("tools/call", { name: "get_preview_screenshot", arguments: { ...query, captureId } });
        assert.equal(fresh.content[0].type, "image");
        assert.equal(fresh.structuredContent.captureId, captureId);
      }
      assert.equal((await client.call("tools/call", { name: "get_preview_screenshot", arguments: { ...query, revision: revision + 1 } })).isError, true);
    }
    const validation = await client.call("tools/call", { name: "validate_slint", arguments: { path, revision: 6 } });
    await writeFile(path, source.replace("press me", "changed"));
    const stale = await client.call("tools/call", { name: "render_slint", arguments: { path, revision: 6, validatedProjectHash: validation.structuredContent.projectHash } });
    assert.equal(stale.isError, true);
    assert.match(stale.structuredContent.message, /changed after validation/);
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
    assert.deepEqual(tools.map(tool => tool.name), ["validate_slint", "render_slint", "get_preview_screenshot", "publish_preview_capture", "open_slint_file", "load_slint_file_preview"]);
    assert.deepEqual(tools.find(tool => tool.name === "publish_preview_capture")._meta.ui.visibility, ["app"]);
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
    for (const arguments_ of [{ source, revision: 1 }, { source, revision: 0 }, { source, revision: 1, width: 0 }]) {
      const rendering = await client.call("tools/call", { name: "render_slint", arguments: arguments_ });
      assert.equal(rendering.isError, true);
    }
  } finally {
    await client.close();
    await rm(directory, { recursive: true, force: true });
  }
});


test("source-only installs start without exposing unavailable preview tools", async () => {
  const directory = await mkdtemp(join(tmpdir(), "slint-source-only-"));
  let client;
  try {
    const root = join(directory, "codex");
    await mkdir(join(root, "examples"), { recursive: true });
    await mkdir(join(root, "components"));
    for (const file of ["server.mjs", "captures.mjs", "project.mjs", "runtime-assets.mjs", "package.json", "examples/button.slint", "components/slint-button.slint"]) {
      await copyFile(new URL("../" + file, import.meta.url), join(root, file));
    }
    await copyFile(new URL("../../icon.svg", import.meta.url), join(directory, "icon.svg"));
    client = connect(join(root, "server.mjs"));
    assert.equal((await client.call("initialize", {})).serverInfo.name, "slint");
    assert.deepEqual((await client.call("tools/list")).tools, []);
    assert.deepEqual((await client.call("resources/list")).resources, []);
    const result = await client.call("tools/call", { name: "render_slint", arguments: {} });
    assert.equal(result.isError, true);
    assert.match(result.structuredContent.message, /Build the Codex runtime/);
    await client.close();
    await mkdir(join(root, "runtime"));
    await copyFile(new URL("../runtime/runtime.json", import.meta.url), join(root, "runtime/runtime.json"));
    client = connect(join(root, "server.mjs"));
    assert.equal((await client.call("initialize", {})).serverInfo.name, "slint");
    assert.deepEqual((await client.call("tools/list")).tools, []);
  } finally {
    await client?.close();
    await rm(directory, { recursive: true, force: true });
  }
});


test("validation reports imported syntax errors consistently", async () => {
  const client = connect();
  const directory = await mkdtemp(join(tmpdir(), "slint-import-diagnostics-"));
  const path = join(directory, "main.slint");
  try {
    await writeFile(path, 'import { Card } from "card.slint"; export component Preview inherits Window {width:320px;height:160px;Card{}}');
    await writeFile(join(directory, "card.slint"), 'export component Card inherits Rectangle { background: ; }');
    {
      const revision = 1;
      const result = await client.call("tools/call", { name: "validate_slint", arguments: { path, revision } });
      assert.equal(result.structuredContent.status, "error");
      assert(result.structuredContent.diagnostics.some(d => d.severity === 1 && d.uri.endsWith("/card.slint")));
    }
    await writeFile(join(directory, "card.slint"), 'export component Card inherits Rectangle { background: blue; }');
    assert.equal((await client.call("tools/call", { name: "validate_slint", arguments: { path, revision: 2 } })).structuredContent.status, "valid");
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});


test("project validation rejects changed imports and assets before rendering", async () => {
  const client = connect();
  const directory = await mkdtemp(join(tmpdir(), "slint-project-validation-"));
  const path = join(directory, "main.slint");
  try {
    await writeFile(path, 'import { Card } from "card.slint"; export component Preview inherits Window {width:320px;height:160px;Card{}}');
    const card = join(directory, "card.slint");
    const image = join(directory, "check.svg");
    await writeFile(card, 'export component Card inherits Rectangle {Image {source:@image-url("check.svg");}}');
    await writeFile(image, '<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>');
    const checked = (await client.call("tools/call", {name:"validate_slint",arguments:{path,revision:1}})).structuredContent;
    assert.equal(checked.status, "valid");
    const args = {path, revision:1, validatedProjectHash:checked.projectHash};
    assert.equal((await client.call("tools/call", {name:"render_slint",arguments:args})).structuredContent.projectHash, checked.projectHash);
    const original = await readFile(card, "utf8");
    await writeFile(card, original + "\n");
    assert.match((await client.call("tools/call", {name:"render_slint",arguments:args})).structuredContent.message, /project changed after validation/);
    await writeFile(card, original);
    await writeFile(image, '<svg xmlns="http://www.w3.org/2000/svg" width="9" height="8"/>');
    assert.match((await client.call("tools/call", {name:"render_slint",arguments:args})).structuredContent.message, /project changed after validation/);
    assert.equal((await client.call("tools/call", {name:"render_slint",arguments:{path,revision:2}})).isError, true);
  } finally {await client.close();await rm(directory,{recursive:true,force:true});}
});


test("a project changed during validation receives no validation token", async () => {
  const directory = await mkdtemp(join(tmpdir(), "slint-validation-race-"));
  const path = join(directory, "main.slint");
  const fake = join(directory, "validator.mjs");
  let client;
  try {
    await writeFile(path, 'import { Card } from "card.slint"; export component Preview inherits Window {Card{}}');
    await writeFile(join(directory, "card.slint"), 'export component Card inherits Rectangle {}');
    await writeFile(fake, '#!/usr/bin/env node\nimport fs from "node:fs"; import {createHash} from "node:crypto"; import path from "node:path"; const entry=process.argv[3]; fs.appendFileSync(path.join(path.dirname(entry),"card.slint"),"\\n"); console.log(JSON.stringify({status:"valid", sourceHash:createHash("sha256").update(fs.readFileSync(entry)).digest("hex")}));');
    await chmod(fake, 0o755);
    client = connect(undefined, { SLINT_PYTHON_BIN: fake });
    const result = await client.call("tools/call", {name:"validate_slint",arguments:{path,revision:1}});
    assert.equal(result.isError, true);
    assert.equal(result.structuredContent.projectHash, undefined);
    assert.match(result.structuredContent.message, /project changed during validation/);
  } finally {await client?.close();await rm(directory,{recursive:true,force:true});}
});

test("file entrypoints render host buffers with imports without overwriting saved source", async () => {
  const client = connect();
  const directory = await mkdtemp(join(tmpdir(), "slint-file-view-"));
  const path = join(directory, "main.slint");
  try {
    const saved = 'import { Card } from "card.slint"; export component Preview inherits Window {width:320px;height:160px;Card{}}';
    await writeFile(path, saved);
    await writeFile(join(directory, "card.slint"), 'export component Card inherits Rectangle { background: red; }');
    const opened = await client.call("tools/call", { name: "open_slint_file", arguments: { file: { name: "main.slint", resourceUri: "host-resource://opaque" } } });
    assert.equal(opened.structuredContent.status, "opening-file");
    assert.equal(opened._meta.file.resourceUri, "host-resource://opaque");
    const tool = (await client.call("tools/list")).tools.find(tool => tool.name === "open_slint_file");
    assert.deepEqual(tool._meta["openai/ui"].entrypoints, [{ type: "file", extensions: [".slint"] }]);
    const source = saved.replace("320px", "400px");
    const args = { source, revision: 1 };
    assert.equal((await client.call("tools/call", { name: "load_slint_file_preview", arguments: args })).isError, true);
    const result = await client.call("tools/call", { name: "load_slint_file_preview", arguments: args, _meta: { "openai/resource": { path } } });
    assert.equal(result.isError, undefined, JSON.stringify(result));
    assert.equal(result.structuredContent.sourceState, "unsaved");
    assert.equal(result._meta.preview.source, source);
    assert(result._meta.preview.project.files["card.slint"]);
    assert.equal(await readFile(path, "utf8"), saved);
    await writeFile(path, source);
    const next = await client.call("tools/call", { name: "load_slint_file_preview", arguments: { source, revision: 2 }, _meta: { "openai/resource": { path } } });
    assert.equal(next.structuredContent.sourceState, "saved");
    assert.equal(next.structuredContent.sourceHash, result.structuredContent.sourceHash);
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});
