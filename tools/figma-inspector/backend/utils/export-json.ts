// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/**
 * JSON export utilities for Figma variable collections
 *
 * This module provides JSON export functionality for testing and debugging.
 * It uses the centralized data acquisition layer for consistency with other exporters.
 */

import {
    acquireFigmaVariableData,
    getVariablesForCollection,
    type FigmaMode,
} from "./figma-data-acquisition";

export interface JsonVariableAlias {
    type: "VARIABLE_ALIAS";
    id: string;
}

export interface JsonVariable {
    id: string;
    name: string;
    variableCollectionId: string;
    resolvedType: "COLOR" | "FLOAT" | "STRING" | "BOOLEAN";
    valuesByMode: Record<string, any>;
    hiddenFromPublishing: boolean;
    scopes: string[];
}

export interface JsonVariableCollection {
    id: string;
    name: string;
    defaultModeId: string;
    hiddenFromPublishing: boolean;
    modes: FigmaMode[];
    variables: JsonVariable[];
}

/**
 * Export Figma variable collections to JSON format using the data acquisition layer
 */
export async function exportFigmaVariablesToJson(
    includeInZip: boolean = false,
): Promise<{
    collections: JsonVariableCollection[];
    shouldIncludeInZip: boolean;
}> {
    try {
        // Use the centralized data acquisition
        const figmaData = await acquireFigmaVariableData();
        const jsonCollections: JsonVariableCollection[] = [];

        for (const collection of figmaData.collections) {
            const jsonCollection: JsonVariableCollection = {
                id: collection.id,
                name: collection.name,
                defaultModeId: collection.defaultModeId,
                hiddenFromPublishing: collection.hiddenFromPublishing,
                modes: collection.modes,
                variables: [],
            };

            // Get variables for this collection using the data acquisition utilities
            const collectionVariables = getVariablesForCollection(
                figmaData,
                collection.id,
            );

            for (const variable of collectionVariables) {
                const jsonVariable: JsonVariable = {
                    id: variable.id,
                    name: variable.name,
                    variableCollectionId: variable.variableCollectionId,
                    resolvedType: variable.resolvedType,
                    valuesByMode: {},
                    hiddenFromPublishing: variable.hiddenFromPublishing,
                    scopes: variable.scopes,
                };

                // Process values by mode - preserve exact Figma structure
                for (const [modeId, value] of Object.entries(
                    variable.valuesByMode,
                )) {
                    jsonVariable.valuesByMode[modeId] = processVariableValue(
                        value,
                        variable.resolvedType,
                    );
                }

                jsonCollection.variables.push(jsonVariable);
            }

            jsonCollections.push(jsonCollection);
        }

        return {
            collections: jsonCollections,
            shouldIncludeInZip: includeInZip,
        };
    } catch (error) {
        console.error("Error exporting Figma variables to JSON:", error);
        throw error;
    }
}

/**
 * Process a variable value to maintain exact Figma structure
 */
function processVariableValue(value: any, resolvedType: string): any {
    // Handle variable aliases (references to other variables)
    if (
        typeof value === "object" &&
        value &&
        "type" in value &&
        value.type === "VARIABLE_ALIAS"
    ) {
        return {
            type: "VARIABLE_ALIAS",
            id: value.id,
        } as JsonVariableAlias;
    }

    // For all other values, preserve the exact structure that Figma provides
    switch (resolvedType) {
        case "COLOR":
            if (typeof value === "object" && value && "r" in value) {
                return {
                    r: value.r,
                    g: value.g,
                    b: value.b,
                    a: "a" in value ? value.a : 1,
                };
            }
            break;
        case "FLOAT":
            return typeof value === "number" ? value : 0;
        case "STRING":
            return typeof value === "string" ? value : "";
        case "BOOLEAN":
            return typeof value === "boolean" ? value : false;
    }

    // Fallback: return the value as-is
    return value;
}

/**
 * Export collections to JSON string formatting
 */
export async function exportFigmaVariablesToJsonString(
    indent: number = 2,
    includeInZip: boolean = false,
): Promise<string> {
    const result = await exportFigmaVariablesToJson(includeInZip);
    return JSON.stringify(result.collections, null, indent);
}
