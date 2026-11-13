#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// MCP server over stdio that renders .slint source inline as an MCP App.
// See https://github.com/modelcontextprotocol/ext-apps for the UI extension.

import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import { extname, isAbsolute } from "node:path";
import { pathToFileURL } from "node:url";
import { createInterface } from "node:readline";

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
const PREVIEW_POLL_TIMEOUT_MS = 5000;

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
        name: "check_slint",
        title: "Check Slint",
        icons,
        description:
            "Compile Slint source and render its initial frame with the MCP Wasm software interpreter. " +
            "Inspect all diagnostics and the returned PNG before publishing. " +
            "Pass path for a saved file with imports, images, and optional projectRoot, or source for a snippet. " +
            "Set prepare_preview to retain a successful check for 15 minutes; publish_preview accepts its previewId. " +
            "The MCP checker supports PNG, JPEG, SVG, and SVGZ images.",
        inputSchema: {
            type: "object",
            properties: {
                prepare_preview: {
                    type: "boolean",
                    default: false,
                    description:
                        "Retain successful checked inputs for publish_preview.",
                },
                path: {
                    type: "string",
                    description:
                        "Absolute path of a .slint file, a directory containing slint-project.json, or slint-project.json itself. Project inputs use their main entry. The last exported component is shown.",
                },
                source: {
                    type: "string",
                    description:
                        "The .slint source. The last exported component is shown.",
                },
                projectRoot: {
                    type: "string",
                    description:
                        "Absolute path of a project directory or slint-project.json whose settings apply to `path`. Pass a different .slint path to preview it instead of the project's main entry.",
                },
                style: {
                    type: "string",
                    enum: ["fluent", "material", "cupertino", "cosmic"],
                    description: "Widget style for std-widgets.slint.",
                },
            },
            oneOf: [{ required: ["path"] }, { required: ["source"] }],
            additionalProperties: false,
        },
        annotations: {
            readOnlyHint: true,
            destructiveHint: false,
            openWorldHint: false,
        },
    },
    {
        name: "publish_preview",
        title: "Publish Slint Preview",
        icons,
        description:
            "Open a successful prepared Slint check as a live browser preview. " +
            "Call check_slint with prepare_preview first and inspect its diagnostics and image. " +
            "Pass only its previewId. Use the PNG returned by check_slint for the initial visual review; no additional screenshot is needed after publication. " +
            "Preparations are consumed on publication; expired or used IDs require a new check.",
        inputSchema: {
            type: "object",
            properties: { previewId: { type: "string" } },
            required: ["previewId"],
            additionalProperties: false,
        },
        annotations: {
            readOnlyHint: true,
            destructiveHint: false,
            openWorldHint: true,
        },
        _meta: { ui: { resourceUri: VIEW_URI } },
    },
    {
        name: "get_preview_screenshot",
        title: "Get Slint Preview Screenshot",
        icons,
        description:
            "Capture the current state of a publish_preview preview as a PNG, with the Slint version it ran on. " +
            "Pass its previewId to select a preview, or omit it for the latest opened preview. " +
            "Waits for the first render if the preview is still starting, or returns pending with its current stage. " +
            "If pending, call again with the same previewId to keep waiting. Stop retrying on timeout. " +
            "Use this for changes after interaction, browser rendering problems, or an explicit screenshot request. Use check_slint's PNG for the initial visual review.",
        inputSchema: {
            type: "object",
            properties: { previewId: { type: "string" } },
        },
        annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    },
    ...["read_slint_file", "read_binary_file"].map((name) => ({
        name,
        description: "Read a local source, image, or font file for the preview.",
        annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
        inputSchema: {
            type: "object",
            properties: { path: { type: "string" } },
            required: ["path"],
        },
        _meta: { ui: { visibility: ["app"] } },
    })),
    {
        name: "read_preview_input",
        description:
            "Read preview source, resolving a project's main entry and root.",
        annotations: {
            readOnlyHint: true,
            destructiveHint: false,
            openWorldHint: false,
        },
        inputSchema: {
            type: "object",
            properties: {
                path: { type: "string" },
                projectRoot: { type: "string" },
            },
            required: ["path"],
        },
        _meta: { ui: { visibility: ["app"] } },
    },
    {
        name: "poll_preview_capture",
        title: "Check for a Slint Preview Capture Request",
        description: "Check whether this open preview should capture a fresh screenshot.",
        annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
        inputSchema: {
            type: "object",
            properties: { previewId: { type: "string" } },
            required: ["previewId"],
        },
        _meta: { ui: { visibility: ["app"] } },
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
                captureId: { type: "string" },
                stage: {
                    type: "string",
                    enum: ["started", "reading", "loading", "compiling", "rendering", "ready"],
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

const preparations = new Map();
const PREPARATION_TTL_MS = 15 * 60 * 1000;
const MAX_PREPARATIONS = 16;
const previews = new Map();
let latestPreviewId = null;

function removePreparation(id) {
    clearTimeout(preparations.get(id)?.timer);
    preparations.delete(id);
}

function registerPreview(id, stage = "waiting") {
    const preview = {
        id,
        requestedAt: Date.now(),
        lastPolledAt: null,
        stage,
        timings: {},
        failure: null,
        captureRequest: null,
        listeners: [],
    };
    previews.set(id, preview);
    return preview;
}

setInterval(() => {
    const now = Date.now();
    for (const [id, preview] of previews) {
        const deadline = preview.lastPolledAt === null
            ? preview.requestedAt + Math.max(PREVIEW_TIMEOUT_MS, PREVIEW_POLL_TIMEOUT_MS)
            : preview.lastPolledAt + PREVIEW_POLL_TIMEOUT_MS;
        if (now <= deadline) continue;
        previews.delete(id);
        preview.failure = {
            error: "The preview stopped polling. It may be suspended or closed. " +
                "Ask the user to open its chat, then retry with the same previewId.",
        };
        notify(preview);
    }
}, 1000).unref();

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

function readPreviewFile(path, encoding) {
    if (typeof path !== "string" || !isAbsolute(path))
        throw rpcError(-32602, "'path' must be an absolute path");
    try {
        const data = readFileSync(path).toString(encoding);
        if (encoding === "utf8") return textResult(data);
        return {
            content: [
                {
                    type: "resource",
                    resource: {
                        uri: pathToFileURL(path).href,
                        mimeType:
                            BINARY_MIME_TYPES[extname(path).toLowerCase()] ??
                            "application/octet-stream",
                        blob: data,
                    },
                },
            ],
        };
    } catch (error) {
        return textResult(error.message, true);
    }
}

function textResult(text, isError = false) {
    return { content: [{ type: "text", text }], ...(isError && { isError }) };
}

const toolCalls = {
    check_slint: async (args) => {
        if (
            (typeof args.path === "string") ===
                (typeof args.source === "string") ||
            (Object.hasOwn(args, "path") && typeof args.path !== "string") ||
            (Object.hasOwn(args, "source") && typeof args.source !== "string")
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
        if (
            args.prepare_preview !== undefined &&
            typeof args.prepare_preview !== "boolean"
        ) {
            throw rpcError(-32602, "'prepare_preview' must be a boolean");
        }
        if (
            args.style !== undefined &&
            !["fluent", "material", "cupertino", "cosmic"].includes(args.style)
        ) {
            throw rpcError(
                -32602,
                "'style' must be fluent, material, cupertino, or cosmic",
            );
        }
        let checked;
        try {
            const { checkSlint } = await import("./checker.mjs");
            checked = await checkSlint(args);
        } catch (error) {
            return textResult(error.message, true);
        }
        const { image, ...structuredContent } = checked;
        if (image)
            Object.assign(structuredContent, {
                width: image.width,
                height: image.height,
            });
        if (checked.ok && args.prepare_preview) {
            for (const [id, preparation] of preparations) {
                if (preparation.expiresAt <= Date.now()) removePreparation(id);
            }
            while (preparations.size >= MAX_PREPARATIONS)
                removePreparation(preparations.keys().next().value);
            const previewId = randomUUID();
            const { prepare_preview, ...input } = args;
            preparations.set(previewId, {
                input,
                expiresAt: Date.now() + PREPARATION_TTL_MS,
                timer: setTimeout(
                    () => preparations.delete(previewId),
                    PREPARATION_TTL_MS,
                ).unref(),
            });
            structuredContent.previewId = previewId;
        }
        const summary = [
            `${checked.ok ? "Check passed" : "Check failed"} with ${checked.runtime}.`,
            ...checked.diagnostics.map(
                (diagnostic) =>
                    `${diagnostic.fileName}:${diagnostic.lineNumber}:${diagnostic.columnNumber}: ${diagnostic.severity}: ${diagnostic.message}`,
            ),
            ...(checked.renderError
                ? [`Rendering failed: ${checked.renderError}`]
                : []),
            ...(structuredContent.previewId
                ? [`Prepared preview ${structuredContent.previewId}.`]
                : []),
        ].join("\n");
        return {
            content: [
                { type: "text", text: summary },
                ...(image
                    ? [
                          {
                              type: "image",
                              data: image.png,
                              mimeType: "image/png",
                          },
                      ]
                    : []),
            ],
            structuredContent,
            ...(!checked.ok && { isError: true }),
        };
    },
    read_slint_file: (args) => readPreviewFile(args.path, "utf8"),
    read_binary_file: (args) => readPreviewFile(args.path, "base64"),
    read_preview_input: async (args) => {
        try {
            const { readPreviewInput } = await import("./checker.mjs");
            return { content: [], structuredContent: readPreviewInput(args) };
        } catch (error) {
            return textResult(error.message, true);
        }
    },
    publish_preview: (args) => {
        if (
            typeof args.previewId !== "string" ||
            Object.keys(args).length !== 1
        ) {
            throw rpcError(
                -32602,
                "Pass only 'previewId' from a successful prepared check",
            );
        }
        const preparation = preparations.get(args.previewId);
        if (!preparation || preparation.expiresAt <= Date.now()) {
            removePreparation(args.previewId);
            return textResult(
                "This preparation is unknown or expired. Run check_slint with prepare_preview: true, inspect its diagnostics and image, then publish the returned previewId.",
                true,
            );
        }
        removePreparation(args.previewId);
        if (!previews.has(args.previewId)) registerPreview(args.previewId);
        latestPreviewId = args.previewId;
        return {
            content: [
                {
                    type: "text",
                    text: `Requested Slint preview ${args.previewId}. Use the screenshot returned by check_slint for the visual review.`,
                },
            ],
            _meta: {
                "slint/previewId": args.previewId,
                "slint/preview": preparation.input,
            },
        };
    },
    get_preview_screenshot: async (args) => {
        if (args.previewId !== undefined && typeof args.previewId !== "string") {
            throw rpcError(-32602, "'previewId' must be a string");
        }
        const id = args.previewId ?? latestPreviewId;
        if (id === null) {
            return textResult("Call check_slint with prepare_preview, then publish_preview first.", true);
        }
        const preview = previews.get(id);
        if (!preview) {
            return textResult(
                `Preview ${id} is unknown or inactive. Ask the user to open its chat, then retry with the same previewId.`,
                true,
            );
        }
        const viewDeadline = preview.requestedAt + VIEW_START_TIMEOUT_MS;
        const overallDeadline = preview.requestedAt + PREVIEW_TIMEOUT_MS;
        if (!preview.captureRequest || Date.now() >= preview.captureRequest.deadline) {
            preview.captureRequest = {
                id: randomUUID(),
                deadline: Date.now() + SCREENSHOT_TIMEOUT_MS,
            };
        }
        const request = preview.captureRequest;
        while (!request.capture && !preview.failure) {
            const deadline = Math.min(
                request.deadline,
                preview.stage === "ready" ? Infinity : overallDeadline,
                preview.stage === "waiting" ? viewDeadline : Infinity,
            );
            const remaining = deadline - Date.now();
            if (remaining <= 0) break;
            await nextChange(preview, remaining);
        }
        if (preview.captureRequest === request) preview.captureRequest = null;
        const capture = preview.failure ?? request.capture;
        if (preview.stage === "waiting") {
            return textResult(
                "The preview has not confirmed startup yet. " +
                "It may be waiting for the user to open this chat. " +
                "Ask the user to open the chat, then call `get_preview_screenshot` again for the existing preview.",
                true,
            );
        }
        const ready = preview.stage === "ready";
        const structuredContent = {
            status: capture
                ? (capture.error ? "error" : "ready")
                : (ready || Date.now() >= overallDeadline ? "timeout" : "pending"),
            previewId: id,
            stage: ready && !capture ? "capturing" : capture?.error ? "error" : preview.stage,
            elapsedMs: Date.now() - preview.requestedAt,
            timings: preview.timings,
        };
        if (!capture) {
            return {
                ...textResult(
                    structuredContent.status === "timeout"
                        ? `Preview ${id} timed out (${structuredContent.stage}). Stop retrying automatically.`
                        : `Preview ${id} is pending (${preview.stage}). Call get_preview_screenshot again to keep waiting for this preview.`,
                    ready,
                ),
                structuredContent,
            };
        }
        const runtime = capture.runtime ? ` with ${capture.runtime}` : "";
        if (capture.error) {
            return {
                ...textResult(`Preview ${id}${runtime} has no screenshot: ${capture.error}`, true),
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
    // The card polls for requests because the server cannot call an open card directly.
    // https://apps.extensions.modelcontextprotocol.io/api/documents/patterns.html#polling-for-live-data
    poll_preview_capture: (args) => {
        if (typeof args.previewId !== "string") {
            throw rpcError(-32602, "'previewId' must be a string");
        }
        const preview = previews.get(args.previewId) ?? registerPreview(args.previewId, "ready");
        preview.lastPolledAt = Date.now();
        const active = !preview.failure;
        const request = active ? preview.captureRequest : null;
        return {
            content: [],
            structuredContent: {
                active,
                ...(request && Date.now() < request.deadline && { captureId: request.id }),
            },
        };
    },
    publish_preview_capture: (args) => {
        if (typeof args.previewId !== "string") {
            throw rpcError(-32602, "'previewId' must be a string");
        }
        const preview = previews.get(args.previewId) ?? (
            args.stage === "started" && args.captureId === undefined
                ? registerPreview(args.previewId)
                : undefined
        );
        if (!preview) {
            return textResult("This preview is unknown or inactive.");
        }
        if (preview.failure) {
            return textResult("This preview has already failed.");
        }
        const { captureId, stage, png, width, height, error } = args;
        const runtime = typeof args.runtime === "string" ? args.runtime : undefined;
        if (captureId !== undefined) {
            const request = preview.captureRequest;
            if (!request || captureId !== request.id || Date.now() >= request.deadline) {
                return textResult("This capture request is no longer active.");
            }
            request.capture = typeof png === "string" && error === undefined
                ? { png, width, height, runtime }
                : { error: String(error ?? "unknown error"), runtime };
            preview.captureRequest = null;
        } else {
            if (error !== undefined) preview.failure = { error: String(error), runtime };
            preview.stage = preview.failure ? "error" : stage ?? preview.stage;
            preview.timings[preview.stage] ??= Date.now() - preview.requestedAt;
        }
        notify(preview);
        return textResult("Received.");
    },

};

// The .in suffix prevents Codex from offering this MCP App template as a standalone website.
const viewHtml = readFileSync(new URL("view.html.in", import.meta.url), "utf8")
    .replace("__CURSOR_PNG__", readFileSync(new URL("assets/canvas-cursor.png", import.meta.url)).toString("base64"))
    .replace(/__ICON_([a-z-]+)__/g, (_, name) =>
        readFileSync(new URL(`assets/${name}.svg`, import.meta.url), "utf8"),
    )
    .replace("__INTERPRETER_URL__", interpreter.url)
    .replace("__CHANNEL__", channel);

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
                    text: viewHtml,
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
