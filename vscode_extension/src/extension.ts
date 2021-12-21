// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

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
            program_name: "SixtyFPS Live Preview.app/Contents/MacOS/sixtyfps-lsp"
        };
    }
    else if (process.platform === "linux") {
        let remote_env_options = null;
        if (typeof vscode.env.remoteName !== "undefined") {
            remote_env_options = {
                "DISPLAY": ":0",
                "SIXTYFPS_FULLSCREEN": "1"
            };
        }
        if (process.arch === "x64") {
            return {
                program_name: "sixtyfps-lsp-x86_64-unknown-linux-gnu",
                options: {
                    env: remote_env_options
                }
            };
        } else if (process.arch === "arm") {
            return {
                program_name: "sixtyfps-lsp-armv7-unknown-linux-gnueabihf",
                options: {
                    env: remote_env_options
                }
            };
        } else if (process.arch === "arm64") {
            return {
                program_name: "sixtyfps-lsp-aarch64-unknown-linux-gnu",
                options: {
                    env: remote_env_options
                }
            };
        }
    }
    else if (process.platform === "win32") {
        return {
            program_name: "sixtyfps-lsp-x86_64-pc-windows-gnu.exe"
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
        context.asAbsolutePath(path.join('..', 'target', 'debug', 'sixtyfps-lsp' + program_extension)),
        path.join(context.extensionPath, "bin", "sixtyfps-lsp" + program_extension),
        path.join(context.extensionPath, "bin", lsp_platform.program_name),
    ];

    let serverModule = lspSearchPaths.find(path => existsSync(path));

    if (serverModule === undefined) {
        console.warn("Could not locate sixtyfps-server server binary, neither in bundled bin/ directory nor relative in ../target");
        return;
    }

    let options = Object.assign({}, lsp_platform.options);
    options.env = Object.assign({}, process.env, lsp_platform.options?.env);

    const devBuild = serverModule !== lspSearchPaths[lspSearchPaths.length - 1];
    if (devBuild) {
        options.env["RUST_BACKTRACE"] = "1";
    }

    console.log(`Starting LSP server from ${serverModule}`);

    let args = vscode.workspace.getConfiguration('sixtyfps').get<[string]>('lsp-args');

    let serverOptions: ServerOptions = {
        run: { command: serverModule, options: options, args: args },
        debug: { command: serverModule, options: options, args: args }
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

    client.start();
    let initClient = () => {
        client.onNotification(serverStatus, (params) => setServerStatus(params, statusBar));
    };
    client.onReady().then(initClient);

}

export function activate(context: vscode.ExtensionContext) {

    statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left);
    context.subscriptions.push(statusBar);
    statusBar.text = "SixtyFPS";

    startClient(context);

    context.subscriptions.push(vscode.commands.registerCommand('sixtyfps.showPreview', function () {
        let ae = vscode.window.activeTextEditor;
        if (!ae) {
            return;
        }
        client.sendNotification("sixtyfps/showPreview", ae.document.uri.fsPath.toString());
    }));

    context.subscriptions.push(vscode.commands.registerCommand('sixtyfps.reload', async function () {
        statusBar.hide();
        await client.stop();
        startClient(context);
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
    statusBar.show();
}
