/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

import * as path from 'path';
import { existsSync } from 'fs';
import * as vscode from 'vscode';

import {
    LanguageClient,
    LanguageClientOptions,
    ServerCapabilities,
    ServerOptions,
    NotificationType,
    //	TransportKind
} from 'vscode-languageclient/node';


export interface ServerStatusParams {
    health: "ok" | "warning" | "error";
    quiescent: boolean;
    message?: string;
}
export const serverStatus = new NotificationType<ServerStatusParams>("experimental/serverStatus");


let client: LanguageClient;

const program_extension = process.platform === "win32" ? ".exe" : "";

function lspPlatform(): string | null {
    if (process.platform === "darwin") {
        if (process.arch === "x64") {
            return "x86_64-apple-darwin";
        } else if (process.arch == "arm64") {
            return "aarch64-apple-darwin";
        }
    }
    else if (process.platform === "linux") {
        if (process.arch === "x64") {
            return "x86_64-unknown-linux-gnu";
        }
    }
    else if (process.platform === "win32") {
        return "x86_64-pc-windows-gnu";
    }
    return null;
}

export function activate(context: vscode.ExtensionContext) {

    /*let test_output = vscode.window.createOutputChannel("Test Output");
    test_output.appendLine("Hello from extension");*/

    let lsp_platform = lspPlatform();
    if (lsp_platform === null) {
        return;
    }

    // Try a local ../target build first, then try the plain bundled binary and finally the architecture specific one.
    // A debug session will find the first one, a local package build the second and the distributed vsix the last.
    const lspSearchPaths = [
        context.asAbsolutePath(path.join('..', 'target', 'debug', 'sixtyfps-lsp' + program_extension)),
        path.join(context.extensionPath, "bin", "sixtyfps-lsp" + program_extension),
        path.join(context.extensionPath, "bin", "sixtyfps-lsp-" + lsp_platform + program_extension),
    ];

    let serverModule = lspSearchPaths.find(path => existsSync(path));

    if (serverModule === undefined) {
        console.warn("Could not locate sixtyfps-server server binary, neither in bundled bin/ directory nor relative in ../target");
        return;
    }

    console.log(`Starting LSP server from ${serverModule}`);

    let serverOptions: ServerOptions = {
        run: { command: serverModule },
        debug: { command: serverModule }
    };

    let clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'sixtyfps' }],
    };

    client = new LanguageClient(
        'sixtyfps-lsp',
        'SixtyFPS LSP',
        serverOptions,
        clientOptions
    );


    const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left);
    context.subscriptions.push(statusBar);
    statusBar.text = "SixtyFPS";

    client.start();
    let initClient = () => {
        client.onNotification(serverStatus, (params) => setServerStatus(params, statusBar));
    };
    client.onReady().then(initClient);

    context.subscriptions.push(vscode.commands.registerCommand('sixtyfps.showPreview', function () {
        let ae = vscode.window.activeTextEditor;
        if (!ae) {
            return;
        }
        statusBar.show();
        client.sendNotification("sixtyfps/showPreview", ae.document.uri.fsPath.toString());

    }));

    context.subscriptions.push(vscode.commands.registerCommand('sixtyfps.reload', async function () {
        statusBar.hide();
        await client.stop();
        client.start();
        await client.onReady();
        initClient();
    }));
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
    statusBar.tooltip = "SixtyFPS";
    statusBar.text = `${icon} ${status.message ?? "SixtyFPS"}`;
}
