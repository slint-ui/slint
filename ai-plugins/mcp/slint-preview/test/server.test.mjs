// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { spawn } from "node:child_process";
import { once } from "node:events";
import {
    cpSync,
    existsSync,
    mkdtempSync,
    readFileSync,
    rmSync,
    writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { pathToFileURL } from "node:url";
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

const callTool = async (name, args) =>
    (await request("tools/call", { name, arguments: args })).result;

before(async () => {
    server = spawn(process.execPath, [serverPath.pathname], {
        stdio: ["pipe", "pipe", "inherit"],
        env: {
            ...process.env,
            SLINT_PREVIEW_VIEW_START_TIMEOUT_MS: "200",
            SLINT_PREVIEW_SCREENSHOT_TIMEOUT_MS: "400",
            SLINT_PREVIEW_TIMEOUT_MS: "700",
        },
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
    for (const name of [
        "read_slint_file",
        "read_binary_file",
        "publish_preview_capture",
    ]) {
        const tool = result.tools.find((t) => t.name === name);
        assert.deepEqual(tool._meta.ui.visibility, ["app"], name);
    }
    const screenshot = result.tools.find(
        (t) => t.name === "get_preview_screenshot",
    );
    assert.equal(screenshot._meta, undefined);
});

function assertServesInterpreter(content, url, hosts) {
    assert.equal(content.mimeType, "text/html;profile=mcp-app");
    assert.doesNotMatch(content.text, /__[A-Z_]+__/);
    assert.ok(content.text.includes(url), url);
    const { csp } = content._meta.ui;
    assert.deepEqual(csp.connectDomains, hosts);
    assert.deepEqual(csp.resourceDomains, [...hosts, "blob:"]);
}

test("serves the view with the snapshot interpreter", async () => {
    const { result } = await request("resources/read", {
        uri: "ui://slint/preview.html",
    });
    assertServesInterpreter(
        result.contents[0],
        "https://snapshots.slint.dev/master/wasm-interpreter/slint_wasm_interpreter.js",
        ["https://snapshots.slint.dev"],
    );
});

test("serves the view with the interpreter of the plugin's release on the release channel", async () => {
    const root = mkdtempSync(join(tmpdir(), "slint-preview-release-"));
    try {
        for (const file of [
            ".claude-plugin/plugin.json",
            "assets/slint.svg",
            "mcp/slint-preview/server.mjs",
            "mcp/slint-preview/view.html",
        ]) {
            cpSync(new URL(file, pluginRoot), join(root, file));
        }
        writeFileSync(
            join(root, "mcp/slint-preview/channel.json"),
            JSON.stringify({ channel: "release" }),
        );
        const release = spawn(
            process.execPath,
            [join(root, "mcp/slint-preview/server.mjs")],
            { stdio: ["pipe", "pipe", "inherit"] },
        );
        const lines = createInterface({ input: release.stdout });
        release.stdin.write(
            JSON.stringify({
                jsonrpc: "2.0",
                id: 1,
                method: "resources/read",
                params: { uri: "ui://slint/preview.html" },
            }) + "\n",
        );
        const [line] = await once(lines, "line");
        release.kill();
        const { version } = readManifest(".claude-plugin/plugin.json");
        assertServesInterpreter(
            JSON.parse(line).result.contents[0],
            `https://releases.slint.dev/${version}/wasm-interpreter/slint_wasm_interpreter.js`,
            ["https://releases.slint.dev", "https://docs.slint.dev"],
        );
    } finally {
        rmSync(root, { recursive: true, force: true });
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

test("reads Slint, image, and font files for the view", async () => {
    const dir = mkdtempSync(join(tmpdir(), "slint-preview-"));
    try {
        const slintFile = join(dir, "main.slint");
        const source = 'Image { source: @image-url("dot.png"); }';
        writeFileSync(slintFile, source);
        const slint = await callTool("read_slint_file", { path: slintFile });
        assert.deepEqual(slint.content, [{ type: "text", text: source }]);

        for (const [file, mimeType, bytes] of [
            ["dot.png", "image/png", Buffer.from([0x89, 0x50])],
            ["icon.svg", "image/svg+xml", Buffer.from("<svg/>")],
            ["font.ttf", "font/ttf", Buffer.from([0, 1, 0, 0])],
        ]) {
            const path = join(dir, file);
            writeFileSync(path, bytes);
            const blob = bytes.toString("base64");
            const result = await callTool("read_binary_file", { path });
            assert.deepEqual(
                result.content,
                [
                    {
                        type: "resource",
                        resource: {
                            uri: pathToFileURL(path).href,
                            mimeType,
                            blob,
                        },
                    },
                ],
                file,
            );
        }
    } finally {
        rmSync(dir, { recursive: true });
    }
});

test("reports a file it can't read as a tool error", async () => {
    for (const name of ["read_slint_file", "read_binary_file"]) {
        const result = await callTool(name, {
            path: "/nonexistent/main.slint",
        });
        assert.equal(result.isError, true, name);
        assert.match(result.content[0].text, /ENOENT/, name);
    }
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

// Starts a preview and returns its ID, which the view reads from the result.
async function startPreview(args) {
    const result = await callTool("preview_slint", args);
    const previewId = result._meta["slint/previewId"];
    assert.equal(
        result.content[0].text,
        `Rendering Slint preview ${previewId}.`,
    );
    return previewId;
}

test("gives every preview its own ID", async () => {
    const args = { path: "/project/main.slint" };
    assert.notEqual(await startPreview(args), await startPreview(args));
});

test("returns the capture the view publishes for the latest preview", async () => {
    const previewId = await startPreview({ source: "export component A {}" });
    const screenshot = callTool("get_preview_screenshot", {});
    await callTool("publish_preview_capture", { previewId, started: true });
    await callTool("publish_preview_capture", {
        previewId,
        png: "iVBORw0K",
        width: 320,
        height: 120,
        runtime: "Slint 1.19.0 (snapshot)",
    });
    const { content, isError } = await screenshot;
    assert.equal(isError, undefined);
    assert.deepEqual(content[0], {
        type: "image",
        data: "iVBORw0K",
        mimeType: "image/png",
    });
    assert.equal(
        content[1].text,
        `Screenshot of preview ${previewId}, rendered with Slint 1.19.0 (snapshot), 320×120 px.`,
    );

    // A capture that already arrived is returned right away.
    const again = await callTool("get_preview_screenshot", {});
    assert.equal(again.content[0].type, "image");
});

test("reports why a preview has no screenshot", async () => {
    const previewId = await startPreview({
        source: "export component A { colour: red; }",
    });
    await callTool("publish_preview_capture", {
        previewId,
        error: "main.slint:1:23: error: Unknown property colour",
    });
    const { content, structuredContent } = await callTool("get_preview_screenshot", {});
    assert.match(content[0].text, /Unknown property colour/);
    assert.equal(structuredContent.status, "error");
});

test("ignores a late capture of an earlier preview of the same file", async () => {
    const args = { path: "/project/main.slint" };
    const earlier = await startPreview(args);
    await startPreview(args);
    const result = await callTool("publish_preview_capture", {
        previewId: earlier,
        png: "iVBORw0K",
    });
    assert.match(result.content[0].text, /older preview/);
    const { content } = await callTool("get_preview_screenshot", {});
    assert.equal(content[0].type, "text");
});

test("returns a fallback when no view starts, then accepts a late view", async () => {
    const previewId = await startPreview({ source: "export component B {}" });
    const result = await callTool("get_preview_screenshot", {});
    assert.equal(result.isError, true);
    assert.ok(result.content[0].text.includes("slint-viewer --screenshot"));
    assert.ok(!result.content[0].text.includes("again"));
    const again = await callTool("get_preview_screenshot", {});
    assert.equal(again.isError, true);

    await callTool("publish_preview_capture", { previewId, started: true });
    await callTool("publish_preview_capture", { previewId, png: "iVBORw0K" });
    assert.equal((await callTool("get_preview_screenshot", {})).structuredContent.status, "ready");
});

test("reports the current stage and timings while rendering remains pending", async () => {
    const previewId = await startPreview({ source: "export component C {}" });
    await callTool("publish_preview_capture", { previewId, started: true });
    const screenshot = callTool("get_preview_screenshot", {});
    const stages = ["started", "reading", "loading", "compiling"];
    for (const stage of stages.slice(1)) {
        await callTool("publish_preview_capture", { previewId, stage });
    }
    const { content, structuredContent } = await screenshot;
    assert.match(content[0].text, /pending \(compiling\)/);
    assert.equal(structuredContent.status, "pending");
    assert.equal(structuredContent.stage, "compiling");
    assert.deepEqual(Object.keys(structuredContent.timings), stages);
    let previous = 0;
    for (const elapsed of Object.values(structuredContent.timings)) {
        assert.ok(elapsed >= previous && elapsed <= structuredContent.elapsedMs);
        previous = elapsed;
    }
    const timeout = await callTool("get_preview_screenshot", {});
    assert.equal(timeout.structuredContent.status, "timeout");
    assert.equal(timeout.structuredContent.stage, "compiling");
});

test("tells a waiting call that a newer preview replaced it", async () => {
    const previewId = await startPreview({ source: "export component D {}" });
    const screenshot = callTool("get_preview_screenshot", {});
    await startPreview({ source: "export component E {}" });
    const { content } = await screenshot;
    assert.match(
        content[0].text,
        new RegExp(`newer preview_slint call replaced preview ${previewId}`),
    );
});
