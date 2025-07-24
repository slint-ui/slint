// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/**
 * Figma data acquisition utilities
 *
 * This module handles the acquisition of data from Figma APIs (both plugin and potentially REST API)
 * and provides it in a structured format for various export formats (Slint, JSON, etc.)
 *
 * The separation allows for:
 * - Reusing the same data acquisition logic across different exporters
 * - Easy switching between plugin API and REST API in the future
 * - Better testability by mocking this layer
 * - Centralized batching and error handling
 */

export interface FigmaMode {
    modeId: string;
    name: string;
}

export interface FigmaCollection {
    id: string;
    name: string;
    defaultModeId: string;
    hiddenFromPublishing: boolean;
    modes: FigmaMode[];
    variableIds: string[];
}

export interface FigmaVariable {
    id: string;
    name: string;
    variableCollectionId: string;
    resolvedType: "COLOR" | "FLOAT" | "STRING" | "BOOLEAN";
    valuesByMode: Record<string, any>;
    hiddenFromPublishing: boolean;
    scopes: string[];
}

export interface FigmaVariableData {
    collections: FigmaCollection[];
    variables: Map<string, FigmaVariable>;
}

/**
 * Acquire all Figma variable data using the plugin API
 *
 * This function handles the core data acquisition process:
 * - Fetches collections and their metadata
 * - Batches variable requests for performance
 * - Filters out invalid/empty variables
 * - Returns structured data ready for processing by exporters
 */
export async function acquireFigmaVariableData(): Promise<FigmaVariableData> {
    try {
        // Get all variable collections
        const rawCollections =
            await figma.variables.getLocalVariableCollectionsAsync();
        const collections: FigmaCollection[] = [];
        const variables = new Map<string, FigmaVariable>();

        // Process each collection
        for (const rawCollection of rawCollections) {
            // Convert to our standardized format
            const collection: FigmaCollection = {
                id: rawCollection.id,
                name: rawCollection.name,
                defaultModeId: rawCollection.defaultModeId,
                hiddenFromPublishing: rawCollection.hiddenFromPublishing,
                modes: rawCollection.modes.map((mode) => ({
                    modeId: mode.modeId,
                    name: mode.name,
                })),
                variableIds: [...rawCollection.variableIds], // Create a copy
            };

            // Process variables in batches for performance
            const batchSize = 5;
            for (
                let i = 0;
                i < rawCollection.variableIds.length;
                i += batchSize
            ) {
                const batch = rawCollection.variableIds.slice(i, i + batchSize);
                const batchPromises = batch.map((id) =>
                    figma.variables.getVariableByIdAsync(id),
                );
                const batchResults = await Promise.all(batchPromises);

                for (const variable of batchResults) {
                    if (!variable) {
                        continue;
                    }

                    // Skip variables with no values
                    if (
                        !variable.valuesByMode ||
                        Object.keys(variable.valuesByMode).length === 0
                    ) {
                        continue;
                    }

                    // Store standardized variable data
                    variables.set(variable.id, {
                        id: variable.id,
                        name: variable.name,
                        variableCollectionId: variable.variableCollectionId,
                        resolvedType: variable.resolvedType,
                        valuesByMode: { ...variable.valuesByMode }, // Create a copy
                        hiddenFromPublishing: variable.hiddenFromPublishing,
                        scopes: [...(variable.scopes || [])], // Create a copy
                    });
                }

                // Small delay to prevent overwhelming the API
                await new Promise((resolve) => setTimeout(resolve, 0));
            }

            collections.push(collection);
        }

        return {
            collections,
            variables,
        };
    } catch (error) {
        console.error("Error acquiring Figma variable data:", error);
        throw error;
    }
}

/**
 * Get a variable by ID from acquired data
 */
export function getVariableById(
    data: FigmaVariableData,
    variableId: string,
): FigmaVariable | undefined {
    return data.variables.get(variableId);
}

/**
 * Get all variables for a specific collection
 */
export function getVariablesForCollection(
    data: FigmaVariableData,
    collectionId: string,
): FigmaVariable[] {
    const collection = data.collections.find((c) => c.id === collectionId);
    if (!collection) {
        return [];
    }

    return collection.variableIds
        .map((id) => data.variables.get(id))
        .filter(
            (variable): variable is FigmaVariable => variable !== undefined,
        );
}

/**
 * Get a collection by ID from acquired data
 */
export function getCollectionById(
    data: FigmaVariableData,
    collectionId: string,
): FigmaCollection | undefined {
    return data.collections.find((c) => c.id === collectionId);
}
