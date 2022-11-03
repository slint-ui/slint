// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import * as path from 'path';
import { existsSync } from 'fs';
import * as vscode from 'vscode';

import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    NotificationType,
    ExecutableOptions,
} from 'vscode-languageclient/node';
import { Property, PropertyQuery } from '../../../tools/online_editor/src/shared/properties';


export interface ServerStatusParams {
    health: "ok" | "warning" | "error";
    quiescent: boolean;
    message?: string;
}
export const serverStatus = new NotificationType<ServerStatusParams>("experimental/serverStatus");


let client: LanguageClient;
let statusBar: vscode.StatusBarItem;

const program_extension = process.platform === "win32" ? ".exe" : "";

interface Platform {
    program_name: string,
    options?: ExecutableOptions
}

function lspPlatform(): Platform | null {
    if (process.platform === "darwin") {
        return {
            program_name: "Slint Live Preview.app/Contents/MacOS/slint-lsp"
        };
    }
    else if (process.platform === "linux") {
        let remote_env_options = null;
        if (typeof vscode.env.remoteName !== "undefined") {
            remote_env_options = {
                "DISPLAY": ":0",
                "SLINT_FULLSCREEN": "1"
            };
        }
        if (process.arch === "x64") {
            return {
                program_name: "slint-lsp-x86_64-unknown-linux-gnu",
                options: {
                    env: remote_env_options
                }
            };
        } else if (process.arch === "arm") {
            return {
                program_name: "slint-lsp-armv7-unknown-linux-gnueabihf",
                options: {
                    env: remote_env_options
                }
            };
        } else if (process.arch === "arm64") {
            return {
                program_name: "slint-lsp-aarch64-unknown-linux-gnu",
                options: {
                    env: remote_env_options
                }
            };
        }
    }
    else if (process.platform === "win32") {
        return {
            program_name: "slint-lsp-x86_64-pc-windows-gnu.exe"
        };
    }
    return null;
}

function startClient(context: vscode.ExtensionContext) {

    let lsp_platform = lspPlatform();
    if (lsp_platform === null) {
        return;
    }

    // Try a local ../target build first, then try the plain bundled binary and finally the architecture specific one.
    // A debug session will find the first one, a local package build the second and the distributed vsix the last.
    const lspSearchPaths = [
        path.join(context.extensionPath, '..', '..', 'target', 'debug', 'slint-lsp' + program_extension),
        path.join(context.extensionPath, '..', '..', 'target', 'release', 'slint-lsp' + program_extension),
        path.join(context.extensionPath, "bin", "slint-lsp" + program_extension),
        path.join(context.extensionPath, "bin", lsp_platform.program_name),
    ];

    let serverModule = lspSearchPaths.find(path => existsSync(path));

    if (serverModule === undefined) {
        console.warn("Could not locate slint-lsp server binary, neither in bundled bin/ directory nor relative in ../target");
        return;
    }

    let options = Object.assign({}, lsp_platform.options);
    options.env = Object.assign({}, process.env, lsp_platform.options?.env);

    const devBuild = serverModule !== lspSearchPaths[lspSearchPaths.length - 1];
    if (devBuild) {
        options.env["RUST_BACKTRACE"] = "1";
    }

    console.log(`Starting LSP server from ${serverModule}`);

    let args = vscode.workspace.getConfiguration('slint').get<[string]>('lsp-args');

    let serverOptions: ServerOptions = {
        run: { command: serverModule, options: options, args: args },
        debug: { command: serverModule, options: options, args: args }
    };

    let clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'slint' }],
    };

    client = new LanguageClient(
        'slint-lsp',
        'Slint LSP',
        serverOptions,
        clientOptions
    );

    client.start();
    let initClient = () => {
        client.onNotification(serverStatus, (params) => setServerStatus(params, statusBar));
    };
    client.onReady().then(initClient);

}

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

    const properties_provider = new PropertiesViewProvider(context.extensionUri);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(PropertiesViewProvider.viewType, properties_provider));
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

function setServerStatus(status: ServerStatusParams, statusBar: vscode.StatusBarItem) {
    let icon = "";
    switch (status.health) {
        case "ok":
            statusBar.color = undefined;
            break;
        case "warning":
            statusBar.color = new vscode.ThemeColor("notificationsWarningIcon.foreground");
            icon = "$(warning) ";
            break;
        case "error":
            statusBar.color = new vscode.ThemeColor("notificationsErrorIcon.foreground");
            icon = "$(error) ";
            break;
    }
    statusBar.tooltip = "Slint";
    statusBar.text = `${icon} ${status.message ?? "Slint"}`;
    statusBar.show();
}


class PropertiesViewProvider implements vscode.WebviewViewProvider {

    public static readonly viewType = 'slint.propertiesView';

    private _view?: vscode.WebviewView;

    constructor(private readonly _extensionUri: vscode.Uri) { }

    public resolveWebviewView(
        webviewView: vscode.WebviewView,
        _context: vscode.WebviewViewResolveContext,
        _token: vscode.CancellationToken,
    ) {
        this._view = webviewView;

        webviewView.webview.options = {
            // Allow scripts in the webview
            enableScripts: true,

            localResourceRoots: [
                this._extensionUri
            ]
        };

        webviewView.webview.html = this._getHtmlForWebview(webviewView.webview);

        webviewView.webview.onDidReceiveMessage(data => {
            switch (data.command) {
                case 'property_clicked':
                    if (vscode.window.activeTextEditor) {
                        const p = data.property as Property;
                        if (p.defined_at && p.defined_at.property_definition_range) {
                            let range = client.protocol2CodeConverter.asRange(p.defined_at.property_definition_range);
                            vscode.window.activeTextEditor.revealRange(range);
                            vscode.window.activeTextEditor.selection = new vscode.Selection(range.start, range.end);
                        }
                    }
                    break;
                case 'change_property':
                    if (vscode.window.activeTextEditor) {
                        const p = data.property as Property;
                        if (p.defined_at && p.defined_at.expression_range) {
                            let range = client.protocol2CodeConverter.asRange(p.defined_at.expression_range);
                            let old = vscode.window.activeTextEditor.document.getText(range);
                            console.log("maybe", old, data.old_value)
                            if (old === data.old_value) {
                                vscode.window.activeTextEditor.edit(b => b.replace(range, data.new_value));
                            }
                        }
                    }
                    break;
            }
        });


        vscode.window.onDidChangeTextEditorSelection(async (event: vscode.TextEditorSelectionChangeEvent) => {
            //client.comma sendRequest("slint/showPreview", ae.document.uri.fsPath.toString());

            if (event.selections.length == 0) {
                return;
            }
            let selection = event.selections[0];

            let r = await vscode.commands.executeCommand(
                "queryProperties",
                event.textEditor.document.uri.toString(),
                selection.active.line,
                selection.active.character,
            );
            const result = r as PropertyQuery;
            const result_str = JSON.stringify(result);
            const msg = {
                command: "set_properties",
                properties: r,
                code: event.textEditor.document.getText(),
            };
            webviewView.webview.postMessage(msg);
        });
    }

    private _getHtmlForWebview(webview: vscode.Webview) {

        const scriptUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'out/propertiesView.js'));
        const nonce = getNonce();


        // FIXME: share with the online editor?
        const css = `
            :root {
                /* Slint colors */
                --slint-blue: #0025ff;
                --slint-black: #000000;
                --slint-dark-grey: #191c20;
                --slint-grey: #2c2f36;
                --slint-white: #ffffff;
                --slint-green: #dbff00;

                /* style properties */
                --error-bg: #ff0000;
                --error-fg: var(--slint-black);

                --highlight-bg: var(--slint-blue);
                --highlight-fg: var(--slint-white);

                --highlight2-bg: var(--slint-green);
                --highlight2-fg: var(--slint-black);

                --undefined-bg: #c0c0c0;
                --undefined-fg: var(--slint-black);

                --default-font: 12px Helvetica, Arial, sans-serif;
            }


            .properties-editor .element-header {
                background: var(--highlight-bg);
                color: var(--highlight-fg);
                font-size: 140%;
                font-weight: bold;

                width: 100%;
                height: 50px;
            }

            .properties-editor .name-column {
                white-space: nowrap;
            }

            .properties-editor .value-column {
                width: 90%;
            }

            .properties-editor .properties-table {
                width: 100%;
            }

            .properties-editor .properties-table .group-header td {
                background-color: var(--highlight2-bg);
                color: var(--highlight2-fg);
                font-weight: bold;
            }

            .properties-editor .properties-table .undefined {
                background-color: var(--undefined-bg);
                color: var(--undefined-fg);
            }

            .properties-editor .value-column.type-unknown:before {
                content: "??? ";
                padding: 2px;
            }

            .properties-editor .properties-table input {
                margin: 0px;
                border: none;
                width: 100%;
            }

            .properties-editor .properties-table input.value-changed {
                color: var(--error-bg);
            }
        `;

        return `<!DOCTYPE html>
			<html lang="en">
			<head>
				<meta charset="UTF-8">
                <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${nonce}';">
				<meta name="viewport" content="width=device-width, initial-scale=1.0">
                <title>Slint preview</title>
                <style nonce=${nonce}">${css}</style>
			</head>
			<body class="properties-editor">
                <script nonce="${nonce}" src="${scriptUri}"></script>
			</body>
			</html>`;
    }
}

function getNonce() {
    let text = '';
    const possible = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    for (let i = 0; i < 32; i++) {
        text += possible.charAt(Math.floor(Math.random() * possible.length));
    }
    return text;
}