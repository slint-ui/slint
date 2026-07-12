// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Browser test runner: serves the repository over HTTP, loads harness.html in
// a headless Chromium via playwright, and executes one test case per JSON line
// received on stdin, answering with one JSON line on stdout. Diagnostics go to
// stderr; stdout carries only the protocol.

import { createServer } from "node:http";
import { createReadStream, existsSync, statSync } from "node:fs";
import { join, normalize, extname } from "node:path";
import { createInterface } from "node:readline";
import { chromium } from "playwright";

const repoRoot = process.argv[2];
if (!repoRoot) {
    console.error("usage: node runner.mjs <repository-root>");
    process.exit(1);
}

const MIME = {
    ".html": "text/html",
    ".js": "text/javascript",
    ".mjs": "text/javascript",
    ".wasm": "application/wasm",
    ".slint": "text/plain",
    ".json": "application/json",
    ".png": "image/png",
    ".svg": "image/svg+xml",
    ".ttf": "font/ttf",
};

const server = createServer((req, res) => {
    const path = normalize(decodeURIComponent(new URL(req.url, "http://localhost").pathname));
    let file = join(repoRoot, path);
    if (!file.startsWith(repoRoot)) {
        res.writeHead(404);
        res.end("not found");
        return;
    }
    if (extname(file) === "" && !existsSync(file)) {
        // The tsc ESM output uses extensionless relative imports.
        file += ".js";
    }
    if (!existsSync(file) || !statSync(file).isFile()) {
        res.writeHead(404);
        res.end("not found");
        return;
    }
    res.writeHead(200, {
        "Content-Type": MIME[extname(file)] ?? "application/octet-stream",
    });
    createReadStream(file).pipe(res);
});

await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const port = server.address().port;
const harnessUrl = `http://127.0.0.1:${port}/tests/driver/browser/harness.html`;

const browser = await chromium.launch();
const page = await browser.newPage();

// Browser console output, collected per test case for failure reports.
let consoleLines = [];
page.on("console", (msg) => consoleLines.push(`[${msg.type()}] ${msg.text()}`));
page.on("pageerror", (err) => consoleLines.push(`[pageerror] ${err.message}`));

async function loadHarness() {
    await page.goto(harnessUrl);
    await page.waitForFunction(() => globalThis.runCase !== undefined, null, {
        timeout: 30000,
    });
}

await loadHarness();
process.stdout.write(`${JSON.stringify({ ready: true })}\n`);

const CASE_TIMEOUT_MS = 30000;

let needsReload = false;
for await (const line of createInterface({ input: process.stdin })) {
    if (!line.trim()) {
        continue;
    }
    const request = JSON.parse(line);
    consoleLines = [];
    let response;
    try {
        if (needsReload) {
            // Each case leaves global state behind (the mocked clock, timers
            // of undestroyed instances, or a broken module after a wasm
            // panic); reload the harness so every case starts clean, like the
            // per-process isolation of the Node.js driver.
            await loadHarness();
        }
        needsReload = true;
        const result = await page.evaluate(
            ([req, timeout]) =>
                Promise.race([
                    globalThis.runCase(req),
                    new Promise((_, reject) =>
                        setTimeout(() => reject(new Error("test case timed out")), timeout),
                    ),
                ]),
            [request, CASE_TIMEOUT_MS],
        );
        response = { id: request.id, ...result };
    } catch (error) {
        response = { id: request.id, ok: false, error: String(error?.message ?? error) };
    }
    if (!response.ok) {
        response.console = consoleLines;
    }
    process.stdout.write(`${JSON.stringify(response)}\n`);
}

await browser.close();
server.close();
