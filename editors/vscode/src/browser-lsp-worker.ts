// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint_init, * as slint_lsp from "../out/slint_lsp_wasm.js";
//@ts-ignore
import slint_wasm_data from "../out/slint_lsp_wasm_bg.wasm";
import type { InitializeParams, InitializeResult } from "vscode-languageserver";
import {
    createConnection,
    BrowserMessageReader,
    BrowserMessageWriter,
} from "vscode-languageserver/browser";

slint_init(slint_wasm_data).then((_) => {
    const reader = new BrowserMessageReader(self);
    const writer = new BrowserMessageWriter(self);

    let the_lsp: slint_lsp.SlintServer;

    const connection = createConnection(reader, writer);

    function send_notification(method: string, params: any): boolean {
        connection.sendNotification(method, params);
        return true;
    }

    async function send_request(method: string, params: any): Promise<unknown> {
        return await connection.sendRequest(method, params);
    }

    async function load_file(path: string): Promise<string> {
        return await connection.sendRequest("slint/load_file", path);
    }

    connection.onInitialize((params: InitializeParams): InitializeResult => {
        the_lsp = slint_lsp.create(
            params,
            send_notification,
            send_request,
            load_file,
        );

        setTimeout(() => {
            the_lsp.startup_lsp();
        }, 0);

        return the_lsp.server_initialize_result(params.capabilities);
    });

    connection.onRequest(async (method, params, token) => {
        return await the_lsp.handle_request(token, method, params);
    });

    connection.onNotification(
        "slint/preview_to_lsp",
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        async (params: any) => {
            await the_lsp.process_preview_to_lsp_message(params);
        },
    );
    connection.onDidChangeWatchedFiles(async (param) => {
        for (const event of param.changes) {
            await the_lsp.trigger_file_watcher(event.uri, event.type);
        }
    });

    connection.onDidOpenTextDocument(async (param) => {
        await the_lsp.open_document(
            param.textDocument.text,
            param.textDocument.uri,
            param.textDocument.version,
        );
    });

    connection.onDidChangeTextDocument(async (param) => {
        await the_lsp.reload_document(
            param.contentChanges[param.contentChanges.length - 1].text,
            param.textDocument.uri,
            param.textDocument.version,
        );
    });

    connection.onDidCloseTextDocument(async (param) => {
        await the_lsp.close_document(param.textDocument.uri);
    });

    connection.onDidChangeConfiguration(async (_param: unknown) => {
        await the_lsp.reload_config();
    });

    // Listen on the connection
    connection.listen();

    // Now that we listen, the client is ready to send the init message
    self.postMessage("OK");
});
