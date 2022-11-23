// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import { PropertyQuery, SetBindingResponse } from "./properties";

import {
    ExecuteCommandRequest,
    ExecuteCommandParams,
} from "vscode-languageserver-protocol";
import {
    OptionalVersionedTextDocumentIdentifier,
    Position as LspPosition,
    Range as LspRange,
    URI as LspURI,
    WorkspaceEdit,
} from "vscode-languageserver-types";

import { BaseLanguageClient } from "vscode-languageclient";

export type WorkspaceEditor = (_we: WorkspaceEdit) => boolean;
export type SetPropertiesHelper = (_p: PropertyQuery) => void;

export async function change_property(
    client: BaseLanguageClient | null,
    doc: OptionalVersionedTextDocumentIdentifier,
    element_range: LspRange,
    property_name: string,
    current_text: string,
    dry_run: boolean,
): Promise<SetBindingResponse> {
    if (client != null) {
        console.log("Calling into LSP");
        const result = await client.sendRequest(ExecuteCommandRequest.type, {
            command: "setBinding",
            arguments: [
                doc,
                element_range,
                property_name,
                current_text,
                dry_run,
            ],
        } as ExecuteCommandParams);

        console.log("Returning from LSP:", result.diagnostics);
        return result;
    }
    return new Promise((accept, _) => accept({ diagnostics: [] }));
}

export function query_properties(
    client: BaseLanguageClient | null,
    uri: LspURI,
    position: LspPosition,
    set_properties_helper: SetPropertiesHelper,
) {
    if (client != null) {
        client
            .sendRequest(ExecuteCommandRequest.type, {
                command: "queryProperties",
                arguments: [{ uri: uri.toString() }, position],
            } as ExecuteCommandParams)
            .then((r: PropertyQuery) => {
                set_properties_helper(r);
            });
    }
}
