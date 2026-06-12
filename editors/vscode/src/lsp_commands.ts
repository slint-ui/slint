// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import type { URI as LspURI } from "vscode-languageserver-types";
import * as vscode from "vscode";

// Use the auto-registered VSCode command for the custom executables offered
// by our language server.
//
// Talking to the server directly like this:
//
// ```typescript
//     return await client.sendRequest(ExecuteCommandRequest.type, {
//         command: "slint/showPreview",
//         arguments: [url, component],
//     } as ExecuteCommandParams);
// ```
//
// has the side effect of going around our middleware.

export function showPreview(url: LspURI, component: string): Thenable<unknown> {
    return vscode.commands.executeCommand("slint/showPreview", url, component);
}

// Send the LSP-side `slint/renameWithHostAccessors` command. The LSP runs the
// Slint rename, merges in textual rewrites of the generated Rust/C++
// accessor at workspace call sites, and sends back a `workspace/applyEdit`
// for the editor to apply.
export function renameWithHostAccessors(
    uri: LspURI,
    position: { line: number; character: number },
    newName: string,
): Thenable<unknown> {
    return vscode.commands.executeCommand(
        "slint/renameWithHostAccessors",
        uri,
        position,
        newName,
    );
}
