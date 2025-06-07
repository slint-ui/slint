// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT


// Not all api data is collected. The following properties are not included:
// - variable.key
// - variable.remote
// - variable.description

interface VariableData {
    id: string;
    name: string;
    variableCollectionId: string;
    resolvedType: string;
    valuesByMode: { [modeId: string]: any };
    hiddenFromPublishing: boolean;
    scopes: string[];
}

export async function getRawVariableCollectionsData(): Promise<Array<{ name: string; content: string }>> {
    try {

        const [collections, allVariables] = await Promise.all([
            figma.variables.getLocalVariableCollectionsAsync(),
            figma.variables.getLocalVariablesAsync()
        ]);

        const variablesByCollection = new Map<string, VariableData[]>();
        for (const variable of allVariables) {
            const collectionId = variable.variableCollectionId;
            if (!variablesByCollection.has(collectionId)) {
                variablesByCollection.set(collectionId, []);
            }
            variablesByCollection.get(collectionId)!.push({
                id: variable.id,
                name: variable.name,
                variableCollectionId: variable.variableCollectionId,
                resolvedType: variable.resolvedType,
                valuesByMode: variable.valuesByMode,
                hiddenFromPublishing: variable.hiddenFromPublishing,
                scopes: variable.scopes
            });
        }

        // Build the final collections data
        const detailedCollections = collections.map(collection => ({
            id: collection.id,
            name: collection.name,
            defaultModeId: collection.defaultModeId,
            hiddenFromPublishing: collection.hiddenFromPublishing,
            modes: collection.modes.map(mode => ({
                modeId: mode.modeId,
                name: mode.name
            })),
            variables: variablesByCollection.get(collection.id) || []
        }));

        console.log("Total variables:", allVariables.length);

        const jsonData = JSON.stringify(detailedCollections, null, 2);
        
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