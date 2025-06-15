// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/// <reference types="@figma/plugin-typings" />

// branded types
export type CollectionId = string & { readonly brand: unique symbol };
export type VariableId = string & { readonly brand: unique symbol };

// Create our own types that extend the Figma ones
export interface VariableCollectionSU extends Omit<VariableCollection, "id"> {
    id: CollectionId;
    variables: Map<VariableId, VariableSU>;
}

export interface VariableSU
    extends Omit<Variable, "id" | "variableCollectionId"> {
    id: VariableId;
    variableCollectionId: CollectionId;
}

export interface VariableAliasSU
    extends Omit<VariableAlias, "id"> {
    id: VariableId;
}

