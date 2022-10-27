// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import {
  Position as LspPosition,
  Range as LspRange,
} from "vscode-languageserver-types";
import * as monaco from "monaco-editor/esm/vs/editor/editor.api";

function find_model(
  model: string | monaco.editor.ITextModel,
): monaco.editor.ITextModel | null {
  if (typeof model === "string") {
    return monaco.editor.getModel(monaco.Uri.parse(model));
  }
  return model;
}

export function lsp_position_to_editor_position(
  model_: string | monaco.editor.ITextModel,
  pos: LspPosition,
): monaco.IPosition | null {
  const model = find_model(model_);
  if (model == null) {
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
  return new monaco.Position(monacoLineNumber, line_until_character.length + 1); // LSP is 0-based, so add 1 as Monaco is 1 based!
}

export function lsp_range_to_editor_range(
  model_: string | monaco.editor.ITextModel,
  range: LspRange,
): monaco.IRange | null {
  const model = find_model(model_);
  if (model == null) {
    return null;
  }

  const startPos = lsp_position_to_editor_position(model, range.start);
  const endPos = lsp_position_to_editor_position(model, range.end);

  if (startPos == null || endPos == null) {
    return null;
  }

  return {
    startLineNumber: startPos.lineNumber,
    startColumn: startPos.column,
    endLineNumber: endPos.lineNumber,
    endColumn: endPos.column - 1, // LSP reports the first letter *not* part of the range anymore, so go for the one before that!
  };
}

export interface DeclarationPosition {
  uri: string;
  start_position: LspPosition;
}

export interface DefinitionPosition {
  property_definition_range: LspRange;
  expression_range: LspRange;
}

export interface Property {
  name: string;
  group: string;
  type_name: string;
  declared_at: DeclarationPosition | null;
  defined_at: DefinitionPosition | null;
}

export interface Element {
  id: string;
  type_name: string;
}

export interface PropertyQuery {
  source_uri: string;
  element: Element | null;
  properties: Property[];
}

export interface BindingTextProvider {
  binding_text(_location: DefinitionPosition): string;
}
