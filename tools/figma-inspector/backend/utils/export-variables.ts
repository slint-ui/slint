// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Helper to convert Figma color values to Slint format
function convertColor(color: RGB | RGBA): string {
    const r = Math.round(color.r * 255);
    const g = Math.round(color.g * 255);
    const b = Math.round(color.b * 255);

    if ("a" in color) {
        if (color.a === 1) {
            return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
        } else {
            return `rgba(${r}, ${g}, ${b}, ${color.a})`;
        }
    }

    return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
}

// Helper to format struct/global name for Slint (PascalCase)
function formatStructName(name: string): string {
    return name
        .split(/[-_\s\/]/)
        .map(
            (part) =>
                part.charAt(0).toUpperCase() + part.slice(1).toLowerCase(),
        )
        .join("");
}

// Helper to format property name for Slint (kebab-case)
function formatPropertyName(name: string): string {
    return name
        .replace(/([a-z])([A-Z])/g, "$1-$2")
        .replace(/\s+/g, "-")
        .toLowerCase();
}
// Helper to format variable name for Slint (kebab-case)
function formatVariableName(name: string): string {
    return name
        .replace(/([a-z])([A-Z])/g, "$1-$2")
        .replace(/\s+/g, "-")
        .toLowerCase()
        .trim();
}

// Extract hierarchy from variable name (e.g. "colors/primary/base" → ["colors", "primary", "base"])
function extractHierarchy(name: string): string[] {
    // Split by common hierarchy separators
    const parts = name.split(/\/|\.|:|--|-(?=[a-z])/);
    return parts.map((part) => formatVariableName(part));
}

// Create a path string for referencing variables
function createVariablePath(path: string[]): string {
    return path.join(".");
}

function createSimplifiedStructure(
    variableCollections: any[],
    variableMap: Map<string, any>,
    hierarchyMap: any,
): any {
    // Create simplified collections overview with modes
    const simplifiedCollections = variableCollections.map((collection) => {
        // Extract all modes for this collection
        // Define interfaces for types
        interface InputMode {
            modeId: string;
            name: string;
        }

        interface OutputMode {
            id: string;
            name: string;
            column: number;
        }

        const modes: OutputMode[] = collection.modes.map(
            (mode: InputMode, index: number): OutputMode => ({
                id: mode.modeId,
                name: mode.name,
                column: index,
            }),
        );

        return {
            name: collection.name,
            id: collection.id,
            variableCount: collection.variableIds.length,
            modes: modes,
            defaultModeId: collection.defaultModeId,
        };
    });

    // Create simplified variable map (first 10 entries only to keep it manageable)
    const simplifiedVariables = [];
    let count = 0;
    for (const [id, data] of variableMap.entries()) {
        if (count++ > 10) break; // Limit to 10 entries

        // Extract basic variable info
        const variable = data.resolvedVariable;

        // Create a simplified record of values across all modes
        const valuesByMode: Record<string, any> = {};
        if (data.valuesByMode) {
            for (const [modeId, modeValue] of Object.entries(
                data.valuesByMode,
            )) {
                // Format value for readability
                let formattedValue = modeValue;

                if (
                    typeof modeValue === "object" &&
                    modeValue &&
                    "r" in modeValue
                ) {
                    const colorValue = modeValue as {
                        r: number;
                        g: number;
                        b: number;
                    };
                    formattedValue = `Color(R:${Math.round(colorValue.r * 255)}, G:${Math.round(colorValue.g * 255)}, B:${Math.round(colorValue.b * 255)})`;
                } else if (
                    typeof modeValue === "object" &&
                    modeValue &&
                    "type" in modeValue &&
                    modeValue.type === "VARIABLE_ALIAS" &&
                    "id" in modeValue
                ) {
                    formattedValue = `Reference(${modeValue.id})`;
                }

                valuesByMode[modeId] = formattedValue;
            }
        }

        simplifiedVariables.push({
            id: id.slice(0, 8) + "...", // Truncate ID
            name: variable.name,
            type: data.type,
            path: data.path,
            valuesByMode: valuesByMode,
        });
    }

    // Create the complete simplified structure
    const simplifiedStructure = {
        collectionsOverview: simplifiedCollections,
        sampleVariables: simplifiedVariables,
        hierarchy: hierarchyMap, // This is already reasonably simple
    };

    console.log(
        "Simplified Figma structure:",
        JSON.stringify(simplifiedStructure, null, 2),
    );
    return simplifiedStructure;
}

function createReferenceExpression(
    referenceId: string,
    sourceColumnName: string,
    variablePathsById: Map<string, { collection: string; row: string }>,
    variableCollections: Map<
        string,
        {
            formattedName: string;
            modes: Set<string>;
            // Other fields...
        }
    >,
): string | null {
    // Get the target variable path
    const targetPath = variablePathsById.get(referenceId);
    if (!targetPath) {
        console.warn(`Reference path not found for ID: ${referenceId}`);
        return null;
    }

    // Get the target collection
    const targetCollection = variableCollections.get(targetPath.collection);
    if (!targetCollection) {
        console.warn(`Collection not found: ${targetPath.collection}`);
        return null;
    }

    // IMPORTANT: Find equivalent mode in target collection
    // The referenced variable might use different mode names
    // Try to find a matching mode by position or by light/dark designation
    const targetModes = [...targetCollection.modes];
    let targetColumnName = targetModes[0]; // Default to first mode

    // Try to find a better match - prefer same position or light/dark match
    if (sourceColumnName.includes("light")) {
        // Find a light-like mode in target
        const lightMode = targetModes.find(
            (m) => m.includes("light") || m === "mode-1",
        );
        if (lightMode) targetColumnName = lightMode;
    } else if (sourceColumnName.includes("dark")) {
        // Find a dark-like mode in target
        const darkMode = targetModes.find((m) => m.includes("dark"));
        if (darkMode) targetColumnName = darkMode;
    }

    // Format the reference expression with PascalCase for the global name
    return `${targetCollection.formattedName}.${targetPath.row}-${targetColumnName}`;
}

// For Figma Plugin - Export function with hierarchical structure
export async function exportFigmaVariablesToSlint(): Promise<string> {
    try {
        // Get collections asynchronously
        const variableCollections =
            await figma.variables.getLocalVariableCollectionsAsync();

        // Initialize code output
        let slintCode = `// Generated from Figma variables\n\n`;

        // Track collections and their structure
        const collectionStructure = new Map<
            string,
            {
                name: string;
                formattedName: string;
                modes: Set<string>;
                variables: Map<
                    string,
                    Map<string, { value: string; type: string; refId?: string }>
                >;
            }
        >();

        // Create a map of all variable IDs to their actual values (for resolving references)
        const variableValuesById = new Map<
            string,
            Map<string, { value: string; type: string }>
        >();
        const variableNameById = new Map<string, string>();

        // NEW: Track where each variable ID will appear in the generated code
        const variablePathsById = new Map<
            string,
            { collection: string; row: string }
        >();

        // First pass: collect all variables and store references
        for (const collection of variableCollections) {
            const collectionName = formatPropertyName(collection.name);
            const formattedCollectionName = formatStructName(collection.name);

            // Skip empty collections
            if (!collection.variableIds || collection.variableIds.length === 0)
                continue;

            // Initialize collection structure
            if (!collectionStructure.has(collectionName)) {
                collectionStructure.set(collectionName, {
                    name: collection.name,
                    formattedName: formattedCollectionName,
                    modes: new Set<string>(),
                    variables: new Map<
                        string,
                        Map<
                            string,
                            { value: string; type: string; refId?: string }
                        >
                    >(),
                });
            }

            // Add modes to collection
            collection.modes.forEach((mode) => {
                collectionStructure
                    .get(collectionName)!
                    .modes.add(formatPropertyName(mode.name));
            });

            // Process variables in batches
            const batchSize = 5;
            for (let i = 0; i < collection.variableIds.length; i += batchSize) {
                const batch = collection.variableIds.slice(i, i + batchSize);
                const batchPromises = batch.map((id) =>
                    figma.variables.getVariableByIdAsync(id),
                );
                const batchResults = await Promise.all(batchPromises);

                for (const variable of batchResults) {
                    if (!variable) continue;
                    if (
                        !variable.valuesByMode ||
                        Object.keys(variable.valuesByMode).length === 0
                    )
                        continue;

                    // Store variable name by ID for later reference resolution
                    variableNameById.set(variable.id, variable.name);

                    // Initialize variable in valuesByID map
                    if (!variableValuesById.has(variable.id)) {
                        variableValuesById.set(
                            variable.id,
                            new Map<string, { value: string; type: string }>(),
                        );
                    }

                    // Use extractHierarchy to break up variable names
                    const nameParts = extractHierarchy(variable.name);
                    const propertyName =
                        nameParts.length > 0
                            ? nameParts[nameParts.length - 1]
                            : formatPropertyName(variable.name);

                    const path =
                        nameParts.length > 1
                            ? nameParts.slice(0, -1).join("_")
                            : "";

                    const rowName = path
                        ? `${path}_${propertyName}`
                        : propertyName;

                    // NEW: Store the path to this variable for reference lookup
                    variablePathsById.set(variable.id, {
                        collection: collectionName,
                        row: rowName,
                    });

                    // Initialize row in variables map
                    if (
                        !collectionStructure
                            .get(collectionName)!
                            .variables.has(rowName)
                    ) {
                        collectionStructure.get(collectionName)!.variables.set(
                            rowName,
                            new Map<
                                string,
                                {
                                    value: string;
                                    type: string;
                                    refId?: string;
                                }
                            >(),
                        );
                    }

                    // Process values for each mode
                    for (const [modeId, value] of Object.entries(
                        variable.valuesByMode,
                    )) {
                        const modeInfo = collection.modes.find(
                            (m) => m.modeId === modeId,
                        );
                        if (!modeInfo) continue;

                        const modeName = formatPropertyName(modeInfo.name);

                        // Format value and track references
                        let formattedValue = "";
                        let refId: string | undefined;

                        if (variable.resolvedType === "COLOR") {
                            if (
                                typeof value === "object" &&
                                value &&
                                "r" in value
                            ) {
                                formattedValue = convertColor(value);
                            } else if (
                                typeof value === "object" &&
                                value &&
                                value.type === "VARIABLE_ALIAS"
                            ) {
                                // Store reference ID for later reference preservation (not resolution)
                                refId = value.id;
                                formattedValue = `@ref:${value.id}`;
                            }
                        } else if (variable.resolvedType === "FLOAT") {
                            formattedValue = `${value}px`;
                        } else if (variable.resolvedType === "STRING") {
                            formattedValue = `"${value}"`;
                        }

                        // Store in variable value map (for reference resolution)
                        variableValuesById.get(variable.id)!.set(modeName, {
                            value: formattedValue,
                            type: variable.resolvedType,
                        });

                        // Store in collection structure with reference ID if present
                        collectionStructure
                            .get(collectionName)!
                            .variables.get(rowName)!
                            .set(modeName, {
                                value: formattedValue,
                                type: variable.resolvedType,
                                refId: refId,
                            });
                    }
                }

                // Force GC between batches
                await new Promise((resolve) => setTimeout(resolve, 0));
            }
        }

        // We'll use the global createReferenceExpression function defined earlier in the file

        // Second pass: preserve references with correct formatting
        for (const [
            collectionKey,
            collection,
        ] of collectionStructure.entries()) {
            for (const [rowName, columns] of collection.variables.entries()) {
                for (const [colName, data] of columns.entries()) {
                    if (data.refId) {
                        // Use the improved reference expression function
                        const refExpression = createReferenceExpression(
                            data.refId,
                            colName,
                            variablePathsById,
                            collectionStructure,
                        );

                        if (refExpression) {
                            // Update with reference expression instead of resolved value
                            collectionStructure
                                .get(collectionKey)!
                                .variables.get(rowName)!
                                .set(colName, {
                                    value: refExpression,
                                    type: data.type,
                                    refId: data.refId,
                                });
                        } else {
                            // Couldn't create reference, use a placeholder
                            console.warn(
                                `Couldn't create reference expression for: ${data.refId} for ${rowName}-${colName}`,
                            );
                            collectionStructure
                                .get(collectionKey)!
                                .variables.get(rowName)!
                                .set(colName, {
                                    value:
                                        data.type === "COLOR"
                                            ? "#808080"
                                            : data.type === "FLOAT"
                                              ? "0px"
                                              : '""',
                                    type: data.type,
                                });
                        }
                    }
                }
            }
        }

        // Third pass: generate code
        for (const [
            collectionKey,
            collection,
        ] of collectionStructure.entries()) {
            // Only generate if there are variables
            if (collection.variables.size === 0) continue;

            // Convert modes to an array for consistent indexing
            const modes = [...collection.modes];

            // 1. Generate enum for columns (modes)
            slintCode += `// ${collection.name} Modes\n`;
            slintCode += `export enum ${collection.formattedName}Column {\n`;
            modes.forEach((mode) => {
                slintCode += `    ${mode},\n`;
            });
            slintCode += `}\n\n`;

            // 2. Generate global table
            slintCode += `// ${collection.name} Variables\n`;
            slintCode += `export global ${collection.formattedName} {\n`;

            // Current column property
            slintCode += `    in-out property <${collection.formattedName}Column> current-column: ${modes[0] || "light"};\n\n`;

            // Determine types for all variables
            const variableTypes = new Map<string, string>();
            for (const [rowName, columns] of collection.variables.entries()) {
                for (const [, data] of columns.entries()) {
                    if (!variableTypes.has(rowName)) {
                        variableTypes.set(
                            rowName,
                            data.type === "COLOR"
                                ? "color"
                                : data.type === "FLOAT"
                                  ? "length"
                                  : "string",
                        );
                    }
                    break;
                }
            }

            // 3. Add individual cell properties with references preserved
            slintCode += `    // Individual cell values\n`;
            for (const [rowName, columns] of collection.variables.entries()) {
                const rowType = variableTypes.get(rowName) || "color";

                for (const [colName, data] of columns.entries()) {
                    let valueExpression = data.value;

                    // MODIFIED: If this is a reference (still has refId), add a comment explaining
                    if (data.refId) {
                        const refName =
                            variableNameById.get(data.refId) || data.refId;
                        valueExpression = `${data.value} /* Reference to ${refName} */`;
                    }

                    slintCode += `    out property <${rowType}> ${rowName}-${colName}: ${valueExpression};\n`;
                }
            }

            // 4. Generate row accessor functions
            slintCode += `\n    // Row accessor functions\n`;
            for (const [rowName, columns] of collection.variables.entries()) {
                const rowType = variableTypes.get(rowName) || "color";

                slintCode += `    function ${rowName}(column: ${collection.formattedName}Column) -> ${rowType} {\n`;
                slintCode += `        if (`;

                let isFirst = true;
                for (const [colName] of columns.entries()) {
                    if (!isFirst) slintCode += `} else if (`;
                    slintCode += `column == ${collection.formattedName}Column.${colName}`;
                    if (isFirst) isFirst = false;

                    slintCode += `) {\n`;
                    // Function returns property directly - references are preserved
                    slintCode += `            return ${rowName}-${colName};\n`;
                    slintCode += `        `;
                }

                // Default case using first column
                const firstCol = [...columns.keys()][0];
                slintCode += `} else {\n`;
                slintCode += `            return ${rowName}-${firstCol};\n`;
                slintCode += `        }\n`;
                slintCode += `    }\n`;
            }

            // 5. Generate current value properties
            slintCode += `\n    // Current values based on current-column\n`;
            for (const [rowName] of collection.variables.entries()) {
                const rowType = variableTypes.get(rowName) || "color";
                slintCode += `    out property <${rowType}> current-${rowName}: ${rowName}(self.current-column);\n`;
            }

            slintCode += `}\n\n`;
        }

        return slintCode;
    } catch (error) {
        console.error("Error in exportFigmaVariablesToSlint:", error);
        return `// Error generating variables: ${error}`;
    }
}
// Helper function to resolve variable references
// Improved reference resolution function with better debugging and more flexible mode matching
function resolveReference(
    referenceId: string,
    modeName: string,
    variableValuesById: Map<
        string,
        Map<string, { value: string; type: string }>
    >,
    visited: Set<string>,
): { value: string; type: string } | null {
    // Check for circular references
    if (visited.has(referenceId)) {
        console.warn("Circular reference detected:", referenceId);
        return null;
    }

    visited.add(referenceId);

    // Debug logging
    console.log(`Resolving reference: ${referenceId} for mode: ${modeName}`);

    // Get the target variable values
    const targetValues = variableValuesById.get(referenceId);
    if (!targetValues) {
        console.warn(`Reference ID not found in variable map: ${referenceId}`);
        return null;
    }

    // Log available modes to debug
    console.log(
        `Available modes for this reference:`,
        Array.from(targetValues.keys()),
    );

    // Get the value for this exact mode
    let modeValue = targetValues.get(modeName);

    // If exact mode not found, try alternative mode matching strategies
    if (!modeValue) {
        console.log(
            `Mode "${modeName}" not found directly, trying alternatives...`,
        );

        // Strategy 1: Try case-insensitive matching
        for (const [availableMode, value] of targetValues.entries()) {
            if (availableMode.toLowerCase() === modeName.toLowerCase()) {
                console.log(
                    `Found matching mode with different case: ${availableMode}`,
                );
                modeValue = value;
                break;
            }
        }

        // Strategy 2: If "light" or "dark" are in the name, try variations
        if (!modeValue) {
            if (modeName.includes("light")) {
                for (const [availableMode, value] of targetValues.entries()) {
                    if (availableMode.includes("light")) {
                        console.log(
                            `Found alternative light mode: ${availableMode}`,
                        );
                        modeValue = value;
                        break;
                    }
                }
            } else if (modeName.includes("dark")) {
                for (const [availableMode, value] of targetValues.entries()) {
                    if (availableMode.includes("dark")) {
                        console.log(
                            `Found alternative dark mode: ${availableMode}`,
                        );
                        modeValue = value;
                        break;
                    }
                }
            }
        }

        // Strategy 3: Fall back to the first available mode as last resort
        if (!modeValue && targetValues.size > 0) {
            const firstMode = Array.from(targetValues.keys())[0];
            console.log(`Falling back to first available mode: ${firstMode}`);
            modeValue = targetValues.get(firstMode);
        }

        // If still no match, report failure
        if (!modeValue) {
            console.warn(`No matching mode found for ${referenceId}`);
            return null;
        }
    }

    // If this is another reference, resolve it recursively
    if (modeValue.value.startsWith("@ref:")) {
        console.log(`Found nested reference: ${modeValue.value}`);
        const nestedRefId = modeValue.value.substring(5); // Remove '@ref:' prefix
        return resolveReference(
            nestedRefId,
            modeName,
            variableValuesById,
            visited,
        );
    }

    // Return the resolved value
    console.log(`Successfully resolved reference to: ${modeValue.value}`);
    return modeValue;
}

// Process a single collection for a specific mode - memory efficient approach
async function processCollectionForMode(
    collection: any,
    modeName: string,
    callback: (name: string, data: any) => void,
): Promise<void> {
    // Process variables in smaller batches
    const batchSize = 10;

    for (let i = 0; i < collection.variableIds.length; i += batchSize) {
        const batch = collection.variableIds.slice(i, i + batchSize);
        interface VariableAlias {
            type: "VARIABLE_ALIAS";
            id: string;
        }

        type VariableValue = RGB | RGBA | VariableAlias | number | string;

        interface FigmaVariable {
            name: string;
            resolvedType: "COLOR" | "FLOAT" | "STRING";
            valuesByMode: Record<string, VariableValue>;
        }

        const batchPromises: Promise<FigmaVariable | null>[] = batch.map(
            (id: string) => figma.variables.getVariableByIdAsync(id),
        );
        const batchResults = await Promise.all(batchPromises);

        for (const variable of batchResults) {
            if (!variable) continue;

            // Skip variables without values for all modes
            if (
                !variable.valuesByMode ||
                Object.keys(variable.valuesByMode).length === 0
            )
                continue;

            // Find the mode ID for this mode name
            interface VariableCollectionMode {
                name: string;
                modeId: string;
            }

            const modeInfo: VariableCollectionMode | undefined =
                collection.modes.find(
                    (m: VariableCollectionMode) =>
                        m.name.toLowerCase() === modeName.toLowerCase(),
                );
            if (!modeInfo) continue;

            const modeId = modeInfo.modeId;

            // Skip if there's no value for this mode
            if (!variable.valuesByMode[modeId]) continue;

            const value = variable.valuesByMode[modeId];

            // Use extractHierarchy to break up hierarchical names
            const nameParts = extractHierarchy(variable.name);

            // Format the last part as the property name
            const propertyName =
                nameParts.length > 0
                    ? formatPropertyName(nameParts[nameParts.length - 1])
                    : formatPropertyName(variable.name);

            // Format the value based on type
            let formattedValue = "";
            if (variable.resolvedType === "COLOR") {
                if (typeof value === "object" && value && "r" in value) {
                    formattedValue = convertColor(value);
                } else if (
                    typeof value === "object" &&
                    value &&
                    value.type === "VARIABLE_ALIAS"
                ) {
                    // For references, we'll handle this later - store as a specially formatted string
                    formattedValue = `@ref:${value.id}`;
                }
            } else if (variable.resolvedType === "FLOAT") {
                formattedValue = `${value}px`;
            } else if (variable.resolvedType === "STRING") {
                formattedValue = `"${value}"`;
            }

            // Create a hierarchical path for this variable
            // Start with collection name, then add all parts except the last one
            const path = [formatPropertyName(collection.name)];
            for (let i = 0; i < nameParts.length - 1; i++) {
                path.push(formatPropertyName(nameParts[i]));
            }

            // Join with underscores instead of slashes
            const fullPath = path.join("_");
            // Add the last part (property name)
            const fullName = fullPath
                ? `${fullPath}_${propertyName}`
                : propertyName;

            callback(fullName, {
                value: formattedValue,
                type: variable.resolvedType,
            });
        }

        // Force a micro-task to allow garbage collection
        await new Promise((resolve) => setTimeout(resolve, 0));
    }
}
