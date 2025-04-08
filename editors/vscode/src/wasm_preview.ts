// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { Uri } from "vscode";

import * as vscode from "vscode";
import type { BaseLanguageClient } from "vscode-languageclient";

let previewPanel: vscode.WebviewPanel | null = null;
let to_lsp_queue: object[] = [];

let language_client: BaseLanguageClient | null = null;

function use_wasm_preview(): boolean {
    return vscode.workspace
        .getConfiguration("slint")
        .get("preview.providedByEditor", false);
}

export function panel(): vscode.WebviewPanel | null {
    return previewPanel;
}

export function update_configuration() {
    if (language_client) {
        send_to_lsp({
            PreviewTypeChanged: {
                is_external: previewPanel !== null || use_wasm_preview(),
            },
        });
    }
}

/// Initialize the callback on the client to make the web preview work
export function initClientForPreview(
    context: vscode.ExtensionContext,
    client: BaseLanguageClient | null,
) {
    language_client = client;

    if (client) {
        update_configuration();

        client.onNotification("slint/lsp_to_preview", async (message: any) => {
            if ("ShowPreview" in message) {
                if (open_preview(context)) {
                    return;
                }
            }

            previewPanel?.webview.postMessage({
                command: "slint/lsp_to_preview",
                params: message,
            });
        });

        // Send messages that got queued while LS was down...
        for (const m of to_lsp_queue) {
            send_to_lsp(m);
        }
        to_lsp_queue = [];
    }
}

function send_to_lsp(message: any): boolean {
    if (language_client) {
        language_client.sendNotification("slint/preview_to_lsp", message);
    } else {
        to_lsp_queue.push(message);
    }

    return language_client !== null;
}

function open_preview(context: vscode.ExtensionContext): boolean {
    if (previewPanel !== null) {
        return false;
    }

    // Create and show a new webview
    const panel = vscode.window.createWebviewPanel(
        "slint-preview",
        "Slint Preview",
        vscode.ViewColumn.Beside,
        { enableScripts: true, retainContextWhenHidden: true },
    );
    previewPanel = initPreviewPanel(context, panel);

    return true;
}

function getPreviewHtml(
    slint_wasm_preview_url: Uri,
    default_style: string,
): string {
    const experimental =
        typeof process !== "undefined" &&
        process.env.hasOwnProperty("SLINT_ENABLE_EXPERIMENTAL_FEATURES");
    const result = `<!DOCTYPE html>
<html lang="en" style="height: 100%; width: 100%;">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Slint Preview</title>
    <script type="module">
    "use strict";
    import * as slint_preview from '${slint_wasm_preview_url}';
    await slint_preview.default();

    const vscode = acquireVsCodeApi();
    let promises = {};
    slint_preview.run_event_loop();

    const canvas_id = "canvas";

    const canvas = document.createElement("canvas");

    const pending_mapping_requests = {};

    canvas.id = canvas_id;
    canvas.className = canvas_id;
    canvas.style.width = "100%";
    canvas.style.height = "100%";
    canvas.style.outline = "none";
    canvas.style.touchAction = "none";
    canvas.width = canvas.offsetWidth;
    canvas.height = canvas.offsetHeight;

    canvas.dataset.slintAutoResizeToPreferred = "false";

    document.body.replaceChildren(canvas);

    new ResizeObserver(() => {
        canvas.style.minWidth = "100%";
        canvas.style.width = "100%";
        canvas.style.maxWidth = "100%";
        canvas.style.minHeight = "100%";
        canvas.style.height = "100%";
        canvas.style.maxHeight = "100%";
    }).observe(document.body);

    let preview_connector = await slint_preview.PreviewConnector.create(
        (data) => { vscode.postMessage({ command: "slint/preview_to_lsp", params: data }); },
        (url) => { return new Promise((resolve, _) => {
            pending_mapping_requests[url] = resolve;
            vscode.postMessage({ command: "map_url", url: url });
        })},
        "${default_style}",
        ${experimental ? "true" : "false"}
    );

    window.addEventListener('message', async message => {
        if (message.data.command === "slint/lsp_to_preview") {
            preview_connector.process_lsp_to_preview_message(
                message.data.params,
            );

            return true;
        }
        if (message.data.command === "map_response") {
            const original = message.data.original;

            const resolve = pending_mapping_requests[original];
            delete pending_mapping_requests[original];
            if (resolve) {
                resolve(message.data.mapped);
            }
        }
    });

    preview_connector.show_ui().then(() => {
        canvas.style.width = "100%";
        canvas.style.height = "100%";
        vscode.postMessage({ command: 'preview_ready' });
    });

    </script>
</head>
<body style="padding: 0; height: 100%; width: 100%" data-vscode-context='{"webviewSection": "slint-previewer"}'>>
</body>
</html>`;

    return result;
}

export class PreviewSerializer implements vscode.WebviewPanelSerializer {
    context: vscode.ExtensionContext;

    constructor(context: vscode.ExtensionContext) {
        this.context = context;
    }

    async deserializeWebviewPanel(
        webviewPanel: vscode.WebviewPanel,
        _state: any,
    ) {
        previewPanel = initPreviewPanel(this.context, webviewPanel);
        //// How can we load this state? We can not query the necessary data...
    }
}

function map_url(webview: vscode.Webview, url_: string) {
    let result: string | undefined;

    try {
        const url = Uri.parse(url_, false);
        if (vscode.workspace.getWorkspaceFolder(url)) {
            result = previewPanel?.webview.asWebviewUri(url)?.toString();
        }
    } catch (_) {
        /* nothing to handle */
    }

    webview.postMessage({
        command: "map_response",
        original: url_,
        mapped: result,
    });
}

function initPreviewPanel(
    context: vscode.ExtensionContext,
    panel: vscode.WebviewPanel,
): vscode.WebviewPanel {
    panel.iconPath = Uri.joinPath(context.extensionUri, "slint-file-icon.svg");
    // we will get a preview_ready when the html is loaded and message are ready to be sent
    panel.webview.onDidReceiveMessage(
        async (message) => {
            switch (message.command) {
                case "map_url":
                    map_url(panel.webview, message.url);
                    return;
                case "preview_ready":
                    send_to_lsp({ RequestState: { unused: true } });
                    return;
                case "slint/preview_to_lsp":
                    send_to_lsp(message.params);
                    return;
            }
        },
        undefined,
        context.subscriptions,
    );
    const lsp_wasm_url = Uri.joinPath(
        context.extensionUri,
        "out/slint_lsp_wasm.js",
    );
    const default_style = vscode.workspace
        .getConfiguration("slint")
        .get("preview.style", "");
    panel.webview.html = getPreviewHtml(
        panel.webview.asWebviewUri(lsp_wasm_url),
        default_style,
    );
    panel.onDidDispose(
        () => {
            previewPanel = null;
            update_configuration();
        },
        undefined,
        context.subscriptions,
    );

    return panel;
}
