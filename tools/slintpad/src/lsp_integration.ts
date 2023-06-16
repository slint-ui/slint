// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

export {
    Position as LspPosition,
    Range as LspRange,
    URI as LspURI,
} from "vscode-languageserver-types";
import {
    Position as LspPosition,
    Range as LspRange,
} from "vscode-languageserver-types";

import { TextRange, TextPosition } from "./text";

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";

function find_model(
    model: string | monaco.editor.ITextModel | null | undefined,
): monaco.editor.ITextModel | null {
    if (typeof model === "string") {
        return monaco.editor.getModel(monaco.Uri.parse(model));
    }
    return model ?? null;
}

export function editor_position_to_lsp_position(
    model_: string | monaco.editor.ITextModel | null | undefined,
    pos: TextPosition | null | undefined,
): LspPosition | null {
    const model = find_model(model_);
    if (model == null || pos == null) {
        return null;
    }

    // LSP line numbers are zero based (https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocuments)
    // and Monaco's line numbers start at 1 (https://microsoft.github.io/monaco-editor/api/classes/monaco.Position.html#lineNumber)
    const lspLine = pos.lineNumber - 1;
    // Convert lsp utf-8 character index to JavaScript utf-16 string index
    const line = model.getLineContent(pos.lineNumber);
    const line_utf8 = new TextEncoder().encode(line.slice(0, pos.column));
    const lspCharacter = line_utf8.length;

    return {
        line: lspLine,
        character: lspCharacter,
    };
}

export function editor_range_to_lsp_range(
    model: string | monaco.editor.ITextModel | null | undefined,
    range: TextRange | null | undefined,
): LspRange | null {
    if (range == null) {
        return null;
    }

    const start = editor_position_to_lsp_position(model, {
        lineNumber: range.startLineNumber,
        column: range.startColumn,
    });
    const end = editor_position_to_lsp_position(model, {
        lineNumber: range.endLineNumber,
        column: range.endColumn,
    });

    if (start == null || end == null) {
        return null;
    }

    return {
        start: start,
        end: end,
    };
}

export function lsp_position_to_editor_position(
    model_: string | monaco.editor.ITextModel | null | undefined,
    pos: LspPosition | null | undefined,
): TextPosition | null {
    const model = find_model(model_);
    if (model == null || pos == null) {
        return null;
    }

    // LSP line numbers are zero based (https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocuments)
    // and Monaco's line numbers start at 1 (https://microsoft.github.io/monaco-editor/api/classes/monaco.Position.html#lineNumber)
    const monacoLineNumber = pos.line + 1;
    // Convert lsp utf-8 character index to JavaScript utf-16 string index
    const line = model.getLineContent(monacoLineNumber);
    const line_utf8 = new TextEncoder().encode(line);
    const line_utf8_until_character = line_utf8.slice(0, pos.character);
    const line_until_character = new TextDecoder().decode(
        line_utf8_until_character,
    );
    return new monaco.Position(
        monacoLineNumber,
        line_until_character.length + 1,
    ); // LSP is 0-based, so add 1 as Monaco is 1 based!
}

export function lsp_range_to_editor_range(
    model: string | monaco.editor.ITextModel | null | undefined,
    range: LspRange | null | undefined,
): TextRange | null {
    const startPos = lsp_position_to_editor_position(model, range?.start);
    const endPos = lsp_position_to_editor_position(model, range?.end);

    if (startPos == null || endPos == null) {
        return null;
    }

    return {
        startLineNumber: startPos.lineNumber,
        startColumn: startPos.column,
        endLineNumber: endPos.lineNumber,
        endColumn: endPos.column,
    };
}
