// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

// cSpell: ignore codespaces

// This file is common code shared by both vscode plugin entry points

import * as vscode from "vscode";

import { PropertiesViewProvider } from "./properties_webview";
import * as wasm_preview from "./wasm_preview";
import * as lsp_commands from "../../../tools/slintpad/src/shared/lsp_commands";
import * as snippets from "./snippets";

import {
    BaseLanguageClient,
    LanguageClientOptions,
    NotificationType,
} from "vscode-languageclient";

// server status:
export interface ServerStatusParams {
    health: "ok" | "warning" | "error";
    quiescent: boolean;
    message?: string;
}

export const serverStatus = new NotificationType<ServerStatusParams>(
    "experimental/serverStatus",
);

export class ClientHandle {
    #client: BaseLanguageClient | null = null;
    #updaters: ((c: BaseLanguageClient | null) => void)[] = [];

    constructor() {}

    get client(): BaseLanguageClient | null {
        return this.#client;
    }

    set client(c: BaseLanguageClient | null) {
        this.#client = c;
        for (let u of this.#updaters) {
            u(this.#client);
        }
    }

    public add_updater(u: (c: BaseLanguageClient | null) => void) {
        u(this.#client);
        this.#updaters.push(u);
    }

    async stop() {
        let to_stop = this.client;
        this.client = null;
        for (let u of this.#updaters) {
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

export function setServerStatus(
    status: ServerStatusParams,
    statusBar: vscode.StatusBarItem,
) {
    let icon = "";
    switch (status.health) {
        case "ok":
            statusBar.color = undefined;
            break;
        case "warning":
            statusBar.color = new vscode.ThemeColor(
                "notificationsWarningIcon.foreground",
            );
            icon = "$(warning) ";
            break;
        case "error":
            statusBar.color = new vscode.ThemeColor(
                "notificationsErrorIcon.foreground",
            );
            icon = "$(error) ";
            break;
    }
    statusBar.tooltip = "Slint";
    statusBar.text = `${icon} ${status.message ?? "Slint"}`;
    statusBar.show();
}

// LSP related:

// Set up our middleware. It is used to redirect/forward to the WASM preview
// as needed and makes the triggering side so much simpler!

export function languageClientOptions(): LanguageClientOptions {
    return {
        documentSelector: [{ language: "slint" }, { language: "rust" }],
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
): [vscode.StatusBarItem, PropertiesViewProvider] {
    const statusBar = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Left,
    );

    const properties_provider = new PropertiesViewProvider(
        context.extensionUri,
    );

    context.subscriptions.push(statusBar);
    statusBar.text = "Slint";

    client.add_updater((cl) => {
        if (cl !== null) {
            cl.onNotification(serverStatus, (params: ServerStatusParams) =>
                setServerStatus(params, statusBar),
            );
        }
        wasm_preview.initClientForPreview(context, cl);

        properties_provider.refresh_view();
    });

    vscode.workspace.onDidChangeConfiguration(async (ev) => {
        if (ev.affectsConfiguration("slint")) {
            client.client?.sendNotification(
                "workspace/didChangeConfiguration",
                { settings: "" },
            );
            wasm_preview.update_configuration();
        }
    });

    startClient(client, context);

    client.add_updater((c) => {
        properties_provider.client = c;
    });

    context.subscriptions.push(
        vscode.commands.registerCommand("slint.showPreview", async function () {
            let ae = vscode.window.activeTextEditor;
            if (!ae) {
                return;
            }

            lsp_commands.showPreview(ae.document.uri.toString(), "");
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

    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(
            PropertiesViewProvider.viewType,
            properties_provider,
        ),
    );
    properties_provider.refresh_view();

    vscode.workspace.onDidChangeTextDocument(async (ev) => {
        if (
            ev.document.languageId !== "slint" &&
            ev.document.languageId !== "rust"
        ) {
            return;
        }

        // Send a request for properties information after passing through the
        // event loop once to make sure the LSP got signaled to update.
        setTimeout(() => {
            properties_provider.refresh_view();
        }, 1);
    });

    vscode.workspace.onDidChangeConfiguration(async (ev) => {
        if (ev.affectsConfiguration("slint")) {
            client.client?.sendNotification(
                "workspace/didChangeConfiguration",
                { settings: "" },
            );
        }
    });

    return [statusBar, properties_provider];
}

export function deactivate(): Thenable<void> | undefined {
    if (!client.client) {
        return undefined;
    }
    return client.stop();
}
