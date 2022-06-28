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

    const documentSelector = [{ language: 'slint' }];

    // Options to control the language client
    const clientOptions: LanguageClientOptions = {
        documentSelector,
        synchronize: {},
        initializationOptions: {}
    };

    const serverMain = Uri.joinPath(context.extensionUri, 'out/browserServerMain.js');
    const worker = new Worker(serverMain.toString(true));
    worker.onmessage = m => {
        // We cannot start sending messages to the client before we start listening which
        // the server only does in a future after the wasm is loaded.
        if (m.data === "OK") {

            client = new LanguageClient('slint-lsp', 'Slint LSP', clientOptions, worker);
            const disposable = client.start();
            context.subscriptions.push(disposable);

            client.onReady().then(() => {
                client.onRequest("slint/load_file", async (param: string) => {
                    return await vscode.workspace.fs.readFile(Uri.parse(param));
                });
                client.onRequest("slint/showPreview", async (param: string[]) => {
                    showPreview(context, param[0], param[1]);
                    return;
                });
                //client.onNotification(serverStatus, (params) => setServerStatus(params, statusBar));
            });
        }
    };
}

let previewPanel: vscode.WebviewPanel | undefined = undefined;
let previewUrl: string = "";
let previewAccessedFiles = new Set();
let previewComponent: string = "";
let queuedPreviewMsg: any = undefined;
let previewBusy = false;

function reload_preview(url: string, content: string) {
    if (!previewPanel) { return; }
    previewAccessedFiles.clear();
    previewAccessedFiles.add(url);
    const msg = {
        command: "preview",
        base_url: url,
        content: content
    };
    if (previewBusy) {
        queuedPreviewMsg = msg;
    } else {
        previewPanel.webview.postMessage(msg);
        previewBusy = true;
    }
}


// this method is called when vs code is activated
export function activate(context: vscode.ExtensionContext) {
    statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left);
    context.subscriptions.push(statusBar);
    statusBar.text = "Slint";

    startClient(context);

    context.subscriptions.push(vscode.commands.registerCommand('slint.showPreview', async function () {
        let ae = vscode.window.activeTextEditor;
        if (!ae) {
            return;
        }
        await showPreview(context, ae.document.uri.toString(), "");
    }));

    context.subscriptions.push(vscode.commands.registerCommand('slint.reload', async function () {
        statusBar.hide();
        await client.stop();
        startClient(context);
    }));

    vscode.workspace.onDidChangeTextDocument(async event => {
        let uri = event.document.uri.toString();
        if (previewAccessedFiles.has(event.document.uri.toString())) {
            let content_str = uri === previewUrl ? event.document.getText() :
                await getDocumentSource(previewUrl);
            if (previewComponent) {
                content_str += "\n_Preview := " + previewComponent + " {}\n";
            }
            reload_preview(previewUrl, content_str);
        }
    });
}

async function showPreview(context: vscode.ExtensionContext, path: string, component: string) {

    previewUrl = path;
    previewComponent = component;

    if (previewPanel) {
        previewPanel.reveal(vscode.ViewColumn.Beside);
    } else {
        // Create and show a new webview
        const panel = vscode.window.createWebviewPanel(
            'slint-preview',
            'Slint Preview',
            vscode.ViewColumn.Beside,
            { enableScripts: true }
        );
        previewPanel = panel;
        // we will get a preview_ready when the html is loaded and message are ready to be sent
        previewBusy = true;
        panel.webview.onDidReceiveMessage(
            async message => {
                switch (message.command) {
                    case 'load_file':
                        let content_str = await getDocumentSource(message.url);
                        previewAccessedFiles.add(message.url);
                        panel.webview.postMessage({ command: "file_loaded", url: message.url, content: content_str });
                        return;
                    case 'preview_ready':
                        if (queuedPreviewMsg) {
                            panel.webview.postMessage(queuedPreviewMsg);
                            queuedPreviewMsg = undefined;
                        } else {
                            previewBusy = false;
                        }
                        return;
                }
            },
            undefined,
            context.subscriptions
        );
        let slint_wasm_interpreter_url = panel.webview.asWebviewUri(Uri.joinPath(context.extensionUri, 'out/slint_wasm_interpreter.js'));
        panel.webview.html = getPreviewHtml(slint_wasm_interpreter_url);
        panel.onDidDispose(
            () => {
                previewPanel = undefined;
            },
            undefined,
            context.subscriptions);
    }

    let content_str = await getDocumentSource(path);
    if (component) {
        content_str += "\n_Preview := " + component + " {}\n";
    }
    reload_preview(path, content_str);

}

async function getDocumentSource(url: string): Promise<string> {
    // FIXME: is there a faster way to get the document
    let x = vscode.workspace.textDocuments.find(d => d.uri.toString() === url);
    if (x) {
        return x.getText();
    }
    return new TextDecoder().decode(
        await vscode.workspace.fs.readFile(Uri.parse(url)));
}


function getPreviewHtml(slint_wasm_interpreter_url: Uri): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Slint Preview</title>
    <script type="module">
    "use strict";
    import * as slint from '${slint_wasm_interpreter_url}';
    await slint.default();

    const vscode = acquireVsCodeApi();
    let promises = {};
    let current_instance = undefined;

    async function load_file(url) {
        let promise = new Promise(resolve => {
            promises[url] = resolve;
        });
        vscode.postMessage({ command: 'load_file',  url: url });
        return await promise;
    }

    async function render(source, base_url) {
        let { component, error_string } = await slint.compile_from_string(source, base_url, async(url) => await load_file(url));
        if (error_string != "") {
            var text = document.createTextNode(error_string);
            var p = document.createElement('pre');
            p.appendChild(text);
            document.getElementById("slint_error_div").innerHTML = "<pre style='color: red; background-color:#fee; margin:0'>" + p.innerHTML + "</pre>";
        }
        vscode.postMessage({ command: 'preview_ready' });
        if (component !== undefined) {
            document.getElementById("slint_error_div").innerHTML = "";
            let instance = component.create("slint_canvas");
            instance.show();
            if (current_instance) {
                current_instance.hide();
            } else {
                slint.run_event_loop();
            }
            current_instance = instance;
        }
    }

    window.addEventListener('message', async event => {
        if (event.data.command === "preview") {
            await render(event.data.content, event.data.base_url);
        } else if (event.data.command === "file_loaded") {
            let resolve = promises[event.data.url];
            if (resolve) {
                delete promises[event.data.url];
                resolve(event.data.content);
            }
        }
    });

    vscode.postMessage({ command: 'preview_ready' });
    </script>
</head>
<body>
  <div id="slint_error_div"></div>
  <canvas id="slint_canvas"></canvas>
</body>
</html>`;
}
