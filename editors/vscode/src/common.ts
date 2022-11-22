// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// This file contains the common code for both the normal and the browser extension

import { Property } from "../../../tools/online_editor/src/shared/properties";
import { query_properties } from "../../../tools/online_editor/src/shared/properties_client";

import * as vscode from "vscode";
import { BaseLanguageClient } from "vscode-languageclient";

let client: BaseLanguageClient | null = null;
export function set_client(c: BaseLanguageClient) {
    client = c;
}

export class PropertiesViewProvider implements vscode.WebviewViewProvider {
    public static readonly viewType = "slint.propertiesView";

    private _view?: vscode.WebviewView;
    private property_shown: boolean = false;

    constructor(private readonly _extensionUri: vscode.Uri) {}

    public resolveWebviewView(
        webviewView: vscode.WebviewView,
        _context: vscode.WebviewViewResolveContext,
        _token: vscode.CancellationToken,
    ) {
        this._view = webviewView;

        webviewView.webview.options = {
            // Allow scripts in the webview
            enableScripts: true,

            localResourceRoots: [this._extensionUri],
        };

        webviewView.webview.html = this._getHtmlForWebview(webviewView.webview);

        webviewView.webview.onDidReceiveMessage((data) => {
            switch (data.command) {
                case "property_clicked":
                    if (vscode.window.activeTextEditor) {
                        const p = data.property as Property;
                        if (
                            p.defined_at &&
                            p.defined_at.property_definition_range
                        ) {
                            if (client === null) {
                                return;
                            }

                            let range = client.protocol2CodeConverter.asRange(
                                p.defined_at.property_definition_range,
                            );
                            vscode.window.activeTextEditor.revealRange(range);
                            vscode.window.activeTextEditor.selection =
                                new vscode.Selection(range.start, range.end);
                        }
                    }
                    break;
                case "change_property":
                    if (vscode.window.activeTextEditor) {
                        const p = data.property as Property;
                        if (p.defined_at && p.defined_at.expression_range) {
                            let range = client.protocol2CodeConverter.asRange(
                                p.defined_at.expression_range,
                            );
                            let old =
                                vscode.window.activeTextEditor.document.getText(
                                    range,
                                );
                            if (old === data.old_value) {
                                vscode.window.activeTextEditor.edit((b) =>
                                    b.replace(range, data.new_value),
                                );
                            }
                        }
                    }
                    break;
            }
        });

        vscode.window.onDidChangeTextEditorSelection(
            async (event: vscode.TextEditorSelectionChangeEvent) => {
                if (event.selections.length === 0 || client === null) {
                    return;
                }
                if (event.textEditor.document.languageId !== "slint") {
                    if (this.property_shown) {
                        webviewView.webview.postMessage({ command: "clear" });
                        this.property_shown = false;
                    }
                    return;
                }
                let selection = event.selections[0];

                query_properties(
                    client,
                    event.textEditor.document.uri,
                    {
                        line: selection.active.line,
                        character: selection.active.character,
                    },
                    (p) => {
                        const msg = {
                            command: "set_properties",
                            properties: p,
                        };
                        webviewView.webview.postMessage(msg);
                    },
                );
                this.property_shown = true;
            },
        );
    }

    private _getHtmlForWebview(webview: vscode.Webview) {
        const scriptUri = webview.asWebviewUri(
            vscode.Uri.joinPath(this._extensionUri, "out/propertiesView.js"),
        );
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
    let text = "";
    const possible =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    for (let i = 0; i < 32; i++) {
        text += possible.charAt(Math.floor(Math.random() * possible.length));
    }
    return text;
}
