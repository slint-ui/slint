// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";

export type TextRange = monaco.IRange;
export type TextPosition = monaco.IPosition;
export type Uri = monaco.Uri;

import { LspRange, LspPosition } from "./lsp_integration";

export type DocumentAndPosition = { uri: string; position: LspPosition };
export type VersionedDocumentAndPosition = {
    uri: string;
    position: LspPosition;
    version: number;
};

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
export type PositionChangeCallback = (
    _pos: VersionedDocumentAndPosition,
) => void;

export type HighlightRequestCallback = (
    _url: string,
    _start: { line: number; column: number },
    _end: { line: number; column: number },
) => void;
