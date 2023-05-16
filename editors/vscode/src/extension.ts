// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

// This file is the entry point for the vscode extension (not the browser one)

// cSpell: ignore aarch armv gnueabihf vsix

import * as path from "path";
import { existsSync } from "fs";
import * as vscode from "vscode";

import { PropertiesViewProvider } from "./properties_webview";
import * as wasm_preview from "./wasm_preview";
import * as common from "./common";

import {
    LanguageClient,
    ServerOptions,
    ExecutableOptions,
    State,
} from "vscode-languageclient/node";

let client = new common.ClientHandle();
let statusBar: vscode.StatusBarItem;
let properties_provider: PropertiesViewProvider;

const program_extension = process.platform === "win32" ? ".exe" : "";

interface Platform {
    program_name: string;
    options?: ExecutableOptions;
}

function lspPlatform(): Platform | null {
    if (process.platform === "darwin") {
        return {
            program_name: "Slint Live Preview.app/Contents/MacOS/slint-lsp",
        };
    } else if (process.platform === "linux") {
        let remote_env_options = null;
        if (typeof vscode.env.remoteName !== "undefined") {
            remote_env_options = {
                DISPLAY: ":0",
                SLINT_FULLSCREEN: "1",
            };
        }
        if (process.arch === "x64") {
            return {
                program_name: "slint-lsp-x86_64-unknown-linux-gnu",
                options: {
                    env: remote_env_options,
                },
            };
        } else if (process.arch === "arm") {
            return {
                program_name: "slint-lsp-armv7-unknown-linux-gnueabihf",
                options: {
                    env: remote_env_options,
                },
            };
        } else if (process.arch === "arm64") {
            return {
                program_name: "slint-lsp-aarch64-unknown-linux-gnu",
                options: {
                    env: remote_env_options,
                },
            };
        }
    } else if (process.platform === "win32") {
        return {
            program_name: "slint-lsp-x86_64-pc-windows-gnu.exe",
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
        path.join(
            context.extensionPath,
            "..",
            "..",
            "target",
            "debug",
            "slint-lsp" + program_extension,
        ),
        path.join(
            context.extensionPath,
            "..",
            "..",
            "target",
            "release",
            "slint-lsp" + program_extension,
        ),
        path.join(
            context.extensionPath,
            "bin",
            "slint-lsp" + program_extension,
        ),
        path.join(context.extensionPath, "bin", lsp_platform.program_name),
    ];

    let serverModule = lspSearchPaths.find((path) => existsSync(path));

    if (serverModule === undefined) {
        console.warn(
            "Could not locate slint-lsp server binary, neither in bundled bin/ directory nor relative in ../target",
        );
        return;
    }

    let options = Object.assign({}, lsp_platform.options);
    options.env = Object.assign({}, process.env, lsp_platform.options?.env);

    const devBuild = serverModule !== lspSearchPaths[lspSearchPaths.length - 1];
    if (devBuild) {
        options.env["RUST_BACKTRACE"] = "1";
    }

    let args = vscode.workspace
        .getConfiguration("slint")
        .get<[string]>("lsp-args");

    let serverOptions: ServerOptions = {
        run: { command: serverModule, options: options, args: args },
        debug: { command: serverModule, options: options, args: args },
    };

    const clientOptions = common.languageClientOptions(
        (args: any) => {
            if (
                vscode.workspace
                    .getConfiguration("slint")
                    .get<boolean>("preview.providedByEditor")
            ) {
                wasm_preview.showPreview(
                    context,
                    vscode.Uri.parse(args[0], true),
                    args[1],
                );
                return true;
            }
            return false;
        },
        (_) => {
            if (
                vscode.workspace
                    .getConfiguration("slint")
                    .get<boolean>("preview.providedByEditor")
            ) {
                wasm_preview.toggleDesignMode();
                return true;
            }
            return false;
        },
    );

    const cl = new LanguageClient(
        "slint-lsp",
        "Slint LSP",
        serverOptions,
        clientOptions,
    );

    cl.onDidChangeState((event) => {
        let properly_stopped = cl.hasOwnProperty("slint_stopped");
        if (!properly_stopped && event.newState === State.Stopped && event.oldState == State.Running) {
            cl.outputChannel.appendLine("The Slint Language Server crashed. This is a bug.\nPlease open an issue on https://github.com/slint-ui/slint/issues");
            cl.outputChannel.show();
            vscode.commands.executeCommand('workbench.action.output.focus');
            vscode.window.showErrorMessage("The Slint Language Server crashed! Please open a bug on the Slint bug tracker with the panic message.");
        }
    });

    cl.start();
    client.client = cl;

    let initClient = () => {
        wasm_preview.initClientForPreview(cl);

        properties_provider.refresh_view();
        cl.onNotification(common.serverStatus, (params: any) =>
            common.setServerStatus(params, statusBar),
        );
    };
    cl.onReady().then(initClient);
}

export function activate(context: vscode.ExtensionContext) {
    [statusBar, properties_provider] = common.activate(context, client, (ctx) =>
        startClient(ctx),
    );
}

export function deactivate(): Thenable<void> | undefined {
    return common.deactivate(client);
}
