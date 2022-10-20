// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import { Position, Range } from 'vscode-languageserver-types'

export interface DeclarationPosition {
  uri: string;
  start_position: Position;
}

export interface DefinitionPosition {
  property_definition_range: Range;
  expression_range: Range;
}

export interface Property {
  name: string;
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
