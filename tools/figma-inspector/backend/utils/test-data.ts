// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

interface VariableMode {
    modeId: string;
    name: string;
}

interface VariableData {
    id: string;
    name: string;
    variableCollectionId: string;
    resolvedType: string;
    valuesByMode: { [modeId: string]: any };
    description: string;
    hiddenFromPublishing: boolean;
    scopes: string[];
}

interface CollectionData {
    id: string;
    name: string;
    defaultModeId: string;
    hiddenFromPublishing: boolean;
    modes: VariableMode[];
    variables: VariableData[];
}

export async function getRawVariableCollectionsData(): Promise<Array<{ name: string; content: string }>> {
    try {
        let variableCount = 0;
        const collections = await figma.variables.getLocalVariableCollectionsAsync();
        const detailedCollections: CollectionData[] = [];

        for (const collection of collections) {
            const collectionData: CollectionData = {
                id: collection.id,
                name: collection.name,
                defaultModeId: collection.defaultModeId,
                hiddenFromPublishing: collection.hiddenFromPublishing,
                modes: collection.modes.map(mode => ({
                    modeId: mode.modeId,
                    name: mode.name
                })),
                variables: []
            };

            // Get detailed data for each variable in the collection
            for (const variableId of collection.variableIds) {
                const variable = await figma.variables.getVariableByIdAsync(variableId);
                
                if (variable) {
                    variableCount++;
                    const variableData: VariableData = {
                        id: variable.id,
                        name: variable.name,
                        variableCollectionId: variable.variableCollectionId,
                        resolvedType: variable.resolvedType,
                        valuesByMode: variable.valuesByMode,
                        description: variable.description,
                        hiddenFromPublishing: variable.hiddenFromPublishing,
                        scopes: variable.scopes
                    };
                    collectionData.variables.push(variableData);
                }
            }

            detailedCollections.push(collectionData);
        }

        const jsonData = JSON.stringify(detailedCollections, null, 2);
        console.log("variableCount", variableCount);
        
        return [{
            name: "figma-variables.json",
            content: jsonData
        }];
    } catch (error) {
        console.error("Error getting raw variable collections:", error);
        return [{
            name: "error.json",
            content: JSON.stringify({ error: String(error) })
        }];
    }
}