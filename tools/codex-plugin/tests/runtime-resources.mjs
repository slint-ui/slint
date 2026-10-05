import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { readFile } from "node:fs/promises";
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
