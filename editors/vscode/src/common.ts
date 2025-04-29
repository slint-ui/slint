// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore codespaces

// This file is common code shared by both vscode plugin entry points

import * as vscode from "vscode";

import * as wasm_preview from "./wasm_preview";
import * as lsp_commands from "./lsp_commands";
import * as snippets from "./snippets";

import {
    type BaseLanguageClient,
    type LanguageClientOptions,
    NotificationType,
} from "vscode-languageclient";

export class ClientHandle {
    #client: BaseLanguageClient | null = null;
    #updaters: ((c: BaseLanguageClient | null) => void)[] = [];

    get client(): BaseLanguageClient | null {
        return this.#client;
    }

    set client(c: BaseLanguageClient | null) {
        this.#client = c;
        for (const u of this.#updaters) {
            u(this.#client);
        }
    }

    public add_updater(u: (c: BaseLanguageClient | null) => void) {
        u(this.#client);
        this.#updaters.push(u);
    }

    async stop() {
        const to_stop = this.client;
        this.client = null;
        for (const u of this.#updaters) {
            u(this.#client);
        }

        if (to_stop) {
            // mark as stopped so that we don't detect it as a crash
            Object.defineProperty(to_stop, "slint_stopped", {
                value: true,
            });
            await to_stop.stop();
        }
    }
}

const client = new ClientHandle();

// LSP related:

// Set up our middleware. It is used to redirect/forward to the WASM preview
// as needed and makes the triggering side so much simpler!

export function languageClientOptions(
    schemes: string[],
    telemetryLogger: vscode.TelemetryLogger,
): LanguageClientOptions {
    var document_selector = [];
    for (var scheme of schemes) {
        document_selector.push({ scheme: scheme, language: "slint" });
        document_selector.push({ scheme: scheme, language: "rust" });
    }

    return {
        documentSelector: document_selector,
        middleware: {
            async provideCodeActions(
                document: vscode.TextDocument,
                range: vscode.Range,
                context: vscode.CodeActionContext,
                token: vscode.CancellationToken,
                next: any,
            ) {
                const actions = await next(document, range, context, token);
                if (actions) {
                    snippets.detectSnippetCodeActions(actions);
                }
                return actions;
            },
            window: {
                async showDocument(params, next: any) {
                    const cl = client.client;
                    if (!params.external && cl) {
                        // If the preview panel is open, the default behavior would be to open a document on the same column.
                        // But we want to open the document next to it instead.
                        const panel = wasm_preview.panel();
                        if (panel && panel.active) {
                            const uri = cl.protocol2CodeConverter.asUri(
                                params.uri,
                            );
                            const col = panel.viewColumn || 1;
                            const options: vscode.TextDocumentShowOptions = {
                                viewColumn: col > 1 ? col - 1 : col + 1,
                                preserveFocus: !params.takeFocus,
                            };
                            if (params.selection !== undefined) {
                                options.selection =
                                    cl.protocol2CodeConverter.asRange(
                                        params.selection,
                                    );
                            }
                            await vscode.window.showTextDocument(uri, options);

                            return { success: true };
                        }
                    }
                    return await next(params);
                },
            },
            async provideCodeLenses(document, token, next) {
                const lenses = await next(document, token);
                if (lenses && lenses.length > 0) {
                    maybeSendStartupTelemetryEvent(telemetryLogger);
                }
                return lenses;
            },
        },
    };
}

// Setup code to be run *before* the client is started.
// Use the ClientHandle for code that runs after the client is started.

export function prepare_client(client: BaseLanguageClient) {
    client.registerFeature(new snippets.SnippetTextEditFeature());
}

// VSCode Plugin lifecycle related:

export function activate(
    context: vscode.ExtensionContext,
    startClient: (_client: ClientHandle, _ctx: vscode.ExtensionContext) => void,
): vscode.StatusBarItem {
    const statusBar = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Left,
    );

    context.subscriptions.push(statusBar);
    statusBar.text = "Slint";

    client.add_updater((cl) => {
        wasm_preview.initClientForPreview(context, cl);
    });

    vscode.workspace.onDidChangeConfiguration(async (ev) => {
        if (ev.affectsConfiguration("slint")) {
            await client.client?.sendNotification(
                "workspace/didChangeConfiguration",
                { settings: "" },
            );
            wasm_preview.update_configuration();
        }
    });

    startClient(client, context);

    context.subscriptions.push(
        vscode.commands.registerCommand("slint.showPreview", async function () {
            const ae = vscode.window.activeTextEditor;
            if (!ae) {
                return;
            }

            await lsp_commands.showPreview(ae.document.uri.toString(), "");
        }),
    );

    context.subscriptions.push(
        vscode.commands.registerCommand("slint.reload", async function () {
            statusBar.hide();
            await client.stop();
            startClient(client, context);
        }),
    );

    vscode.window.registerWebviewPanelSerializer(
        "slint-preview",
        new wasm_preview.PreviewSerializer(context),
    );

    vscode.workspace.onDidChangeConfiguration(async (ev) => {
        if (ev.affectsConfiguration("slint")) {
            await client.client?.sendNotification(
                "workspace/didChangeConfiguration",
                { settings: "" },
            );
        }
    });

    return statusBar;
}

export function deactivate(): Thenable<void> | undefined {
    if (!client.client) {
        return undefined;
    }
    return client.stop();
}

let telemetryEventSent = false;
async function maybeSendStartupTelemetryEvent(
    telemetryLogger: vscode.TelemetryLogger,
) {
    if (telemetryEventSent) {
        return;
    }
    telemetryEventSent = true;

    let usageData = {};

    enum ProgrammingLanguage {
        Rust = "Rust",
        Cpp = "Cpp",
        JavaScript = "JavaScript",
        Python = "Python",
    }

    const projectLanguages = new Set<ProgrammingLanguage>();

    if (vscode.workspace.workspaceFolders) {
        const workspaceFolderContents = await Promise.all(
            vscode.workspace.workspaceFolders.map((workspaceFolder) => {
                return vscode.workspace.fs.readDirectory(workspaceFolder.uri);
            }),
        );

        for (const path of workspaceFolderContents.flatMap((fileEntries) =>
            fileEntries.map((fileEntry) => fileEntry[0].toLowerCase()),
        )) {
            if (path.endsWith("cargo.toml")) {
                projectLanguages.add(ProgrammingLanguage.Rust);
            } else if (path.endsWith("cmakelists.txt")) {
                projectLanguages.add(ProgrammingLanguage.Cpp);
            } else if (path.endsWith("package.json")) {
                projectLanguages.add(ProgrammingLanguage.JavaScript);
            } else if (
                path.endsWith("pyproject.toml") ||
                path.endsWith("requirements.txt")
            ) {
                projectLanguages.add(ProgrammingLanguage.Python);
            }
        }
    }

    if (projectLanguages.size > 0) {
        usageData = Object.assign(usageData, {
            projectLanguages: Array.from(projectLanguages.values()),
        });
    }

    telemetryLogger.logUsage("extension-activated", usageData);
}
