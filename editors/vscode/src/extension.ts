// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// This file is the entry point for the vscode extension (not the browser one)

// cSpell: ignore codespace codespaces gnueabihf vsix

import * as path from "node:path";
import { existsSync } from "node:fs";
import * as vscode from "vscode";
import { SlintTelemetrySender } from "./telemetry";
import * as common from "./common";

import {
    LanguageClient,
    type ServerOptions,
    type ExecutableOptions,
    State,
} from "vscode-languageclient/node";
import { newProject } from "./quick_picks.js";

let statusBar: vscode.StatusBarItem;

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
    telemetryLogger: vscode.TelemetryLogger,
) {
    const lsp_platform = lspPlatform();
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

    const serverModule = lspSearchPaths.find((path) => existsSync(path));

    if (serverModule === undefined) {
        console.warn(
            "Could not locate slint-lsp server binary, neither in bundled bin/ directory nor relative in ../target",
        );
        return;
    }

    const options = Object.assign({}, lsp_platform.options);
    options.env = Object.assign({}, process.env, lsp_platform.options?.env);

    const devBuild = serverModule !== lspSearchPaths[lspSearchPaths.length - 1];
    if (devBuild) {
        options.env["RUST_BACKTRACE"] = "1";
    }

    const slint_lsp_panic_file = vscode.Uri.joinPath(
        context.logUri,
        "slint-lsp-panic.log",
    );
    options.env["SLINT_LSP_PANIC_LOG"] = slint_lsp_panic_file.fsPath;

    const args = vscode.workspace
        .getConfiguration("slint")
        .get<[string]>("lsp-args");

    const serverOptions: ServerOptions = {
        run: { command: serverModule, options: options, args: args },
        debug: { command: serverModule, options: options, args: args },
    };

    // Add setup common between native and wasm LSP to common.setup_client_handle!
    client.add_updater((cl) => {
        // Just make sure that the output channel is always present.
        cl?.outputChannel.append("");

        cl?.onDidChangeState((event) => {
            const properly_stopped = cl.hasOwnProperty("slint_stopped");
            if (
                !properly_stopped &&
                event.newState === State.Stopped &&
                event.oldState === State.Running
            ) {
                cl.outputChannel.appendLine(
                    "The Slint Language Server crashed. This is a bug. Please open an issue on https://github.com/slint-ui/slint/issues",
                );
                cl.outputChannel.show();
                vscode.window.showErrorMessage(
                    "The Slint Language Server crashed! Please open a bug on the Slint bug tracker with the panic message.",
                );
                vscode.workspace.fs
                    .readFile(slint_lsp_panic_file)
                    .then((data) => {
                        const contents = Buffer.from(data).toString("utf-8");
                        const lines = contents.split("\n");
                        const version = lines[0];
                        // Location is trusted because it is a path within the LSP (as build on our CI)
                        const location = new vscode.TelemetryTrustedValue(
                            lines[1],
                        );
                        const backtrace = lines[2];
                        const message = lines.slice(3).join("\n");
                        telemetryLogger.logError("lsp-panic", {
                            version: version,
                            location: location,
                            message: message,
                            backtrace: backtrace,
                        });
                        vscode.workspace.fs.delete(slint_lsp_panic_file);
                    });
            }
        });
    });

    const cl = new LanguageClient(
        "slint-lsp",
        "Slint",
        serverOptions,
        common.languageClientOptions(["file"], telemetryLogger),
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

    const telemetryLogger = vscode.env.createTelemetryLogger(
        new SlintTelemetrySender(context.extensionMode),
        {
            ignoreBuiltInCommonProperties: true,
            additionalCommonProperties: {
                common: {
                    machineId: vscode.env.machineId,
                    extname: context.extension.packageJSON.name,
                    extversion: context.extension.packageJSON.version,
                    vscodeversion: vscode.version,
                    platform: process.platform,
                    language: vscode.env.language,
                },
            },
        },
    );

    statusBar = common.activate(context, (cl, ctx) =>
        startClient(cl, ctx, telemetryLogger),
    );

    context.subscriptions.push(
        vscode.commands.registerCommand("slint.newProject", newProject),
    );
}

export function deactivate(): Thenable<void> | undefined {
    return common.deactivate();
}
