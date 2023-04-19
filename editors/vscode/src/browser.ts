// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// This file is the entry point for the vscode web extension

import { Uri } from "vscode";
import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/browser";

import { PropertiesViewProvider } from "./properties_webview";
import * as wasm_preview from "./wasm_preview";
import * as common from "./common";

let client = new common.ClientHandle();
let statusBar: vscode.StatusBarItem;
let properties_provider: PropertiesViewProvider;

function startClient(context: vscode.ExtensionContext) {
    //let args = vscode.workspace.getConfiguration('slint').get<[string]>('lsp-args');

    // Options to control the language client
    const clientOptions = common.languageClientOptions((args: any) => {
        wasm_preview.showPreview(
            context,
            vscode.Uri.parse(args[0], true),
            args[1],
        );
        return true;
    });

    clientOptions.synchronize = {};
    clientOptions.initializationOptions = {};

    const serverMain = Uri.joinPath(
        context.extensionUri,
        "out/browserServerMain.js",
    );

    const worker = new Worker(serverMain.toString(true));
    worker.onmessage = (m) => {
        // We cannot start sending messages to the client before we start listening which
        // the server only does in a future after the wasm is loaded.
        if (m.data === "OK") {
            const cl = new LanguageClient(
                "slint-lsp",
                "Slint LSP",
                clientOptions,
                worker,
            );
            const disposable = cl.start();
            context.subscriptions.push(disposable);

            cl.onReady().then(() => {
                client.client = cl;
                cl.onRequest("slint/load_file", async (param: string) => {
                    return await vscode.workspace.fs.readFile(
                        Uri.parse(param, true),
                    );
                });
                wasm_preview.initClientForPreview(context, cl);
                //client.onNotification(serverStatus, (params) => setServerStatus(params, statusBar));

                vscode.workspace.onDidChangeConfiguration(async (ev) => {
                    if (ev.affectsConfiguration("slint")) {
                        cl.sendNotification(
                            "workspace/didChangeConfiguration",
                            { settings: "" },
                        );
                        wasm_preview.refreshPreview();
                    }
                });
            });
        }
    };
}

// this method is called when vs code is activated
export function activate(context: vscode.ExtensionContext) {
    [statusBar, properties_provider] = common.activate(context, client, (ctx) =>
        startClient(ctx),
    );
}

export function deactivate(): Thenable<void> | undefined {
    return common.deactivate(client);
}
