// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import {
    PropertyQuery,
    SetBindingResponse,
} from "../../../tools/slintpad/src/shared/properties";

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

export async function change_property(
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

export async function query_properties(
    uri: LspURI,
    position: LspPosition,
): Promise<PropertyQuery> {
    return vscode.commands.executeCommand(
        "slint/queryProperties",
        { uri: uri.toString() },
        position,
    );
}

export async function remove_binding(
    doc: OptionalVersionedTextDocumentIdentifier,
    element_range: LspRange,
    property_name: string,
): Promise<SetBindingResponse> {
    return vscode.commands.executeCommand(
        "slint/removeBinding",
        doc,
        element_range,
        property_name,
    );
}
