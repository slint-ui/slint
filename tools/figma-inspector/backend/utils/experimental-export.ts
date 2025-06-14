// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { dispatchTS } from "./code-utils.js";
import { rgbToHex } from "./property-parsing.js";
import type {
    CollectionId,
    VariableId,
    VariableCollectionSU,
    VariableSU,
} from "../../shared/custom-figma-types.d.ts";

interface ProcessedCollection {
    id: string;
    name: string;
    defaultModeId: string;
    hiddenFromPublishing: boolean;
    modes: Array<{
        modeId: string;
        name: string;
    }>;
    variables: Variable[];
}

interface VariableCollectionOld {
    id: string;
    name: string;
    defaultModeId: string;
    hiddenFromPublishing: boolean;
    modes: Array<{
        modeId: string;
        name: string;
    }>;
    variables: Variable[];
}

interface VariableReference {
    path?: string;
    variable: Variable;
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
        const collections =
            await figma.variables.getLocalVariableCollectionsAsync();

        const allVariables = await figma.variables.getLocalVariablesAsync();
        const variablesByCollection = new Map<string, Variable[]>();
        for (const variable of allVariables) {
            const collectionId = variable.variableCollectionId;
            if (!collectionId) {
                continue;
            }

            if (!variablesByCollection.has(collectionId)) {
                variablesByCollection.set(collectionId, []);
            }
            // Ensure all required properties exist with defaults
            const safeVariable = {
                id: variable.id || "",
                name: variable.name || "",
                variableCollectionId: collectionId,
                resolvedType: variable.resolvedType || "STRING",
                valuesByMode: variable.valuesByMode || {},
                hiddenFromPublishing: variable.hiddenFromPublishing ?? false,
                scopes: variable.scopes || [],
            };

            const vars = variablesByCollection.get(collectionId);
            if (vars && collectionId) {
                vars.push(safeVariable as Variable);
            } else {
                console.log("Collection ID not found", collectionId);
            }
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

function createVariableCollectionSU(
    collection: VariableCollection,
): VariableCollectionSU {
    return {
        id: collection.id,
        defaultModeId: collection.defaultModeId,
        name: collection.name,
        hiddenFromPublishing: collection.hiddenFromPublishing,
        modes: collection.modes,
        variableIds: collection.variableIds,
        variables: new Map<VariableId, VariableSU>(),
    } as VariableCollectionSU;
}

function createVariableSU(variable: Variable): VariableSU {
    return {
        id: variable.id,
        name: variable.name,
        variableCollectionId: variable.variableCollectionId,
        resolvedType: variable.resolvedType,
        valuesByMode: variable.valuesByMode,
        scopes: variable.scopes || [],
    } as VariableSU;
}

async function handleDeletedVariable(
    id: VariableId,
    variableMap: Map<VariableId, VariableSU>,
    collectionsMap: Map<CollectionId, VariableCollectionSU>
): Promise<void> {
    //TODO: Support reporting the deleted variables and collections via the included readme.txt
    const variable = await figma.variables.getVariableByIdAsync(id);
    if (!variable) { return; }

    const collectionId = variable.variableCollectionId as CollectionId;
    const collection = collectionsMap.get(collectionId);

    // If collection exists, just add the variable
    if (collection) {
        const newVariable = createVariableSU(variable);
        newVariable.name = variable.name + "_DELETED";
        variableMap.set(newVariable.id, newVariable);
        collection.variables.set(newVariable.id, newVariable);
        return;
    }

    // Collection doesn't exist, need to recreate it
    const deletedCollection = await figma.variables.getVariableCollectionByIdAsync(collectionId);
    if (!deletedCollection) { return }

    const newCollection = createVariableCollectionSU(deletedCollection);
    newCollection.name = deletedCollection.name + "_DELETED";
    collectionsMap.set(newCollection.id, newCollection);

    // Add all variables from the deleted collection
    for (const variableId of deletedCollection.variableIds) {
        const v = await figma.variables.getVariableByIdAsync(variableId);
        if (v) {
            const newVariable = createVariableSU(v);
            newVariable.name = v.name + "_DELETED";
            variableMap.set(newVariable.id, newVariable);
            newCollection.variables.set(newVariable.id, newVariable);
        }
    }

    // Just in-case this variable is missing from the collection, add it
    if (!newCollection.variables.has(id)) {
        const newVariable = createVariableSU(variable);
        variableMap.set(newVariable.id, newVariable);
        newCollection.variables.set(newVariable.id, newVariable);
    }
}

async function processVariableAliases(
    collectionsMap: Map<CollectionId, VariableCollectionSU>,
    variableMap: Map<VariableId, VariableSU>
): Promise<void> {
    for (const collection of collectionsMap.values()) {
        for (const variable of collection.variables.values()) {
            for (const value of Object.values(variable.valuesByMode)) {
                if (!isVariableAlias(value)) { continue; }
                const id = (value as VariableAlias).id as VariableId;
                if (!variableMap.has(id)) {
                    await handleDeletedVariable(id, variableMap, collectionsMap);
                }
            }
        }
    }
}

export async function createVariableCollections(): Promise<
    Map<CollectionId, VariableCollectionSU>
> {
    try {
        const [collections, allVariables] = await Promise.all([
            figma.variables.getLocalVariableCollectionsAsync(),
            figma.variables.getLocalVariablesAsync(),
        ]);

        const collectionsMap = new Map<CollectionId, VariableCollectionSU>();
        const variableMap = new Map<VariableId, VariableSU>();

        // Create collections and add variables
        for (const collection of collections) {
            const newCollection = createVariableCollectionSU(collection);
            collectionsMap.set(newCollection.id, newCollection);
        }

        for (const variable of allVariables) {
            const collectionId = variable.variableCollectionId as CollectionId;
            if (!collectionId) { continue; }

            const safeVariable = createVariableSU(variable);
            variableMap.set(safeVariable.id, safeVariable);

            const collection = collectionsMap.get(collectionId);
            if (collection) {
                collection.variables.set(safeVariable.id, safeVariable);
            }
        }

        // Handle any deleted variables referenced by aliases
        await processVariableAliases(collectionsMap, variableMap);

        return collectionsMap;
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

        // Build a map of variable IDs to their references and data
        const variableRefMap = new Map<string, VariableReference>();
        const collectionDefaultModes = new Map<string, string>();
        for (const collection of sanitizedCollections) {
            collectionDefaultModes.set(collection.id, collection.defaultModeId);
            for (const variable of collection.variables) {
                variableRefMap.set(variable.id, {
                    path: `${collection.name}.vars.${variable.name}`,
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
            const structName = `Vars${collectionCount}`;
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
                const slintType = getSlintTypeInfo(variable).type;
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
            const structName = `Vars${collectionIndex}`;

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
                allSlintCode += `${indent}out property <${structName}> vars: `;
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
                    allSlintCode += await generateVariablesForMode(
                        collection.variables,
                        mode.modeId,
                        collection.name,
                        collectionDefaultModes,
                        variableRefMap,
                    );
                }
            } else {
                // For collections with only one mode, just create a simple property
                allSlintCode += `${indent}out property <${structName}> vars: {\n`;
                allSlintCode += await generateVariablesForMode(
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
): VariableCollectionOld[] {
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

export async function saveVariableCollectionsToFile(): Promise<string> {
    try {
        const start = Date.now();
        const variableCollection = await createVariableCollections();
        console.log("createVariableCollections took", Date.now() - start, "ms");

        // Convert the Map to an array of collections, with variables as arrays
        const serializedCollections = Array.from(variableCollection.values()).map(collection => ({
            ...collection,
            variables: Array.from(collection.variables.values())
        }));

        return JSON.stringify(serializedCollections, null, 2);
    } catch (error) {
        console.error("Error saving variable collections:", error);
        return JSON.stringify({ error: String(error) });
    }
}

function getSlintTypeInfo(variable: Variable): {
    type: string;
    defaultValue: string;
} {
    switch (variable.resolvedType) {
        case "COLOR":
            return { type: "brush", defaultValue: "#000000" };
        case "FLOAT":
            // Filter out FONT_VARIATIONS as it can be ignored
            const relevantScopes = variable.scopes.filter(
                (scope) => scope !== ("FONT_VARIATIONS" as VariableScope),
            );
            if (relevantScopes.length === 1) {
                if (relevantScopes[0] === "OPACITY") {
                    return { type: "float", defaultValue: "0.0" };
                }
            }
            // If it's ALL_SCOPES or no specific scope matches, return length
            return { type: "length", defaultValue: "0px" };
        case "STRING":
            return { type: "string", defaultValue: '""' };
        case "BOOLEAN":
            return { type: "bool", defaultValue: "false" };
        default:
            return { type: "brush", defaultValue: "#000000" }; // Default to brush
    }
}

function isVariableAlias(value: any): boolean {
    return (
        value && typeof value === "object" && value.type === "VARIABLE_ALIAS"
    );
}

function formatValueForSlint(variable: Variable, value: any): string {
    const slintType = getSlintTypeInfo(variable).type;
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

export async function generateVariableValue(
    variable: Variable,
    value: any,
    collectionName: string,
    collectionDefaultModes: Map<string, string>,
    variableRefMap: Map<string, VariableReference>,
): Promise<string> {
    if (isVariableAlias(value)) {
        // Figma allows designers to go wild with variables the reference other variables or even other
        // references. This quickly leads to binding loops in this current export. This function simplifies the
        // problem by allowing a single variable in another struct. If the variable references the current struct
        // or another reference the alias chain is simply resolved to a final value based on defaultModeId's
        // if it has no path it probably a variable from a deleted collection that handleDeadEndValue has recreated.
        const variableFromAlias = variableRefMap.get(value.id);
        if (variableFromAlias && variableFromAlias.path === undefined) {
            const key0 = Object.keys(
                variableFromAlias.variable.valuesByMode,
            )[0];
            const value = variableFromAlias.variable.valuesByMode[key0];
            return formatValueForSlint(variable, value);
        }

        const variableAlias = variableFromAlias?.path;
        const aliasCollection = variableFromAlias?.path?.split(".")[0];
        if (variableAlias) {
            if (aliasCollection === collectionName) {
                return await followAliasChain(
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
                            return await followAliasChain(
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
            return await handleDeadEndValue(
                variable,
                value,
                collectionDefaultModes,
                variableRefMap,
            );
        }
    } else {
        return formatValueForSlint(variable, value);
    }
}

async function generateVariablesForMode(
    variables: Variable[],
    modeId: string,
    collectionName: string,
    collectionDefaultModes: Map<string, string>,
    variableRefMap: Map<string, VariableReference>,
): Promise<string> {
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
        result += await generateVariableValue(
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

// Figma allows you to delete variables and even collections that other variables reference. When this happens
// figma.variables.getLocalVariableCollectionsAsync() will be missing those items. This function then uses
// figma.variables.getVariableByIdAsync() to get the variable and then uses the valuesByMode to get the value.
// It then puts a reference in the variableRefMap to save constant potential calls to figma.variables.getVariableByIdAsync()
async function handleDeadEndValue(
    variable: Variable,
    value: any,
    collectionDefaultModes: Map<string, string>,
    variableRefMap: Map<string, VariableReference>,
): Promise<string> {
    const v = await figma.variables.getVariableByIdAsync(value.id);

    if (v) {
        if (!variableRefMap.has(v.id)) {
            variableRefMap.set(v.id, {
                variable: v,
            });
        }
        const collectionId = v.variableCollectionId;
        const defaultModeId = collectionDefaultModes.get(collectionId);
        if (defaultModeId !== undefined) {
            const newValue = v.valuesByMode[defaultModeId];
            if (newValue !== undefined) {
                return formatValueForSlint(variable, newValue);
            }
        }
        const anyValue = v.valuesByMode[Object.keys(v.valuesByMode)[0]];
        if (anyValue !== undefined) {
            return formatValueForSlint(variable, anyValue);
        }
    }
    const { defaultValue } = getSlintTypeInfo(variable);
    return `// Figma file is pointing at a deleted Variable "${variable.name}"\n${indent2}${variable.name}: ${defaultValue},\n`;
}

async function followAliasChain(
    variable: Variable,
    value: any,
    variableRefMap: Map<string, VariableReference>,
    collectionDefaultModes: Map<string, string>,
): Promise<string> {
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
                return await followAliasChain(
                    variable,
                    nextValue,
                    variableRefMap,
                    collectionDefaultModes,
                );
            } else {
                return formatValueForSlint(variable, nextValue);
            }
        } else {
            return await handleDeadEndValue(
                variable,
                value,
                collectionDefaultModes,
                variableRefMap,
            );
        }
    }
    return formatValueForSlint(variable, value);
}
