// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { after, before, test } from "node:test";
import assert from "node:assert/strict";

const pluginRoot = new URL("../../../", import.meta.url);
const serverPath = new URL("../server.mjs", import.meta.url);
const readManifest = (path) =>
    JSON.parse(readFileSync(new URL(path, pluginRoot), "utf8"));

let server;
let nextId = 1;
const pending = new Map();

function request(method, params) {
    const id = nextId++;
    server.stdin.write(
        JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n",
    );
    return new Promise((resolve) => pending.set(id, resolve));
}

before(async () => {
    server = spawn(process.execPath, [serverPath.pathname], {
        stdio: ["pipe", "pipe", "inherit"],
    });
    createInterface({ input: server.stdout }).on("line", (line) => {
        const message = JSON.parse(line);
        pending.get(message.id)?.(message);
        pending.delete(message.id);
    });
    await request("initialize", {
        protocolVersion: "2025-11-25",
        capabilities: {},
    });
});

after(() => server.kill());

test("reports the plugin version", async () => {
    const { result } = await request("initialize", {
        protocolVersion: "2025-11-25",
    });
    assert.equal(
        result.serverInfo.version,
        readManifest(".claude-plugin/plugin.json").version,
    );
});

test("lists preview_slint with its view", async () => {
    const { result } = await request("tools/list");
    const tool = result.tools.find((t) => t.name === "preview_slint");
    assert.equal(tool._meta.ui.resourceUri, "ui://slint/preview.html");
    assert.deepEqual(tool.inputSchema.required, ["source"]);
});

test("serves the view as an MCP App with the interpreter hosts allowed", async () => {
    const { result } = await request("resources/read", {
        uri: "ui://slint/preview.html",
    });
    const [content] = result.contents;
    assert.equal(content.mimeType, "text/html;profile=mcp-app");
    assert.doesNotMatch(content.text, /__[A-Z]+_URL__/);

    const { csp } = content._meta.ui;
    const urls = [
        "https://releases.slint.dev/1.18.1/wasm-interpreter/slint_wasm_interpreter.js",
        "https://snapshots.slint.dev/master/wasm-interpreter/slint_wasm_interpreter.js",
    ];
    for (const url of urls) {
        assert.ok(content.text.includes(url), url);
        const { origin } = new URL(url);
        assert.ok(csp.resourceDomains.includes(origin), origin);
        assert.ok(csp.connectDomains.includes(origin), origin);
    }
});

test("lists the view resource", async () => {
    const { result } = await request("resources/list");
    assert.deepEqual(
        result.resources.map((r) => [r.uri, r.mimeType]),
        [["ui://slint/preview.html", "text/html;profile=mcp-app"]],
    );
});

test("answers a call with text only", async () => {
    const { result } = await request("tools/call", {
        name: "preview_slint",
        arguments: { source: "export component Main {}" },
    });
    assert.equal(result.content[0].type, "text");
    assert.equal(result.structuredContent, undefined);
});

test("rejects a call without source", async () => {
    const { error } = await request("tools/call", {
        name: "preview_slint",
        arguments: {},
    });
    assert.equal(error.code, -32602);
});

test("rejects unknown tools, resources, and methods", async () => {
    assert.equal(
        (await request("tools/call", { name: "nope", arguments: {} })).error
            .code,
        -32602,
    );
    assert.equal(
        (await request("resources/read", { uri: "ui://nope" })).error.code,
        -32002,
    );
    assert.equal((await request("nope")).error.code, -32601);
});

test("the Claude and Codex manifests start this server", () => {
    const claude = readManifest(".claude-plugin/plugin.json").mcpServers[
        "slint-preview"
    ];
    assert.deepEqual(claude.args, [
        "${CLAUDE_PLUGIN_ROOT}/mcp/slint-preview/server.mjs",
    ]);

    // Codex expands no variables in plugin MCP args and resolves `cwd` against the plugin root.
    const codex = readManifest(".codex-plugin/plugin.json").mcpServers[
        "slint-preview"
    ];
    assert.equal(codex.cwd, ".");
    assert.ok(existsSync(new URL(codex.args[0], pluginRoot)));
});
