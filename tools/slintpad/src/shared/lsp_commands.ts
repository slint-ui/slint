// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import {
    OptionalVersionedTextDocumentIdentifier,
    Range as LspRange,
    URI as LspURI,
    WorkspaceEdit,
} from "vscode-languageserver-types";

import * as vscode from "vscode";

export type WorkspaceEditor = (_we: WorkspaceEdit) => boolean;

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

export async function showPreview(
    url: LspURI,
    component: string,
): Promise<unknown> {
    return vscode.commands.executeCommand("slint/showPreview", url, component);
}

export async function removeBinding(
    doc: OptionalVersionedTextDocumentIdentifier,
    element_range: LspRange,
    property_name: string,
): Promise<boolean> {
    return vscode.commands.executeCommand(
        "slint/removeBinding",
        doc,
        element_range,
        property_name,
    );
}
