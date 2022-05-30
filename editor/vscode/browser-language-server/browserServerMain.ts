// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import { createConnection, BrowserMessageReader, BrowserMessageWriter } from 'vscode-languageserver/browser';

import { InitializeParams, InitializeResult } from 'vscode-languageserver';

import slint_init, * as slint_lsp from "../../../tools/lsp/pkg/index.js";
import slint_wasm_data from "../../../tools/lsp/pkg/index_bg.wasm";

const messageReader = new BrowserMessageReader(self);
const messageWriter = new BrowserMessageWriter(self);

const connection = createConnection(messageReader, messageWriter);

slint_init(slint_wasm_data).then((_) => {

    let the_lsp: slint_lsp.SlintServer;

    connection.onInitialize((params: InitializeParams): InitializeResult => {
        the_lsp = slint_lsp.create(params);
        return { capabilities: the_lsp.capabilities() };
    });

    connection.onRequest((method, params, token) => {
        console.log("request", method);
        let x = the_lsp.handle_request(token, method, params);
        console.log("REPLY", x);
        return x;
    });

    connection.onDidChangeTextDocument((param) => {
        the_lsp.reload_document(param.contentChanges[param.contentChanges.length - 1].text, param.textDocument.uri);
    });

    connection.onDidOpenTextDocument((param) => {
        the_lsp.reload_document(param.textDocument.text, param.textDocument.uri);
    });

    // Listen on the connection
    connection.listen();

    self.postMessage("OK");
});

export function send_notification(method: string, params: any): bool {
    connection.sendNotification(method, params);
    return true;
}


