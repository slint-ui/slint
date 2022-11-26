// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import {
    PropertyQuery,
    SetBindingResponse,
} from "../../../tools/online_editor/src/shared/properties";

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
        "setBinding",
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
        "queryProperties",
        { uri: uri.toString() },
        position,
    );
}
