// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free OR LicenseRef-Slint-commercial

// This file is the entry point for the vscode extension (not the browser one)

// cSpell: ignore codespace codespaces gnueabihf vsix

import * as path from "path";
import { existsSync } from "fs";
import * as vscode from "vscode";

import { PropertiesViewProvider } from "./properties_webview";
import * as common from "./common";

import {
    LanguageClient,
    ServerOptions,
    ExecutableOptions,
    State,
} from "vscode-languageclient/node";

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
            program_name: "slint-lsp-x86_64-pc-windows-msvc.exe",
        };
    }
    return null;
}

// Please add changes to the BaseLanguageClient via
// `client.add_updater((cl: BaseLanguageClient | null): void)`
//
// That makes sure the code is run even when the LSP gets restarted, etc.
//
// Please add setup common between web and native VSCode by adding updaters
// to the client in common.ts!
function startClient(
    client: common.ClientHandle,
    context: vscode.ExtensionContext,
) {
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

    // Add setup common between native and wasm LSP to common.setup_client_handle!
    client.add_updater((cl) => {
        cl?.onNotification(common.serverStatus, (params: any) =>
            common.setServerStatus(params, statusBar),
        );

        cl?.onDidChangeState((event) => {
            let properly_stopped = cl.hasOwnProperty("slint_stopped");
            if (
                !properly_stopped &&
                event.newState === State.Stopped &&
                event.oldState === State.Running
            ) {
                cl.outputChannel.appendLine(
                    "The Slint Language Server crashed. This is a bug.\nPlease open an issue on https://github.com/slint-ui/slint/issues",
                );
                cl.outputChannel.show();
                vscode.commands.executeCommand("workbench.action.output.focus");
                vscode.window.showErrorMessage(
                    "The Slint Language Server crashed! Please open a bug on the Slint bug tracker with the panic message.",
                );
            }
        });
    });

    const cl = new LanguageClient(
        "slint-lsp",
        "Slint LSP",
        serverOptions,
        common.languageClientOptions(),
    );

    common.prepare_client(cl);

    cl.start().then(() => (client.client = cl));
}

export function activate(context: vscode.ExtensionContext) {
    // Disable native preview in Codespace.
    //
    // We want to have a good default (WASM preview), but we also need to
    // support users that have special setup in place that allows them to run
    // the native previewer remotely.
    if (process.env.hasOwnProperty("CODESPACES")) {
        vscode.workspace
            .getConfiguration("slint")
            .update(
                "preview.providedByEditor",
                true,
                vscode.ConfigurationTarget.Global,
            );
    }

    [statusBar, properties_provider] = common.activate(context, (cl, ctx) =>
        startClient(cl, ctx),
    );
}

export function deactivate(): Thenable<void> | undefined {
    return common.deactivate();
}
