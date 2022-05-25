// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial


import { ExtensionContext, Uri } from 'vscode';
import * as vscode from 'vscode';
import { LanguageClientOptions } from 'vscode-languageclient';
import { LanguageClient } from 'vscode-languageclient/browser';

let client: LanguageClient;
let statusBar: vscode.StatusBarItem;

function startClient(context: vscode.ExtensionContext) {

    //let args = vscode.workspace.getConfiguration('slint').get<[string]>('lsp-args');

    const documentSelector = [{ scheme: 'file', language: 'slint' }];

    // Options to control the language client
    const clientOptions: LanguageClientOptions = {
        documentSelector,
        synchronize: {},
        initializationOptions: {}
    };

    const serverMain = Uri.joinPath(context.extensionUri, 'out/browserServerMain.js');
    const worker = new Worker(serverMain.toString(true));
    client = new LanguageClient('slint-lsp', 'Slint LSP', clientOptions, worker);
    const disposable = client.start();
    context.subscriptions.push(disposable);

    client.onReady().then(() => {
        //client.onNotification(serverStatus, (params) => setServerStatus(params, statusBar));
    });
}


// this method is called when vs code is activated
export function activate(context: vscode.ExtensionContext) {

    statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left);
    context.subscriptions.push(statusBar);
    statusBar.text = "Slint";

    startClient(context);

    context.subscriptions.push(vscode.commands.registerCommand('slint.showPreview', function () {
        let ae = vscode.window.activeTextEditor;
        if (!ae) {
            return;
        }
        client.sendNotification("slint/showPreview", ae.document.uri.fsPath.toString());
    }));

    context.subscriptions.push(vscode.commands.registerCommand('slint.reload', async function () {
        statusBar.hide();
        await client.stop();
        startClient(context);
    }));
}


