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
  window.annotationMessages = [];
  window.chatDraft = [];
  window.sentMessages = [];
  window.addEventListener("message", async (event) => {
    if (event.source !== frame.contentWindow) return;
    const { id, method, params } = event.data;
    if (method === "ui/notifications/initialized") window.viewInitialized = true;
    if (id === undefined) return;
    if (method === "ui/initialize") {
      post({ id, result: {
        protocolVersion: params.protocolVersion,
        hostContext: { theme: "light" },
        hostCapabilities: { experimental: window.noDraftSupport ? {} : { "openai/message": {} } },
      } });
    } else if (method === "tools/call") {
      post({ id, ...(await window.callServer(params)) });
    } else if (method === "ui/message") {
      window.annotationMessages.push(params);
      if (!window.messageError) {
        if (params._meta?.["openai/message"]?.send === false) window.chatDraft.push(...params.content);
        else window.sentMessages.push(params);
      }
      post({ id, ...(window.messageError ? { error: window.messageError } : { result: {} }) });
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
            SLINT_PREVIEW_TIMEOUT_MS: "120000",
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
    // The view drops tool notifications that arrive before it listens.
    await page.waitForFunction(() => window.viewInitialized);

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

test("resolves imports through the project's include and library paths, and by absolute path", async () => {
    writeFiles({
        "project/slint-project.json": JSON.stringify({
            "edition": "1.20",
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
            `import { Direct } from "${join(projects, "project/direct/direct.slint")}";`,
            "export component Main inherits Window { HorizontalLayout { Card {} Relative {} Absolute {} Direct {} } }",
        ].join("\n"),
        "project/shared/include/card.slint": COMPONENT("Card"),
        "project/libraries/relative/relative.slint": COMPONENT("Relative"),
        "project/libraries/absolute/absolute.slint": COMPONENT("Absolute"),
        "project/direct/direct.slint": COMPONENT("Direct"),
    });
    const path = join(projects, "project/ui/main.slint");

    const projectRoot = join(projects, "project");
    assertRendered(await preview({ path, projectRoot }));
    // A style compiles through compile_from_string_with_style, which takes the root as another argument.
    assertRendered(await preview({ path, projectRoot, style: "fluent" }));

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

// Returns the [r, g, b, a] of the screenshot's center pixel.
function centerPixel(screenshot) {
    return page.evaluate(async (png) => {
        const response = await fetch(`data:image/png;base64,${png}`);
        const bitmap = await createImageBitmap(await response.blob());
        const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
        const context = canvas.getContext("2d");
        context.drawImage(bitmap, 0, 0);
        const x = Math.floor(bitmap.width / 2);
        const y = Math.floor(bitmap.height / 2);
        return [...context.getImageData(x, y, 1, 1).data];
    }, screenshot.content[0].data);
}

test("captures the current state after interacting with the preview", async () => {
    const initial = await preview({ source: `
        export component Main inherits Window {
            width: 40px;
            height: 40px;
            property <bool> clicked: false;
            background: clicked ? blue : red;
            TouchArea { clicked => { root.clicked = true; } }
        }
    ` });
    assertRendered(initial);
    assert.deepEqual(await centerPixel(initial), [255, 0, 0, 255]);

    await page.frameLocator("iframe").locator("#preview").click();
    const refreshed = await callTool("get_preview_screenshot", {});
    assertRendered(refreshed);
    assert.deepEqual(await centerPixel(refreshed), [0, 0, 255, 255]);
});

const BLUE_SVG =
    '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="#00f"/></svg>';
const ICON =
    'export component Icon inherits Image { source: @image-url("blue.svg"); width: 20px; height: 20px; }';

// Each previews a blue image on a red window, so the center pixel shows whether the image loaded.
const IMAGE_CASES = {
    "a relative library path": (root) => ({
        project: { "library-paths": { icons: "icons" } },
        import: '"@icons/icon.slint"',
        icon: ICON,
    }),
    "an absolute library path": (root) => ({
        project: { "library-paths": { icons: join(root, "icons") } },
        import: '"@icons/icon.slint"',
        icon: ICON,
    }),
    "an absolute import": (root) => ({
        import: JSON.stringify(join(root, "icons/icon.slint")),
        icon: ICON,
    }),
    "an absolute image URL": (root) => ({
        import: '"icons/icon.slint"',
        icon: ICON.replace(
            '"blue.svg"',
            JSON.stringify(join(root, "icons/blue.svg")),
        ),
    }),
};

for (const [name, files] of Object.entries(IMAGE_CASES)) {
    test(`loads images of a component through ${name}`, async () => {
        const directory = `images-${name.replaceAll(" ", "-")}`;
        const root = join(projects, directory);
        const { project, import: from, icon } = files(root);
        writeFiles({
            [`${directory}/slint-project.json`]: JSON.stringify({
                "edition": "1.20",
                ...project,
            }),
            [`${directory}/main.slint`]: [
                `import { Icon } from ${from};`,
                "export component Main inherits Window { width: 20px; height: 20px; background: red; Icon {} }",
            ].join("\n"),
            [`${directory}/icons/icon.slint`]: icon,
            [`${directory}/icons/blue.svg`]: BLUE_SVG,
        });
        const screenshot = await preview({
            path: join(root, "main.slint"),
            projectRoot: root,
        });
        assertRendered(screenshot);
        assert.deepEqual(await centerPixel(screenshot), [0, 0, 255, 255]);
    });
}

test("stacks annotations in the existing chat draft with matching screenshots and coordinates", async () => {
    await page.evaluate(() => {
        window.annotationMessages = [];
        window.chatDraft = [{ type: "text", text: "Existing draft" }];
        window.sentMessages = [];
    });
    await preview({ source: "export component Main inherits Window { width: 1000px; height: 400px; background: blue; }" });
    const frame = page.frameLocator("iframe");
    const canvas = frame.locator("#preview");
    const bounds = await canvas.boundingBox();
    assert.ok(bounds.width < 1000, "the preview must be fitted for this test");
    await frame.getByRole("button", { name: "Annotate", exact: true }).click();
    const layer = frame.locator("#annotation-layer");
    await layer.click({ position: { x: bounds.width / 4, y: bounds.height / 2 } });
    const comment = frame.getByRole("textbox");
    await comment.fill("Make this part green");
    await comment.press("Enter");
    assert.equal(await comment.inputValue(), "Make this part green\n");
    assert.equal(await page.evaluate(() => window.annotationMessages.length), 0);
    const snapshot = await frame.locator("#annotation-snapshot").getAttribute("src");
    await comment.press("Control+Enter");
    await page.waitForFunction(() => window.annotationMessages.length === 1);
    const message = await page.evaluate(() => window.annotationMessages[0]);
    assert.equal(message.role, "user");
    assert.deepEqual(message._meta, { "openai/message": { target: "active", send: false } });
    assert.equal(message.content[0]._meta["openai/title"], "Make this part green");
    for (const block of message.content) {
        assert.deepEqual(block._meta["openai/group"], { title: "Preview Annotations" });
    }
    assert.ok(message.content[0].text.startsWith("Make this part green\n\n"));
    assert.match(message.content[0].text, /Slint position: x=250, y=200 in a 1000×400 UI/);
    assert.ok(message.content.every((block) => !block.annotations?.audience));
    const image = message.content[1];
    assert.equal(image.type, "image");
    assert.equal(image.mimeType, "image/png");
    assert.equal(`data:image/png;base64,${image.data}`, snapshot);
    const size = await page.evaluate(async (data) => {
        const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${data}`)).blob());
        return { width: bitmap.width, height: bitmap.height };
    }, image.data);
    assert.ok(message.content[0].text.includes(
        `Clicked point: x=${Math.round(size.width / 4)}, y=${Math.round(size.height / 2)} in the attached ${size.width}×${size.height} screenshot.`,
    ), message.content[0].text);
    assert.equal(await layer.isVisible(), false);
    assert.equal(await frame.getByRole("button", { name: "Annotate", exact: true }).evaluate((element) => getComputedStyle(element).outlineStyle), "none");
    assert.equal(await frame.locator("#status").textContent(), "");
    await frame.getByRole("button", { name: "Annotate", exact: true }).click();
    await layer.click({ position: { x: bounds.width / 2, y: bounds.height / 2 } });
    await comment.press("Meta+Enter");
    await page.waitForFunction(() => window.annotationMessages.length === 2);
    const draft = await page.evaluate(() => window.chatDraft);
    assert.equal(draft.length, 5);
    assert.deepEqual(draft[0], { type: "text", text: "Existing draft" });
    assert.equal(draft[3]._meta["openai/title"], "Slint annotation");
    assert.equal(await page.evaluate(() => window.sentMessages.length), 0);
});

test("keeps a refused annotation editable and cancels without sending", async () => {
    await page.evaluate(() => {
        window.annotationMessages = [];
        window.messageError = { code: -32000, message: "Sending is unavailable" };
    });
    await preview({ source: "export component Main inherits Window { width: 320px; height: 200px; background: blue; }" });
    const frame = page.frameLocator("iframe");
    await frame.getByRole("button", { name: "Annotate", exact: true }).click();
    const canvas = frame.locator("#preview");
    await canvas.evaluate((element) => { element.toDataURL = () => { throw new Error("Capture unavailable"); }; });
    await frame.locator("#annotation-layer").click({ position: { x: 10, y: 10 } });
    await canvas.evaluate((element) => { delete element.toDataURL; });
    assert.match(await frame.locator("#status").textContent(), /Capture unavailable/);
    await frame.locator("#annotation-layer").click({ position: { x: 10, y: 10 } });
    assert.equal(await frame.locator("#status").textContent(), "");
    assert.equal(await frame.getByRole("button", { name: "Add to chat", exact: true }).isEnabled(), true);
    const comment = frame.getByRole("textbox");
    const shortHeight = (await comment.boundingBox()).height;
    const draft = "Round this corner\n".repeat(20);
    await comment.fill(draft);
    assert.ok((await comment.boundingBox()).height > shortHeight, "the comment grows without a resize handle");
    await frame.getByRole("button", { name: "Add to chat", exact: true }).click();
    await frame.getByRole("alert").filter({ hasText: "Sending is unavailable" }).waitFor();
    assert.equal(await comment.inputValue(), draft);
    await frame.getByRole("textbox", { name: "Annotation comment" }).press("Escape");
    assert.equal(await frame.getByRole("textbox").isVisible(), false);
    assert.equal(await page.evaluate(() => window.annotationMessages.length), 1);
    await page.evaluate(() => { delete window.messageError; });
});

test("anchors the tools to the plugin viewport and switches the Slint theme", async () => {
    assertRendered(await preview({ source: `import { Palette } from "std-widgets.slint";
        export component Main inherits Window { width: 320px; height: 240px; background: Palette.background; }` }));
    const frame = page.frameLocator("iframe");
    const toolbar = frame.getByRole("toolbar", { name: "Preview tools", exact: true });
    const view = page.frames().find((item) => item.url() === "about:srcdoc");
    const waitForBackground = (dark) => view.waitForFunction((dark) => {
        const canvas = document.getElementById("preview");
        const pixel = new Uint8Array(4);
        const gl = canvas.getContext("webgl2");
        gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, pixel);
        return pixel[3] > 0 && (dark ? pixel[0] < 100 : pixel[0] > 200);
    }, dark);
    const checkPosition = async () => {
        const position = await view.evaluate(() => {
            const button = document.getElementById("preview-tools").getBoundingClientRect();
            const canvas = document.getElementById("preview").getBoundingClientRect();
            return { left: button.left, bottom: window.innerHeight - button.bottom, height: button.height, outsideContent: button.left < canvas.left };
        });
        assert.equal(Math.round(position.left), 10);
        assert.equal(Math.round(position.bottom), 10);
        assert.equal(position.height, 28);
        assert.ok(position.outsideContent, "the toolbar is positioned relative to the plugin viewport");
    };
    await checkPosition();
    await page.locator("iframe").evaluate((element) => { element.style.width = "480px"; element.style.height = "420px"; });
    await checkPosition();
    assert.equal(await toolbar.isVisible(), true);
    const annotate = frame.getByRole("button", { name: "Annotate", exact: true });
    await annotate.click();
    await frame.locator("#annotation-layer").click({ position: { x: 30, y: 30 } });
    await frame.getByRole("textbox").press("Escape");
    assert.equal(await annotate.evaluate((element) => element === document.activeElement && getComputedStyle(element).outlineStyle === "solid"), true);
    await annotate.press("Tab");
    const light = frame.getByRole("button", { name: "Light", exact: true });
    assert.equal(await light.evaluate((element) => element === document.activeElement && getComputedStyle(element).outlineStyle === "solid"), true);
    await light.click();
    assert.equal(await frame.getByRole("button", { name: "Dark", exact: true }).isVisible(), true);
    await frame.getByRole("button", { name: "Dark", exact: true }).click();
    await view.waitForFunction(() => document.documentElement.style.colorScheme === "dark");
    await waitForBackground(true);
    await page.evaluate(() => window.notifyView("ui/notifications/host-context-changed", { theme: "light" }));
    assert.equal(await view.evaluate(() => document.documentElement.style.colorScheme), "dark");
    await frame.getByRole("button", { name: "Light", exact: true }).click();
    await waitForBackground(false);
    await page.evaluate(() => window.notifyView("ui/notifications/host-context-changed", { theme: "dark" }));
    assert.equal(await light.getAttribute("aria-pressed"), "true");
    await light.click();
    await waitForBackground(true);
    assert.equal(await light.getAttribute("aria-pressed"), "false");
    assert.equal(await frame.getByRole("button", { name: "Dark", exact: true }).getAttribute("aria-pressed"), "false");
    await page.evaluate(() => window.notifyView("ui/notifications/host-context-changed", { theme: "light" }));
    await waitForBackground(false);
    assert.equal(await toolbar.isVisible(), true);
    await page.locator("iframe").evaluate((element) => { element.style.width = "800px"; element.style.height = "600px"; });
});

test("hides Annotate when the host cannot stage a draft", async () => {
    await page.evaluate(() => {
        window.noDraftSupport = true;
        window.viewInitialized = false;
        const frame = document.querySelector("iframe");
        frame.srcdoc = frame.srcdoc;
    });
    await page.waitForFunction(() => window.viewInitialized);
    await preview({ source: "export component Main inherits Window { width: 320px; height: 200px; background: blue; }" });
    const frame = page.frameLocator("iframe");
    assert.equal(await frame.getByRole("toolbar", { name: "Preview tools", exact: true }).isVisible(), true);
    assert.equal(await frame.getByRole("button", { name: "Light", exact: true }).isVisible(), true);
    assert.equal(await frame.locator("#annotate").isVisible(), false);
});
