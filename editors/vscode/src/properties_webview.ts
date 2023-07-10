// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore codicon codicons

// This file contains the properties web view component

import {
    Property,
    PropertyQuery,
} from "../../../tools/slintpad/src/shared/properties";
import * as lsp_commands from "../../../tools/slintpad/src/shared/lsp_commands";

import * as vscode from "vscode";
import { BaseLanguageClient } from "vscode-languageclient";

export class PropertiesViewProvider implements vscode.WebviewViewProvider {
    #current_uri = "";
    #current_version = -1;
    #current_cursor_line = -1;
    #current_cursor_character = -1;
    #client: BaseLanguageClient | null = null;

    public static readonly viewType = "slint.propertiesView";

    private _view?: vscode.WebviewView;

    constructor(private readonly _extensionUri: vscode.Uri) {}

    public set client(c: BaseLanguageClient | null) {
        this.#client = c;
    }

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
                            if (this.#client === null) {
                                return;
                            }

                            let range =
                                this.#client.protocol2CodeConverter.asRange(
                                    p.defined_at.property_definition_range,
                                );
                            vscode.window.activeTextEditor.revealRange(range);
                            vscode.window.activeTextEditor.selection =
                                new vscode.Selection(range.start, range.end);
                        }
                    }
                    break;
                case "change_property":
                    lsp_commands.setBinding(
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
                case "remove_binding":
                    lsp_commands.removeBinding(
                        data.document,
                        data.element_range,
                        data.property_name,
                    ).catch((_) => {
                        // catch this to avoid errors showing up in the console
                        return;
                    });

                    break;
            }
        });

        vscode.window.onDidChangeTextEditorSelection(
            (event: vscode.TextEditorSelectionChangeEvent) => {
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
            (editor: vscode.TextEditor | undefined) => {
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
        vscode.workspace.onDidOpenTextDocument((doc: vscode.TextDocument) => {
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
        });
    }

    refresh_view() {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
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
        if (!doc) {
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

        if (!this._view) {
            return;
        }
        if (language !== "slint" && language !== "rust") {
            this._view?.webview.postMessage({
                command: "show_welcome",
                message: "The active editor does not contain a Slint file.",
            });
            // We will get notified about this changing!
            return;
        }
        if (this.#client === null) {
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
        lsp_commands.queryProperties(uri, {
            line: line,
            character: character,
        })
            .then((p: PropertyQuery) => {
                const msg = {
                    command: "set_properties",
                    properties: p,
                };
                this._view?.webview.postMessage(msg);
            })
            .catch((_) => {
                return;
            });
    }

    private _getHtmlForWebview(webview: vscode.Webview) {
        const scriptUri = webview.asWebviewUri(
            vscode.Uri.joinPath(this._extensionUri, "out/propertiesView.js"),
        );
        const styleUri = webview.asWebviewUri(
            vscode.Uri.joinPath(this._extensionUri, "css", "content.css"),
        );
        const codiconsUri = webview.asWebviewUri(
            vscode.Uri.joinPath(
                this._extensionUri,
                "node_modules",
                "@vscode/codicons",
                "dist",
                "codicon.css",
            ),
        );

        return `<!DOCTYPE html>
			<html lang="en">
			<head>
				<meta charset="UTF-8">
                <meta http-equiv="Content-Security-Policy" content="default-src 'none'; font-src ${webview.cspSource}; style-src ${webview.cspSource}; script-src ${webview.cspSource}">
				<meta name="viewport" content="width=device-width, initial-scale=1.0">
                <title>Slint preview</title>
                <link href="${styleUri}" rel="stylesheet" />
				<link href="${codiconsUri}" rel="stylesheet" />
			</head>
			<body class="properties-editor">
                <script src="${scriptUri}"></script>
			</body>
			</html>`;
    }
}
