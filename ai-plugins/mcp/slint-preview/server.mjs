#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// MCP server over stdio that renders .slint source inline as an MCP App.
// See https://github.com/modelcontextprotocol/ext-apps for the UI extension.

import { readFileSync } from "node:fs";
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
            "Render self-contained .slint source as a live, interactive preview in the conversation. " +
            "The source may import from std-widgets.slint but no other files. " +
            "Compile errors are reported back to you once the preview has loaded.",
        inputSchema: {
            type: "object",
            properties: {
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
            required: ["source"],
        },
        _meta: { ui: { resourceUri: VIEW_URI } },
    },
];

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
        if (params?.name !== "preview_slint") {
            throw rpcError(-32602, `Unknown tool: ${params?.name}`);
        }
        const args = params.arguments ?? {};
        if (typeof args.source !== "string") {
            throw rpcError(-32602, "'source' must be a string");
        }
        return {
            content: [{ type: "text", text: "Rendering the Slint preview." }],
        };
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
