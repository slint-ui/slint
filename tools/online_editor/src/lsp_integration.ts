// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Slint-commercial

export interface DeclarationPosition {
  uri: string;
  start_offset: number;
}

export interface DefinitionPosition {
  start_offset: number;
  end_offset: number;
  expression_start: number;
  expression_end: number;
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
