#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// MCP server over stdio that renders .slint source inline as an MCP App.
// See https://github.com/modelcontextprotocol/ext-apps for the UI extension.

import { readFileSync } from "node:fs";
import { dirname, extname, isAbsolute, resolve } from "node:path";
import { createInterface } from "node:readline";

const VIEW_URI = "ui://slint/preview.html";
const VIEW_MIME_TYPE = "text/html;profile=mcp-app";
const SLINT_RELEASE = "1.18.1";
const INTERPRETER_URLS = {
    release: `https://releases.slint.dev/${SLINT_RELEASE}/wasm-interpreter/slint_wasm_interpreter.js`,
    snapshot:
        "https://snapshots.slint.dev/master/wasm-interpreter/slint_wasm_interpreter.js",
};
const INTERPRETER_HOSTS = [
    "https://releases.slint.dev",
    // releases.slint.dev redirects the newest release to docs.slint.dev/latest.
    "https://docs.slint.dev",
    "https://snapshots.slint.dev",
];

const pluginVersion = JSON.parse(
    readFileSync(
        new URL("../../.claude-plugin/plugin.json", import.meta.url),
        "utf8",
    ),
).version;

const tools = [
    {
        name: "preview_slint",
        title: "Preview Slint",
        description:
            "Render .slint code as a live, interactive preview in the conversation. " +
            "Pass `path` to preview a file on disk, with its imports and images, or `source` for a snippet that imports only from std-widgets.slint. " +
            "Compile errors are reported back to you once the preview has loaded.",
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
                style: {
                    type: "string",
                    enum: ["fluent", "material", "cupertino", "cosmic"],
                    description: "Widget style for std-widgets.slint.",
                },
                channel: {
                    type: "string",
                    enum: ["release", "snapshot"],
                    description: `'release' uses Slint ${SLINT_RELEASE}, 'snapshot' the latest development build.`,
                },
            },
        },
        _meta: { ui: { resourceUri: VIEW_URI } },
    },
    {
        name: "read_slint_file",
        title: "Read Slint File",
        description:
            "Read a .slint file for the preview, with its @image-url images inlined as data: URIs.",
        inputSchema: {
            type: "object",
            properties: { path: { type: "string" } },
            required: ["path"],
        },
        _meta: { ui: { visibility: ["app"] } },
    },
];

const IMAGE_MIME_TYPES = {
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".gif": "image/gif",
    ".webp": "image/webp",
    ".svg": "image/svg+xml",
    ".bmp": "image/bmp",
};

// The 1.18 interpreter loads an image path with an <img> element, which can't reach the disk from the
// host's sandbox, so each local image becomes a data: URI.
function inlineImages(source, file) {
    return source.replace(/@image-url\(\s*"([^"\\]*)"/g, (match, url) => {
        if (/^(data|https?):/.test(url) || url.startsWith("@")) return match;
        const imagePath = isAbsolute(url) ? url : resolve(dirname(file), url);
        let data;
        try {
            data = readFileSync(imagePath);
        } catch {
            return match;
        }
        const mimeType =
            IMAGE_MIME_TYPES[extname(imagePath).toLowerCase()] ??
            "application/octet-stream";
        return `@image-url("data:${mimeType};base64,${data.toString("base64")}"`;
    });
}

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
        return textResult("Rendering the Slint preview.");
    },
    read_slint_file: (args) => {
        if (typeof args.path !== "string" || !isAbsolute(args.path)) {
            throw rpcError(-32602, "'path' must be an absolute path");
        }
        try {
            return textResult(
                inlineImages(readFileSync(args.path, "utf8"), args.path),
            );
        } catch (e) {
            return textResult(e.message, true);
        }
    },
};

function readView() {
    return readFileSync(new URL("view.html", import.meta.url), "utf8");
}

const handlers = {
    initialize: (params) => ({
        protocolVersion: params?.protocolVersion ?? "2025-06-18",
        capabilities: { tools: {}, resources: {} },
        serverInfo: { name: "slint-preview", version: pluginVersion },
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
                        .replace("__RELEASE_URL__", INTERPRETER_URLS.release)
                        .replace("__SNAPSHOT_URL__", INTERPRETER_URLS.snapshot),
                    _meta: {
                        ui: {
                            csp: {
                                connectDomains: INTERPRETER_HOSTS,
                                resourceDomains: INTERPRETER_HOSTS,
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
    try {
        send({
            jsonrpc: "2.0",
            id: message.id,
            result: handler(message.params),
        });
    } catch (e) {
        send({
            jsonrpc: "2.0",
            id: message.id,
            error: { code: e.code ?? -32603, message: e.message },
        });
    }
});
