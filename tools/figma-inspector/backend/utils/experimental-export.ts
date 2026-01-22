// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { dispatchTS } from "./code-utils.js";
import { rgbToHex } from "./property-parsing.js";
import type {
    CollectionId,
    VariableId,
    VariableCollectionSU,
    VariableSU,
    VariableAliasSU,
    CollectionsMap,
} from "../../shared/custom-figma-types.d.ts";

export const indent = "    ";
export const indent2 = indent + indent;

// Not all api data is collected. The following properties are not included:
// - variable.key
// - variable.remote
// - variable.description // Useful for comments in the code.
// - variable.codeSyntax // This might be useful in the future as it allows figma to give
// an actual name for the variable to be used in code. However its for CSS and Swift only right now.
export async function createVariableCollections(): Promise<CollectionsMap> {
    try {
        const [collections, allVariables] = await Promise.all([
            figma.variables.getLocalVariableCollectionsAsync(),
            figma.variables.getLocalVariablesAsync(),
        ]);

        const collectionsMap: CollectionsMap = new Map<
            CollectionId,
            VariableCollectionSU
        >();

        // Create collections and add variables
        for (const collection of collections) {
            const newCollection = createVariableCollectionSU(collection);
            collectionsMap.set(newCollection.id, newCollection);
        }

        for (const variable of allVariables) {
            const collectionId = variable.variableCollectionId as CollectionId;
            if (!collectionId) {
                continue;
            }

            const safeVariable = createVariableSU(variable);

            const collection = collectionsMap.get(collectionId);
            if (collection) {
                collection.variables.set(safeVariable.id, safeVariable);
            }
        }

        // Handle any deleted variables referenced by aliases
        await processVariableAliases(collectionsMap);

        return collectionsMap;
    } catch (error) {
        console.error("Error processing variable collections:", error);
        throw error;
    }
}

export async function createSlintExport(): Promise<void> {
    try {
        const start = Date.now();
        const collectionsMap = await createVariableCollections();

        sanitizeCollections(collectionsMap);

        let allSlintCode = "";
        let collectionCount = 1;

        for (const collection of collectionsMap.values()) {
            if (collection.variables.size === 0) {
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
            for (const variable of collection.variables.values()) {
                const slintType = getSlintTypeInfo(variable);
                allSlintCode += `${indent}${variable.name}: ${slintType},\n`;
            }
            allSlintCode += `}\n\n`;
        }

        // Create a global for each collection
        let collectionIndex = 1;
        for (const collection of collectionsMap.values()) {
            if (collection.variables.size === 0) {
                continue;
            }

            const enumName = `Mode${collectionIndex}`;
            const structName = `Vars${collectionIndex}`;

            collectionIndex++;
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
                    allSlintCode += generateVariablesForMode(
                        Array.from(collection.variables.values()),
                        mode.modeId,
                        collection.name,
                        collectionsMap,
                    );
                }
            } else {
                // For collections with only one mode, just create a simple property
                allSlintCode += `${indent}out property <${structName}> vars: {\n`;
                allSlintCode += generateVariablesForMode(
                    Array.from(collection.variables.values()),
                    collection.modes[0].modeId,
                    collection.name,
                    collectionsMap,
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
    collectionsMap: CollectionsMap,
): Promise<void> {
    //TODO: Support reporting the deleted variables and collections via the included readme.txt
    const variable = await figma.variables.getVariableByIdAsync(id);
    if (!variable) {
        return;
    }

    const collectionId = variable.variableCollectionId as CollectionId;
    const collection = collectionsMap.get(collectionId);

    // If collection exists, just add the variable
    if (collection) {
        const newVariable = createVariableSU(variable);
        newVariable.name = variable.name + "_DELETED";
        collection.variables.set(newVariable.id, newVariable);
        return;
    }

    // Collection doesn't exist, need to recreate it
    const deletedCollection =
        await figma.variables.getVariableCollectionByIdAsync(collectionId);
    if (!deletedCollection) {
        return;
    }

    const newCollection = createVariableCollectionSU(deletedCollection);
    newCollection.name = deletedCollection.name + "_DELETED";
    collectionsMap.set(newCollection.id, newCollection);

    // Add all variables from the deleted collection
    for (const variableId of deletedCollection.variableIds) {
        const v = await figma.variables.getVariableByIdAsync(variableId);
        if (v) {
            const newVariable = createVariableSU(v);
            newVariable.name = v.name + "_DELETED";
            newCollection.variables.set(newVariable.id, newVariable);
        }
    }

    // Just in-case this variable is missing from the collection, add it
    if (!newCollection.variables.has(id)) {
        const newVariable = createVariableSU(variable);
        newCollection.variables.set(newVariable.id, newVariable);
    }
}

async function processVariableAliases(
    collectionsMap: CollectionsMap,
): Promise<void> {
    for (const collection of collectionsMap.values()) {
        for (const variable of collection.variables.values()) {
            if (Object.values(variable.valuesByMode).length === 0) {
                console.log(
                    "Unexpected error! Variable has no values",
                    variable.name,
                    variable.id,
                );
            }
            for (const value of Object.values(variable.valuesByMode)) {
                if (!isVariableAlias(value)) {
                    continue;
                }
                const id = value.id;
                if (!variableFromId(id, collectionsMap)) {
                    await handleDeletedVariable(id, collectionsMap);
                }
            }
        }
    }
}

function variableFromId(
    id: VariableId,
    collectionsMap: CollectionsMap,
): VariableSU | undefined {
    for (const collection of collectionsMap.values()) {
        const variable = collection.variables.get(id);
        if (variable) {
            return variable;
        }
    }
    return undefined;
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

function sanitizeCollections(collectionsMap: CollectionsMap): CollectionsMap {
    for (const collection of collectionsMap.values()) {
        collection.name = sanitizeSlintPropertyName(collection.name);

        for (const mode of collection.modes) {
            mode.name = sanitizeSlintPropertyName(mode.name);
        }

        for (const variable of collection.variables.values()) {
            variable.name = sanitizeSlintPropertyName(variable.name);
        }
    }
    return collectionsMap;
}

export async function saveVariableCollectionsToFile(): Promise<string> {
    try {
        const start = Date.now();
        const collectionsMap = await createVariableCollections();
        console.log("createVariableCollections took", Date.now() - start, "ms");

        // Convert the Map to an array of collections, with variables as arrays
        const serializedCollections = Array.from(collectionsMap.values()).map(
            (collection) => ({
                ...collection,
                variables: Array.from(collection.variables.values()),
            }),
        );

        return JSON.stringify(serializedCollections, null, 2);
    } catch (error) {
        console.error("Error saving variable collections:", error);
        return JSON.stringify({ error: String(error) });
    }
}

function getSlintTypeInfo(variable: VariableSU): string {
    switch (variable.resolvedType) {
        case "COLOR":
            return "brush";
        case "FLOAT":
            // Filter out FONT_VARIATIONS as it can be ignored
            const relevantScopes = variable.scopes.filter(
                (scope) => scope !== ("FONT_VARIATIONS" as VariableScope),
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

function isVariableAlias(value: VariableValue): value is VariableAliasSU {
    return (
        value !== null &&
        typeof value === "object" &&
        "type" in value &&
        value.type === "VARIABLE_ALIAS" &&
        "id" in value
    );
}

function formatValueForSlint(
    variable: VariableSU,
    value: VariableValue,
): string {
    const slintType = getSlintTypeInfo(variable);
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

export function createPath(
    variable: VariableSU,
    collectionsMap: CollectionsMap,
): string {
    const collectionName = collectionsMap.get(
        variable.variableCollectionId,
    )?.name;
    return `${collectionName}.vars.${variable.name}`;
}

export function generateVariableValue(
    variable: VariableSU,
    value: VariableValue,
    collectionName: string,
    collectionsMap: CollectionsMap,
): string {
    if (isVariableAlias(value)) {
        // Figma allows designers to go wild with variables the reference other variables or even other
        // references. This quickly leads to binding loops in this current export. This function simplifies the
        // problem by allowing a single variable in another struct. If the variable references the current struct
        // or another reference the alias chain is simply resolved to a final value based on defaultModeId's
        // if it has no path it probably a variable from a deleted collection that handleDeadEndValue has recreated.
        const variableFromAlias = variableFromId(value.id, collectionsMap);

        if (variableFromAlias) {
            const variablesCollectionName = collectionsMap.get(
                variableFromAlias.variableCollectionId,
            )?.name;
            if (variablesCollectionName === collectionName) {
                return followAliasChain(variable, value, collectionsMap);
            }
            // check if next item is value or alias
            const nextVariable = variableFromId(value.id, collectionsMap);
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
                                collectionsMap,
                            );
                        }
                    }
                }
            }
            const variableName = createPath(variableFromAlias, collectionsMap);
            return `${indent2}${variable.name}: ${variableName},\n`;
        } else {
            console.log(
                "The createCollections should not create data that allows this situation to happen",
            );
        }
    }
    return formatValueForSlint(variable, value);
}

function generateVariablesForMode(
    variables: VariableSU[],
    modeId: string,
    collectionName: string,
    collectionsMap: CollectionsMap,
): string {
    let result = "";
    for (const variable of variables) {
        let value = variable.valuesByMode[modeId];
        // If value is undefined this might be a variable that shares a single value with all modes
        if (!value) {
            const defaultModeId = collectionsMap.get(
                variable.variableCollectionId,
            )?.defaultModeId;
            value = variable.valuesByMode[defaultModeId!];
        }

        result += generateVariableValue(
            variable,
            value,
            collectionName,
            collectionsMap,
        );
    }
    result += `${indent}};\n\n`;
    return result;
}

function followAliasChain(
    variable: VariableSU,
    value: VariableValue,
    collectionsMap: CollectionsMap,
): string {
    if (isVariableAlias(value)) {
        // get the next variable in the chain
        const nextVariable = variableFromId(value.id, collectionsMap);

        if (nextVariable) {
            const defaultModeId = collectionsMap.get(
                nextVariable.variableCollectionId,
            )?.defaultModeId;
            const nextValue = nextVariable.valuesByMode[defaultModeId!];
            if (isVariableAlias(nextValue)) {
                return followAliasChain(variable, nextValue, collectionsMap);
            } else {
                return formatValueForSlint(variable, nextValue);
            }
        } else {
            console.log(
                "followAliasChain: The createCollections should not create data that allows this situation to happen",
            );
        }
    }
    return formatValueForSlint(variable, value);
}
