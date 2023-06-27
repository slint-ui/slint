// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import {
    createConnection,
    BrowserMessageReader,
    BrowserMessageWriter,
} from "vscode-languageserver/browser";

import {
    ExecuteCommandParams,
    InitializeParams,
    InitializeResult,
} from "vscode-languageserver";

import slint_init, * as slint_lsp from "../../../tools/lsp/pkg/index.js";
import slint_wasm_data from "../../../tools/lsp/pkg/index_bg.wasm";

slint_init(slint_wasm_data).then((_) => {
    const messageReader = new BrowserMessageReader(self);
    const messageWriter = new BrowserMessageWriter(self);
    const connection = createConnection(messageReader, messageWriter);

    let the_lsp: slint_lsp.SlintServer;

    function send_notification(method: string, params: any): boolean {
        connection.sendNotification(method, params);
        return true;
    }

    async function load_file(path: string): Promise<string> {
        const contents: Uint8Array = await connection.sendRequest(
            "slint/load_file",
            path,
        );
        return new TextDecoder().decode(contents);
    }

    async function send_request(method: string, params: any): Promise<any> {
        return await connection.sendRequest(method, params);
    }

    function highlight(path: string, offset: number) {
        connection.sendRequest("slint/preview_message", {
            command: "highlight",
            data: { path: path, offset: offset },
        });
    }

    connection.onInitialize((params: InitializeParams): InitializeResult => {
        the_lsp = slint_lsp.create(
            params,
            send_notification,
            send_request,
            load_file,
            highlight,
        );
        return the_lsp.server_initialize_result(params.capabilities);
    });

    connection.onRequest(async (method, params, token) => {
        return await the_lsp.handle_request(token, method, params);
    });

    connection.onDidChangeConfiguration(async (_) => {
        the_lsp.reload_config();
    });

    connection.onDidChangeTextDocument(async (param) => {
        await the_lsp.reload_document(
            param.contentChanges[param.contentChanges.length - 1].text,
            param.textDocument.uri,
            param.textDocument.version,
        );
    });

    connection.onDidOpenTextDocument(async (param) => {
        await the_lsp.reload_document(
            param.textDocument.text,
            param.textDocument.uri,
            param.textDocument.version,
        );
    });

    // Listen on the connection
    connection.listen();

    // Now that we listen, the client is ready to send the init message
    self.postMessage("OK");
});
