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
function formatStructName(name: string): string {
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

function sanitizePropertyName(name: string): string {
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

// Extract hierarchy from variable name (e.g. "colors/primary/base" → ["colors", "primary", "base"])
function extractHierarchy(name: string): string[] {
    // First try splitting by slashes (the expected format)
    if (name.includes('/')) {
        return name.split('/').map(part => formatVariableName(part));
    }

    // Default case for simple names
    return [formatVariableName(name)];
}

function createReferenceExpression(
    referenceId: string,
    sourceModeName: string,
    variablePathsById: Map<
        string,
        { collection: string; node: VariableNode; path: string[] }
    >,
    collectionStructure: Map<string, any>,
    currentCollection: string = "",
    currentPath: string[] = [],
): {
    value: string | null;
    importStatement?: string;
    isCircular?: boolean;
    comment?: string;
} {
    // Get the target variable info
    const targetInfo = variablePathsById.get(referenceId);
    if (!targetInfo) {
        console.warn(`Reference path not found for ID: ${referenceId}`);
        return { value: null };
    }

    // Access properties directly from stored data
    const {
        collection: targetCollection,
        node: targetNode,
        path: targetPath,
    } = targetInfo;

    // IMPROVED CIRCULAR REFERENCE DETECTION
    // Now we can directly compare paths as arrays
    let commonParts = 0;
    for (let i = 0; i < Math.min(currentPath.length, targetPath.length); i++) {
        if (currentPath[i] === targetPath[i]) {
            commonParts++;
        } else {
            break;
        }
    }

    // Consider it circular if they share at least 2 path parts
    const isCircularReference =
        commonParts >= 2 && currentPath.length >= 3 && targetPath.length >= 3;

    if (isCircularReference) {
        console.warn(
            `Detected circular reference: ${currentPath.join(".")} -> ${targetPath.join(".")}`,
        );

        // Handle circular reference by resolving the actual value
        try {
            const targetCollectionData =
                collectionStructure.get(targetCollection);
            if (!targetCollectionData) return { value: null };

            // Access value directly from the node
            if (targetNode.valuesByMode) {
                const targetValue =
                    targetNode.valuesByMode.get(sourceModeName) ||
                    targetNode.valuesByMode.values().next().value;

                if (targetValue && !targetValue.value.startsWith("@ref:")) {
                    console.log(
                        `Resolved circular reference to actual value: ${targetValue.value}`,
                    );
                    return {
                        value: targetValue.value,
                        isCircular: true,
                        comment: `Original reference: ${targetCollectionData.formattedName}.${targetPath.join(".")}.${sourceModeName}`,
                    };
                }
            }
        } catch (error) {
            console.error("Error resolving circular reference:", error);
        }

        return { value: null, isCircular: true };
    }

    const isSelfReference =
        targetCollection === currentCollection &&
        targetPath.length > currentPath.length &&
        targetPath
            .slice(0, currentPath.length)
            .every((part, i) => part === currentPath[i]);

    if (isSelfReference) {
        console.log(
            `Detected self-reference: ${currentPath.join(".")} -> ${targetPath.join(".")}`,
        );

        // Get the actual value instead of creating a reference
        if (targetNode.valuesByMode) {
            const targetValue =
                targetNode.valuesByMode.get(sourceModeName) ||
                targetNode.valuesByMode.values().next().value;

            if (targetValue && !targetValue.value.startsWith("@ref:")) {
                return {
                    value: targetValue.value,
                    comment: `Self-reference resolved: ${targetPath.join(".")}.${sourceModeName}`,
                };
            }
        }
    }

    // Get the target collection data
    const targetCollectionData = collectionStructure.get(targetCollection);
    if (!targetCollectionData) {
        console.warn(
            `Collection not found: ${targetCollection}`,
            "Available collections:",
            Array.from(collectionStructure.keys()).join(", "),
        );
        return { value: null };
    }

    // Check if this is a cross-collection reference
    const isCrossCollection = targetCollection !== currentCollection;
    console.log(`Is cross-collection reference: ${isCrossCollection}`);

    // Get all modes from target collection
    const targetModes = [...targetCollectionData.modes];
    if (targetModes.length === 0) {
        console.warn(
            `No modes found in target collection: ${targetCollection}`,
        );
        return { value: null };
    }

    // Find the appropriate mode
    const targetModeName =
        targetModes.find(
            (mode) =>
                sanitizeModeForEnum(mode) ===
                sanitizeModeForEnum(sourceModeName),
        ) ||
        targetModes.find((mode) => mode === sourceModeName) ||
        targetModes[0];

    const sanitizedMode = sanitizeModeForEnum(targetModeName);

    // If this is a cross-collection reference, we need an import statement
    let importStatement: string | undefined = undefined;
    if (isCrossCollection) {
        if (targetCollectionData.modes.size > 1) {
            importStatement = `import { ${targetCollectionData.formattedName}, ${targetCollectionData.formattedName}Mode } from "${targetCollectionData.formattedName}.slint";\n`;
        } else {
            importStatement = `import { ${targetCollectionData.formattedName} } from "${targetCollectionData.formattedName}.slint";\n`;
        }
    }

    // Build the property path using the stored path array
    // Ensure all path components are sanitized
    const propertyPath = targetPath
        .map((part) => sanitizePropertyName(part))
        .join(".");

    // Format the reference expression
    let referenceExpr = "";
    if (targetCollectionData.modes.size > 1) {
        referenceExpr = `${targetCollectionData.formattedName}.${propertyPath}.${sanitizedMode}`;
    } else {
        referenceExpr = `${targetCollectionData.formattedName}.${propertyPath}`;
    }

    console.log(`Created reference expression: ${referenceExpr}`);

    return {
        value: referenceExpr,
        importStatement: importStatement,
    };
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

// For Figma Plugin - Export function with hierarchical structure
// Export each collection to a separate virtual file
export async function exportFigmaVariablesToSeparateFiles(): Promise<
    Array<{ name: string; content: string }>
> {
    console.log("Starting variable export...");

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
                    if (!variable) continue;
                    if (
                        !variable.valuesByMode ||
                        Object.keys(variable.valuesByMode).length === 0
                    )
                        continue;

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
                        console.log(
                            `Variable ${variable.name} (${variable.id}) has value type: ${typeof value} value: ${JSON.stringify(value)}`,
                        );
                        if (!modeInfo) continue;

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
                            console.log(
                                `Final formatted value stored for ${sanitizedRowName}.${modeName}: ${formattedValue}`,
                            );

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

        // FINALLY process references after all collections are initialized
        for (const collection of variableCollections) {
            const collectionName = formatPropertyName(collection.name);

            for (const [rowName, columns] of collectionStructure
                .get(collectionName)!
                .variables.entries()) {
                for (const [colName, data] of columns.entries()) {
                    if (data.refId) {
                        const refResult = createReferenceExpression(
                            data.refId,
                            colName,
                            variablePathsById,
                            collectionStructure,
                            collectionName,
                            rowName.split("/"), // Convert string to array by splitting on path separator
                        );

                        if (refResult.value) {
                            // Update to handle circular references
                            const valueToStore = refResult.value;
                            const updatedValue = {
                                value: valueToStore,
                                type: data.type,
                                refId: refResult.isCircular
                                    ? undefined
                                    : data.refId, // Remove refId if resolved circular ref
                                comment: refResult.comment, // Add this line to preserve the comment
                            };

                            collectionStructure
                                .get(collectionName)!
                                .variables.get(rowName)!
                                .set(colName, updatedValue);
                        } else {
                            // Fallback...
                        }

                        // When processing references:
                        if (
                            refResult.importStatement &&
                            !refResult.isCircular
                        ) {
                            requiredImports.add(refResult.importStatement);
                        }
                    }
                }
            }
        }

        for (const [
            collectionName,
            collectionData,
        ] of collectionStructure.entries()) {
            // Skip collections with no variables
            if (collectionData.variables.size === 0) {
                console.log(`Skipping empty collection: ${collectionName}`);
                continue;
            }

            // Generate the enum for modes
            let content = `// Generated Slint file for ${collectionData.name}\n\n`;

            // // Generate global singleton
            // content += `export global ${collectionData.formattedName} {\n`;

            // Build a hierarchical tree from the flat variables
            const variableTree: VariableNode = {
                name: "root",
                children: new Map(),
            };

            // Process each variable to build the tree
            for (const [varName, modes] of collectionData.variables.entries()) {
                // Split the path by forward slashes to get the hierarchy
                const parts = extractHierarchy(varName);
                
                // Navigate the tree and create nodes as needed
                let currentNode = variableTree;

                // Process all parts except the last one (which is the property name)
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

                // The last part is the property name
                const propertyName = sanitizePropertyName(
                    parts[parts.length - 1],
                );

                // Create the leaf node with the value
                if (!currentNode.children.has(propertyName)) {
                    // Create a new Map for valuesByMode
                    const valuesByMode = new Map<
                        string,
                        { value: string; refId?: string; comment?: string }
                    >();

                    // Get the type from the first mode (or default to 'COLOR' if undefined)
                    const firstModeValue = modes.values().next().value;
                    const type = firstModeValue?.type || "COLOR";

                    // Process each mode's value
                    for (const [modeName, valueData] of modes.entries()) {
                        valuesByMode.set(modeName, {
                            value: valueData.value,
                            refId: valueData.refId,
                            comment: valueData.comment, // Add this to preserve comments
                        });
                    }

                    // Add the node to the tree
                    currentNode.children.set(propertyName, {
                        name: propertyName,
                        type: type,
                        valuesByMode: valuesByMode,
                        children: new Map(),
                    });
                }
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
            ): {
                structs: string;
                instances: string;
            } {
                // Data structures to hold our model
                const structDefinitions = new Map<string, StructDefinition>();
                const propertyInstances = new Map<string, PropertyInstance>();

                // First pass: Build the struct model
                function buildStructModel(
                  node: VariableNode,
                  path: string[] = [],
              ) {
                  if (node.name === "root") {
                      console.log("DEBUG: Processing root node children:", Array.from(node.children.keys()));
                      
                      for (const [childName, childNode] of node.children.entries()) {
                          console.log(`DEBUG: Root child: ${childName}, hasChildren: ${childNode.children.size > 0}`);
                          
                          if (childNode.children.size > 0) {
                              // Add debug output
                              console.log(`DEBUG: Creating struct for: ${childName}`);
                              
                              // Define a struct for this node
                              const structName = sanitizePropertyName(childName);
                              structDefinitions.set(childName, {
                                  name: `${collectionData.formattedName}_${structName}`,
                                  fields: [],
                                  path: [childName],
                              });
              
                              buildStructModel(childNode, [childName]);
                          }
                      }
                      return;
                  }
                    const currentPath = [...path];
                    const pathKey = currentPath.join("/");
                    const typeName = currentPath.join("_");

                    // Only generate struct for nodes with children
                    if (node.children.size > 0) {
                        // Process child nodes first (recursive definition from deepest to shallowest)
                        for (const [
                            childName,
                            childNode,
                        ] of node.children.entries()) {
                            if (childNode.children.size > 0) {
                                buildStructModel(childNode, [
                                    ...currentPath,
                                    childName,
                                ]);
                            }
                        }

                        // Create or update the struct definition
                        if (!structDefinitions.has(pathKey)) {
                            structDefinitions.set(pathKey, {
                                name: typeName,
                                fields: [],
                                path: [...currentPath],
                            });
                        }

                        // Add fields to the struct
                        for (const [
                            childName,
                            childNode,
                        ] of node.children.entries()) {
                            // Skip empty property names
                            if (!childName || childName.trim() === "") {
                                continue;
                            }

                            const sanitizedChildName =
                                sanitizePropertyName(childName);
                            if (childNode.valuesByMode) {
                                const slintType = getSlintType(
                                    childNode.type || "COLOR",
                                );
                                const isMultiMode =
                                    collectionData.modes.size > 1;

                                structDefinitions.get(pathKey)!.fields.push({
                                    name: sanitizedChildName,
                                    type: isMultiMode
                                        ? `mode${collectionData.modes.size}_${slintType}`
                                        : slintType,
                                    isMultiMode,
                                });
                            } else if (childNode.children.size > 0) {
                                // Reference to another struct
                                const childPath = [...currentPath, childName];
                                structDefinitions.get(pathKey)!.fields.push({
                                    name: sanitizedChildName,
                                    // Instead of flattening with underscores, use a nested type:
                                    type: `${path.join("_")}_${sanitizedChildName}` // Create proper hierarchical reference
                                });
                            }
                        }
                    }
                    console.log("BUILD STRUCT MODEL:",structDefinitions );

                }

                // Build the instance model
                function buildInstanceModel(
                  node: VariableNode,
                  path: string[] = [],
              ) {
                  if (node.name === "root") {
                      console.log("DEBUG: buildInstanceModel processing root children:", Array.from(node.children.keys()));
                      
                      for (const [childName, childNode] of node.children.entries()) {
                          console.log(`DEBUG: Instance for child: ${childName}, hasChildren: ${childNode.children.size > 0}`);
                                          const sanitizedChildName =
                                sanitizePropertyName(childName);

                            if (childNode.children.size > 0) {
                                // Create container object with proper children map
                                propertyInstances.set(sanitizedChildName, {
                                    name: sanitizedChildName,
                                    type: `${collectionData.formattedName}_${sanitizedChildName}`, // Properly reference the struct
                                    children: new Map(),
                                });

                                // Process children
                                buildInstanceModel(childNode, [
                                    sanitizedChildName,
                                ]);
                            } else if (childNode.valuesByMode) {
                                // Direct value property
                                const slintType = getSlintType(
                                    childNode.type || "COLOR",
                                );
                                const instance: PropertyInstance = {
                                    name: sanitizedChildName,
                                    type: slintType,
                                };

                                if (collectionData.modes.size > 1) {
                                    instance.isMultiMode = true;
                                    instance.values = new Map();

                                    for (const [
                                        modeName,
                                        data,
                                    ] of childNode.valuesByMode.entries()) {
                                        instance.values.set(
                                            modeName,
                                            data.value,
                                        );
                                        if (data.comment) {
                                            instance.comment = data.comment;
                                        }
                                    }
                                } else {
                                    // Single mode
                                    const firstMode = childNode.valuesByMode
                                        .values()
                                        .next().value;
                                    instance.values = new Map();
                                    instance.values.set(
                                        "value",
                                        firstMode?.value || "",
                                    );
                                }

                                propertyInstances.set(
                                    sanitizedChildName,
                                    instance,
                                );
                            }
                        }
                        return;
                    }

                    // For non-root nodes
                    const pathKey = path.join("/");

                    for (const [
                        childName,
                        childNode,
                    ] of node.children.entries()) {
                        const sanitizedChildName =
                            sanitizePropertyName(childName);
                        const childPath = [...path, sanitizedChildName];
                        const childPathKey = childPath.join("/");

                        if (childNode.children.size > 0) {
                            // Get parent instance
                            const parentInstance =
                                propertyInstances.get(pathKey);
                            if (!parentInstance || !parentInstance.children)
                                continue;

                            // Add child instance to parent
                            parentInstance.children.set(sanitizedChildName, {
                                name: sanitizedChildName,
                                type: childPath.join("_"),
                                children: new Map(),
                            });

                            buildInstanceModel(childNode, childPath);
                        } else if (childNode.valuesByMode) {
                            // Get parent instance
                            const parentInstance =
                                propertyInstances.get(pathKey);
                            if (!parentInstance || !parentInstance.children)
                                continue;

                            const slintType = getSlintType(
                                childNode.type || "COLOR",
                            );
                            const instance: PropertyInstance = {
                                name: sanitizedChildName,
                                type: slintType,
                            };

                            if (collectionData.modes.size > 1) {
                                instance.isMultiMode = true;
                                instance.values = new Map();

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
                                const firstMode = childNode.valuesByMode
                                    .values()
                                    .next().value;
                                instance.values = new Map();
                                instance.values.set(
                                    "value",
                                    firstMode?.value || "",
                                );
                            }

                            parentInstance.children.set(
                                sanitizedChildName,
                                instance,
                            );
                        }
                    }
                    console.log("BUILD INSTANCE MODEL:",propertyInstances );

                }


                // First: Generate multi-mode structs
                const multiModeStructs: string[] = [];
                collectMultiModeStructs(
                    variableTree,
                    collectionData,
                    multiModeStructs,
                );

                // Second: Build struct model
                buildStructModel(variableTree);

                // Third: Build instance model
                buildInstanceModel(variableTree);

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

                function generateInstanceCode(
                    instance: PropertyInstance,
                    path: string[] = [],
                    indent: string = "    ",
                ): string {
                    let result = "";

                    if (path.length === 0) {
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

                                for (const [
                                    modeName,
                                    value,
                                ] of instance.values.entries()) {
                                    if (instance.comment) {
                                        result += `${indent}    // ${instance.comment}\n`;
                                    }
                                    result += `${indent}    ${modeName}: ${value},\n`;
                                }

                                result += `${indent}};\n\n`;
                            } else {
                                // Single mode property
                                const value =
                                    instance.values.get("value") || "";

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

                                for (const [
                                    modeName,
                                    value,
                                ] of instance.values.entries()) {
                                    if (instance.comment) {
                                        result += `${indent}    // ${instance.comment}\n`;
                                    }
                                    result += `${indent}    ${modeName}: ${value},\n`;
                                }

                                result += `${indent}},\n`;
                            } else {
                                // Single mode nested value
                                const value =
                                    instance.values.get("value") || "";

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

                // Generate all root level instances
                for (const [
                    instanceName,
                    instance,
                ] of propertyInstances.entries()) {
                    instancesCode += generateInstanceCode(instance);
                }

                return {
                    structs: structsCode,
                    instances: instancesCode,
                };
            }
            // Get structures and instances
            const { structs, instances } = generateStructsAndInstances(
                variableTree,
                collectionData.formattedName,
            );

            // Generate the scheme structs (only for multi-mode collections)
            let schemeStruct = "";
            let schemeModeStruct = "";
            let schemeInstance = "";
            let currentSchemeInstance = "";

            if (collectionData.modes.size > 1) {
                const schemeResult = generateSchemeStructs(
                    variableTree,
                    collectionData,
                );
                schemeStruct = schemeResult.schemeStruct;
                schemeModeStruct = schemeResult.schemeModeStruct;
                schemeInstance = schemeResult.schemeInstance;
                currentSchemeInstance = schemeResult.currentSchemeInstance; // ADD THIS LINE to capture the value
            }

            // Start with file comment

            // Now filter imports based on what's actually used in the instances
            for (const importStmt of requiredImports) {
                // Extract the collection name from the import statement
                const match = importStmt.match(/import { ([^,}]+)/);
                if (match) {
                    const targetCollection = match[1].trim();

                    // Skip self-imports
                    if (targetCollection === collectionData.formattedName) {
                        continue;
                    }

                    // Only include if there's an actual reference to this collection in the instances
                    if (
                        instances.includes(`${targetCollection}.`) ||
                        instances.includes(`${targetCollection}(`)
                    ) {
                        content += importStmt;
                    }
                }
            }

            // Add a blank line after imports if any were added
            if (content.includes("import ")) {
                content += "\n";
            }

            // Add the mode enum if needed
            if (collectionData.modes.size > 1) {
                content += `export enum ${collectionData.formattedName}Mode {\n`;
                for (const mode of collectionData.modes) {
                    content += `    ${mode},\n`;
                }
                content += `}\n\n`;
            }

            // Build the content
            content += structs;
            content += schemeStruct;
            content += schemeModeStruct;
            content += `export global ${collectionData.formattedName} {\n`;
            content += instances;
            content += schemeInstance;
            if (collectionData.modes.size > 1) {
                content += currentSchemeInstance;
            }
            content += `}\n`;

            // Add file to exported files
            exportedFiles.push({
                name: `${collectionData.formattedName}.slint`,
                content: content,
            });

            console.log(
                `Generated file for collection: ${collectionData.name}`,
            );
        }

        for (const file of exportedFiles) {
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
        return exportedFiles;
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
                    schemeInstance += `${currentIndent}${childName}: ${collectionData.formattedName}.${currentPath.join(".")}.${mode},\n`;
                }
            }
        }

        // Build the mode instance
        addHierarchicalValues();
        schemeInstance += `        },\n`;
    }

    // Close the mode instance
    schemeInstance += `    };\n`;

    // 6. Generate the current scheme property with current-scheme toggle
    let currentSchemeInstance = `    in-out property <${collectionData.formattedName}Mode> current-scheme: ${[...collectionData.modes][0]};\n`;

    // Add the current-mode property that dynamically selects based on the enum
    currentSchemeInstance += `    out property <${schemeName}> current-mode: `;

    const modeArray = [...collectionData.modes];
    if (modeArray.length === 0) {
        // No modes - empty object
        currentSchemeInstance += `{};\n\n`;
    } else if (modeArray.length === 1) {
        // One mode - direct reference
        currentSchemeInstance += `root.mode.${modeArray[0]};\n\n`;
    } else {
        // Multiple modes - build a ternary chain
        let expression = "";

        // Build the ternary chain from the first mode to the second-to-last
        for (let i = 0; i < modeArray.length - 1; i++) {
            if (i > 0) expression += "\n        ";
            expression += `current-scheme == ${collectionData.formattedName}Mode.${modeArray[i]} ? root.mode.${modeArray[i]} : `;
        }

        // Add the final fallback (last mode)
        expression += `root.mode.${modeArray[modeArray.length - 1]}`;

        // Add the expression with proper indentation
        currentSchemeInstance += `\n        ${expression};\n\n`;
    }

    // Now add the current property that references current-mode
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
                currentSchemeInstance += `${currentIndent}${childName}: current-mode.${dotPath},\n`;
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
    if (collectionData.modes.size <= 1) return;

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
