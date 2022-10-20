// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";

export type TextRange = monaco.IRange;
export type TextPosition = monaco.IPosition;
export type Uri = monaco.Uri;

export type DocumentAndTextPosition = { uri: string; position: TextPosition };

export type GotoPositionCallback = (
  _uri: string,
  _position: TextPosition | TextRange,
) => void;
export type PositionChangeCallback = (_pos: DocumentAndTextPosition) => void;
