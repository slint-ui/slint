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
