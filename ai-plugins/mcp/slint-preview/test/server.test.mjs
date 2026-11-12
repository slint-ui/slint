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

async function captureRequestId(previewId) {
    const poll = await callTool("poll_preview_capture", { previewId });
    assert.ok(poll.structuredContent.captureId);
    return poll.structuredContent.captureId;
}

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

test("lists check_slint, publish_preview with its view, and app-only tools", async () => {
    const { result } = await request("tools/list");
    const preview = result.tools.find((t) => t.name === "publish_preview");
    assert.equal(
        result.tools.find((tool) => tool.name === "check_slint")._meta,
        undefined,
    );
    assert.equal(preview._meta.ui.resourceUri, "ui://slint/preview.html");
    for (const name of [
        "read_slint_file",
        "read_binary_file",
        "poll_preview_capture",
        "publish_preview_capture",
    ]) {
        const tool = result.tools.find((t) => t.name === name);
        assert.deepEqual(tool._meta.ui.visibility, ["app"], name);
    }
    const screenshot = result.tools.find(
        (t) => t.name === "get_preview_screenshot",
    );
    assert.equal(screenshot._meta, undefined);
    assert.deepEqual(screenshot.inputSchema, {
        type: "object",
        properties: { previewId: { type: "string" } },
    });
});

test("all tools declare their annotations", async () => {
    const { result } = await request("tools/list");
    for (const tool of result.tools) {
        for (const hint of [
            "readOnlyHint",
            "destructiveHint",
            "openWorldHint",
        ]) {
            assert.equal(
                typeof tool.annotations?.[hint],
                "boolean",
                `${tool.name}: ${hint}`,
            );
        }
    }
});

function assertServesInterpreter(content, url, hosts) {
    assert.equal(content.mimeType, "text/html;profile=mcp-app");
    assert.doesNotMatch(content.text, /__[A-Za-z_-]+__/);
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
            "mcp/slint-preview/checker.mjs",
            "mcp/slint-preview/view.html.in",
            "mcp/slint-preview/assets",
        ]) {
            cpSync(new URL(file, pluginRoot), join(root, file), {
                recursive: true,
                dereference: true,
            });
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

test("checks a render before publishing with text only", async () => {
    const checked = await callTool("check_slint", {
        source: 'export component Main inherits Rectangle { width: 20px; height: 20px; background: blue; init => { debug("MCP check"); } }',
        prepare_preview: true,
    });
    assert.equal(checked.structuredContent.ok, true);
    assert.deepEqual(checked.structuredContent.diagnostics, []);
    assert.equal(checked.content[1].type, "image");
    assert.equal(checked.content[1].mimeType, "image/png");
    assert.match(checked.structuredContent.runtime, /MCP Wasm software/);
    const result = await callTool("publish_preview", {
        previewId: checked.structuredContent.previewId,
    });
    assert.equal(result.content[0].type, "text");
    assert.equal(result.structuredContent, undefined);
    assert.ok(
        result._meta["slint/preview"].source.includes("background: blue"),
    );
});

test("takes either an absolute path or source, and a project root only with a path", async () => {
    const call = (args) =>
        request("tools/call", { name: "check_slint", arguments: args });
    assert.ok((await call({ path: "/project/main.slint" })).result);
    assert.ok(
        (
            await call({
                path: "/project/ui/main.slint",
                projectRoot: "/project",
            })
        ).result,
    );
    for (const args of [
        {},
        { path: "/project/main.slint", source: "" },
        { path: "main.slint" },
        { source: "", projectRoot: "/project" },
        { path: "/project/main.slint", projectRoot: "project" },
    ]) {
        assert.equal(
            (await call(args)).error?.code,
            -32602,
            JSON.stringify(args),
        );
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
    const checked = await callTool("check_slint", {
        ...args,
        prepare_preview: true,
    });
    assert.equal(checked.structuredContent?.ok, true, JSON.stringify(checked));
    const previewId = checked.structuredContent.previewId;
    const result = await callTool("publish_preview", { previewId });
    assert.equal(result._meta["slint/previewId"], previewId);
    assert.match(result.content[0].text, /Use the screenshot returned by check_slint/);
    return previewId;
}

test("gives every preview its own ID", async () => {
    const args = {
        source: "export component Main inherits Rectangle { width: 20px; height: 20px; }",
    };
    assert.notEqual(await startPreview(args), await startPreview(args));
});

test("returns the capture the view publishes for the latest preview", async () => {
    const previewId = await startPreview({ source: "export component A inherits Rectangle { width: 20px; height: 20px; }" });
    const screenshot = callTool("get_preview_screenshot", {});
    await callTool("publish_preview_capture", { previewId, stage: "started" });
    await callTool("publish_preview_capture", { previewId, stage: "ready" });
    await callTool("publish_preview_capture", {
        previewId,
        captureId: await captureRequestId(previewId),
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

    const firstRefresh = callTool("get_preview_screenshot", {});
    const secondRefresh = callTool("get_preview_screenshot", {});
    const captureId = await captureRequestId(previewId);
    await callTool("publish_preview_capture", { previewId, captureId, png: "ZnJlc2g=" });
    for (const refreshed of await Promise.all([firstRefresh, secondRefresh])) {
        assert.equal(refreshed.content[0].data, "ZnJlc2g=");
    }
    assert.deepEqual(
        (await callTool("poll_preview_capture", { previewId })).structuredContent,
        { active: true },
    );
});

test("times out a fresh capture and ignores its late response", async () => {
    const previewId = await startPreview({ source: "export component A inherits Rectangle { width: 20px; height: 20px; }" });
    await callTool("publish_preview_capture", { previewId, stage: "ready" });
    const screenshot = callTool("get_preview_screenshot", {});
    const expiredCaptureId = await captureRequestId(previewId);
    const timeout = await screenshot;
    assert.equal(timeout.structuredContent.status, "timeout");
    assert.equal(timeout.content[0].type, "text");

    const retry = callTool("get_preview_screenshot", {});
    const captureId = await captureRequestId(previewId);
    assert.notEqual(captureId, expiredCaptureId);
    const late = await callTool("publish_preview_capture", {
        previewId, captureId: expiredCaptureId, png: "bGF0ZQ==",
    });
    assert.match(late.content[0].text, /no longer active/);
    await callTool("publish_preview_capture", { previewId, captureId, png: "ZnJlc2g=" });
    assert.equal((await retry).content[0].data, "ZnJlc2g=");
});

test("reports why a restored preview has no screenshot", async () => {
    const previewId = "restored-preview";
    await callTool("publish_preview_capture", { previewId, stage: "started" });
    await callTool("publish_preview_capture", {
        previewId,
        error: "main.slint:1:23: error: Unknown property colour",
    });
    const { content, structuredContent, isError } = await callTool(
        "get_preview_screenshot",
        { previewId },
    );
    assert.match(content[0].text, /Unknown property colour/);
    assert.equal(structuredContent.status, "error");
    assert.equal(isError, true);
});

test("reports a crash while a screenshot is pending and ignores a late capture", async () => {
    const previewId = await startPreview({ source: "export component A inherits Rectangle { width: 20px; height: 20px; }" });
    await callTool("publish_preview_capture", { previewId, stage: "ready" });
    const screenshot = callTool("get_preview_screenshot", {});
    const captureId = await captureRequestId(previewId);
    await callTool("publish_preview_capture", {
        previewId,
        error: "The Slint interpreter crashed",
    });
    assert.equal((await screenshot).structuredContent.status, "error");
    for (const update of [{ stage: "ready" }, { captureId, png: "iVBORw0K" }]) {
        await callTool("publish_preview_capture", { previewId, ...update });
        const result = await callTool("get_preview_screenshot", {});
        assert.equal(result.isError, true);
        assert.equal(result.structuredContent.status, "error");
        assert.equal(result.structuredContent.stage, "error");
        assert.ok(
            result.content[0].text.includes("The Slint interpreter crashed"),
        );
        assert.ok(result.content.every((block) => block.type !== "image"));
    }
});

test("captures multiple previews independently while defaulting to the latest opened preview", async () => {
    // cspell:ignore Zmlyc Vmcm
    const args = { source: "export component Main inherits Rectangle { width: 20px; height: 20px; }" };
    const firstPreviewId = await startPreview(args);
    const firstScreenshot = callTool("get_preview_screenshot", {});
    const latestPreviewId = await startPreview(args);
    const screenshots = [
        [firstPreviewId, "Zmlyc3Q=", firstScreenshot],
        [latestPreviewId, "bGF0ZXN0", callTool("get_preview_screenshot", {})],
    ];
    for (const [previewId, png, screenshot] of screenshots) {
        await callTool("publish_preview_capture", { previewId, stage: "ready" });
        await callTool("publish_preview_capture", {
            previewId, captureId: await captureRequestId(previewId), png,
        });
        const result = await screenshot;
        assert.equal(result.structuredContent.previewId, previewId);
        assert.equal(result.content[0].data, png);
    }
    const screenshot = callTool("get_preview_screenshot", { previewId: firstPreviewId });
    await callTool("publish_preview_capture", {
        previewId: firstPreviewId,
        captureId: await captureRequestId(firstPreviewId),
        png: "cmVmcmVzaA==",
    });
    assert.equal((await screenshot).content[0].data, "cmVmcmVzaA==");
});

test("rejects invalid preview IDs and reports unknown screenshot targets", async () => {
    for (const name of ["get_preview_screenshot", "poll_preview_capture"]) {
        for (const previewId of [null, 42, {}]) {
            const result = await request("tools/call", { name, arguments: { previewId } });
            assert.equal(result.error?.code, -32602);
        }
    }
    const result = await callTool("get_preview_screenshot", { previewId: "unknown" });
    assert.equal(result.isError, true);
    assert.match(result.content[0].text, /unknown or inactive/);
});

test("evicts previews that stop polling and reactivates them without changing the default target", async () => {
    const previewId = await startPreview({ source: "export component A inherits Rectangle { width: 20px; height: 20px; }" });
    await callTool("poll_preview_capture", { previewId });
    const latestPreviewId = await startPreview({ source: "export component B inherits Rectangle { width: 20px; height: 20px; }" });
    await callTool("publish_preview_capture", { previewId: latestPreviewId, stage: "ready" });
    const keepPolling = setInterval(() => {
        callTool("poll_preview_capture", { previewId: latestPreviewId });
    }, 500);
    try {
        await new Promise((resolve) => setTimeout(resolve, 6100));
        const expired = await callTool("get_preview_screenshot", { previewId });
        assert.equal(expired.isError, true);
        assert.match(expired.content[0].text, /unknown or inactive/);
        const late = await callTool("publish_preview_capture", { previewId, stage: "ready" });
        assert.match(late.content[0].text, /unknown or inactive/);

        assert.deepEqual(
            (await callTool("poll_preview_capture", { previewId })).structuredContent,
            { active: true },
        );
        for (const [args, expectedId] of [[{ previewId }, previewId], [{}, latestPreviewId]]) {
            const screenshot = callTool("get_preview_screenshot", args);
            await callTool("publish_preview_capture", {
                previewId: expectedId,
                captureId: await captureRequestId(expectedId),
                png: "cmVzdW1lZA==",
            });
            const result = await screenshot;
            assert.equal(result.structuredContent.previewId, expectedId);
            assert.equal(result.content[0].data, "cmVzdW1lZA==");
        }
    } finally {
        clearInterval(keepPolling);
    }
});

test("asks the user to open the chat when startup is unconfirmed, then accepts a late view", async () => {
    const previewId = await startPreview({ source: "export component B inherits Rectangle { width: 20px; height: 20px; }" });
    const result = await callTool("get_preview_screenshot", {});
    assert.equal(result.isError, true);
    assert.equal(
        result.content[0].text,
        "The preview has not confirmed startup yet. " +
        "It may be waiting for the user to open this chat. " +
        "Ask the user to open the chat, then call `get_preview_screenshot` again for the existing preview.",
    );
    const again = await callTool("get_preview_screenshot", {});
    assert.equal(again.isError, true);

    await callTool("publish_preview_capture", { previewId, stage: "started" });
    const screenshot = callTool("get_preview_screenshot", {});
    await callTool("publish_preview_capture", { previewId, stage: "ready" });
    const captureId = await captureRequestId(previewId);
    await callTool("publish_preview_capture", { previewId, captureId, png: "iVBORw0K" });
    assert.equal((await screenshot).structuredContent.status, "ready");
});

test("reports the current stage and timings while rendering remains pending", async () => {
    const previewId = await startPreview({ source: "export component C inherits Rectangle { width: 20px; height: 20px; }" });
    await callTool("publish_preview_capture", { previewId, stage: "started" });
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
