// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore winit

import { Uri } from "vscode";
import * as vscode from "vscode";
import { BaseLanguageClient } from "vscode-languageclient";

let previewPanel: vscode.WebviewPanel | null = null;
let to_lsp_queue: object[] = [];

let language_client: BaseLanguageClient | null = null;

function use_wasm_preview(): boolean {
    return vscode.workspace
        .getConfiguration("slint")
        .get("preview.providedByEditor", false);
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

function getPreviewHtml(slint_wasm_preview_url: Uri): string {
    const result = `<!DOCTYPE html>
<html lang="en">
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
    try {
        slint_preview.run_event_loop();
    } catch (_) {
        // This is actually not an error:-/
    }

    let preview_connector = await slint_preview.PreviewConnector.create(
        (data) => { vscode.postMessage({ command: "slint/preview_to_lsp", params: data }); }
    );

    window.addEventListener('message', async message => {
        if (message.data.command === "slint/lsp_to_preview") {
            preview_connector.process_lsp_to_preview_message(
                message.data.params,
            );

            return true;
        }
    });

    preview_connector.show_ui().then(() => vscode.postMessage({ command: 'preview_ready' }));
    </script>
</head>
<body>
  <canvas style="margin-top: 10px; width: 100%; height:100%" id="canvas"></canvas>
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

function initPreviewPanel(
    context: vscode.ExtensionContext,
    panel: vscode.WebviewPanel,
): vscode.WebviewPanel {
    // we will get a preview_ready when the html is loaded and message are ready to be sent
    panel.webview.onDidReceiveMessage(
        async (message) => {
            switch (message.command) {
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
    panel.webview.html = getPreviewHtml(
        panel.webview.asWebviewUri(lsp_wasm_url),
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
