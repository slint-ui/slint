// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// This file is the entry point for the vscode web extension

import { Uri } from "vscode";
import * as vscode from "vscode";
import { LanguageClientOptions } from "vscode-languageclient";
import { LanguageClient } from "vscode-languageclient/browser";

import { PropertiesViewProvider } from "./properties_webview";
import {
    PreviewSerializer,
    showPreview,
    initClientForPreview,
    refreshPreview,
} from "./web_preview";

let client: LanguageClient;
let statusBar: vscode.StatusBarItem;
let properties_provider: PropertiesViewProvider;

function startClient(context: vscode.ExtensionContext) {
    //let args = vscode.workspace.getConfiguration('slint').get<[string]>('lsp-args');

    const documentSelector = [{ language: "slint" }, { language: "rust" }];

    // Options to control the language client
    const clientOptions: LanguageClientOptions = {
        documentSelector,
        synchronize: {},
        initializationOptions: {},
    };

    const serverMain = Uri.joinPath(
        context.extensionUri,
        "out/browserServerMain.js",
    );
    const worker = new Worker(serverMain.toString(true));
    worker.onmessage = (m) => {
        // We cannot start sending messages to the client before we start listening which
        // the server only does in a future after the wasm is loaded.
        if (m.data === "OK") {
            client = new LanguageClient(
                "slint-lsp",
                "Slint LSP",
                clientOptions,
                worker,
            );
            const disposable = client.start();
            context.subscriptions.push(disposable);

            client.onReady().then(() => {
                if (properties_provider) {
                    properties_provider.client = client;
                }

                client.onRequest("slint/load_file", async (param: string) => {
                    return await vscode.workspace.fs.readFile(Uri.parse(param));
                });
                initClientForPreview(context, client);
                //client.onNotification(serverStatus, (params) => setServerStatus(params, statusBar));

                vscode.workspace.onDidChangeConfiguration(async (ev) => {
                    if (ev.affectsConfiguration("slint")) {
                        client.sendNotification(
                            "workspace/didChangeConfiguration",
                            { settings: "" },
                        );
                        refreshPreview();
                    }
                });
            });
        }
    };
}

// this method is called when vs code is activated
export function activate(context: vscode.ExtensionContext) {
    statusBar = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Left,
    );
    context.subscriptions.push(statusBar);
    statusBar.text = "Slint";

    startClient(context);

    context.subscriptions.push(
        vscode.commands.registerCommand("slint.showPreview", async function () {
            let ae = vscode.window.activeTextEditor;
            if (!ae) {
                return;
            }
            await showPreview(context, ae.document.uri, "");
        }),
    );

    context.subscriptions.push(
        vscode.commands.registerCommand("slint.reload", async function () {
            statusBar.hide();
            await client.stop();
            startClient(context);
        }),
    );

    vscode.workspace.onDidChangeTextDocument(async (event) => {
        await refreshPreview(event);

        // Send a request for properties information after passing through the
        // event loop once to make sure the LSP got signaled to update.
        setTimeout(() => {
            properties_provider.refresh_view();
        }, 1);
    });

    vscode.window.registerWebviewPanelSerializer(
        "slint-preview",
        new PreviewSerializer(context),
    );

    properties_provider = new PropertiesViewProvider(context.extensionUri);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(
            PropertiesViewProvider.viewType,
            properties_provider,
        ),
    );

    properties_provider.refresh_view();
}
