// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import { PropertyQuery } from "./properties";

import {
    ExecuteCommandRequest,
    ExecuteCommandParams,
} from "vscode-languageserver-protocol";
import {
    Position as LspPosition,
    URI as LspURI,
} from "vscode-languageserver-types";

import { BaseLanguageClient } from "vscode-languageclient";

export type SetPropertiesHelper = (_p: PropertyQuery) => void;

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
