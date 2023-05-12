// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import { PropertyQuery, SetBindingResponse } from "./properties";

import {
    OptionalVersionedTextDocumentIdentifier,
    Position as LspPosition,
    Range as LspRange,
    URI as LspURI,
    WorkspaceEdit,
} from "vscode-languageserver-types";

import * as vscode from "vscode";

export type WorkspaceEditor = (_we: WorkspaceEdit) => boolean;
export type SetPropertiesHelper = (_p: PropertyQuery) => void;

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
): Promise<SetBindingResponse> {
    return vscode.commands.executeCommand("slint/showPreview", url, component);
}

export async function setDesignMode(
    enable: boolean,
): Promise<SetBindingResponse> {
    return vscode.commands.executeCommand("slint/setDesignMode", enable);
}

export async function toggleDesignMode(): Promise<SetBindingResponse> {
    return vscode.commands.executeCommand("slint/toggleDesignMode");
}

export async function setBinding(
    doc: OptionalVersionedTextDocumentIdentifier,
    element_range: LspRange,
    property_name: string,
    current_text: string,
    dry_run: boolean,
): Promise<SetBindingResponse> {
    return vscode.commands.executeCommand(
        "slint/setBinding",
        doc,
        element_range,
        property_name,
        current_text,
        dry_run,
    );
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

export async function queryProperties(
    uri: LspURI,
    position: LspPosition,
): Promise<PropertyQuery> {
    return vscode.commands.executeCommand(
        "slint/queryProperties",
        { uri: uri.toString() },
        position,
    );
}
