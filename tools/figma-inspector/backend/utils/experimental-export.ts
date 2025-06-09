// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { dispatchTS } from "./code-utils.js";
import { rgbToHex } from "./property-parsing.js";

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

interface VariableReference {
    path: string;
    variable: VariableData;
}

export const indent = "    ";
export const indent2 = indent + indent;

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
        const start = Date.now();
        const collections = await figma.variables.getLocalVariableCollectionsAsync();
        const allVariables = await figma.variables.getLocalVariablesAsync();

        const variablesByCollection = new Map<string, VariableData[]>();
        for (const variable of allVariables) {
            const collectionId = variable.variableCollectionId;
            if (!collectionId) { continue }
            
            if (!variablesByCollection.has(collectionId)) {
                variablesByCollection.set(collectionId, []);
            }
            
            // Ensure all required properties exist with defaults
            const safeVariable: VariableData = {
                id: variable.id || '',
                name: variable.name || '',
                variableCollectionId: collectionId,
                resolvedType: variable.resolvedType || 'STRING',
                valuesByMode: variable.valuesByMode || {},
                hiddenFromPublishing: variable.hiddenFromPublishing ?? false,
                scopes: variable.scopes || [],
            };
            variablesByCollection.get(collectionId)!.push(safeVariable);
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

        console.log("get data took", Date.now() - start, "ms");

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
        console.log("Part1 took", Date.now() - start, "ms");
        const sanitizedCollections = sanitizeCollections(collections);

        

        // Build a map of variable IDs to their references and data
        const variableRefMap = new Map<string, VariableReference>();
        const collectionDefaultModes = new Map<string, string>();
        for (const collection of sanitizedCollections) {
            collectionDefaultModes.set(collection.id, collection.defaultModeId);
            for (const variable of collection.variables) {
                variableRefMap.set(variable.id, {
                    path: `${collection.name}.collection.${variable.name}`,
                    variable: variable,
                });
            }
        }
        

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
                const slintType = getSlintType(variable);
                allSlintCode += `${indent}${variable.name}: ${slintType},\n`;
            }
            allSlintCode += `}\n\n`;
        }

        // Create a global for each collection
        for (const collection of sanitizedCollections) {
            if (collection.variables.length === 0) {
                continue;
            }

            const collectionIndex =
                sanitizedCollections.indexOf(collection) + 1;
            const enumName = `Mode${collectionIndex}`;
            const structName = `Collection${collectionIndex}`;

            allSlintCode += `export global ${collection.name} {\n`;

            if (collection.modes.length > 1) {
                // Find the default mode name using defaultModeId
                const defaultMode =
                    collection.modes.find(
                        (mode) => mode.modeId === collection.defaultModeId,
                    )?.name || collection.modes[0].name;

                // Add input property for mode selection
                allSlintCode += `${indent}in property <${enumName}> current-mode: ${enumName}.${defaultMode};\n`;

                // Add output property that selects the appropriate mode
                allSlintCode += `${indent}out property <${structName}> collection: `;
                if (collection.modes.length > 1) {
                    const conditions = collection.modes
                        .map((mode, index) => {
                            if (index === collection.modes.length - 1) {
                                return `${mode.name}`;
                            }
                            return `current-mode == ${enumName}.${mode.name} ? ${mode.name} : `;
                        })
                        .join("");
                    allSlintCode += conditions + ";\n\n";
                }

                // Add properties for each mode
                for (const mode of collection.modes) {
                    allSlintCode += `${indent}property <${structName}> ${mode.name}: {\n`;
                    allSlintCode += generateVariablesForMode(
                        collection.variables,
                        mode.modeId,
                        collection.name,
                        collectionDefaultModes,
                        variableRefMap,
                    );
                }
            } else {
                // For collections with only one mode, just create a simple property
                allSlintCode += `${indent}out property <${structName}> collection: {\n`;
                allSlintCode += generateVariablesForMode(
                    collection.variables,
                    collection.modes[0].modeId,
                    collection.name,
                    collectionDefaultModes,
                    variableRefMap,
                );
            }

            allSlintCode += `}\n\n`;
        }

        console.log("Code gen took", Date.now() - start, "ms");

        dispatchTS("saveTextFile", {
            filename: "example.slint",
            content: allSlintCode,
        });
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

export function getSlintType(variable: VariableData): string {
    switch (variable.resolvedType) {
        case "COLOR":
            return "brush";
        case "FLOAT":
            // Filter out FONT_VARIATIONS as it can be ignored
            const relevantScopes = variable.scopes.filter(
                (scope) => scope !== "FONT_VARIATIONS",
            );

            if (relevantScopes.length === 1) {
                if (relevantScopes[0] === "OPACITY") {
                    return "float";
                }
            }
            // If it's ALL_SCOPES or no specific scope matches, return length
            return "length";
        case "STRING":
            return "string";
        case "BOOLEAN":
            return "bool";
        default:
            return "brush"; // Default to brush
    }
}

function isVariableAlias(value: any): boolean {
    return (
        value && typeof value === "object" && value.type === "VARIABLE_ALIAS"
    );
}

function getVariableReference(
    value: any,
    variableRefMap: Map<string, VariableReference>,
): string {
    return variableRefMap.get(value.id)?.path || "";
}

function formatValueForSlint(variable: VariableData, value: any): string {
    const slintType = getSlintType(variable);
    switch (slintType) {
        case "string":
            return `${indent2}${variable.name}: "${value}",\n`;
        case "bool":
            return `${indent2}${variable.name}: ${value},\n`;
        case "brush":
            if (
                value &&
                typeof value === "object" &&
                "r" in value &&
                "g" in value &&
                "b" in value
            ) {
                return `${indent2}${variable.name}: ${rgbToHex(value)},\n`;
            }
            return `// unable to convert ${variable.name} to brush,\n`;
        case "length":
            return `${indent2}${variable.name}: ${value}px,\n`;
        case "float":
            return `${indent2}${variable.name}: ${Number(value).toFixed(1)},\n`;
        case "int":
            return `${indent2}${variable.name}: ${value},\n`;
        default:
            return `${indent2}${variable.name}: ${value},\n`;
    }
}

export function generateVariableValue(
    variable: VariableData,
    value: any,
    collectionName: string,
    collectionDefaultModes: Map<string, string>,
    variableRefMap: Map<string, VariableReference>,
): string {
    if (isVariableAlias(value)) {
        // Figma allows designers to go wild with variables the reference other variables or even other
        // references. This quickly leads to binding loops in this current export. This function simplifies the
        // problem by allowing a single variable in another struct. If the variable references the current struct
        // or another reference the alias chain is simply resolved to a final value based on defaultModeId's
        const variableAlias = getVariableReference(value, variableRefMap);
        const aliasCollection = variableAlias.split(".")[0];
        if (variableAlias) {
            if (aliasCollection === collectionName) {
                return followAliasChain(
                    variable,
                    value,
                    variableRefMap,
                    collectionDefaultModes,
                );
            }
            // check if next item is value or alias
            const nextVariable = variableRefMap.get(value.id)?.variable;
            if (nextVariable) {
                // Check all values in valuesByMode for variable aliases
                if (nextVariable.valuesByMode) {
                    for (const [_modeId, modeValue] of Object.entries(
                        nextVariable.valuesByMode,
                    )) {
                        if (isVariableAlias(modeValue)) {
                            return followAliasChain(
                                variable,
                                value,
                                variableRefMap,
                                collectionDefaultModes,
                            );
                        }
                    }
                }
            }

            return `${indent2}${variable.name}: ${variableAlias},\n`;
        } else {
            return handleDeletedVariable(variable);
        }
    } else {
        return formatValueForSlint(variable, value);
    }
}

function generateVariablesForMode(
    variables: VariableData[],
    modeId: string,
    collectionName: string,
    collectionDefaultModes: Map<string, string>,
    variableRefMap: Map<string, VariableReference>,
): string {
    let result = "";
    for (const variable of variables) {
        let value = variable.valuesByMode[modeId];
        // If value is undefined this might be a variable that shares a single value with all modes
        if (
            value === undefined &&
            Object.keys(variable.valuesByMode).length > 0
        ) {
            const firstModeId = Object.keys(variable.valuesByMode)[0];
            value = variable.valuesByMode[firstModeId];
        }
        result += generateVariableValue(
            variable,
            value,
            collectionName,
            collectionDefaultModes,
            variableRefMap,
        );
    }
    result += `${indent}};\n\n`;
    return result;
}

function handleDeletedVariable(variable: VariableData): string {
    const slintType = getSlintType(variable);
    let defaultValue = "";
    switch (slintType) {
        case "string":
            defaultValue = '""';
            break;
        case "bool":
            defaultValue = "false";
            break;
        case "brush":
            defaultValue = "#000000";
            break;
        case "length":
            defaultValue = "0px";
            break;
        case "float":
            defaultValue = "0.0";
            break;
        case "int":
            defaultValue = "0";
            break;
        default:
            defaultValue = "0";
    }
    return `// Figma file is pointing at a deleted Variable "${variable.name}"\n${indent2}${variable.name}: ${defaultValue},\n`;
}

function followAliasChain(
    variable: VariableData,
    value: any,
    variableRefMap: Map<string, VariableReference>,
    collectionDefaultModes: Map<string, string>,
): string {
    if (isVariableAlias(value)) {
        // get the next variable in the chain
        const nextVariable = variableRefMap.get(value.id)?.variable;
        if (nextVariable) {
            const nextValue =
                nextVariable.valuesByMode[
                    collectionDefaultModes.get(
                        nextVariable.variableCollectionId,
                    )!
                ];
            if (isVariableAlias(nextValue)) {
                return followAliasChain(
                    variable,
                    nextValue,
                    variableRefMap,
                    collectionDefaultModes,
                );
            } else {
                return formatValueForSlint(variable, nextValue);
            }
        } else {
            return handleDeletedVariable(variable);
        }
    }
    return formatValueForSlint(variable, value);
}
