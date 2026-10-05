import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { createInterface } from "node:readline";
import { gunzipSync } from "node:zlib";

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
  const server = inventory.data.find(entry => entry.pluginId === "slint@slint-prototype");
  assert(server, "Install and enable Slint from Slint Prototype before running this test.");
  assert.equal(server.toolsError, null);
  const preview = server.resources.find(resource => resource.mimeType === "text/html;profile=mcp-app");
  assert(preview, "The installed plugin has no preview resource.");
  const read = uri => call("mcpServer/resource/read", { server: server.name, uri });
  const html = (await read(preview.uri)).contents[0].text;
  assert(Buffer.byteLength(html) < 1024 * 1024);
  assert(!html.includes("127.0.0.1"));
  const metadata = JSON.parse(html.match(/<script id="slint-runtime" type="application\/json">(.*?)<\/script>/)[1]);
  const javascript = (await read(metadata.javascriptUri)).contents[0].text;
  assert(javascript.includes("compile_from_string"));
  const chunks = [];
  for (const uri of metadata.wasmChunkUris) {
    const bytes = Buffer.from((await read(uri)).contents[0].blob, "base64");
    assert(bytes.length <= 256 * 1024);
    chunks.push(bytes);
  }
  const wasm = gunzipSync(Buffer.concat(chunks));
  assert.equal(createHash("sha256").update(wasm).digest("hex"), metadata.wasmHash);
  console.log(JSON.stringify({ status: "passed", server: server.name, preview: preview.uri, htmlBytes: Buffer.byteLength(html), runtimeVersion: metadata.version, runtimeReads: chunks.length + 1 }));
} finally {
  child.kill();
}
