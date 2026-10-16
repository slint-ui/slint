// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import {
    existsSync,
    mkdirSync,
    mkdtempSync,
    readFileSync,
    rmSync,
    writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
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
        env: { ...process.env, SLINT_PREVIEW_SCREENSHOT_TIMEOUT_MS: "300" },
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

test("lists preview_slint with its view, and the view's own tools for the view only", async () => {
    const { result } = await request("tools/list");
    const preview = result.tools.find((t) => t.name === "preview_slint");
    assert.equal(preview._meta.ui.resourceUri, "ui://slint/preview.html");
    for (const name of ["read_slint_file", "publish_preview_capture"]) {
        const tool = result.tools.find((t) => t.name === name);
        assert.deepEqual(tool._meta.ui.visibility, ["app"], name);
    }
    const screenshot = result.tools.find(
        (t) => t.name === "get_preview_screenshot",
    );
    assert.equal(screenshot._meta, undefined);
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
    assert.ok(csp.resourceDomains.includes("blob:"));
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

test("takes either an absolute path or source", async () => {
    const call = (args) =>
        request("tools/call", { name: "preview_slint", arguments: args });
    assert.ok((await call({ path: "/project/main.slint" })).result);
    for (const args of [
        {},
        { path: "/project/main.slint", source: "" },
        { path: "main.slint" },
    ]) {
        assert.equal(
            (await call(args)).error?.code,
            -32602,
            JSON.stringify(args),
        );
    }
});

test("reads a file with its local images inlined", async () => {
    const dir = mkdtempSync(join(tmpdir(), "slint-preview-"));
    try {
        mkdirSync(join(dir, "ui"));
        mkdirSync(join(dir, "images"));
        writeFileSync(
            join(dir, "images", "dot.png"),
            Buffer.from([0x89, 0x50]),
        );
        writeFileSync(join(dir, "images", "icon.svg"), "<svg/>");
        const file = join(dir, "ui", "main.slint");
        writeFileSync(
            file,
            [
                '@image-url("../images/dot.png")',
                `@image-url( "${join(dir, "images", "icon.svg")}", nine-slice(1))`,
                '@image-url("missing.png")',
                '@image-url("https://example.com/a.png")',
                '@image-url("data:image/png;base64,AA==")',
            ].join("\n"),
        );

        const { result } = await request("tools/call", {
            name: "read_slint_file",
            arguments: { path: file },
        });
        assert.equal(result.isError, undefined);
        assert.deepEqual(result.content[0].text.split("\n"), [
            '@image-url("data:image/png;base64,iVA=")',
            `@image-url("data:image/svg+xml;base64,${Buffer.from("<svg/>").toString("base64")}", nine-slice(1))`,
            '@image-url("missing.png")',
            '@image-url("https://example.com/a.png")',
            '@image-url("data:image/png;base64,AA==")',
        ]);
    } finally {
        rmSync(dir, { recursive: true });
    }
});

test("reports a file it can't read as a tool error", async () => {
    const { result } = await request("tools/call", {
        name: "read_slint_file",
        arguments: { path: "/nonexistent/main.slint" },
    });
    assert.equal(result.isError, true);
    assert.match(result.content[0].text, /ENOENT/);
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

// The key the view computes from the preview_slint arguments.
function previewKey({ path, source, style, channel }) {
    const json = JSON.stringify([
        path ?? null,
        source ?? null,
        style ?? null,
        channel ?? null,
    ]);
    return createHash("sha256").update(json).digest("hex");
}

const callTool = async (name, args) =>
    (await request("tools/call", { name, arguments: args })).result;

test("returns the capture the view publishes for the latest preview", async () => {
    const args = { source: "export component A {}", style: "fluent" };
    await callTool("preview_slint", args);
    const screenshot = callTool("get_preview_screenshot", {});
    await callTool("publish_preview_capture", {
        key: previewKey(args),
        png: "iVBORw0K",
        width: 320,
        height: 120,
    });
    const { content, isError } = await screenshot;
    assert.equal(isError, undefined);
    assert.deepEqual(content[0], {
        type: "image",
        data: "iVBORw0K",
        mimeType: "image/png",
    });
    assert.match(content[1].text, /320×120/);

    // A capture that already arrived is returned right away.
    const again = await callTool("get_preview_screenshot", {});
    assert.equal(again.content[0].type, "image");
});

test("reports why a preview has no screenshot", async () => {
    const args = { source: "export component A { colour: red; }" };
    await callTool("preview_slint", args);
    await callTool("publish_preview_capture", {
        key: previewKey(args),
        error: "main.slint:1:23: error: Unknown property colour",
    });
    const { content } = await callTool("get_preview_screenshot", {});
    assert.match(content[0].text, /Unknown property colour/);
});

test("ignores captures of other previews and times out", async () => {
    const args = { source: "export component B {}" };
    await callTool("preview_slint", args);
    await callTool("publish_preview_capture", {
        key: previewKey({ source: "export component C {}" }),
        png: "iVBORw0K",
    });
    const { content } = await callTool("get_preview_screenshot", {});
    assert.match(content[0].text, /No screenshot arrived/);
});

test("forgets the capture when the same preview is requested again", async () => {
    const args = { path: "/project/main.slint" };
    await callTool("preview_slint", args);
    await callTool("publish_preview_capture", {
        key: previewKey(args),
        png: "iVBORw0K",
    });
    await callTool("preview_slint", args);
    const { content } = await callTool("get_preview_screenshot", {});
    assert.match(content[0].text, /No screenshot arrived/);
});

test("tells a waiting call that a newer preview replaced it", async () => {
    await callTool("preview_slint", { source: "export component D {}" });
    const screenshot = callTool("get_preview_screenshot", {});
    await callTool("preview_slint", { source: "export component E {}" });
    const { content } = await screenshot;
    assert.match(content[0].text, /newer preview_slint call/);
});
