// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// This file is common code shared by both vscode plugin entry points

import * as vscode from "vscode";

import { PropertiesViewProvider } from "./properties_webview";
import * as wasm_preview from "./wasm_preview";
import * as lsp_commands from "../../../tools/slintpad/src/shared/lsp_commands";
import * as welcome from "./welcome/welcomepanel";

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
            u(c);
        }
    }

    public add_updater(u: (c: BaseLanguageClient | null) => void) {
        u(this.#client);
        this.#updaters.push(u);
    }

    async stop() {
        if (this.#client) {
            // mark as stopped so that we don't detect it as a crash
            Object.defineProperty(this.#client, "slint_stopped", {
                value: true,
            });
            await this.#client.stop();
        }
    }
}

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

export function languageClientOptions(
    showPreview: (args: any) => boolean,
    toggleDesignMode: (args: any) => boolean,
): LanguageClientOptions {
    return {
        documentSelector: [{ language: "slint" }, { language: "rust" }],
        middleware: {
            executeCommand(command: string, args: any, next: any) {
                if (command === "slint/showPreview") {
                    if (showPreview(args)) {
                        return;
                    }
                } else if (command == "slint/toggleDesignMode") {
                    if (toggleDesignMode(args)) {
                        return;
                    }
                }
                return next(command, args);
            },
        },
    };
}

// VSCode Plugin lifecycle related:

export function activate(
    context: vscode.ExtensionContext,
    client: ClientHandle,
    startClient: (_ctx: vscode.ExtensionContext) => void,
): [vscode.StatusBarItem, PropertiesViewProvider] {
    const statusBar = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Left,
    );
    context.subscriptions.push(statusBar);
    statusBar.text = "Slint";

    startClient(context);

    const properties_provider = new PropertiesViewProvider(
        context.extensionUri,
    );

    client.add_updater((c) => {
        properties_provider.client = c;
    });

    context.subscriptions.push(
        vscode.commands.registerCommand("slint.showPreview", function () {
            let ae = vscode.window.activeTextEditor;
            if (!ae) {
                return;
            }

            lsp_commands.showPreview(ae.document.uri.toString(), "");
        }),
    );
    context.subscriptions.push(
        vscode.commands.registerCommand("slint.showWelcome", function () {
            welcome.WelcomePanel.createOrShow(context.extensionPath);
        }),
    );
    context.subscriptions.push(
        vscode.commands.registerCommand("slint.toggleDesignMode", function () {
            lsp_commands.toggleDesignMode();
        }),
    );

    context.subscriptions.push(
        vscode.commands.registerCommand("slint.reload", async function () {
            statusBar.hide();
            await client.stop();
            startClient(context);
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

    vscode.workspace.onDidChangeConfiguration(async (ev) => {
        if (ev.affectsConfiguration("slint")) {
            properties_provider.client?.sendNotification(
                "workspace/didChangeConfiguration",
                {
                    settings: "",
                },
            );
            wasm_preview.refreshPreview();
            welcome.WelcomePanel.updateShowConfig();
        }
    });

    vscode.workspace.onDidChangeTextDocument(async (ev) => {
        if (
            ev.document.languageId !== "slint" &&
            ev.document.languageId !== "rust"
        ) {
            return;
        }
        wasm_preview.refreshPreview(ev);

        // Send a request for properties information after passing through the
        // event loop once to make sure the LSP got signaled to update.
        setTimeout(() => {
            properties_provider.refresh_view();
        }, 1);
    });

    setTimeout(() => welcome.WelcomePanel.maybeShow(context.extensionPath), 1);

    return [statusBar, properties_provider];
}

export function deactivate(client: ClientHandle): Thenable<void> | undefined {
    if (!client.client) {
        return undefined;
    }
    return client.stop();
}
