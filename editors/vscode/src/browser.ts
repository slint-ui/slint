// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// This file is the entry point for the vscode web extension

import { Uri } from "vscode";
import * as vscode from "vscode";
import {
    BaseLanguageClient,
    LanguageClient,
} from "vscode-languageclient/browser";

import * as wasm_preview from "./wasm_preview";
import * as common from "./common";
import { SlintTelemetrySender } from "./telemetry";

let statusBar: vscode.StatusBarItem;

function startClient(
    client: common.ClientHandle,
    context: vscode.ExtensionContext,
    telemetryLogger: vscode.TelemetryLogger,
) {
    //let args = vscode.workspace.getConfiguration('slint').get<[string]>('lsp-args');

    // Options to control the language client
    // Note: This works with way more schemes than the native LSP as it goes
    // through VSCode to open files by necessity.
    // https://github.com/microsoft/vscode/blob/main/src/vs/base/common/network.ts
    // lists all the known schemes in VSCode (without extensions). I err on the
    // side of allowing too much here I think...
    const clientOptions = common.languageClientOptions(
        [
            "file",
            "http",
            "https",
            "inmemory",
            "vscode-file",
            "vscode-remote",
            "vscode-remote-resource",
            "vscode-vfs", // github.dev uses this
            "vsls",
        ],
        telemetryLogger,
    );
    clientOptions.synchronize = {};
    clientOptions.initializationOptions = {};

    const serverMain = Uri.joinPath(
        context.extensionUri,
        "out/browserServerMain.js",
    );

    const worker = new Worker(serverMain.toString(true));
    worker.onmessage = (m) => {
        // We cannot start sending messages to the client before we start listening which
        // the server only does in a future after the wasm is loaded.
        if (m.data === "OK") {
            const cl = new LanguageClient(
                "slint-lsp",
                "Slint",
                clientOptions,
                worker,
            );

            common.prepare_client(cl);

            client.add_updater((cl) => {
                cl?.onRequest("slint/load_file", async (param: string) => {
                    const contents = await vscode.workspace.fs.readFile(
                        Uri.parse(param, true),
                    );
                    return new TextDecoder().decode(contents);
                });
            });

            cl.start().then(() => (client.client = cl));
        }
    };
}

// this method is called when vs code is activated
export function activate(context: vscode.ExtensionContext) {
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
                    platform: "web",
                    language: vscode.env.language,
                },
            },
        },
    );

    statusBar = common.activate(context, (cl, ctx) =>
        startClient(cl, ctx, telemetryLogger),
    );
}

export function deactivate(): Thenable<void> | undefined {
    return common.deactivate();
}
