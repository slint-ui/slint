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
            if (collection.variables.length === 0) {
                continue;
            }

            const modeNames = collection.modes.map((m) => m.name);
            const structName = `Collection${collectionCount}`;
            collectionCount++;

            // Enums are only needed when > 1 modes
            if (modeNames.length > 1) {
                const enumName = `Mode${collectionCount - 1}`;
                allSlintCode += `enum ${enumName} {\n`;
                for (const mode of modeNames) {
                    allSlintCode += `${indent}${mode},\n`;
                }
                allSlintCode += `}\n\n`;
            }

            // Generate a struct for the collection
            allSlintCode += `struct ${structName} {\n`;
            for (const variable of collection.variables) {
                const slintType = getSlintType(variable.resolvedType);
                allSlintCode += `${indent}${variable.name}: ${slintType},\n`;
            }
            allSlintCode += `}\n\n`;
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

const validChars = new Set(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-",
);
const numberChars = new Set("0123456789");

export function sanitizeSlintPropertyName(name: string): string {
    name = name.trim();

    // Replace forward slashes with hyphen
    name = name.replaceAll("/", "-");

    // Remove all invalid characters, keeping only:
    // - ASCII letters (a-z, A-Z)
    // - Numbers (0-9), Underscores (_) and Hyphens (-)
    name = name
        .split("")
        .filter((char) => validChars.has(char))
        .join("");

    // Remove consecutive duplicate words
    const parts = name.split("-");
    name = parts
        .filter((part, i) => i === 0 || part !== parts[i - 1])
        .join("-");

    // names start with a letter or underscore
    const firstChar = name.charAt(0);
    if (numberChars.has(firstChar)) {
        name = `_${name}`;
    } else if (firstChar === "-") {
        name = `_${name.substring(1)}`;
    }

    // handle empty name
    if (name === "") {
        return "_";
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
