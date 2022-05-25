// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import { createConnection, BrowserMessageReader, BrowserMessageWriter } from 'vscode-languageserver/browser';

import { Color, ColorInformation, Range, InitializeParams, InitializeResult, ServerCapabilities, TextDocuments, ColorPresentation, TextEdit, TextDocumentIdentifier } from 'vscode-languageserver';
import { TextDocument } from 'vscode-languageserver-textdocument';

import { default as slint_lsp_init } from "../../../tools/lsp/pkg/index.js";
import slint_wasm_data from "../../../tools/lsp/pkg/index_bg.wasm";

slint_lsp_init(slint_wasm_data).then((slint_lsp) => {

    console.log('Hello from the worker', slint_lsp);


    const messageReader = new BrowserMessageReader(self);
    const messageWriter = new BrowserMessageWriter(self);

    const connection = createConnection(messageReader, messageWriter);

    let the_lsp: slint_lsp.SlintServer;

    connection.onInitialize((params: InitializeParams): InitializeResult => {
        the_lsp = slint_lsp.create(params);
        return { capabilities: the_lsp.capabilities() };
    });

    connection.onRequest((method, params, token) => {
        //the_lsp.handle_request(token, method, params);
    });

    connection.onDidChangeTextDocument((param) => {
        //the_lsp.reload_document(param.contentChanges[param.contentChanges.length - 1].text, param.textDocument.uri);
    });

    connection.onDidOpenTextDocument((param) => {
        //the_lsp.reload_document(param.textDocument.text, param.textDocument.uri);
    });

    // Listen on the connection
    connection.listen();

})

