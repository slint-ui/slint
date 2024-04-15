// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

// This file is the entry point for the vscode web extension

import { Uri } from "vscode";
import * as vscode from "vscode";
import {
    BaseLanguageClient,
    LanguageClient,
} from "vscode-languageclient/browser";

import { PropertiesViewProvider } from "./properties_webview";
import * as wasm_preview from "./wasm_preview";
import * as common from "./common";

let statusBar: vscode.StatusBarItem;
let properties_provider: PropertiesViewProvider;

function startClient(
    client: common.ClientHandle,
    context: vscode.ExtensionContext,
) {
    //let args = vscode.workspace.getConfiguration('slint').get<[string]>('lsp-args');

    // Options to control the language client
    const clientOptions = common.languageClientOptions();
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

            common.prepare_client(cl);

            client.add_updater((cl) => {
                cl?.onRequest("slint/load_file", async (param: string) => {
                    let contents = await vscode.workspace.fs.readFile(
                        Uri.parse(param, true),
                    );
                    return new TextDecoder().decode(contents);
                });
            });

            cl.start().then(() => (client.client = cl));
        }
    };
}

// this method is called when vs code is activated
export function activate(context: vscode.ExtensionContext) {
    [statusBar, properties_provider] = common.activate(context, (cl, ctx) =>
        startClient(cl, ctx),
    );
}

export function deactivate(): Thenable<void> | undefined {
    return common.deactivate();
}
