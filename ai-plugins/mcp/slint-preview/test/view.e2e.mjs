// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Renders previews end to end: the server, the view in Chromium behind a minimal MCP host,
// and the Wasm interpreter that `wasm-pack build --target web api/wasm-interpreter` builds.
// Run with SLINT_PREVIEW_INTERPRETER_DIR set to that build's `pkg` directory.

import { spawn } from "node:child_process";
import {
    mkdirSync,
    mkdtempSync,
    readFileSync,
    rmSync,
    writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { chromium } from "playwright";

const interpreterDir = process.env.SLINT_PREVIEW_INTERPRETER_DIR;
const serverPath = new URL("../server.mjs", import.meta.url);

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

// Relays the view's requests to the server, like an MCP host.
const HOST_PAGE = `<!doctype html>
<iframe style="width: 800px; height: 600px"></iframe>
<script>
  const frame = document.querySelector("iframe");
  const post = (message) => frame.contentWindow.postMessage({ jsonrpc: "2.0", ...message }, "*");
  window.notifyView = (method, params) => post({ method, params });
  window.addEventListener("message", async (event) => {
    if (event.source !== frame.contentWindow) return;
    const { id, method, params } = event.data;
    if (id === undefined) return;
    if (method === "ui/initialize") {
      post({ id, result: { protocolVersion: params.protocolVersion, hostContext: { theme: "light" } } });
    } else if (method === "tools/call") {
      post({ id, ...(await window.callServer(params)) });
    } else {
      post({ id, result: {} });
    }
  });
  window.showView = (html) => (frame.srcdoc = html);
</script>`;

let httpServer;
let browser;
let page;
let projects;

function writeFiles(files) {
    for (const [path, contents] of Object.entries(files)) {
        mkdirSync(dirname(join(projects, path)), { recursive: true });
        writeFileSync(join(projects, path), contents);
    }
}

before(async () => {
    assert.ok(
        interpreterDir,
        "Set SLINT_PREVIEW_INTERPRETER_DIR to the pkg directory of the Wasm interpreter",
    );
    server = spawn(process.execPath, [serverPath.pathname], {
        stdio: ["pipe", "pipe", "inherit"],
        env: {
            ...process.env,
            // A debug build of the interpreter takes long to compile.
            SLINT_PREVIEW_SCREENSHOT_TIMEOUT_MS: "120000",
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

    httpServer = createServer((req, res) => {
        if (req.url === "/host.html") {
            res.writeHead(200, { "content-type": "text/html" });
            res.end(HOST_PAGE);
            return;
        }
        const name = req.url.replace(/^\/interpreter\//, "").split(/[?#]/)[0];
        try {
            const body = readFileSync(join(interpreterDir, name));
            const type = name.endsWith(".wasm")
                ? "application/wasm"
                : name.endsWith(".json")
                  ? "application/json"
                  : "text/javascript";
            res.writeHead(200, { "content-type": type });
            res.end(body);
        } catch {
            res.writeHead(404);
            res.end();
        }
    }).listen(0);
    const origin = `http://localhost:${httpServer.address().port}`;

    const view = (
        await request("resources/read", { uri: "ui://slint/preview.html" })
    ).result.contents[0].text.replace(
        /const INTERPRETER_URL = "[^"]*"/,
        `const INTERPRETER_URL = "${origin}/interpreter/slint_wasm_interpreter.js"`,
    );

    // Headless Chromium renders WebGL only in software.
    // cspell:ignore swiftshader
    browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader"] });
    page = await browser.newPage();
    await page.exposeFunction("callServer", async (params) => {
        const { result, error } = await request("tools/call", params);
        return error ? { error } : { result };
    });
    await page.goto(`${origin}/host.html`);
    await page.evaluate((html) => window.showView(html), view);

    projects = mkdtempSync(join(tmpdir(), "slint-preview-e2e-"));
});

after(async () => {
    await browser?.close();
    httpServer?.close();
    server?.kill();
    if (projects) rmSync(projects, { recursive: true, force: true });
});

// Renders a preview and returns the get_preview_screenshot result.
async function preview(args) {
    const result = await callTool("preview_slint", args);
    await page.evaluate(
        ([args, result]) => {
            window.notifyView("ui/notifications/tool-input", {
                arguments: args,
            });
            window.notifyView("ui/notifications/tool-result", result);
        },
        [args, result],
    );
    return callTool("get_preview_screenshot", {});
}

function assertRendered(screenshot) {
    assert.equal(
        screenshot.content[0].type,
        "image",
        screenshot.content[0].text,
    );
}

const COMPONENT = (name) =>
    `export component ${name} inherits Rectangle { width: 20px; height: 20px; background: blue; }`;

test("resolves imports through the include and library paths of the project root", async () => {
    writeFiles({
        "project/slint-project.json": JSON.stringify({
            "slint-edition": "1.19",
            "include-paths": ["shared/include"],
            "library-paths": {
                relative: "libraries/relative",
                absolute: join(projects, "project/libraries/absolute"),
            },
        }),
        "project/ui/main.slint": [
            'import { Card } from "card.slint";',
            'import { Relative } from "@relative/relative.slint";',
            'import { Absolute } from "@absolute/absolute.slint";',
            "export component Main inherits Window { HorizontalLayout { Card {} Relative {} Absolute {} } }",
        ].join("\n"),
        "project/shared/include/card.slint": COMPONENT("Card"),
        "project/libraries/relative/relative.slint": COMPONENT("Relative"),
        "project/libraries/absolute/absolute.slint": COMPONENT("Absolute"),
    });
    const path = join(projects, "project/ui/main.slint");

    assertRendered(
        await preview({ path, projectRoot: join(projects, "project") }),
    );

    const withoutRoot = await preview({ path });
    assert.equal(withoutRoot.content[0].type, "text");
    assert.match(
        withoutRoot.content[0].text,
        /Cannot find requested import "card\.slint" in the include search path/,
    );
});

test("previews a file whose project root has no project file", async () => {
    writeFiles({ "plain/main.slint": COMPONENT("Main") });
    assertRendered(
        await preview({
            path: join(projects, "plain/main.slint"),
            projectRoot: join(projects, "plain"),
        }),
    );
});
