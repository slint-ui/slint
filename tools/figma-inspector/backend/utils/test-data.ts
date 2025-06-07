// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { dispatchTS } from "./code-utils.js";
import { getSlintType } from "./export-variables";

interface VariableData {
    id: string;
    name: string;
    variableCollectionId: string;
    resolvedType: string;
    valuesByMode: { [modeId: string]: any };
    hiddenFromPublishing: boolean;
    scopes: string[];
}

interface ProcessedCollection {
    id: string;
    name: string;
    defaultModeId: string;
    hiddenFromPublishing: boolean;
    modes: Array<{
        modeId: string;
        name: string;
    }>;
    variables: VariableData[];
}

interface VariableCollection {
    id: string;
    name: string;
    defaultModeId: string;
    hiddenFromPublishing: boolean;
    modes: Array<{
        modeId: string;
        name: string;
    }>;
    variables: Array<{
        id: string;
        name: string;
        variableCollectionId: string;
        resolvedType: string;
        valuesByMode: { [modeId: string]: any };
        hiddenFromPublishing: boolean;
        scopes: string[];
    }>;
}

const indent = "    ";

// Not all api data is collected. The following properties are not included:
// - variable.key
// - variable.remote
// - variable.description // Useful for comments in the code.
// - variable.codeSyntax // This might be useful in the future as it allows figma to give
// an actual name for the variable to be used in code. However its for CSS and Swift only right now.

export async function processVariableCollections(): Promise<
    ProcessedCollection[]
> {
    try {
        const [collections, allVariables] = await Promise.all([
            figma.variables.getLocalVariableCollectionsAsync(),
            figma.variables.getLocalVariablesAsync(),
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
                scopes: variable.scopes,
            });
        }

        // Build the final collections data
        const detailedCollections = collections.map((collection) => ({
            id: collection.id,
            name: collection.name,
            defaultModeId: collection.defaultModeId,
            hiddenFromPublishing: collection.hiddenFromPublishing,
            modes: collection.modes.map((mode) => ({
                modeId: mode.modeId,
                name: mode.name,
            })),
            variables: variablesByCollection.get(collection.id) || [],
        }));

        console.log("Total variables:", allVariables.length);
        return detailedCollections;
    } catch (error) {
        console.error("Error processing variable collections:", error);
        throw error;
    }
}

export async function createSlintExport(): Promise<void> {
    try {
        const start = Date.now();
        const collections = await processVariableCollections();
        const sanitizedCollections = sanitizeCollections(collections);

        let allSlintCode = "";
        let collectionCount = 1;

        for (const collection of sanitizedCollections) {
            const modeNames = collection.modes.map((m) => m.name);
            const enumName = `Mode${collectionCount}`;
            collectionCount++;

            const structFields: { [type: string]: string[] } = {};

            // Group variables by type
            for (const variable of collection.variables) {
                const slintType = getSlintType(variable.resolvedType);
                if (!structFields[slintType]) {
                    structFields[slintType] = [];
                }
                structFields[slintType].push(variable.name);
            }

            // Generate enum for modes
            let slintCode = `enum ${enumName} {\n`;
            for (const mode of modeNames) {
                slintCode += `${indent}${mode},\n`;
            }
            slintCode += `}\n\n`;

            // Generate a struct for each type
            for (const [type, fields] of Object.entries(structFields)) {
                const structName =
                    type.charAt(0).toUpperCase() + type.slice(1) + "Vars";
                slintCode += `struct ${structName} {\n`;
                for (const field of fields) {
                    slintCode += `${indent}${field}: ${type},\n`;
                }
                slintCode += `}\n\n`;
            }

            allSlintCode += slintCode;
        }

        dispatchTS("saveTextFile", {
            filename: "example.slint",
            content: allSlintCode,
        });

        console.log("Code gen took", Date.now() - start, "ms");
    } catch (error) {
        console.error("Error creating Slint export:", error);
        throw error;
    }
}

export function sanitizeSlintPropertyName(name: string): string {
    name = name.trim();

    // Replace forward slashes with hyphen
    name = name.replace(/\//g, "-");

    // Remove all invalid characters, keeping only:
    // - ASCII letters (a-z, A-Z)
    // - Numbers (0-9), Underscores (_) and Hyphens (-)
    name = name.replace(/[^a-zA-Z0-9_-]/g, "");

    // Ensure name starts with a letter or hyphen
    if (!/^[a-zA-Z_]/.test(name)) {
        name = "_" + name;
    }

    return name;
}

function sanitizeCollections(
    collections: ProcessedCollection[],
): VariableCollection[] {
    return collections.map((collection) => {

        const sanitizedName = sanitizeSlintPropertyName(collection.name);

        const sanitizedModes = collection.modes.map((mode) => ({
            modeId: mode.modeId,
            name: sanitizeSlintPropertyName(mode.name),
        }));
        
        const sanitizedVariables = collection.variables.map((variable) => ({
            ...variable,
            name: sanitizeSlintPropertyName(variable.name),
        }));
        
        return {
            id: collection.id,
            name: sanitizedName,
            defaultModeId: collection.defaultModeId,
            hiddenFromPublishing: collection.hiddenFromPublishing,
            modes: sanitizedModes,
            variables: sanitizedVariables,
        };
    });
}

export async function saveVariableCollectionsToFile(
    filename: string = "figma-test-data",
): Promise<Array<{ name: string; content: string }>> {
    try {
        const collections = await processVariableCollections();
        const jsonData = JSON.stringify(collections, null, 2);

        return [
            {
                name: `${filename}.json`,
                content: jsonData,
            },
        ];
    } catch (error) {
        console.error("Error saving variable collections:", error);
        return [
            {
                name: "error.json",
                content: JSON.stringify({ error: String(error) }),
            },
        ];
    }
}
