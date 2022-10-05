// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Slint-commercial

import {
  createConnection,
  BrowserMessageReader,
  BrowserMessageWriter,
} from "vscode-languageserver/browser";
import { InitializeParams, InitializeResult } from "vscode-languageserver";

import slint_init, * as slint_lsp from "@lsp/slint_lsp_wasm.js";

slint_init().then((_) => {
  const reader = new BrowserMessageReader(self);
  const writer = new BrowserMessageWriter(self);

  let the_lsp: slint_lsp.SlintServer;

  const connection = createConnection(reader, writer);

  function send_notification(method: string, params: unknown): boolean {
    connection.sendNotification(method, params);
    return true;
  }

  async function load_file(path: string): Promise<string> {
    return await connection.sendRequest("slint/load_file", path);
  }

  connection.onInitialize((params: InitializeParams): InitializeResult => {
    the_lsp = slint_lsp.create(params, send_notification, load_file);
    const capabilities = the_lsp.capabilities();
    capabilities.codeLensProvider = null; // CodeLenses are not relevant for the online editor
    return { capabilities: capabilities };
  });

  connection.onRequest(async (method, params, token) => {
    return await the_lsp.handle_request(token, method, params);
  });

  connection.onDidChangeTextDocument(async (param) => {
    await the_lsp.reload_document(
      param.contentChanges[param.contentChanges.length - 1].text,
      param.textDocument.uri,
    );
  });

  connection.onDidOpenTextDocument(async (param) => {
    await the_lsp.reload_document(
      param.textDocument.text,
      param.textDocument.uri,
    );
  });

  // Listen on the connection
  connection.listen();

  // Now that we listen, the client is ready to send the init message
  self.postMessage("OK");
});
