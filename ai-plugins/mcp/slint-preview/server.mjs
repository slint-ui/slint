#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// MCP server over stdio that renders .slint source inline as an MCP App.
// See https://github.com/modelcontextprotocol/ext-apps for the UI extension.

import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import { extname, isAbsolute } from "node:path";
import { createInterface } from "node:readline";
import { pathToFileURL } from "node:url";

const VIEW_URI = "ui://slint/preview.html";
const VIEW_MIME_TYPE = "text/html;profile=mcp-app";
const VIEW_START_TIMEOUT_MS = Number(
    process.env.SLINT_PREVIEW_VIEW_START_TIMEOUT_MS ?? 10000,
);
// The first preview downloads and compiles the interpreter, about 9 MB of WebAssembly.
const SCREENSHOT_TIMEOUT_MS = Number(
    process.env.SLINT_PREVIEW_SCREENSHOT_TIMEOUT_MS ?? 20000,
);
const PREVIEW_TIMEOUT_MS = Number(
    process.env.SLINT_PREVIEW_TIMEOUT_MS ?? 60000,
);

const pluginVersion = JSON.parse(
    readFileSync(
        new URL("../../.claude-plugin/plugin.json", import.meta.url),
        "utf8",
    ),
).version;

const { channel } = JSON.parse(
    readFileSync(new URL("channel.json", import.meta.url), "utf8"),
);
const INTERPRETERS = {
    release: {
        url: `https://releases.slint.dev/${pluginVersion}/wasm-interpreter/slint_wasm_interpreter.js`,
        // releases.slint.dev redirects the newest release to docs.slint.dev/latest.
        hosts: ["https://releases.slint.dev", "https://docs.slint.dev"],
    },
    snapshot: {
        url: "https://snapshots.slint.dev/master/wasm-interpreter/slint_wasm_interpreter.js",
        hosts: ["https://snapshots.slint.dev"],
    },
};
const interpreter = INTERPRETERS[channel];
if (!interpreter) {
    throw new Error(`Unknown channel in channel.json: ${channel}`);
}

const icons = [
    {
        src:
            "data:image/svg+xml;base64," +
            readFileSync(
                new URL("../../assets/slint.svg", import.meta.url),
            ).toString("base64"),
        mimeType: "image/svg+xml",
        sizes: ["any"],
    },
];

const tools = [
    {
        name: "preview_slint",
        title: "Preview Slint",
        icons,
        description:
            "Preview saved Slint source using the matching Wasm interpreter. " +
            "Pass `path` to preview a file on disk, with its imports and images, or `source` for a snippet that imports only from std-widgets.slint. " +
            "Compile errors are reported back to you once the preview has loaded. " +
            "Then call get_preview_screenshot to see the preview as the user sees it.",
        inputSchema: {
            type: "object",
            properties: {
                path: {
                    type: "string",
                    description:
                        "Absolute path of the .slint file to preview. The last exported component is shown.",
                },
                source: {
                    type: "string",
                    description:
                        "The .slint source. The last exported component is shown.",
                },
                projectRoot: {
                    type: "string",
                    description:
                        "Absolute path of the directory with the slint-project.json to apply to `path`, for its library and include paths.",
                },
                style: {
                    type: "string",
                    enum: ["fluent", "material", "cupertino", "cosmic"],
                    description: "Widget style for std-widgets.slint.",
                },
            },
        },
        annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: true },
        _meta: { ui: { resourceUri: VIEW_URI } },
    },
    {
        name: "read_slint_file",
        title: "Read Slint File",
        description: "Read a .slint file for the preview.",
        annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
        inputSchema: {
            type: "object",
            properties: { path: { type: "string" } },
            required: ["path"],
        },
        _meta: { ui: { visibility: ["app"] } },
    },
    {
        name: "read_binary_file",
        title: "Read Binary File",
        description: "Read an image or font file that the preview uses.",
        annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
        inputSchema: {
            type: "object",
            properties: { path: { type: "string" } },
            required: ["path"],
        },
        _meta: { ui: { visibility: ["app"] } },
    },
    {
        name: "get_preview_screenshot",
        title: "Get Slint Preview Screenshot",
        icons,
        description:
            "Get a PNG screenshot of the latest preview_slint preview at its full size, with the Slint version it ran on. " +
            "Waits until the preview has rendered, or returns pending with its current stage. " +
            "If pending, call again to keep waiting for the same preview. Stop retrying on timeout. " +
            "Check it before you judge how the UI looks.",
        inputSchema: { type: "object", properties: {} },
        annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    },
    {
        name: "publish_preview_capture",
        title: "Publish Slint Preview Capture",
        description:
            "Report preview startup and progress, or hand the server the preview's PNG capture or why there is none.",
        annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
        inputSchema: {
            type: "object",
            properties: {
                previewId: { type: "string" },
                started: { type: "boolean" },
                stage: {
                    type: "string",
                    enum: ["reading", "loading", "compiling", "rendering", "capturing"],
                },
                png: { type: "string" },
                width: { type: "integer" },
                height: { type: "integer" },
                runtime: { type: "string" },
                error: { type: "string" },
            },
            required: ["previewId"],
        },
        _meta: { ui: { visibility: ["app"] } },
    },
];

// The latest preview_slint call, whether its view has started, its capture once the view
// publishes it, and the get_preview_screenshot calls waiting for any of these to change.
let latestPreview = null;
function notify(preview) {
    for (const listener of preview.listeners.splice(0)) listener();
}

function nextChange(preview, ms) {
    return new Promise((resolve) => {
        const done = () => {
            clearTimeout(timer);
            preview.listeners = preview.listeners.filter((l) => l !== done);
            resolve();
        };
        const timer = setTimeout(done, ms);
        preview.listeners.push(done);
    });
}

const BINARY_MIME_TYPES = {
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".gif": "image/gif",
    ".webp": "image/webp",
    ".svg": "image/svg+xml",
    ".bmp": "image/bmp",
    ".ttf": "font/ttf",
    ".otf": "font/otf",
    ".ttc": "font/collection",
};

function textResult(text, isError = false) {
    return { content: [{ type: "text", text }], ...(isError && { isError }) };
}

const toolCalls = {
    preview_slint: (args) => {
        if (
            (typeof args.path === "string") ===
            (typeof args.source === "string")
        ) {
            throw rpcError(-32602, "Pass either 'path' or 'source'");
        }
        if (typeof args.path === "string" && !isAbsolute(args.path)) {
            throw rpcError(-32602, "'path' must be absolute");
        }
        if (args.projectRoot !== undefined) {
            if (typeof args.path !== "string") {
                throw rpcError(-32602, "'projectRoot' requires 'path'");
            }
            if (
                typeof args.projectRoot !== "string" ||
                !isAbsolute(args.projectRoot)
            ) {
                throw rpcError(-32602, "'projectRoot' must be absolute");
            }
        }
        const previous = latestPreview;
        latestPreview = {
            id: randomUUID(),
            requestedAt: Date.now(),
            viewStarted: false,
            stage: "waiting",
            timings: {},
            capture: null,
            listeners: [],
        };
        if (previous) notify(previous);
        // The view reads the ID from _meta, or from the text if a client drops _meta.
        return {
            content: [
                {
                    type: "text",
                    text: `Rendering Slint preview ${latestPreview.id}.`,
                },
            ],
            _meta: { "slint/previewId": latestPreview.id },
        };
    },
    get_preview_screenshot: async () => {
        const preview = latestPreview;
        if (!preview) {
            return textResult("Call preview_slint first.", true);
        }
        const viewDeadline = preview.requestedAt + VIEW_START_TIMEOUT_MS;
        const captureDeadline = Date.now() + SCREENSHOT_TIMEOUT_MS;
        const overallDeadline = preview.requestedAt + PREVIEW_TIMEOUT_MS;
        while (!preview.capture && preview === latestPreview) {
            const deadline = Math.min(
                overallDeadline,
                captureDeadline,
                preview.viewStarted ? Infinity : viewDeadline,
            );
            const remaining = deadline - Date.now();
            if (remaining <= 0) break;
            await nextChange(preview, remaining);
        }
        const { id, capture } = preview;
        if (preview !== latestPreview) {
            return textResult(
                `A newer preview_slint call replaced preview ${id}. Call get_preview_screenshot again.`,
            );
        }
        if (!capture && !preview.viewStarted) {
            return textResult("The preview view did not start. Use slint-viewer --screenshot instead.", true);
        }
        const structuredContent = {
            status: capture
                ? (capture.error ? "error" : "ready")
                : (Date.now() >= overallDeadline ? "timeout" : "pending"),
            previewId: id,
            stage: preview.stage,
            elapsedMs: Date.now() - preview.requestedAt,
            timings: preview.timings,
        };
        if (!capture) {
            return {
                ...textResult(
                    structuredContent.status === "timeout"
                        ? `Preview ${id} reached the overall ${PREVIEW_TIMEOUT_MS / 1000}-second limit (${preview.stage}). Stop retrying automatically.`
                        : `Preview ${id} is pending (${preview.stage}). Call get_preview_screenshot again to keep waiting for this preview.`,
                ),
                structuredContent,
            };
        }
        const runtime = capture.runtime ? ` with ${capture.runtime}` : "";
        if (capture.error) {
            return {
                ...textResult(`Preview ${id}${runtime} has no screenshot: ${capture.error}`),
                structuredContent,
            };
        }
        return {
            structuredContent,
            content: [
                { type: "image", data: capture.png, mimeType: "image/png" },
                {
                    type: "text",
                    text: `Screenshot of preview ${id}, rendered${runtime}, ${capture.width}×${capture.height} px.`,
                },
            ],
        };
    },
    publish_preview_capture: (args) => {
        if (typeof args.previewId !== "string") {
            throw rpcError(-32602, "'previewId' must be a string");
        }
        const preview = latestPreview;
        if (args.previewId !== preview?.id) {
            return textResult("This belongs to an older preview.");
        }
        preview.viewStarted = true;
        preview.stage = args.stage ?? "started";
        if (!args.started && !args.stage) {
            const runtime =
                typeof args.runtime === "string" ? args.runtime : undefined;
            preview.capture =
                typeof args.png === "string"
                    ? {
                          png: args.png,
                          width: args.width,
                          height: args.height,
                          runtime,
                      }
                    : { error: String(args.error ?? "unknown error"), runtime };
            preview.stage = preview.capture.error ? "error" : "ready";
        }
        preview.timings[preview.stage] ??= Date.now() - preview.requestedAt;
        notify(preview);
        return textResult("Received.");
    },
    read_slint_file: (args) => {
        if (typeof args.path !== "string" || !isAbsolute(args.path)) {
            throw rpcError(-32602, "'path' must be an absolute path");
        }
        try {
            return textResult(readFileSync(args.path, "utf8"));
        } catch (e) {
            return textResult(e.message, true);
        }
    },
    read_binary_file: (args) => {
        if (typeof args.path !== "string" || !isAbsolute(args.path)) {
            throw rpcError(-32602, "'path' must be an absolute path");
        }
        try {
            return {
                content: [
                    {
                        type: "resource",
                        resource: {
                            uri: pathToFileURL(args.path).href,
                            mimeType:
                                BINARY_MIME_TYPES[
                                    extname(args.path).toLowerCase()
                                ] ?? "application/octet-stream",
                            blob: readFileSync(args.path).toString("base64"),
                        },
                    },
                ],
            };
        } catch (e) {
            return textResult(e.message, true);
        }
    },
};

function readView() {
    // The .in suffix prevents Codex from offering this MCP App template as a standalone website.
    return readFileSync(new URL("view.html.in", import.meta.url), "utf8");
}

const handlers = {
    initialize: (params) => ({
        protocolVersion: params?.protocolVersion ?? "2025-06-18",
        capabilities: { tools: {}, resources: {} },
        serverInfo: { name: "slint-preview", version: pluginVersion, icons },
    }),
    ping: () => ({}),
    "tools/list": () => ({ tools }),
    "tools/call": (params) => {
        const call = toolCalls[params?.name];
        if (!call) {
            throw rpcError(-32602, `Unknown tool: ${params?.name}`);
        }
        return call(params.arguments ?? {});
    },
    "resources/list": () => ({
        resources: [
            {
                uri: VIEW_URI,
                name: "Slint preview",
                mimeType: VIEW_MIME_TYPE,
            },
        ],
    }),
    "resources/read": (params) => {
        if (params?.uri !== VIEW_URI) {
            throw rpcError(-32002, `Resource not found: ${params?.uri}`);
        }
        return {
            contents: [
                {
                    uri: VIEW_URI,
                    mimeType: VIEW_MIME_TYPE,
                    text: readView()
                        .replace("__INTERPRETER_URL__", interpreter.url)
                        .replace("__CHANNEL__", channel),
                    _meta: {
                        ui: {
                            csp: {
                                connectDomains: interpreter.hosts,
                                // The interpreter shows inlined images through blob: URLs,
                                // which the Codex app blocks unless declared.
                                resourceDomains: [
                                    ...interpreter.hosts,
                                    "blob:",
                                ],
                            },
                            prefersBorder: true,
                        },
                    },
                },
            ],
        };
    },
};

function rpcError(code, message) {
    return Object.assign(new Error(message), { code });
}

function send(message) {
    process.stdout.write(JSON.stringify(message) + "\n");
}

createInterface({ input: process.stdin }).on("line", (line) => {
    if (!line.trim()) return;
    let message;
    try {
        message = JSON.parse(line);
    } catch {
        send({
            jsonrpc: "2.0",
            id: null,
            error: { code: -32700, message: "Parse error" },
        });
        return;
    }
    // Notifications have no id and get no response.
    if (message.id === undefined) return;
    const handler = handlers[message.method];
    if (!handler) {
        send({
            jsonrpc: "2.0",
            id: message.id,
            error: {
                code: -32601,
                message: `Method not found: ${message.method}`,
            },
        });
        return;
    }
    Promise.resolve()
        .then(() => handler(message.params))
        .then(
            (result) => send({ jsonrpc: "2.0", id: message.id, result }),
            (e) =>
                send({
                    jsonrpc: "2.0",
                    id: message.id,
                    error: { code: e.code ?? -32603, message: e.message },
                }),
        );
});
