// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// This file contains the common code for both the normal and the browser extension

import { Property, SetBindingResponse } from "slint-editor-shared/properties";
import {
    change_property,
    query_properties,
} from "slint-editor-shared/properties_client";

import * as vscode from "vscode";
import { BaseLanguageClient } from "vscode-languageclient";

let client: BaseLanguageClient | null = null;
export function set_client(c: BaseLanguageClient) {
    client = c;
}

export class PropertiesViewProvider implements vscode.WebviewViewProvider {
    #current_uri = "";
    #current_version = -1;
    #current_cursor_line = -1;
    #current_cursor_character = -1;

    public static readonly viewType = "slint.propertiesView";

    private _view?: vscode.WebviewView;

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

        webviewView.webview.postMessage({
            command: "show_welcome",
            message: "Waiting for Slint LSP",
        });

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
                    change_property(
                        client,
                        data.document,
                        data.element_range,
                        data.property_name,
                        data.new_value,
                        data.dry_run,
                    ).then((response) => {
                        webviewView.webview.postMessage({
                            command: "set_binding_response",
                            response: response,
                        });
                    });
                    break;
            }
        });

        vscode.window.onDidChangeTextEditorSelection(
            async (event: vscode.TextEditorSelectionChangeEvent) => {
                if (event.selections.length === 0) {
                    return;
                }
                const selection = event.selections[0];
                const doc = event.textEditor.document;
                const uri = doc.uri.toString();
                const version = doc.version;

                if (
                    this.#current_uri === uri &&
                    this.#current_version === version &&
                    this.#current_cursor_line === selection.active.line &&
                    this.#current_cursor_character ===
                        selection.active.character
                ) {
                    return;
                }
                this.update_view(
                    doc.languageId,
                    uri,
                    version,
                    selection.active.line,
                    selection.active.character,
                );
            },
        );
        vscode.window.onDidChangeActiveTextEditor(
            async (editor: vscode.TextEditor | undefined) => {
                if (editor === undefined) {
                    this.update_view("No buffer!", "", -1, -1, -1);
                    return;
                }

                const doc = editor.document;
                const selection = editor?.selection;
                const uri = doc.uri.toString();
                const version = doc.version;
                if (
                    this.#current_uri === uri &&
                    this.#current_version === version &&
                    this.#current_cursor_line === selection.active.line &&
                    this.#current_cursor_character ===
                        selection.active.character
                ) {
                    return;
                }
                this.update_view(
                    doc.languageId,
                    uri,
                    version,
                    selection.active.line,
                    selection.active.character,
                );
            },
        );
        // This is triggered on changes to the language!
        vscode.workspace.onDidOpenTextDocument(
            async (doc: vscode.TextDocument) => {
                const uri = doc.uri.toString();
                if (uri === this.#current_uri) {
                    this.update_view(
                        doc.languageId,
                        uri,
                        doc.version,
                        this.#current_cursor_line,
                        this.#current_cursor_character,
                    );
                }
            },
        );
    }

    refresh_view() {
        const editor = vscode.window.activeTextEditor;
        if (editor == null) {
            this.update_view(
                "NO EDITOR",
                this.#current_uri,
                this.#current_version,
                this.#current_cursor_line,
                this.#current_cursor_character,
            );
            return;
        }
        const doc = editor.document;
        if (doc === null) {
            this.update_view(
                "NO DOCUMENT",
                this.#current_uri,
                this.#current_version,
                this.#current_cursor_line,
                this.#current_cursor_character,
            );
            return;
        }

        const selection = editor.selection;
        const line = selection?.active.line ?? this.#current_cursor_line;
        const character =
            selection?.active.character ?? this.#current_cursor_character;

        this.update_view(
            doc.languageId,
            doc.uri.toString(),
            doc.version,
            line,
            character,
        );
    }

    private update_view(
        language: string,
        uri: string,
        version: number,
        line: number,
        character: number,
    ) {
        this.#current_uri = uri;
        this.#current_version = version;
        this.#current_cursor_line = line;
        this.#current_cursor_character = character;

        if (this._view === null) {
            return;
        }
        if (language !== "slint") {
            this._view?.webview.postMessage({
                command: "show_welcome",
                message: "The active editor does not contain a Slint file.",
            });
            // We will get notified about this changing!
            return;
        }
        if (client === null) {
            this._view?.webview.postMessage({
                command: "show_welcome",
                message: "Waiting for Slint LSP",
            });
            // We get triggered for this!
            return;
        }

        // We race the LSP: The Document might not have been loaded yet!
        // So retry once with 2s delay...
        // Ideally we could use the progress messages from the LSP to find out when to retry,
        // but we do not have those yet.
        query_properties(client, uri, { line: line, character: character })
            .then((p: PropertyQuery) => {
                const msg = {
                    command: "set_properties",
                    properties: p,
                };
                this._view?.webview.postMessage(msg);
            })
            .catch(() =>
                setTimeout(
                    () =>
                        query_properties(client, uri, {
                            line: line,
                            character: character,
                        }),
                    2000,
                ),
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

            .properties-editor {
                padding-top: 10px;
            }

            .properties-editor .welcome-page {
                color: var(--vscode-disabledForeground);
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
