// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";

export type TextRange = monaco.IRange;
export type TextPosition = monaco.IPosition;
export type Uri = monaco.Uri;

import { LspRange, LspPosition } from "./lsp_integration";

export type DocumentAndPosition = { uri: string; position: LspPosition };

export type ReplaceTextFunction = (
    _uri: string,
    _range: LspRange,
    _new_text: string,
    _validate: (_old: string) => boolean,
) => boolean;
export type GotoPositionCallback = (
    _uri: string,
    _position: LspPosition | LspRange,
) => void;
export type PositionChangeCallback = (_pos: DocumentAndPosition) => void;
