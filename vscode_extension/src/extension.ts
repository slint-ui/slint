/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

import * as path from 'path';
import { workspace, ExtensionContext } from 'vscode';

import {
	LanguageClient,
	LanguageClientOptions,
	ServerCapabilities,
	ServerOptions,
	//	TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {

	let serverModule = context.asAbsolutePath(path.join('..', 'target', 'debug', 'sixtyfps-lsp'));
	console.log(serverModule);

	/*let test_output = vscode.window.createOutputChannel("Test Output");
	test_output.appendLine("Hello from extension");*/

	let serverOptions: ServerOptions = {
		run: { command: serverModule },
		debug: { command: serverModule }
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
}

export function deactivate(): Thenable<void> | undefined {
	if (!client) {
		return undefined;
	}
	return client.stop();
}
