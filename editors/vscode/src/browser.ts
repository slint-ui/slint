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

function reload_preview(url: string, content: string, component: string) {
    if (!previewPanel) { return; }
    if (component) {
        content += "\n_Preview := " + component + " {}\n";
    }
    previewAccessedFiles.clear();
    let webview_uri = previewPanel.webview.asWebviewUri(Uri.parse(url)).toString();
    previewAccessedFiles.add(webview_uri);
    const msg = {
        command: "preview",
        base_url: url,
        webview_uri: webview_uri,
        component: component,
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
        if (previewPanel && previewAccessedFiles.has(previewPanel.webview.asWebviewUri(event.document.uri).toString())) {
            let content_str = uri === previewUrl ? event.document.getText() :
                await getDocumentSource(previewUrl);
            reload_preview(previewUrl, content_str, previewComponent);
        }
    });

    vscode.window.registerWebviewPanelSerializer('slint-preview', new PreviewSerializer(context));
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
            { enableScripts: true, retainContextWhenHidden: true, }
        );
        initPreviewPanel(context, panel);
    }

    let content_str = await getDocumentSource(path);
    reload_preview(path, content_str, previewComponent);

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
    let current_instance = null;

    async function load_file(url) {
        let promise = new Promise(resolve => {
            promises[url] = resolve;
        });
        vscode.postMessage({ command: 'load_file',  url: url });
        let from_editor = await promise;
        return from_editor || await (await fetch(url)).text();
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
            if (current_instance !== null) {
                current_instance = component.create_with_existing_window(current_instance);
            } else {
                current_instance = component.create("slint_canvas");
                current_instance.show();
                slint.run_event_loop();
            }
        }
    }

    window.addEventListener('message', async event => {
        if (event.data.command === "preview") {
            vscode.setState({base_url: event.data.base_url, component: event.data.component});
            await render(event.data.content, event.data.webview_uri);
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

class PreviewSerializer implements vscode.WebviewPanelSerializer {
    context: vscode.ExtensionContext;
    constructor(context: vscode.ExtensionContext) {
        this.context = context;
    }
    async deserializeWebviewPanel(webviewPanel: vscode.WebviewPanel, state: any) {
        initPreviewPanel(this.context, webviewPanel);
        let content_str = await getDocumentSource(state.base_url);
        previewComponent = state.component;
        previewUrl = state.base_url;
        reload_preview(state.base_url, content_str, state.component);
    }
}

function initPreviewPanel(context: vscode.ExtensionContext, panel: vscode.WebviewPanel) {
    previewPanel = panel;
    // we will get a preview_ready when the html is loaded and message are ready to be sent
    previewBusy = true;
    panel.webview.onDidReceiveMessage(
        async message => {
            switch (message.command) {
                case 'load_file':
                    let canonical = Uri.parse(message.url).toString();
                    previewAccessedFiles.add(canonical);
                    let content_str = undefined;
                    let x = vscode.workspace.textDocuments.find(d => panel.webview.asWebviewUri(d.uri).toString() === canonical);
                    if (x) {
                        content_str = x.getText();
                    }
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

