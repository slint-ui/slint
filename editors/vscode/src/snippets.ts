// Copyright © Danny Tuppeny <danny@tuppeny.com>
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// LSP code actions and workspace edits do not yet natively support snippets,
// or allow specifying the cursor position:
// https://github.com/microsoft/language-server-protocol/issues/724
//
// This file implements an experimental SnippetTextEdit feature inspired by
// [rust-analyzer](https://github.com/rust-lang/rust-analyzer/blob/master/docs/dev/lsp-extensions.md#snippet-textedit)
// and [Dart-Code](https://github.com/Dart-Code/Dart-Code/blob/master/src/extension/analysis/analyzer_lsp_snippet_text_edits.ts).

// cSpell: ignore Tuppeny

import * as vscode from "vscode";
import type {
    ClientCapabilities,
    FeatureState,
    StaticFeature,
} from "vscode-languageclient";

export class SnippetTextEditFeature implements StaticFeature {
    private command: vscode.Disposable | undefined;

    fillClientCapabilities(capabilities: ClientCapabilities) {
        capabilities.experimental = capabilities.experimental ?? {};
        Object.assign(capabilities.experimental, { snippetTextEdit: true });
    }

    initialize() {
        this.command = vscode.commands.registerCommand(
            "_slint.applySnippetTextEdit",
            this.applySnippetTextEdit,
        );
    }

    clear() {
        this.command?.dispose();
    }

    private async applySnippetTextEdit(uri: vscode.Uri, edit: vscode.TextEdit) {
        // Compensate for VS Code's automatic indentation
        const doc = await vscode.workspace.openTextDocument(uri);
        const line = doc.lineAt(edit.range.start.line);
        const indent = " ".repeat(line.firstNonWhitespaceCharacterIndex);
        const newText = edit.newText.replaceAll(`\n${indent}`, "\n");

        const editor = await vscode.window.showTextDocument(doc);
        await editor.insertSnippet(
            new vscode.SnippetString(newText),
            edit.range,
        );
    }

    getState(): FeatureState {
        return { kind: "static" };
    }
}

export function detectSnippetCodeActions(
    actions: Array<vscode.Command | vscode.CodeAction>,
) {
    for (const action of actions) {
        if (action instanceof vscode.CodeAction && action.edit) {
            const edits = action.edit.entries();
            if (edits.length === 1 && edits[0][1].length === 1) {
                const uri = edits[0][0];
                const textEdit = edits[0][1][0];
                // Check for "$0" or "${0:foo}" snippet placeholders
                if (/\$(?:0|\{0:(?:[^}]*)\})/.test(textEdit.newText)) {
                    action.edit = undefined;
                    action.command = {
                        title: "Apply snippet text edit",
                        command: "_slint.applySnippetTextEdit",
                        arguments: [uri, textEdit],
                    };
                }
            }
        }
    }
}
