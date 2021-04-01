/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

import * as path from 'path';
import { existsSync } from 'fs';
import * as vscode from 'vscode';

import {
	LanguageClient,
	LanguageClientOptions,
	ServerCapabilities,
	ServerOptions,
	//	TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient;

function lspProgramNameSuffix(): string | null {
	if (process.platform === "darwin") {
		return "x86_64-apple-darwin";
	}
	if (process.platform === "linux") {
		if (process.arch === "x64") {
			return "x86_64-unknown-linux-gnu";
		}
	}
	if (process.platform === "win32") {
		return "x86_64-pc-windows-gnu.exe";
	}
	return null;
}

export function activate(context: vscode.ExtensionContext) {

	/*let test_output = vscode.window.createOutputChannel("Test Output");
	test_output.appendLine("Hello from extension");*/

	const commandHandler = (name: string = 'world') => {
		console.log(`Hello ${name}!!!  (Not yet implemented)`);
	};
	context.subscriptions.push(vscode.commands.registerCommand('sixtyfps.showPreview', commandHandler));
	context.subscriptions.push(vscode.commands.registerCommand('sixtyfps.showPreviewToSide', commandHandler));

	let lspSuffix = lspProgramNameSuffix();
	if (lspSuffix === null) {
		return;
	}

	let serverModule = path.join(context.extensionPath, "bin", "sixtyfps-lsp-" + lspSuffix);

	if (!existsSync(serverModule)) {
		serverModule = context.asAbsolutePath(path.join('..', 'target', 'debug', 'sixtyfps-lsp'));
	}
	if (!existsSync(serverModule)) {
		console.warn("Could not locate sixtyfps-server server binary, neither in bundled bin/ directory nor relative in ../target");
		return;
	}

	console.log(`Starting LSP server from {serverModule)`);

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
