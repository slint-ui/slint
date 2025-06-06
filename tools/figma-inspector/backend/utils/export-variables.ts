// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { rgbToHex } from "./property-parsing";

/**
 * Helper to get the appropriate Slint type for a Figma variable type
 * @param figmaType The Figma variable type ('COLOR', 'FLOAT', 'STRING', 'BOOLEAN')
 * @returns The corresponding Slint type
 */
function getSlintType(figmaType: string): string {
    switch (figmaType) {
        case "COLOR":
            return "brush";
        case "FLOAT":
            return "length";
        case "STRING":
            return "string";
        case "BOOLEAN":
            return "bool";
        default:
            return "brush"; // Default to brush
    }
}

// Helper to format struct/global name for Slint (PascalCase) with sanitization
export function formatStructName(name: string): string {
    let sanitizedName = name.startsWith(".") ? name.substring(1) : name;

    // If that made it empty, use a default
    if (!sanitizedName || sanitizedName.trim() === "") {
        sanitizedName = "property";
    }

    // Replace & with 'and' before other formatting
    sanitizedName = sanitizedName.replace(/&/g, "and");

    // Process commas first, then handle other transformations
    sanitizedName = sanitizedName
        .replace(/,\s*/g, "-") // Replace commas (and following spaces) with hyphens
        .replace(/\+/g, "-") // Replace + with hyphens (add this line)
        .replace(/\:/g, "-") // Replace : with ""
        .replace(/\//g, "-") // Replace / with hyphens (fixes enum names)
        .replace(/—/g, "-") // Replace em dash hyphens
        .replace(/([a-z])([A-Z])/g, "$1-$2") // Add hyphens between camelCase
        .replace(/\s+/g, "-") // Convert spaces to hyphens
        .replace(/--+/g, "-") // Normalize multiple consecutive hyphens to single
        .toLowerCase(); // Convert to lowercase

    return sanitizedName;
}

export function sanitizePropertyName(name: string): string {
    // Handle names starting with "." - remove the dot
    let sanitizedName = name.startsWith(".") ? name.substring(1) : name;

    // Remove leading invalid chars AFTER initial dot check
    sanitizedName = sanitizedName.replace(/^[_\-\.\(\)&]+/, "");

    // If that made it empty, use a default
    if (!sanitizedName || sanitizedName.trim() === "") {
        sanitizedName = "property"; // Or handle as error?
    }

    // Replace problematic characters BEFORE checking for leading digit
    // Within individual hierarchy parts: spaces and special chars become hyphens, preserve existing hyphens
    sanitizedName = sanitizedName
        .replace(/&/g, "and") // Replace & with 'and'
        .replace(/[^a-zA-Z0-9\-]/g, "-") // Replace non-alphanumeric chars (except hyphens) with hyphens
        .replace(/-+/g, "-"); // Collapse multiple hyphens to single hyphen

    // Remove leading and trailing hyphens
    sanitizedName = sanitizedName.replace(/^-+/, "").replace(/-+$/, "");

    // Check if starts with a digit AFTER other sanitization
    if (/^\d/.test(sanitizedName)) {
        return `_${sanitizedName}`;
    }

    // Ensure it's not empty again after trailing cleanup
    if (!sanitizedName || sanitizedName.trim() === "") {
        return "property";
    }

    return sanitizedName.toLowerCase();
}

function sanitizeRowName(rowName: string): string {
    // Replace & with 'and' and other problematic characters
    return rowName
        .replace(/&/g, "and")
        .replace(/\(/g, "_") // Replace ( with _
        .replace(/\)/g, "_"); // Replace ) with _
}

// Helper to sanitize mode names for enum variants
function sanitizeModeForEnum(name: string): string {
    // Check if the mode name is only digits
    if (/^\d+$/.test(name)) {
        return `mode_${name}`;
    }

    // Check if starts with a digit (still invalid for most identifiers)
    if (/^\d/.test(name)) {
        return `m_${name}`;
    }

    // Replace any characters that are invalid in identifiers
    return name.replace(/[^a-zA-Z0-9_]/g, "_");
}

// Extract hierarchy from variable name (e.g. "colors/primary/base" → ["colors", "primary", "base"])
export function extractHierarchy(name: string): string[] {
    // First try splitting by slashes (the expected format)
    if (name.includes("/")) {
        return name.split("/").map((part) => sanitizePropertyName(part));
    }

    // Default case for simple names
    return [sanitizePropertyName(name)];
}

// Helper function to get original Figma variable data
async function getOriginalVariableData(variableId: string) {
    try {
        return await figma.variables.getVariableByIdAsync(variableId);
    } catch (error) {
        console.warn(`Could not fetch variable ${variableId}:`, error);
        return null;
    }
}

// Helper function to follow a reference chain to find a concrete value
async function followChainToConcreteValue(
    refId: string,
    modeName: string,
    visited: Set<string> = new Set(),
): Promise<string | null> {
    if (visited.has(refId)) {
        return null; // Circular reference in the chain
    }
    visited.add(refId);

    // Get the original Figma variable data
    const originalVariable = await getOriginalVariableData(refId);
    if (!originalVariable) {
        return null;
    }

    // Look for a concrete value in any mode
    for (const [, value] of Object.entries(originalVariable.valuesByMode)) {
        // Check if this value is concrete (not a reference)
        if (
            typeof value === "object" &&
            "type" in value &&
            value.type === "VARIABLE_ALIAS"
        ) {
            // This is still a reference, continue following the chain
            const aliasValue = value as any;
            if (aliasValue.id) {
                const result = await followChainToConcreteValue(
                    aliasValue.id,
                    modeName,
                    visited,
                );
                if (result !== null) {
                    return result;
                }
            }
        } else {
            // Found a concrete value - format it properly based on the variable type
            const concreteValue: any = value;

            if (
                originalVariable.resolvedType === "COLOR" &&
                typeof concreteValue === "object" &&
                "r" in concreteValue
            ) {
                // Format color value
                return rgbToHex({
                    r: concreteValue.r,
                    g: concreteValue.g,
                    b: concreteValue.b,
                    a: "a" in concreteValue ? concreteValue.a : 1,
                });
            } else if (
                originalVariable.resolvedType === "FLOAT" &&
                typeof concreteValue === "number"
            ) {
                return `${concreteValue}px`;
            } else if (
                originalVariable.resolvedType === "STRING" &&
                typeof concreteValue === "string"
            ) {
                return `"${concreteValue}"`;
            } else if (
                originalVariable.resolvedType === "BOOLEAN" &&
                typeof concreteValue === "boolean"
            ) {
                return concreteValue ? "true" : "false";
            } else if (typeof concreteValue === "string") {
                return concreteValue;
            }
        }
    }

    return null;
}

function getDefaultValueForType(type: string): string {
    // Handle both Figma types and Slint types - this should never happen in practice
    // but we provide a fallback for clear visibility in the generated code
    switch (type) {
        // Figma types
        case "COLOR":
            return "#FF00FF"; // Magenta for visibility
        case "FLOAT":
            return "0px";
        case "STRING":
            return '""';
        case "BOOLEAN":
            return "false";
        // Slint types
        case "brush":
            return "#FF00FF"; // Magenta for visibility
        case "length":
            return "0px";
        case "string":
            return '""';
        case "bool":
            return "false";
        default:
            return "#FF00FF"; // Magenta fallback for visibility
    }
}

interface VariableNode {
    name: string;
    type?: string;
    valuesByMode?: Map<
        string,
        { value: string; refId?: string; comment?: string }
    >;
    children: Map<string, VariableNode>;
}

// Recursively generate code from the tree structure

interface StructField {
    name: string;
    type: string;
    isMultiMode?: boolean;
}

interface StructDefinition {
    name: string;
    fields: StructField[];
    path: string[];
}

interface PropertyInstance {
    name: string;
    type: string;
    isMultiMode?: boolean;
    modeData?: Map<string, { value: string; comment?: string }>;
    children?: Map<string, PropertyInstance>;
}

function generateStructsAndInstances(
    variableTree: VariableNode,
    _collectionName: string,
    collectionData: CollectionData, // using strict type interface
): {
    structs: string;
    instances: string;
} {
    // Data structures to hold our model
    const structDefinitions = new Map<string, StructDefinition>();
    const propertyInstances = new Map<string, PropertyInstance>();
    const hasRootModeVariable = variableTree.children.has("mode");
    // Local export info tracking
    const exportInfo = {
        renamedVariables: new Set<string>(),
        circularReferences: new Set<string>(),
        warnings: new Set<string>(),
    };

    // Build the struct model
    function buildStructModel(node: VariableNode, path: string[] = []) {
        // Special handling for root
        if (node.name === "root") {
            // Process all root children
            for (const [childName, childNode] of node.children.entries()) {
                // Always process child nodes with proper path propagation
                buildStructModel(childNode, [childName]); // Keep this single-element path for first level
            }
            return;
        }

        const currentPath = [...path];
        const pathKey = currentPath.join("/");
        const typeName = currentPath.join("_");

        // Only create a struct if this node will have fields
        const hasValueChildren = Array.from(node.children.values()).some(
            (child) => child.valuesByMode,
        );
        const hasStructChildren = Array.from(node.children.values()).some(
            (child) => child.children.size > 0,
        );

        if (
            (hasValueChildren || hasStructChildren) &&
            !structDefinitions.has(pathKey)
        ) {
            structDefinitions.set(pathKey, {
                name: `${collectionData.formattedName}_${typeName}`,
                fields: [],
                path: [...currentPath],
            });
        }

        // Process all children recursively, maintaining hierarchical paths
        for (const [childName, childNode] of node.children.entries()) {
            // Always process child nodes, appending to the path
            buildStructModel(childNode, [...currentPath, childName]);

            // Add field to parent struct
            const sanitizedChildName = sanitizePropertyName(childName);
            if (childNode.valuesByMode) {
                // Value field
                const slintType = getSlintType(childNode.type || "COLOR");
                structDefinitions.get(pathKey)!.fields.push({
                    name: sanitizedChildName,
                    type:
                        collectionData.modes.size > 1
                            ? `${collectionData.formattedName}_mode${collectionData.modes.size}_${slintType}`
                            : slintType,
                    isMultiMode: collectionData.modes.size > 1,
                });
            } else if (childNode.children.size > 0) {
                // Struct reference
                const childTypeName = [...currentPath, childName].join("_");
                structDefinitions.get(pathKey)!.fields.push({
                    name: sanitizedChildName,
                    type: `${collectionData.formattedName}_${childTypeName}`,
                });
            }
        }
    }
    // Build the instance model
    function buildInstanceModel(node: VariableNode, path: string[] = []): void {
        if (node.name === "root") {
            for (const [childName, childNode] of node.children.entries()) {
                // Special case for "mode" variable - rename it to avoid collision
                const sanitizedChildName: string =
                    childName === "mode" && hasRootModeVariable
                        ? "mode-var"
                        : sanitizePropertyName(childName);

                if (childName === "mode" && hasRootModeVariable) {
                    exportInfo.renamedVariables.add(
                        `"${childName}" → "${sanitizedChildName}" in ${collectionData.formattedName} (to avoid conflict with scheme mode)`,
                    ); // Use formattedName

                    try {
                        figma.notify(
                            "Renamed Figma 'mode' variable to 'mode-var' to avoid conflict",
                            { timeout: 3000 },
                        );
                    } catch (e) {
                        // Ignore if not in Figma plugin context
                    }
                }

                // Create the property instance with the appropriate name
                if (childNode.children.size > 0) {
                    propertyInstances.set(sanitizedChildName, {
                        name: sanitizedChildName,
                        type: `${collectionData.formattedName}_${childName}`, // Note: use original name in type
                        children: new Map<string, PropertyInstance>(),
                    });
                    // Process children
                    buildInstanceModel(childNode, [sanitizedChildName]);
                } else if (childNode.valuesByMode) {
                    // Direct value property
                    const slintType: string = getSlintType(
                        childNode.type || "COLOR",
                    );
                    const instance: PropertyInstance = {
                        name: sanitizedChildName,
                        type: slintType,
                        modeData: new Map<
                            string,
                            { value: string; comment?: string }
                        >(),
                    };

                    const rowNameKey =
                        childName === "mode" && hasRootModeVariable
                            ? sanitizePropertyName("mode") // Use original "mode" for lookup
                            : sanitizedChildName; // Use normal sanitized name for others
                    const resolvedVariableModesMap =
                        collectionData.variables.get(rowNameKey);

                    if (!resolvedVariableModesMap) {
                        console.error(
                            `Missing resolved variable data for root key: ${rowNameKey}`,
                        );
                        // Add instance even if empty to avoid breaking hierarchy
                        propertyInstances.set(sanitizedChildName, instance);
                        continue; // Skip to next child
                    }

                    if (collectionData.modes.size > 1) {
                        instance.isMultiMode = true;
                        for (const modeName of collectionData.modes) {
                            const resolvedData =
                                resolvedVariableModesMap.get(modeName);
                            if (resolvedData) {
                                instance.modeData!.set(modeName, {
                                    value: resolvedData.value,
                                    comment: resolvedData.comment, // <<< Use resolved comment >>>
                                });
                            } else {
                                console.warn(
                                    `Missing resolved data for ${rowNameKey} in mode ${modeName}`,
                                );
                                instance.modeData!.set(modeName, {
                                    value: "#FF00FF",
                                    comment: `Missing data for mode ${modeName}`,
                                });
                            }
                        }
                    } else {
                        // Single mode
                        const singleModeName =
                            [...collectionData.modes][0] || "value";
                        const resolvedData =
                            resolvedVariableModesMap.get(singleModeName);
                        if (resolvedData) {
                            instance.modeData!.set(singleModeName, {
                                value: resolvedData.value,
                                comment: resolvedData.comment, // <<< Use resolved comment >>>
                            });
                        } else {
                            console.warn(
                                `Missing resolved data for ${rowNameKey} in single mode ${singleModeName}`,
                            );
                            instance.modeData!.set(singleModeName, {
                                value: "#FF00FF",
                                comment: `Missing data for mode ${singleModeName}`,
                            });
                        }
                    }

                    propertyInstances.set(sanitizedChildName, instance);
                }
            }
            return;
        }

        // For non-root nodes
        const pathKey: string = path.join("/");
        if (!propertyInstances.has(pathKey)) {
            propertyInstances.set(pathKey, {
                name: path[path.length - 1],
                type: `${collectionData.formattedName}_${path.join("_")}`,
                children: new Map<string, PropertyInstance>(),
            });
        }
        for (const [childName, childNode] of node.children.entries()) {
            const sanitizedChildName: string = sanitizePropertyName(childName);
            const childPath: string[] = [...path, sanitizedChildName];
            const childPathKey: string = childPath.join("/");

            if (childNode.children.size > 0) {
                // Get parent instance
                const parentInstance: PropertyInstance | undefined =
                    propertyInstances.get(pathKey);
                if (!parentInstance || !parentInstance.children) {
                    continue;
                }

                // Add child instance to parent
                parentInstance.children.set(sanitizedChildName, {
                    name: sanitizedChildName,
                    type: `${collectionData.formattedName}_${childPath.join("_")}`,
                    children: new Map<string, PropertyInstance>(),
                });
                buildInstanceModel(childNode, childPath);
            } else if (childNode.valuesByMode) {
                // Get parent instance
                const parentInstance: PropertyInstance | undefined =
                    propertyInstances.get(pathKey);
                if (!parentInstance || !parentInstance.children) {
                    continue;
                }

                const slintType: string = getSlintType(
                    childNode.type || "COLOR",
                );
                const instance: PropertyInstance = {
                    name: sanitizedChildName,
                    type: slintType,
                    modeData: new Map<
                        string,
                        { value: string; comment?: string }
                    >(),
                };
                const rowNameKey = childPathKey; // Use the pre-calculated childPathKey
                const resolvedVariableModesMap =
                    collectionData.variables.get(rowNameKey);

                if (!resolvedVariableModesMap) {
                    console.error(
                        `Missing resolved variable data for nested key: ${rowNameKey}`,
                    );
                    // Add instance even if empty
                    propertyInstances.set(childPathKey, instance);
                    continue; // Skip to next child
                }

                if (collectionData.modes.size > 1) {
                    instance.isMultiMode = true;
                    for (const modeName of collectionData.modes) {
                        const resolvedData =
                            resolvedVariableModesMap.get(modeName);
                        if (resolvedData) {
                            instance.modeData!.set(modeName, {
                                value: resolvedData.value,
                                comment: resolvedData.comment, // <<< Use resolved comment >>>
                            });
                        } else {
                            console.warn(
                                `Missing resolved data for ${rowNameKey} in mode ${modeName}`,
                            );
                            instance.modeData!.set(modeName, {
                                value: "#FF00FF",
                                comment: `Missing data for mode ${modeName}`,
                            });
                        }
                    }
                } else {
                    // Single mode
                    const singleModeName =
                        [...collectionData.modes][0] || "value";
                    const resolvedData =
                        resolvedVariableModesMap.get(singleModeName);
                    if (resolvedData) {
                        instance.modeData!.set(singleModeName, {
                            value: resolvedData.value,
                            comment: resolvedData.comment, // <<< Use resolved comment >>>
                        });
                    } else {
                        console.warn(
                            `Missing resolved data for ${rowNameKey} in single mode ${singleModeName}`,
                        );
                        instance.modeData!.set(singleModeName, {
                            value: "#FF00FF",
                            comment: `Missing data for mode ${singleModeName}`,
                        });
                    }
                }
                propertyInstances.set(childPathKey, instance);
            }
        }
    }
    function buildPropertyHierarchy() {
        // First find all unique top-level paths
        const topLevelPaths = new Set<string>();

        for (const key of propertyInstances.keys()) {
            if (key.includes("/")) {
                const parts = key.split("/");
                topLevelPaths.add(parts[0]);
            }
        }

        // For each top-level path, create or get the instance
        for (const path of topLevelPaths) {
            if (!propertyInstances.has(path)) {
                propertyInstances.set(path, {
                    name: path,
                    type: `${collectionData.formattedName}_${path}`,
                    children: new Map(),
                });
            }

            // process all children of this path
            const childPaths = Array.from(propertyInstances.keys()).filter(
                (k) => k.startsWith(`${path}/`),
            );

            for (const childPath of childPaths) {
                const childInstance = propertyInstances.get(childPath);
                if (!childInstance) {
                    continue;
                }

                // Get the parent path
                const parts = childPath.split("/");
                const parentPath = parts.slice(0, -1).join("/");
                const childName = parts[parts.length - 1];

                // Get the parent instance
                const parentInstance = propertyInstances.get(parentPath);
                if (!parentInstance || !parentInstance.children) {
                    continue;
                }

                // Add child to parent
                parentInstance.children.set(childName, childInstance);
            }
        }
    }
    function generateInstanceCode(
        instance: PropertyInstance,
        path: string[] = [],
        indent: string = "    ",
    ) {
        let result = "";
        const isRoot = indent === "    ";
        // Special handling for renamed mode variable
        if (isRoot) {
            if (instance.name === "mode-var") {
                result += `${indent}// NOTE: This property was renamed from "mode" to "mode-var" to avoid collision\n`;
                result += `${indent}// with the scheme mode property of the same name.\n`;
            }

            if (instance.children && instance.children.size > 0) {
                // Struct instance
                result += `${indent}out property <${instance.type}> ${instance.name}: {\n`;
                for (const [
                    childName,
                    childInstance,
                ] of instance.children.entries()) {
                    result += generateInstanceCode(
                        childInstance,
                        [instance.name, childName],
                        indent + "    ",
                    );
                }
                result += `${indent}};\n\n`;
            } else if (instance.modeData) {
                const isRoot = indent === "    ";
                const slintType = instance.isMultiMode
                    ? `${collectionData.formattedName}_mode${collectionData.modes.size}_${instance.type}`
                    : instance.type;
                // Root Value Instance
                if (instance.isMultiMode) {
                    result += isRoot
                        ? `${indent}out property <${slintType}> ${instance.name}: {\n`
                        : `${indent}${instance.name}: {\n`;
                    for (const [
                        modeName,
                        modeInfo,
                    ] of instance.modeData.entries()) {
                        if (modeInfo.comment) {
                            // Check if a comment exists for this specific mode
                            result += `${indent}    // ${modeInfo.comment}\n`; // Add comment line
                        }
                        result += `${indent}    ${modeName}: ${modeInfo.value},\n`; // Add value line
                    }
                    result += isRoot ? `${indent}};\n\n` : `${indent}},\n`;
                } else {
                    // Single mode root
                    const singleModeName =
                        instance.modeData.keys().next().value || "value";
                    const modeInfo = instance.modeData.get(singleModeName);
                    if (modeInfo?.comment) {
                        result += `${indent}// ${modeInfo.comment}\n`;
                    }
                    const value = modeInfo?.value || "";
                    // Handle unresolved refs (replace with default value logic if needed)
                    if (value.startsWith("@ref:")) {
                        result += `${indent}out property <${instance.type}> ${instance.name}: ${
                            instance.type === "brush"
                                ? "#808080"
                                : instance.type === "length"
                                  ? "0px"
                                  : instance.type === "string"
                                    ? '""'
                                    : "false"
                        };\n`; // Removed extra newline
                    } else {
                        result += `${indent}out property <${instance.type}> ${instance.name}: ${value};\n`; // Removed extra newline
                    }
                }
            }
        } else {
            // Nested property (inside a struct)
            if (instance.children && instance.children.size > 0) {
                // Nested Struct
                result += `${indent}${instance.name}: {\n`;
                for (const [
                    childName,
                    childInstance,
                ] of instance.children.entries()) {
                    // Recurse for children, increasing indent
                    result += generateInstanceCode(
                        childInstance,
                        [...path, childName],
                        indent + "    ",
                    );
                }
                result += `${indent}},\n`; // Comma for nested struct field
            } else if (instance.modeData) {
                // Nested Value
                if (instance.isMultiMode) {
                    // Multi-mode nested value
                    result += `${indent}${instance.name}: {\n`;
                    for (const [
                        modeName,
                        modeInfo,
                    ] of instance.modeData.entries()) {
                        if (modeInfo.comment) {
                            result += `${indent}    // ${modeInfo.comment}\n`;
                        }
                        result += `${indent}    ${modeName}: ${modeInfo.value},\n`;
                    }
                    result += `${indent}},\n`; // Comma for nested multi-mode field
                } else {
                    // Single mode nested
                    const singleModeName =
                        instance.modeData.keys().next().value || "value";
                    const modeInfo = instance.modeData.get(singleModeName);
                    if (modeInfo?.comment) {
                        result += `${indent}// ${modeInfo.comment}\n`;
                    }
                    const value = modeInfo?.value || "";
                    // Handle unresolved refs (replace with default value logic if needed)
                    if (value.startsWith("@ref:")) {
                        result += `${indent}${instance.name}: ${
                            instance.type === "brush"
                                ? "#808080"
                                : instance.type === "length"
                                  ? "0px"
                                  : instance.type === "string"
                                    ? '""'
                                    : "false"
                        },\n`;
                    } else {
                        result += `${indent}${instance.name}: ${value},\n`;
                    }
                }
            }
        }
        return result;
    }

    // Generate multi-mode structs
    const multiModeStructs: string[] = [];
    collectMultiModeStructs(variableTree, collectionData, multiModeStructs);

    // Build struct model
    buildStructModel(variableTree);

    // Build instance model
    buildInstanceModel(variableTree);
    buildPropertyHierarchy();
    // Generate code from the models
    let structsCode = multiModeStructs.join("");

    // Generate structs in sorted order (deepest first)
    const sortedPaths = Array.from(structDefinitions.keys()).sort(
        (a, b) => b.split("/").length - a.split("/").length,
    );

    for (const pathKey of sortedPaths) {
        const struct = structDefinitions.get(pathKey)!;

        structsCode += `struct ${struct.name} {\n`;
        for (const field of struct.fields) {
            structsCode += `    ${field.name}: ${field.type},\n`;
        }
        structsCode += `}\n\n`;
    }

    // Generate property instances
    let instancesCode = "";

    // Generate all root level instances
    for (const [instanceName, instance] of propertyInstances.entries()) {
        if (!instanceName.includes("/")) {
            instancesCode += generateInstanceCode(instance);
        }
    }

    return {
        structs: structsCode,
        instances: instancesCode,
    };
}
interface VariableModeData {
    value: string;
    type: string;
    refId?: string;
    comment?: string;
}

interface CollectionData {
    name: string;
    formattedName: string;
    modes: Set<string>;
    variables: Map<string, Map<string, VariableModeData>>;
}

// For Figma Plugin - Export function with hierarchical structure
// Export each collection to a separate virtual file
export async function exportFigmaVariablesToSeparateFiles(
    exportAsSingleFile: boolean = false,
): Promise<Array<{ name: string; content: string }>> {
    const exportInfo = {
        renamedVariables: new Set<string>(),
        circularReferences: new Set<string>(),
        warnings: new Set<string>(),
        features: new Set<string>(),
        collections: new Set<string>(),
    };
    const generatedFiles: Array<{ name: string; content: string }> = []; // Store intermediate files

    try {
        // Get collections asynchronously
        const variableCollections =
            await figma.variables.getLocalVariableCollectionsAsync();

        // First, initialize the collection structure for ALL collections
        const collectionStructure = new Map<string, CollectionData>();

        // Build a global map of variable paths
        const variablePathsById = new Map<
            string,
            { collection: string; node: VariableNode; path: string[] }
        >();

        // Build a global map of variable names for readable comments
        const variableNamesById = new Map<string, string>();

        // Initialize structure for all collections first
        for (const collection of variableCollections) {
            const collectionName = sanitizePropertyName(collection.name);
            const formattedCollectionName = formatStructName(collection.name);
            exportInfo.collections.add(collection.name);
            // Initialize the collection structure
            collectionStructure.set(collectionName, {
                name: collection.name,
                formattedName: formattedCollectionName,
                modes: new Set<string>(),
                variables: new Map(),
            });

            // Add modes to collection
            collection.modes.forEach((mode) => {
                const sanitizedMode = sanitizeModeForEnum(
                    sanitizePropertyName(mode.name),
                );
                collectionStructure
                    .get(collectionName)!
                    .modes.add(sanitizedMode);
            });
        }

        // Pre-populate the global variable names map with ALL variables from ALL collections
        // This ensures cross-collection variable references show readable names in comments
        for (const collection of variableCollections) {
            const batchSize = 10; // Use larger batch size for name collection
            for (let i = 0; i < collection.variableIds.length; i += batchSize) {
                const batch = collection.variableIds.slice(i, i + batchSize);
                const batchPromises = batch.map((id) =>
                    figma.variables.getVariableByIdAsync(id),
                );
                const batchResults = await Promise.all(batchPromises);

                for (const variable of batchResults) {
                    if (variable && variable.name) {
                        variableNamesById.set(variable.id, variable.name);
                    }
                }
            }
        }

        // process the variables for each collection
        for (const collection of variableCollections) {
            const collectionName = sanitizePropertyName(collection.name);

            // Process variables in batches
            const batchSize = 5;
            for (let i = 0; i < collection.variableIds.length; i += batchSize) {
                const batch = collection.variableIds.slice(i, i + batchSize);
                const batchPromises = batch.map((id) =>
                    figma.variables.getVariableByIdAsync(id),
                );
                const batchResults = await Promise.all(batchPromises);

                for (const variable of batchResults) {
                    if (!variable) {
                        continue;
                    }
                    if (
                        !variable.valuesByMode ||
                        Object.keys(variable.valuesByMode).length === 0
                    ) {
                        continue;
                    }

                    // Use extractHierarchy to break up variable names
                    const nameParts = extractHierarchy(variable.name);

                    // For flat structure (existing code)
                    const propertyName =
                        nameParts.length > 0
                            ? nameParts[nameParts.length - 1]
                            : sanitizePropertyName(variable.name);

                    const path =
                        nameParts.length > 1
                            ? nameParts.slice(0, -1).join("/")
                            : "";

                    const rowName = path
                        ? `${path}/${propertyName}`
                        : propertyName;
                    const sanitizedRowName = sanitizeRowName(rowName);

                    // Initialize row in variables map
                    if (
                        !collectionStructure
                            .get(collectionName)!
                            .variables.has(sanitizedRowName)
                    ) {
                        collectionStructure
                            .get(collectionName)!
                            .variables.set(
                                sanitizedRowName,
                                new Map<string, VariableModeData>(),
                            );
                    }

                    // Process values for each mode
                    // First, create a reverse lookup map for collection modes by both modeId and name
                    const modeIdToInfo = new Map<
                        string,
                        { modeId: string; name: string }
                    >();
                    const modeNameToInfo = new Map<
                        string,
                        { modeId: string; name: string }
                    >();

                    for (const mode of collection.modes) {
                        modeIdToInfo.set(mode.modeId, mode);
                        // Also index by sanitized name for fallback matching
                        const sanitizedName = sanitizeModeForEnum(
                            sanitizePropertyName(mode.name),
                        );
                        modeNameToInfo.set(sanitizedName, mode);
                    }

                    // Process all modes that the collection expects, not just what's in valuesByMode
                    for (const collectionMode of collection.modes) {
                        const modeName = sanitizeModeForEnum(
                            sanitizePropertyName(collectionMode.name),
                        );

                        let value: any = null;
                        let foundModeId: string | null = null;

                        // Strategy 1: Try exact modeId match
                        if (variable.valuesByMode[collectionMode.modeId]) {
                            value =
                                variable.valuesByMode[collectionMode.modeId];
                            foundModeId = collectionMode.modeId;
                        } else {
                            // Strategy 2: Try to find by mode name matching
                            // Look for a variable mode that when sanitized matches the collection mode name
                            for (const [varModeId, varValue] of Object.entries(
                                variable.valuesByMode,
                            )) {
                                // Extract the potential mode name from the variable's modeId
                                // Common patterns: "mode_light" -> "light", "light_mode" -> "light", etc.
                                const varModeName = varModeId
                                    .replace(/^(mode_|_mode$)/, "") // Remove mode_ prefix or _mode suffix
                                    .replace(/_/g, " ") // Replace underscores with spaces
                                    .toLowerCase();

                                const sanitizedVarModeName =
                                    sanitizeModeForEnum(
                                        sanitizePropertyName(varModeName),
                                    );

                                if (
                                    sanitizedVarModeName ===
                                        modeName.toLowerCase() ||
                                    varModeName ===
                                        collectionMode.name.toLowerCase()
                                ) {
                                    value = varValue;
                                    foundModeId = varModeId;
                                    console.log(
                                        `Found mode name match: ${varModeId} -> ${modeName}`,
                                    );
                                    break;
                                }
                            }

                            // Strategy 3: Enhanced fallback - try to distribute different values to different collection modes
                            if (value === null) {
                                const availableValues = Object.entries(
                                    variable.valuesByMode,
                                );
                                if (availableValues.length > 0) {
                                    // Try to map collection modes to different variable modes when possible
                                    // Get the index of this collection mode
                                    const collectionModeIndex =
                                        collection.modes.findIndex(
                                            (mode) =>
                                                mode.modeId ===
                                                collectionMode.modeId,
                                        );

                                    // Use different available values for different collection modes
                                    const valueIndex = Math.min(
                                        collectionModeIndex,
                                        availableValues.length - 1,
                                    );
                                    [foundModeId, value] =
                                        availableValues[valueIndex];

                                    console.warn(
                                        `Mode mismatch for variable ${variable.name}: using fallback value from mode ${foundModeId} (index ${valueIndex}) for expected mode ${modeName}`,
                                    );
                                }
                            }
                        }

                        // Skip if no value found at all
                        if (value === null) {
                            console.warn(
                                `No value found for variable ${variable.name} in mode ${modeName}`,
                            );
                            continue;
                        }

                        // Format value and resolve all references immediately
                        let formattedValue = "";
                        let comment: string | undefined;

                        // Process different variable types (COLOR, FLOAT, STRING, BOOLEAN)
                        if (variable.resolvedType === "COLOR") {
                            if (
                                typeof value === "object" &&
                                value &&
                                "r" in value
                            ) {
                                formattedValue = rgbToHex({
                                    r: value.r,
                                    g: value.g,
                                    b: value.b,
                                    a: "a" in value ? value.a : 1,
                                });
                            } else if (
                                typeof value === "object" &&
                                value &&
                                "type" in value &&
                                value.type === "VARIABLE_ALIAS"
                            ) {
                                // Resolve reference to concrete value immediately
                                const resolvedValue =
                                    await followChainToConcreteValue(
                                        value.id,
                                        modeName,
                                    );
                                if (resolvedValue) {
                                    formattedValue = resolvedValue;
                                    const referenceName =
                                        variableNamesById.get(value.id) ||
                                        value.id;
                                    comment = `Resolved from reference ${referenceName}`;
                                } else {
                                    formattedValue =
                                        getDefaultValueForType("brush");
                                    const referenceName =
                                        variableNamesById.get(value.id) ||
                                        value.id;
                                    comment = `Failed to resolve reference ${referenceName}, using default`;
                                    exportInfo.warnings.add(
                                        `Failed to resolve COLOR reference ${value.id} for ${variable.name}.${modeName}`,
                                    );
                                }
                            }
                        } else if (variable.resolvedType === "FLOAT") {
                            if (typeof value === "number") {
                                formattedValue = `${value}px`;
                            } else if (
                                typeof value === "object" &&
                                value &&
                                "type" in value &&
                                value.type === "VARIABLE_ALIAS"
                            ) {
                                // Resolve reference to concrete value immediately
                                const resolvedValue =
                                    await followChainToConcreteValue(
                                        value.id,
                                        modeName,
                                    );
                                if (resolvedValue) {
                                    formattedValue = resolvedValue;
                                    const referenceName =
                                        variableNamesById.get(value.id) ||
                                        value.id;
                                    comment = `Resolved from reference ${referenceName}`;
                                } else {
                                    formattedValue =
                                        getDefaultValueForType("length");
                                    const referenceName =
                                        variableNamesById.get(value.id) ||
                                        value.id;
                                    comment = `Failed to resolve reference ${referenceName}, using default`;
                                    exportInfo.warnings.add(
                                        `Failed to resolve FLOAT reference ${value.id} for ${variable.name}.${modeName}`,
                                    );
                                }
                            } else {
                                console.warn(
                                    `Unexpected FLOAT value type: ${typeof value} for ${variable.name}`,
                                );
                                formattedValue =
                                    getDefaultValueForType("length");
                            }
                        } else if (variable.resolvedType === "STRING") {
                            if (typeof value === "string") {
                                formattedValue = `"${value}"`;
                            } else if (
                                typeof value === "object" &&
                                value &&
                                "type" in value &&
                                value.type === "VARIABLE_ALIAS"
                            ) {
                                // Resolve reference to concrete value immediately
                                const resolvedValue =
                                    await followChainToConcreteValue(
                                        value.id,
                                        modeName,
                                    );
                                if (resolvedValue) {
                                    formattedValue = resolvedValue;
                                    const referenceName =
                                        variableNamesById.get(value.id) ||
                                        value.id;
                                    comment = `Resolved from reference ${referenceName}`;
                                } else {
                                    formattedValue =
                                        getDefaultValueForType("string");
                                    const referenceName =
                                        variableNamesById.get(value.id) ||
                                        value.id;
                                    comment = `Failed to resolve reference ${referenceName}, using default`;
                                    exportInfo.warnings.add(
                                        `Failed to resolve STRING reference ${value.id} for ${variable.name}.${modeName}`,
                                    );
                                }
                            } else {
                                console.warn(
                                    `Unexpected STRING value type: ${typeof value} for ${variable.name}`,
                                );
                                formattedValue =
                                    getDefaultValueForType("string");
                            }
                        } else if (variable.resolvedType === "BOOLEAN") {
                            if (typeof value === "boolean") {
                                formattedValue = value ? "true" : "false";
                            } else if (
                                typeof value === "object" &&
                                value &&
                                "type" in value &&
                                value.type === "VARIABLE_ALIAS"
                            ) {
                                // Resolve reference to concrete value immediately
                                const resolvedValue =
                                    await followChainToConcreteValue(
                                        value.id,
                                        modeName,
                                    );
                                if (resolvedValue) {
                                    formattedValue = resolvedValue;
                                    const referenceName =
                                        variableNamesById.get(value.id) ||
                                        value.id;
                                    comment = `Resolved from reference ${referenceName}`;
                                } else {
                                    formattedValue =
                                        getDefaultValueForType("bool");
                                    const referenceName =
                                        variableNamesById.get(value.id) ||
                                        value.id;
                                    comment = `Failed to resolve reference ${referenceName}, using default`;
                                    exportInfo.warnings.add(
                                        `Failed to resolve BOOLEAN reference ${value.id} for ${variable.name}.${modeName}`,
                                    );
                                }
                            } else {
                                console.warn(
                                    `Unexpected BOOLEAN value type: ${typeof value} for ${variable.name}`,
                                );
                                formattedValue = getDefaultValueForType("bool");
                            }
                        }

                        collectionStructure
                            .get(collectionName)!
                            .variables.get(sanitizedRowName)!
                            .set(modeName, {
                                value: formattedValue,
                                type: variable.resolvedType,
                                // No refId - all references are resolved to concrete values
                                comment: comment,
                            });
                    }

                    // Store the path for each variable ID
                    variablePathsById.set(variable.id, {
                        collection: collectionName,
                        node: {
                            name: propertyName,
                            type: variable.resolvedType,
                            valuesByMode: new Map(),
                            children: new Map(),
                        },
                        path: nameParts,
                    });
                }

                // Force GC between batches
                await new Promise((resolve) => setTimeout(resolve, 0));
            }
        }

        // All references have been resolved to concrete values during variable storage
        // No need for post-processing since there are no more refId references

        // Since all references are now resolved to concrete values, there are no cross-collection dependencies
        const collectionDependencies = new Map<string, Set<string>>();
        for (const collection of variableCollections) {
            const collectionName = sanitizePropertyName(collection.name);
            collectionDependencies.set(collectionName, new Set<string>());
        }

        // No dependency cycles possible since all variables now have concrete values
        const finalExportAsSingleFile = exportAsSingleFile;

        // Generate content for each collection
        for (const [
            _collectionName,
            collectionData,
        ] of collectionStructure.entries()) {
            // Skip collections with no variables
            if (collectionData.variables.size === 0) {
                exportInfo.warnings.add(
                    `Skipped empty collection: ${collectionData.name}`,
                );
                continue;
            }

            // Build the variable tree for this collection
            const variableTree: VariableNode = {
                name: "root",
                children: new Map(),
            };
            for (const [varName, modes] of collectionData.variables.entries()) {
                const parts = extractHierarchy(varName);
                let currentNode = variableTree;
                for (let i = 0; i < parts.length - 1; i++) {
                    const part = parts[i];
                    if (!currentNode.children.has(part)) {
                        currentNode.children.set(part, {
                            name: part,
                            children: new Map(),
                        });
                    }
                    currentNode = currentNode.children.get(part)!;
                }
                const propertyName = sanitizePropertyName(
                    parts[parts.length - 1],
                );
                if (!currentNode.children.has(propertyName)) {
                    const valuesByMode = new Map<
                        string,
                        { value: string; refId?: string; comment?: string }
                    >();
                    const firstModeValue = modes.values().next().value;
                    const type = firstModeValue?.type || "COLOR";
                    for (const [modeName, valueData] of modes.entries()) {
                        valuesByMode.set(modeName, {
                            value: valueData.value,
                            refId: valueData.refId,
                            comment: valueData.comment,
                        });
                    }
                    currentNode.children.set(propertyName, {
                        name: propertyName,
                        type: type,
                        valuesByMode: valuesByMode,
                        children: new Map(),
                    });
                }
            }

            // Generate structs and instances code from the tree
            const { structs, instances } = generateStructsAndInstances(
                variableTree,
                collectionData.formattedName,
                collectionData,
                // Pass exportInfo if needed
            );

            // Generate scheme-related code (only if multi-mode)
            let modeEnum = "";
            let schemeStruct = "";
            let schemeModeStruct = "";
            let schemeInstance = "";
            let currentSchemeInstance = "";
            if (collectionData.modes.size > 1) {
                // Generate Enum
                modeEnum += `export enum ${collectionData.formattedName}Mode {\n`;
                for (const mode of collectionData.modes) {
                    modeEnum += `    ${mode},\n`;
                }
                modeEnum += `}\n\n`;

                // Generate Scheme Structs/Instances
                const hasRootModeVariable = variableTree.children.has("mode");
                const schemeResult = generateSchemeStructs(
                    variableTree,
                    collectionData,
                    hasRootModeVariable,
                );
                schemeStruct = schemeResult.schemeStruct;
                schemeModeStruct = schemeResult.schemeModeStruct;
                schemeInstance = schemeResult.schemeInstance;
                currentSchemeInstance = schemeResult.currentSchemeInstance;
            }

            let content = `// Generated Slint file for ${collectionData.formattedName}\n\n`;

            // No imports needed since all references are resolved to concrete values

            // Add Enum (if generated)
            content += modeEnum;

            // Add Structs (multi-mode base structs and specific structs generated by generateStructsAndInstances)
            content += structs;

            // Add Scheme structs (if generated)
            content += schemeStruct;
            content += schemeModeStruct;

            // Add the main global block containing instances and scheme properties
            content += `export global ${collectionData.formattedName} {\n`;
            content += instances; // Add the generated instance code lines
            content += schemeInstance; // Add scheme instance code (if generated)
            content += currentSchemeInstance; // Add current instance code (if generated)
            content += `}\n`; // Close global block (removed extra \n\n)

            // Store the fully assembled content for this collection
            generatedFiles.push({
                name: `${collectionData.formattedName}.slint`,
                content: content.trim() + "\n", // Trim whitespace and add single trailing newline
            });
        }

        // Post-process generated files (e.g., replace unresolved refs)
        for (const file of generatedFiles) {
            // Check if there are any unresolved references left
            if (file.content.includes("@ref:")) {
                exportInfo.warnings.add(
                    `Found unresolved references in ${file.name}`,
                );

                // Replace unresolved references with appropriate defaults based on context
                file.content = file.content.replace(
                    /(@ref:VariableID:[0-9:]+)/g,
                    (_match, reference) => {
                        exportInfo.warnings.add(
                            `  Replacing unresolved reference: ${reference}`,
                        );

                        // Look at surrounding context to determine appropriate replacement
                        if (
                            file.content.includes(`brush,\n`) &&
                            file.content.includes(reference)
                        ) {
                            return "#808080"; // Default color
                        } else if (
                            file.content.includes(`length,\n`) &&
                            file.content.includes(reference)
                        ) {
                            return "0px"; // Default length
                        } else if (
                            file.content.includes(`string,\n`) &&
                            file.content.includes(reference)
                        ) {
                            return '""'; // Default string
                        } else if (
                            file.content.includes(`bool,\n`) &&
                            file.content.includes(reference)
                        ) {
                            return "false"; // Default boolean
                        } else {
                            return "#808080"; // Default fallback
                        }
                    },
                );
            }
        }

        // Determine final output structure (single vs multiple files)
        let finalOutputFiles: Array<{ name: string; content: string }> = [];

        if (finalExportAsSingleFile) {
            // Use the determined flag
            let combinedContent =
                "// Combined Slint Design Tokens\n// Generated on " +
                new Date().toISOString().split("T")[0] +
                "\n\n";
            for (const file of generatedFiles) {
                combinedContent += `// --- Content from ${file.name} ---\n\n`;
                combinedContent += file.content;
                combinedContent += `\n\n// --- End Content from ${file.name} ---\n\n`;
            }
            finalOutputFiles.push({
                name: "design-tokens.slint",
                content: combinedContent.trim(),
            });
        } else {
            // Use individual files
            finalOutputFiles = generatedFiles;
        }

        // Add README
        const readmeContent = generateReadmeContent(exportInfo);
        finalOutputFiles.push({
            name: "README.md",
            content: readmeContent,
        });

        return finalOutputFiles; // Return the final array
    } catch (error) {
        console.error("Error in exportFigmaVariablesToSeparateFiles:", error);
        return [
            {
                name: "error.slint",
                content: `// Error generating variables: ${error}`,
            },
        ];
    }
}

// Define proper type interfaces for our structure
interface SchemeField {
    name: string;
    type: string;
}

interface SchemeStruct {
    name: string;
    fields: SchemeField[];
    path: string[];
}

// Main function for generating scheme structs
function generateSchemeStructs(
    variableTree: VariableNode,
    collectionData: { name: string; formattedName: string; modes: Set<string> },
    hasRootModeVariable: boolean, // Add this parameter
) {
    // Maps to hold our data model
    const schemeStructs = new Map<string, SchemeStruct>();

    // Build structure recursively using a proper object model
    function buildSchemeModel(node: VariableNode, path: string[] = []) {
        if (path.length > 0) {
            const pathKey = path.join("/");

            if (!schemeStructs.has(pathKey)) {
                schemeStructs.set(pathKey, {
                    name: `${collectionData.formattedName}-${path.join("-")}-Scheme`,
                    fields: [],
                    path: [...path],
                });
            }

            // Add fields to the struct
            for (const [childName, childNode] of node.children.entries()) {
                if (childNode.valuesByMode) {
                    schemeStructs.get(pathKey)!.fields.push({
                        name: childName,
                        type: getSlintType(childNode.type || "COLOR"),
                    });
                } else if (childNode.children.size > 0) {
                    const childPath = [...path, childName];
                    schemeStructs.get(pathKey)!.fields.push({
                        name: childName,
                        type: `${collectionData.formattedName}-${childPath.join("-")}-Scheme`,
                    });
                }
            }
        }

        // Recursively process child nodes
        for (const [childName, childNode] of node.children.entries()) {
            if (childNode.children.size > 0) {
                buildSchemeModel(childNode, [...path, childName]);
            }
        }
    }

    // Build the model
    buildSchemeModel(variableTree);

    // Now generate code from the model (separate concern)
    let schemeStruct = "";

    // Generate structs in sorted order (deepest first)
    const sortedPaths = Array.from(schemeStructs.keys()).sort(
        (a, b) => b.split("/").length - a.split("/").length,
    );

    for (const pathKey of sortedPaths) {
        const struct = schemeStructs.get(pathKey)!;

        schemeStruct += `struct ${struct.name} {\n`;
        for (const field of struct.fields) {
            schemeStruct += `    ${field.name}: ${field.type},\n`;
        }
        schemeStruct += `}\n\n`;
    }

    // Main scheme struct is special - it gets top-level fields
    const schemeName = `${collectionData.formattedName}-Scheme`;
    // Change this variable name to avoid redeclaration
    let mainSchemeStruct = `struct ${schemeName} {\n`;

    // Add all direct children of root
    for (const [childName, childNode] of variableTree.children.entries()) {
        if (childNode.valuesByMode) {
            // Direct property
            mainSchemeStruct += `    ${childName}: ${getSlintType(childNode.type || "COLOR")},\n`;
        } else if (childNode.children.size > 0) {
            // Reference to child scheme
            mainSchemeStruct += `    ${childName}: ${collectionData.formattedName}-${childName}-Scheme,\n`;
        }
    }

    mainSchemeStruct += `}\n\n`;

    // Generate the mode struct
    const schemeModeName = `${collectionData.formattedName}-Scheme-Mode`;
    let schemeModeStruct = `struct ${schemeModeName} {\n`;

    for (const mode of collectionData.modes) {
        schemeModeStruct += `    ${mode}: ${schemeName},\n`;
    }

    schemeModeStruct += `}\n\n`;

    // Generate the instance initialization
    let schemeInstance = `    out property <${schemeModeName}> mode: {\n`;

    for (const mode of collectionData.modes) {
        schemeInstance += `        ${mode}: {\n`;

        // Function to add hierarchical values
        function addHierarchicalValues(
            node: VariableNode = variableTree,
            path: string[] = [],
            currentIndent: string = "            ",
        ) {
            for (const [childName, childNode] of node.children.entries()) {
                const currentPath = [...path, childName];

                if (childNode.children.size > 0) {
                    // This is a struct node
                    schemeInstance += `${currentIndent}${childName}: {\n`;
                    // Recursively add its children
                    addHierarchicalValues(
                        childNode,
                        currentPath,
                        currentIndent + "    ",
                    );
                    schemeInstance += `${currentIndent}},\n`;
                } else if (childNode.valuesByMode) {
                    // This is a leaf value
                    // Check if this is the renamed "mode" variable at root level
                    const propertyPath = currentPath.join(".");
                    const referencePath =
                        hasRootModeVariable && propertyPath === "mode"
                            ? "mode-var"
                            : propertyPath;

                    schemeInstance += `${currentIndent}${childName}: ${collectionData.formattedName}.${referencePath}.${mode},\n`;
                }
            }
        }
        // Build the mode instance
        addHierarchicalValues();
        schemeInstance += `        },\n`;
    }

    // Close the mode instance
    schemeInstance += `    };\n`;

    // Generate the current scheme property with current-mode toggle
    let currentSchemeInstance = `    in-out property <${collectionData.formattedName}Mode> current-mode: ${[...collectionData.modes][0]};\n`;

    // Add the current property that dynamically selects based on the enum
    currentSchemeInstance += `    out property <${schemeName}> current: `;

    // for mode specific disentanglement
    const modePropertyName = hasRootModeVariable ? "mode-var" : "mode";

    const modeArray = [...collectionData.modes];
    if (modeArray.length === 0) {
        // No modes - empty object
        currentSchemeInstance += `{};\n\n`;
    } else if (modeArray.length === 1) {
        // One mode - direct reference
        currentSchemeInstance += `root.${modePropertyName}.${modeArray[0]};\n\n`;
    } else {
        // Multiple modes - build a ternary chain
        let expression = "";

        // Build the ternary chain from the first mode to the second-to-last
        for (let i = 0; i < modeArray.length - 1; i++) {
            if (i > 0) {
                expression += "\n        ";
            }
            expression += `current-mode == ${collectionData.formattedName}Mode.${modeArray[i]} ? root.mode.${modeArray[i]} : `;
        }

        // Add the final fallback (last mode)
        expression += `root.mode.${modeArray[modeArray.length - 1]}`;

        // Add the expression with proper indentation
        currentSchemeInstance += `\n        ${expression};\n\n`;
    }

    return {
        // Combine both scheme structs in the return value
        schemeStruct: schemeStruct + mainSchemeStruct,
        schemeModeStruct: schemeModeStruct,
        schemeInstance: schemeInstance,
        currentSchemeInstance: currentSchemeInstance,
    };
}

function collectMultiModeStructs(
    node: VariableNode,
    collectionData: { modes: Set<string>; formattedName: string },
    structDefinitions: string[],
) {
    if (collectionData.modes.size <= 1) {
        return;
    }

    // Define all Slint types we want to support
    const allSlintTypes = ["brush", "length", "string", "bool"];

    // Generate a struct for each type regardless of whether it's used
    for (const slintType of allSlintTypes) {
        const structName = `${collectionData.formattedName}_mode${collectionData.modes.size}_${slintType}`;

        let structDef = `struct ${structName} {\n`;
        for (const mode of collectionData.modes) {
            structDef += `    ${mode}: ${slintType},\n`;
        }
        structDef += `}\n\n`;

        structDefinitions.push(structDef);
    }

    // Still scan the tree for any other types we might have missed (for future proofing)
    function findUniqueTypeConfigs(node: VariableNode) {
        for (const [_childName, childNode] of node.children.entries()) {
            if (childNode.valuesByMode && childNode.valuesByMode.size > 0) {
                const slintType = getSlintType(childNode.type || "COLOR");
                // Skip if we already added this type
                if (!allSlintTypes.includes(slintType)) {
                    // Add a struct for this additional type
                    const structName = `${collectionData.formattedName}_mode${collectionData.modes.size}_${slintType}`;

                    let structDef = `struct ${structName} {\n`;
                    for (const mode of collectionData.modes) {
                        structDef += `    ${mode}: ${slintType},\n`;
                    }
                    structDef += `}\n\n`;

                    structDefinitions.push(structDef);
                }
            } else if (childNode.children.size > 0) {
                findUniqueTypeConfigs(childNode);
            }
        }
    }

    // Look for any additional types
    findUniqueTypeConfigs(node);
}
function generateReadmeContent(exportInfo: {
    renamedVariables: Set<string>;
    circularReferences: Set<string>;
    warnings: Set<string>;
    features: Set<string>; // You can add specific features detected if needed
    collections: Set<string>;
}): string {
    let content = "# Figma Design Tokens Export\n\n";
    content += `Generated on ${new Date().toISOString().split("T")[0]}\n\n`;
    content += "Instructions for use: \n";
    content +=
        "This library attempts to export a working set of slint design tokens.  They are constructed so that the variables can be called using dot notation. \n";
    content +=
        "If attempting to use colors that change using modes, procure to use the .current. after the initial global.  Then changing the current-mode variable (using the appropriate global's enum) will allow to switch the mode for every variable using .current. \n\n";

    if (exportInfo.collections.size > 0) {
        content += "## Exported Collections\n\n";
        exportInfo.collections.forEach((collection) => {
            content += `- ${collection}\n`;
        });
        content += "\n";
    }

    if (exportInfo.renamedVariables.size > 0) {
        content += "## Renamed Variables\n\n";
        content +=
            "The following variables were renamed to avoid conflicts:\n\n";
        exportInfo.renamedVariables.forEach((variable) => {
            content += `- ${variable}\n`;
        });
        content += "\n";
    }

    if (exportInfo.circularReferences.size > 0) {
        content += "## Circular References\n\n";
        content +=
            "The following circular references were detected and resolved with defaults:\n\n";
        exportInfo.circularReferences.forEach((ref) => {
            content += `- ${ref}\n`;
        });
        content += "\n";
    }

    if (exportInfo.warnings.size > 0) {
        content += "## Warnings\n\n";
        exportInfo.warnings.forEach((warning) => {
            content += `- ${warning}\n`;
        });
    }

    return content;
}
