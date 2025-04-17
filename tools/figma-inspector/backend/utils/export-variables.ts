// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// // Helper to convert Figma color values to Slint format
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
        .replace(/([a-z])([A-Z])/g, "$1-$2") // Add hyphens between camelCase
        .replace(/\s+/g, "-") // Convert spaces to hyphens
        .replace(/--+/g, "-") // Normalize multiple consecutive hyphens to single
        .toLowerCase(); // Convert to lowercase

    return sanitizedName;
}

// Helper to format property name for Slint (kebab-case) with sanitization
function formatPropertyName(name: string): string {
    // Handle names starting with "." - remove the dot
    let sanitizedName = name.startsWith(".") ? name.substring(1) : name;
    sanitizedName = sanitizedName.replace(/^[_\-\.]+/, "");

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
        .replace(/([a-z])([A-Z])/g, "$1-$2") // Add hyphens between camelCase
        .replace(/\s+/g, "-") // Convert spaces to hyphens
        .replace(/--+/g, "-") // Normalize multiple consecutive hyphens to single
        .toLowerCase(); // Convert to lowercase

    return sanitizedName;
}

// Helper to format variable name for Slint (kebab-case)
function formatVariableName(name: string): string {
    let sanitizedName = name.startsWith(".") ? name.substring(1) : name;
    sanitizedName = sanitizedName.replace(/^[_\-\.]+/, "");

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
        .replace(/([a-z])([A-Z])/g, "$1-$2") // Add hyphens between camelCase
        .replace(/\s+/g, "-") // Convert spaces to hyphens
        .replace(/--+/g, "-") // Normalize multiple consecutive hyphens to single
        .toLowerCase(); // Convert to lowercase

    return sanitizedName;
}

export function sanitizePropertyName(name: string): string {
    // Check if starts with a digit
    if (/^\d/.test(name)) {
        return `_${name}`;
    }
    return name;
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
// helper to detect cycles in dependency graph
function detectCycle(dependencies: Map<string, Set<string>>): boolean {
    const visiting = new Set<string>(); // Nodes currently in the recursion stack
    const visited = new Set<string>(); // Nodes already fully explored

    function dfs(node: string): boolean {
        visiting.add(node);
        visited.add(node);

        const neighbors = dependencies.get(node) || new Set();
        for (const neighbor of neighbors) {
            if (!visited.has(neighbor)) {
                if (dfs(neighbor)) {
                    return true; // Cycle found downstream
                }
            } else if (visiting.has(neighbor)) {
                // Back edge detected - cycle found!
                console.warn(
                    `Dependency cycle detected involving: ${node} -> ${neighbor}`,
                );
                return true;
            }
        }

        visiting.delete(node); // Remove node from current path stack
        return false; // No cycle found from this node
    }

    // Check for cycles starting from each node
    for (const node of dependencies.keys()) {
        if (!visited.has(node)) {
            if (dfs(node)) {
                return true; // Cycle found
            }
        }
    }

    return false; // No cycles found in the entire graph
}
// Extract hierarchy from variable name (e.g. "colors/primary/base" → ["colors", "primary", "base"])
export function extractHierarchy(name: string): string[] {
    // First try splitting by slashes (the expected format)
    if (name.includes("/")) {
        return name.split("/").map((part) => formatVariableName(part));
    }

    // Default case for simple names
    return [formatVariableName(name)];
}

function createReferenceExpression(
    referenceId: string,
    sourceModeName: string, // The mode of the variable *requesting* the reference
    variablePathsById: Map<
        string,
        { collection: string; node: VariableNode; path: string[] }
    >,
    collectionStructure: Map<string, any>, // Assuming 'any' for now, replace with specific type if available
    currentCollection: string, // The collection of the variable *requesting* the reference
    currentPath: string[], // The path of the variable *requesting* the reference
    resolutionStack: string[] = [], // Stack to detect loops
    exportAsSingleFile: boolean, // Parameter indicating export mode
    exportInfo: {
        renamedVariables: Set<string>;
        circularReferences: Set<string>;
        warnings: Set<string>;
        features: Set<string>; // You can add specific features detected if needed
        collections: Set<string>;
    },
): {
    value: string | null;
    importStatement?: string;
    isCircular?: boolean;
    comment?: string;
} {
    // --- 1. Get Target Info ---
    const targetInfo = variablePathsById.get(referenceId);
    if (!targetInfo) {
        console.warn(`Reference path not found for ID: ${referenceId}`);
        return {
            value: null,
            comment: `Unresolved reference ID: ${referenceId}`,
        };
    }

    const {
        collection: targetCollection,
        node: targetNode,
        path: targetPath,
    } = targetInfo;

    // --- 2. Loop Detection using Resolution Stack ---
    const targetIdentifier = `${targetCollection}.${targetPath.join(".")}`;
    const currentIdentifier = `${currentCollection}.${currentPath.join(".")}`;

    if (resolutionStack.includes(targetIdentifier)) {
        console.warn(
            `Detected cross-collection loop: ${resolutionStack.join(" -> ")} -> ${targetIdentifier}`,
        );
        // Add to exportInfo for README (ensure exportInfo is accessible or passed in)
        exportInfo.circularReferences.add(
            `${resolutionStack.join(" -> ")} -> ${targetIdentifier} (resolved with default)`,
        );

        const targetType = targetNode?.type || "COLOR"; // Use targetNode for type info if available
        const slintType = getSlintType(targetType);
        const defaultValue =
            slintType === "brush"
                ? "#808080"
                : slintType === "length"
                  ? "0px"
                  : slintType === "string"
                    ? '""'
                    : slintType === "bool"
                      ? "false"
                      : "#808080";

        return {
            value: defaultValue,
            isCircular: true,
            comment: `Cross-collection loop resolved with default: ${targetIdentifier}`,
        };
    }

    // --- 3. Resolve Target Value or Nested Reference ---
    const targetCollectionData = collectionStructure.get(targetCollection);
    if (!targetCollectionData) {
        console.warn(`Target collection data not found: ${targetCollection}`);
        return {
            value: null,
            comment: `Missing target collection data: ${targetCollection}`,
        };
    }
    const targetRowName = sanitizeRowName(targetPath.join("/"));
    const targetVariableModesMap =
        targetCollectionData.variables.get(targetRowName);

    if (!targetVariableModesMap) {
        console.warn(
            `Target variable data not found in collectionStructure: ${targetCollection}.${targetRowName}`,
        );
        const targetType = targetNode?.type || "COLOR";
        const slintType = getSlintType(targetType);
        const defaultValue =
            slintType === "brush"
                ? "#FF00FF"
                : // Error color
                  slintType === "length"
                  ? "0px"
                  : slintType === "string"
                    ? '""'
                    : slintType === "bool"
                      ? "false"
                      : "#FF00FF";
        return {
            value: defaultValue,
            isCircular: true,
            comment: `Missing target variable data: ${targetIdentifier}`,
        };
    }

    const targetValueData =
        targetVariableModesMap.get(sourceModeName) ||
        targetVariableModesMap.values().next().value;

    if (targetValueData) {
        // CASE A: Target is another reference (alias)
        if (targetValueData.refId) {
            const nextStack = [...resolutionStack, currentIdentifier];
            return createReferenceExpression(
                targetValueData.refId,
                sourceModeName,
                variablePathsById,
                collectionStructure,
                targetCollection,
                targetPath,
                nextStack,
                exportAsSingleFile,
                exportInfo, // Pass parameter in recursive call
            );
        }
        // CASE B: Target holds a concrete value
        else {
            const sanitizedSourceMode = sanitizeModeForEnum(sourceModeName); // Mode from the requesting variable
            const propertyPath = targetPath
                .map((part) => sanitizePropertyName(part))
                .join(".");

            let referenceExpr = "";
            const targetModes = targetCollectionData.modes as Set<string>; // Cast for type safety/clarity

            if (targetModes.size > 1) {
                // --- Target is multi-mode ---
                let modeToUse: string | undefined = undefined;
                const targetHasSourceMode =
                    targetModes.has(sanitizedSourceMode);

                if (targetHasSourceMode) {
                    // Target has the specific mode we need - use it
                    modeToUse = sanitizedSourceMode;
                } else {
                    // Target is multi-mode BUT doesn't have the source mode.
                    // Fallback to the target's first available mode.
                    const firstTargetMode = targetModes.values().next().value;
                    modeToUse = firstTargetMode; // Assign the first mode

                    if (modeToUse) {
                        const warningMsg = `Mode mismatch: Source mode '${sourceModeName}' not found in target '${targetIdentifier}'. Using target's first mode '${modeToUse}' for reference.`;
                        console.warn(warningMsg);
                        exportInfo.warnings.add(warningMsg);
                    } else {
                        // Should not happen if modes.size > 1, but handle defensively
                        console.error(
                            `Target ${targetIdentifier} has >1 modes but couldn't get first mode.`,
                        );
                    }
                }

                // Append the determined mode if found
                if (modeToUse) {
                    referenceExpr = `${targetCollectionData.formattedName}.${propertyPath}.${modeToUse}`;
                } else {
                    // Fallback if we couldn't determine a mode (should be rare)
                    referenceExpr = `${targetCollectionData.formattedName}.${propertyPath}`;
                }
            } else {
                // --- Target is single-mode ---
                // No mode suffix needed in the reference path
                referenceExpr = `${targetCollectionData.formattedName}.${propertyPath}`;

                // Add a warning if the source was multi-mode but target is single-mode
                const sourceCollectionData =
                    collectionStructure.get(currentCollection);
                if (
                    sourceCollectionData &&
                    sourceCollectionData.modes.size > 1
                ) {
                    const warningMsg = `Mode mismatch: Source '${currentIdentifier}' is multi-mode but target '${targetIdentifier}' is single-mode. Reference uses base path without mode.`;
                    console.warn(warningMsg);
                    exportInfo.warnings.add(warningMsg);
                }
            }

            // --- Import statement logic (remains the same) ---
            let importStatement: string | undefined = undefined;
            const isCrossCollection = targetCollection !== currentCollection;
            if (isCrossCollection && !exportAsSingleFile) {
                // Use the parameter
                if (targetCollectionData.modes.size > 1) {
                    importStatement = `import { ${targetCollectionData.formattedName}, ${targetCollectionData.formattedName}Mode } from "${targetCollectionData.formattedName}.slint";\n`;
                } else {
                    importStatement = `import { ${targetCollectionData.formattedName} } from "${targetCollectionData.formattedName}.slint";\n`;
                }
            }

            return {
                value: referenceExpr,
                importStatement: importStatement,
                isCircular: false,
                comment: targetValueData.comment,
            };
        }
    }
    // CASE C: No value data found for the target variable in any relevant mode
    else {
        console.warn(
            `Missing value data for ${targetIdentifier} in mode ${sourceModeName} or fallback (using collectionStructure)`,
        );
        const targetType = targetNode?.type || "COLOR";
        const slintType = getSlintType(targetType);
        const defaultValue =
            slintType === "brush"
                ? "#808080"
                : slintType === "length"
                  ? "0px"
                  : slintType === "string"
                    ? '""'
                    : slintType === "bool"
                      ? "false"
                      : "#808080";

        return {
            value: defaultValue,
            isCircular: true,
            comment: `Missing value data resolved with default: ${targetIdentifier}`,
        };
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
// Replace your generateStructCode function with this version:

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
    values?: Map<string, string>;
    comment?: string;
    children?: Map<string, PropertyInstance>;
}

function generateStructsAndInstances(
    variableTree: VariableNode,
    collectionName: string,
    collectionData: any, // Using any for now, but ideally define a proper interface
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
                const sanitizedChildName = sanitizePropertyName(childName);

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
                            ? `mode${collectionData.modes.size}_${slintType}`
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
                    console.warn(
                        `⚠️ COLLISION: Found a root-level "mode" variable in ${collectionData.name}.`,
                    );
                    console.warn(
                        `Renaming Figma "mode" variable to "mode-var" to avoid conflict with scheme mode.`,
                    );
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
                    buildPropertyHierarchy();
                } else if (childNode.valuesByMode) {
                    // Direct value property
                    const slintType: string = getSlintType(
                        childNode.type || "COLOR",
                    );
                    const instance: PropertyInstance = {
                        name: sanitizedChildName,
                        type: slintType,
                    };

                    if (collectionData.modes.size > 1) {
                        instance.isMultiMode = true;
                        instance.values = new Map<string, string>();

                        for (const [
                            modeName,
                            data,
                        ] of childNode.valuesByMode.entries()) {
                            instance.values.set(modeName, data.value);
                            if (data.comment) {
                                instance.comment = data.comment;
                            }
                        }
                    } else {
                        // Single mode
                        const firstMode: ModeValue | undefined =
                            childNode.valuesByMode.values().next().value;
                        instance.values = new Map<string, string>();
                        instance.values.set("value", firstMode?.value || "");
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
                buildPropertyHierarchy();
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
                };

                if (collectionData.modes.size > 1) {
                    instance.isMultiMode = true;
                    instance.values = new Map<string, string>();

                    for (const [
                        modeName,
                        data,
                    ] of childNode.valuesByMode.entries()) {
                        instance.values.set(modeName, data.value);
                        if (data.comment) {
                            instance.comment = data.comment;
                        }
                    }
                } else {
                    // Single mode
                    const firstMode: ModeValue | undefined =
                        childNode.valuesByMode.values().next().value;
                    instance.values = new Map<string, string>();
                    instance.values.set("value", firstMode?.value || "");
                }

                parentInstance.children.set(sanitizedChildName, instance);
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

            // Now process all children of this path
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

        if (path.length === 0) {
            // Special handling for renamed mode variable
            if (instance.name === "mode-var") {
                result += `${indent}// NOTE: This property was renamed from "mode" to "mode-var" to avoid collision\n`;
                result += `${indent}// with the scheme mode property of the same name.\n`;
            }
            // Root level property
            const slintType = instance.isMultiMode
                ? `mode${collectionData.modes.size}_${instance.type}`
                : instance.type;

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
            } else if (instance.values) {
                // Value property
                if (instance.isMultiMode) {
                    // Multi-mode property
                    result += `${indent}out property <${slintType}> ${instance.name}: {\n`;

                    for (const [modeName, value] of instance.values.entries()) {
                        if (instance.comment) {
                            result += `${indent}    // ${instance.comment}\n`;
                        }
                        result += `${indent}    ${modeName}: ${value},\n`;
                    }

                    result += `${indent}};\n\n`;
                } else {
                    // Single mode property
                    const value = instance.values.get("value") || "";

                    if (value.startsWith("@ref:")) {
                        // Unresolved reference
                        result += `${indent}out property <${instance.type}> ${instance.name}: ${
                            instance.type === "brush"
                                ? "#808080"
                                : instance.type === "length"
                                  ? "0px"
                                  : instance.type === "string"
                                    ? '""'
                                    : "false"
                        };\n`;
                    } else {
                        result += `${indent}out property <${instance.type}> ${instance.name}: ${value};\n`;
                    }
                }
            }
        } else {
            // Nested property
            if (instance.children && instance.children.size > 0) {
                // Nested struct
                result += `${indent}${instance.name}: {\n`;

                for (const [
                    childName,
                    childInstance,
                ] of instance.children.entries()) {
                    result += generateInstanceCode(
                        childInstance,
                        [...path, childName],
                        indent + "    ",
                    );
                }

                result += `${indent}},\n\n`;
            } else if (instance.values) {
                // Nested value
                if (instance.isMultiMode) {
                    // Multi-mode nested value
                    result += `${indent}${instance.name}: {\n`;

                    for (const [modeName, value] of instance.values.entries()) {
                        if (instance.comment) {
                            result += `${indent}    // ${instance.comment}\n`;
                        }
                        result += `${indent}    ${modeName}: ${value},\n`;
                    }

                    result += `${indent}},\n`;
                } else {
                    // Single mode nested value
                    const value = instance.values.get("value") || "";

                    if (value.startsWith("@ref:")) {
                        // Unresolved reference
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

    // First: Generate multi-mode structs
    const multiModeStructs: string[] = [];
    collectMultiModeStructs(variableTree, collectionData, multiModeStructs);

    // Second: Build struct model
    buildStructModel(variableTree);

    // Third: Build instance model
    buildInstanceModel(variableTree);
    buildPropertyHierarchy();
    // Fourth: Generate code from the models
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
        // ONLY GENERATE CODE FOR ACTUAL ROOT PROPERTIES
        if (!instanceName.includes("/")) {
            // Only true root properties
            instancesCode += generateInstanceCode(instance);
        }
    }

    return {
        structs: structsCode,
        instances: instancesCode,
    };
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
        features: new Set<string>(), // You can add specific features detected if needed
        collections: new Set<string>(),
    };
    const generatedFiles: Array<{ name: string; content: string }> = []; // Store intermediate files

    try {
        // Get collections asynchronously
        const variableCollections =
            await figma.variables.getLocalVariableCollectionsAsync();

        // Array to store all exported files
        const exportedFiles: Array<{ name: string; content: string }> = [];

        // First, initialize the collection structure for ALL collections
        const collectionStructure = new Map<
            string,
            {
                name: string;
                formattedName: string;
                modes: Set<string>;
                variables: Map<
                    string,
                    Map<
                        string,
                        {
                            value: string;
                            type: string;
                            refId?: string;
                            comment?: string;
                        }
                    >
                >;
            }
        >();

        // Build a global map of variable paths
        const variablePathsById = new Map<
            string,
            { collection: string; node: VariableNode; path: string[] }
        >();

        // Initialize structure for all collections first
        for (const collection of variableCollections) {
            const collectionName = formatPropertyName(collection.name);
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
                    formatPropertyName(mode.name),
                );
                collectionStructure
                    .get(collectionName)!
                    .modes.add(sanitizedMode);
            });
        }

        // THEN process the variables for each collection
        for (const collection of variableCollections) {
            const collectionName = formatPropertyName(collection.name);

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
                            : formatPropertyName(variable.name);

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
                        collectionStructure.get(collectionName)!.variables.set(
                            sanitizedRowName,
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
                        if (!modeInfo) {
                            continue;
                        }

                        const modeName = sanitizeModeForEnum(
                            formatPropertyName(modeInfo.name),
                        );

                        // Format value and track references
                        let formattedValue = "";
                        let refId: string | undefined;

                        // Process different variable types (COLOR, FLOAT, STRING, BOOLEAN)
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
                                "type" in value &&
                                value.type === "VARIABLE_ALIAS"
                            ) {
                                refId = value.id;
                                formattedValue = `@ref:${value.id}`;
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
                                refId = value.id;
                                formattedValue = `@ref:${value.id}`;
                            } else {
                                console.warn(
                                    `Unexpected FLOAT value type: ${typeof value} for ${variable.name}`,
                                );
                                formattedValue = "0px";
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
                                refId = value.id;
                                formattedValue = `@ref:${value.id}`;
                            } else {
                                console.warn(
                                    `Unexpected STRING value type: ${typeof value} for ${variable.name}`,
                                );
                                formattedValue = `""`;
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
                                refId = value.id;
                                formattedValue = `@ref:${value.id}`;
                            } else {
                                console.warn(
                                    `Unexpected BOOLEAN value type: ${typeof value} for ${variable.name}`,
                                );
                                formattedValue = "false";
                            }
                        }

                        collectionStructure
                            .get(collectionName)!
                            .variables.get(sanitizedRowName)!
                            .set(modeName, {
                                value: formattedValue,
                                type: variable.resolvedType,
                                refId: refId,
                                comment: undefined, // Add comment property with undefined default value
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

        // Create a Set to track required imports across all collections
        const requiredImports = new Set<string>();
        const collectionDependencies = new Map<string, Set<string>>();
        // Initialize for all collections to handle collections that import nothing
        for (const collection of variableCollections) {
            const collectionName = formatPropertyName(collection.name);
            collectionDependencies.set(collectionName, new Set<string>());
        }
        // FINALLY process references after all collections are initialized
        for (const collection of variableCollections) {
            const collectionName = formatPropertyName(collection.name);

            for (const [rowName, columns] of collectionStructure
                .get(collectionName)!
                .variables.entries()) {
                for (const [colName, data] of columns.entries()) {
                    // Check if this specific mode value is a reference
                    if (data.refId) {
                        // Prepare arguments for the initial call
                        const currentPathArray = rowName.split("/");
                        const currentIdentifier = `${collectionName}.${currentPathArray.join(".")}`;
                        const initialStack = [currentIdentifier];

                        // Call the reference resolution function
                        const refResult = createReferenceExpression(
                            data.refId,
                            colName,
                            variablePathsById,
                            collectionStructure,
                            collectionName,
                            currentPathArray,
                            initialStack,
                            exportAsSingleFile,
                            exportInfo,
                            // Pass the parameter here
                        );

                        // Process the result
                        if (refResult.value !== null) {
                            const updatedValue = {
                                value: refResult.value,
                                type: data.type,
                                refId: refResult.isCircular
                                    ? undefined
                                    : data.refId,
                                comment: refResult.comment,
                            };

                            collectionStructure
                                .get(collectionName)!
                                .variables.get(rowName)!
                                .set(colName, updatedValue);
                            if (
                                refResult.importStatement &&
                                !refResult.isCircular
                            ) {
                                // Parse target collection from import statement
                                const importMatch =
                                    refResult.importStatement.match(
                                        /import { ([^,}]+)/,
                                    );
                                if (importMatch) {
                                    const targetCollectionName =
                                        importMatch[1].trim();
                                    // Record the dependency: currentCollection -> targetCollectionName
                                    collectionDependencies
                                        .get(collectionName)
                                        ?.add(targetCollectionName);
                                }

                                // Add import statement ONLY if multi-file mode is intended *initially*
                                // We'll decide the final mode later based on cycles.
                                if (!exportAsSingleFile) {
                                    // Check initial user intent
                                    requiredImports.add(
                                        refResult.importStatement,
                                    );
                                }
                            }
                            if (
                                refResult.importStatement &&
                                !refResult.isCircular &&
                                !exportAsSingleFile
                            ) {
                                requiredImports.add(refResult.importStatement);
                            }
                        } else {
                            console.warn(
                                `Failed to resolve reference ${data.refId} for ${currentIdentifier}`,
                            );
                            const fallbackValue = {
                                value: "#FF00FF", // Magenta indicates error
                                type: data.type,
                                refId: undefined,
                                comment: `Failed to resolve reference ${data.refId}`,
                            };
                            collectionStructure
                                .get(collectionName)!
                                .variables.get(rowName)!
                                .set(colName, fallbackValue);
                        }
                    }
                }
            }
        }

        // Check for cycles in the dependency graph BEFORE generating file content
        const hasCycle = detectCycle(collectionDependencies);
        // --- Add Detailed Logging Here ---
        console.log(
            `[DEBUG] Before final flag: exportAsSingleFile = ${exportAsSingleFile}, hasCycle = ${hasCycle}`,
        );
        const finalExportAsSingleFile = exportAsSingleFile || hasCycle;
        console.log(
            `[DEBUG] After final flag: finalExportAsSingleFile = ${finalExportAsSingleFile}`,
        );
        if (hasCycle && !exportAsSingleFile) {
            console.warn(
                "Detected collection dependency cycle. Forcing export as single file.",
            );
            exportInfo.warnings.add(
                "Detected collection dependency cycle. Forcing export as single file.",
            );
        }

        // Generate content for each collection
        for (const [
            collectionName,
            collectionData,
        ] of collectionStructure.entries()) {
            // Skip collections with no variables
            if (collectionData.variables.size === 0) {
                console.log(
                    `Skipping empty collection: ${collectionData.name}`,
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
            // This function should return both the struct definitions and the instance code lines
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

            // --- Assemble the content string for this collection ---
            let content = `// Generated Slint file for ${collectionData.name}\n\n`;

            // Add imports ONLY if final mode is multi-file
            console.log(
                "SINGLE FILE: final:",
                finalExportAsSingleFile,
                "not:",
                exportAsSingleFile,
            );
            if (!finalExportAsSingleFile) {
                // Iterate through all potentially required imports collected earlier
                for (const importStmt of requiredImports) {
                    const requiredTargetMatch =
                        importStmt.match(/import { ([^,}]+)/);
                    const requiredTarget = requiredTargetMatch
                        ? requiredTargetMatch[1].trim()
                        : null;
                    const importSourceFileMatch = importStmt.match(
                        /from "([^"]+)\.slint";/,
                    );
                    const importSourceFile = importSourceFileMatch
                        ? importSourceFileMatch[1]
                        : null;

                    // Check if this import is relevant for the current file
                    if (
                        requiredTarget &&
                        importSourceFile &&
                        importSourceFile !== collectionData.formattedName
                    ) {
                        // Check if the instances string actually uses the target
                        if (
                            instances.includes(`${requiredTarget}.`) ||
                            instances.includes(`${requiredTarget}(`)
                        ) {
                            content += importStmt;
                        }
                    }
                }
                // Add blank line if imports were added
                if (content.includes("import ")) {
                    content += "\n";
                }
            }

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
            content += currentSchemeInstance; // Add current-scheme instance code (if generated)
            content += `}\n`; // Close global block (removed extra \n\n)

            // Store the fully assembled content for this collection
            generatedFiles.push({
                name: `${collectionData.formattedName}.slint`,
                content: content.trim() + "\n", // Trim whitespace and add single trailing newline
            });

            console.log(
                `Generated content for collection: ${collectionData.name}`,
            );
        }

        // Post-process generated files (e.g., replace unresolved refs)
        for (const file of generatedFiles) {
            // Check if there are any unresolved references left
            if (file.content.includes("@ref:")) {
                console.warn(`Found unresolved references in ${file.name}`);

                // Replace unresolved references with appropriate defaults based on context
                file.content = file.content.replace(
                    /(@ref:VariableID:[0-9:]+)/g,
                    (match, reference) => {
                        console.warn(
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
            // --- Combine into a single file by simple concatenation ---
            let combinedContent =
                "// Combined Slint Design Tokens\n// Generated on " +
                new Date().toISOString().split("T")[0] +
                "\n\n";
            if (hasCycle) {
                combinedContent +=
                    "// NOTE: Export forced to single file due to cross-collection import cycle.\n\n";
            }
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
            // --- Use individual files ---
            finalOutputFiles = generatedFiles;
        }

        // --- (4) Add README ---
        // (Keep the README generation logic as it is)
        const readmeContent = generateReadmeContent(exportInfo);
        finalOutputFiles.push({
            name: "README.md",
            content: readmeContent,
        });

        return finalOutputFiles; // Return the final array
    } catch (error) {
        console.error("Error in exportFigmaVariablesToSeparateFiles:", error);
        // Return an error file
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

interface ModeValue {
    value: string;
    refId?: string;
    comment?: string;
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

    // 4. Generate the mode struct
    const schemeModeName = `${collectionData.formattedName}-Scheme-Mode`;
    let schemeModeStruct = `struct ${schemeModeName} {\n`;

    for (const mode of collectionData.modes) {
        schemeModeStruct += `    ${mode}: ${schemeName},\n`;
    }

    schemeModeStruct += `}\n\n`;

    // 5. Generate the instance initialization
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

    // Add the current-scheme property that dynamically selects based on the enum
    currentSchemeInstance += `    out property <${schemeName}> current-scheme: `;

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

    // Now add the current property that references current-scheme
    currentSchemeInstance += `    out property <${schemeName}> current: {\n`;

    // Add properties in the same structure as the scheme
    function addCurrentValues(
        node: VariableNode = variableTree,
        path: string[] = [],
        currentIndent: string = "        ",
    ) {
        for (const [childName, childNode] of node.children.entries()) {
            const currentPath = [...path, childName];

            if (childNode.children.size > 0) {
                // This is a nested struct
                currentSchemeInstance += `${currentIndent}${childName}: {\n`;
                addCurrentValues(
                    childNode,
                    currentPath,
                    currentIndent + "    ",
                );
                currentSchemeInstance += `${currentIndent}},\n`;
            } else if (childNode.valuesByMode) {
                // Use dot notation for property access in Slint code
                const dotPath = currentPath.join(".");

                // This is a leaf value
                currentSchemeInstance += `${currentIndent}${childName}: current-scheme.${dotPath},\n`;
            }
        }
    }

    // Build the current structure
    addCurrentValues();

    currentSchemeInstance += `    };\n`;

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
    collectionData: { modes: Set<string> },
    structDefinitions: string[],
) {
    if (collectionData.modes.size <= 1) {
        return;
    }

    // Define all Slint types we want to support
    const allSlintTypes = ["brush", "length", "string", "bool"];

    // Generate a struct for each type regardless of whether it's used
    for (const slintType of allSlintTypes) {
        const structName = `mode${collectionData.modes.size}_${slintType}`;

        let structDef = `struct ${structName} {\n`;
        for (const mode of collectionData.modes) {
            structDef += `    ${mode}: ${slintType},\n`;
        }
        structDef += `}\n\n`;

        structDefinitions.push(structDef);
    }

    // Still scan the tree for any other types we might have missed (for future proofing)
    function findUniqueTypeConfigs(node: VariableNode) {
        for (const [childName, childNode] of node.children.entries()) {
            if (childNode.valuesByMode && childNode.valuesByMode.size > 0) {
                const slintType = getSlintType(childNode.type || "COLOR");
                // Skip if we already added this type
                if (!allSlintTypes.includes(slintType)) {
                    // Add a struct for this additional type
                    const structName = `mode${collectionData.modes.size}_${slintType}`;

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
    content += `Generated on ${new Date().toLocaleDateString()}\n\n`;
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
