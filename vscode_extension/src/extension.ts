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

const program_extension = process.platform === "win32" ? ".exe" : "";

function lspPlatform(): string | null {
	if (process.platform === "darwin") {
		if (process.arch === "x64") {
			return "x86_64-apple-darwin";
		} else if (process.arch == "arm64") {
			return "aarch64-apple-darwin";
		}
	}
	else if (process.platform === "linux") {
		if (process.arch === "x64") {
			return "x86_64-unknown-linux-gnu";
		}
	}
	else if (process.platform === "win32") {
		return "x86_64-pc-windows-gnu";
	}
	return null;
}

function getShowPreviewhandler(toSide: boolean): (...args: any[]) => any {
	return () => {
		let ae = vscode.window.activeTextEditor;
		if (!ae) {
			return;
		}
		let uri = ae.document.uri;
		const webview = vscode.window.createWebviewPanel(
			"sixtyfps.preview",
			"[Preview] " + path.basename(uri.fsPath),
			ae.viewColumn ? ae.viewColumn + (toSide ? 1 : 0) : 0
		);
		webview.webview.html = `<p>Here will soon come a preview for ${uri.toString()}</p>`;
	};
}

export function activate(context: vscode.ExtensionContext) {

	/*let test_output = vscode.window.createOutputChannel("Test Output");
	test_output.appendLine("Hello from extension");*/

	context.subscriptions.push(vscode.commands.registerCommand('sixtyfps.showPreview', getShowPreviewhandler(false)));
	context.subscriptions.push(vscode.commands.registerCommand('sixtyfps.showPreviewToSide', getShowPreviewhandler(true)));

	let lsp_platform = lspPlatform();
	if (lsp_platform === null) {
		return;
	}

	let serverModule = path.join(context.extensionPath, "bin", "sixtyfps-lsp-" + lsp_platform + program_extension);

	if (!existsSync(serverModule)) {
		serverModule = context.asAbsolutePath(path.join('..', 'target', 'debug', 'sixtyfps-lsp' + program_extension));
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
