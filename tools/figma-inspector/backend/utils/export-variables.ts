// Helper to convert Figma color values to Slint format
function convertColor(color: RGB | RGBA): string {
  const r = Math.round(color.r * 255);
  const g = Math.round(color.g * 255);
  const b = Math.round(color.b * 255);

  if ('a' in color) {
    if (color.a === 1) {
      return `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`;
    } else {
      return `rgba(${r}, ${g}, ${b}, ${color.a})`;
    }
  }

  return `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`;
}

// Helper function to resolve variable references

/**
 * Formats a variable value for use in Slint based on its type
 * @param type The Figma variable type ('COLOR', 'FLOAT', 'STRING', 'BOOLEAN')
 * @param value The raw value from Figma
 * @param defaultValue Whether to return a default value if processing fails
 * @returns An object with formatted value and reference ID if applicable
 */
function formatValueForSlint(
  type: string,
  value: any,
  defaultValue: boolean = false
): { value: string, refId?: string } {
  // If value is null/undefined and we want defaults
  if ((value === null || value === undefined) && defaultValue) {
    return {
      value: type === 'COLOR' ? '#808080' :
        type === 'FLOAT' ? '0px' :
          type === 'BOOLEAN' ? 'false' :
            type === 'STRING' ? '""' : ''
    };
  }

  // Handle each type
  if (type === 'COLOR') {
    if (typeof value === 'object' && value && 'r' in value) {
      return { value: convertColor(value) };
    } else if (typeof value === 'object' && value && 'type' in value && value.type === 'VARIABLE_ALIAS') {
      return { value: `@ref:${value.id}`, refId: value.id };
    }
  } else if (type === 'FLOAT') {
    if (typeof value === 'number') {
      return { value: `${value}px` };
    } else if (typeof value === 'object' && value && 'type' in value && value.type === 'VARIABLE_ALIAS') {
      return { value: `@ref:${value.id}`, refId: value.id };
    }
  } else if (type === 'STRING') {
    if (typeof value === 'string') {
      return { value: `"${value}"` };
    } else if (typeof value === 'object' && value && 'type' in value && value.type === 'VARIABLE_ALIAS') {
      return { value: `@ref:${value.id}`, refId: value.id };
    }
  } else if (type === 'BOOLEAN') {
    if (typeof value === 'boolean') {
      return { value: value ? 'true' : 'false' };
    } else if (typeof value === 'object' && value && 'type' in value && value.type === 'VARIABLE_ALIAS') {
      return { value: `@ref:${value.id}`, refId: value.id };
    }
  }

  // Return default if we couldn't process
  return formatValueForSlint(type, null, true);
}

/**
 * Helper to get the appropriate Slint type for a Figma variable type
 * @param figmaType The Figma variable type ('COLOR', 'FLOAT', 'STRING', 'BOOLEAN')
 * @returns The corresponding Slint type
 */
function getSlintType(figmaType: string): string {
  switch (figmaType) {
    case 'COLOR': return 'brush';
    case 'FLOAT': return 'length';
    case 'STRING': return 'string';
    case 'BOOLEAN': return 'bool';
    default: return 'brush'; // Default to brush
  }
}
// Helper to sanitize enum
function sanitizeEnumName(name: string): string {
  return name
    .replace(/[^a-zA-Z0-9_]/g, '_') // Replace any invalid chars with underscore
    .replace(/^(\d)/, '_$1');       // Prefix with underscore if starts with a digit
}

// Helper to format struct/global name for Slint (PascalCase) with sanitization
function formatStructName(name: string): string {
  // Handle names starting with "." - remove the dot
  let sanitizedName = name.startsWith('.') ? name.substring(1) : name;

  // If that made it empty, use a default
  if (!sanitizedName || sanitizedName.trim() === '') {
    sanitizedName = 'DefaultCollection';
  }

  // Remove ALL special characters before splitting
  sanitizedName = sanitizedName
    .replace(/[\[\](){}\/\\&+*?|^$%@#!~`;:.,<>=\-–—]/g, ' ') // Replace special chars with spaces
    .replace(/\s+/g, ' ') // Normalize spaces
    .trim();

  // Then continue with normal PascalCase conversion
  return sanitizedName
    .split(/[-_\s]/)
    .map(part => part.charAt(0).toUpperCase() + part.slice(1).toLowerCase())
    .join('');
}

// Helper to format property name for Slint (kebab-case) with sanitization
function formatPropertyName(name: string): string {
  // Handle names starting with "." - remove the dot
  let sanitizedName = name.startsWith('.') ? name.substring(1) : name;

  // If that made it empty, use a default
  if (!sanitizedName || sanitizedName.trim() === '') {
    sanitizedName = 'property';
  }

  // Replace & with 'and' before other formatting
  sanitizedName = sanitizedName.replace(/&/g, 'and');

  return sanitizedName
    .replace(/([a-z])([A-Z])/g, '$1-$2')
    .replace(/\s+/g, '-')
    .toLowerCase();
}

// Helper to format variable name for Slint (kebab-case)
function formatVariableName(name: string): string {
  return name
    .replace(/([a-z])([A-Z])/g, '$1-$2')
    .replace(/\s+/g, '-')
    .toLowerCase()
    .trim();
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
    .replace(/&/g, 'and')
    .replace(/\(/g, '_')  // Replace ( with _
    .replace(/\)/g, '_'); // Replace ) with _
}

// 3. Create a comprehensive sanitization function for all identifiers
function sanitizeIdentifier(name: string): string {
  return name
    .replace(/&/g, 'and')
    .replace(/\(/g, '_')
    .replace(/\)/g, '_')
    .replace(/[^a-zA-Z0-9_\-]/g, '_');  // Replace any other invalid chars
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
  return name.replace(/[^a-zA-Z0-9_]/g, '_');
}

// Extract hierarchy from variable name (e.g. "colors/primary/base" → ["colors", "primary", "base"])
function extractHierarchy(name: string): string[] {
  // Split by common hierarchy separators
  const parts = name.split('/');
  return parts.map(part => formatVariableName(part));
}

function createReferenceExpression(
  referenceId: string,
  sourceModeName: string,
  variablePathsById: Map<string, { collection: string, row: string }>,
  collectionStructure: Map<string, any>,
  currentCollection: string = "",
  currentPath: string = ""
): { value: string | null, importStatement?: string, isCircular?: boolean, comment?: string } {

  // Get the target variable path
  const targetPath = variablePathsById.get(referenceId);
  if (!targetPath) {
    console.warn(`Reference path not found for ID: ${referenceId}`);
    return { value: null };
  }

  // IMPROVED CIRCULAR REFERENCE DETECTION
  // Split paths into parts to compare common ancestry
  const currentParts = currentPath.split('_');
  const targetParts = targetPath.row.split('_');

  // Check if they share common ancestry (at least 2 parts)
  let commonParts = 0;
  for (let i = 0; i < Math.min(currentParts.length, targetParts.length); i++) {
    if (currentParts[i] === targetParts[i]) {
      commonParts++;
    } else {
      break;
    }
  }

  // Consider it circular if they share at least 2 path parts
  const isCircularReference = commonParts >= 2 && currentParts.length >= 3 && targetParts.length >= 3;

  if (isCircularReference) {
    console.warn(`Detected circular reference: ${currentPath} -> ${targetPath.row}`);

    // For circular references, we'll resolve the actual value instead of using the reference
    // Get the actual value from the target
    try {
      const targetCollection = collectionStructure.get(targetPath.collection);
      if (!targetCollection) return { value: null };

      const targetValues = targetCollection.variables.get(targetPath.row);
      if (!targetValues) return { value: null };

      const targetMode = targetValues.get(sourceModeName) ||
        targetValues.get(targetValues.keys().next().value); // Fallback to first mode

      if (targetMode && !targetMode.value.startsWith('@ref:')) {
        console.log(`Resolved circular reference to actual value: ${targetMode.value}`);
        // Return the actual value and mark as circular
        return {
          value: targetMode.value,
          isCircular: true,
          comment: `Original reference: ${targetCollection.formattedName}.${targetPath.row}.${sourceModeName}`
        };
      }
    } catch (error) {
      console.error("Error resolving circular reference:", error);
    }

    return { value: null, isCircular: true };
  }
  // Get the target collection
  const targetCollection = collectionStructure.get(targetPath.collection);
  if (!targetCollection) {
    console.warn(`Collection not found: ${targetPath.collection}`,
      "Available collections:", Array.from(collectionStructure.keys()).join(", "));
    return { value: null };
  }

  // Check if this is a cross-collection reference
  const isCrossCollection = targetPath.collection !== currentCollection;
  console.log(`Is cross-collection reference: ${isCrossCollection}`);

  // Get all modes from target collection
  const targetModes = [...targetCollection.modes];
  if (targetModes.length === 0) {
    console.warn(`No modes found in target collection: ${targetPath.collection}`);
    return { value: null };
  }

  // Verify the target variable exists in the collection
  if (!targetCollection.variables.has(targetPath.row)) {
    console.warn(`Variable row ${targetPath.row} not found in collection ${targetPath.collection}`);
    console.error(`Variable not found: ${targetPath.row} in collection ${targetCollection.formattedName}`);
    console.log(`Available variables:`, Array.from(targetCollection.variables.keys()).join(', '));
    return { value: null };
  }

  // First try: exact match with sanitized names
  let targetModeName = targetModes.find(mode =>
    sanitizeModeForEnum(mode) === sanitizeModeForEnum(sourceModeName)
  );

  // Second try: direct match without sanitization
  if (!targetModeName) {
    targetModeName = targetModes.find(mode => mode === sourceModeName);
  }

  // Third try: match the collection's first mode
  if (!targetModeName) {
    targetModeName = targetModes[0];
    console.log(`Using default mode ${targetModeName} for reference to ${referenceId}`);
  }

  // Sanitize both row and column names
  const sanitizedRow = targetPath.row; // Already sanitized when stored
  const sanitizedMode = sanitizeModeForEnum(targetModeName);

  // If this is a cross-collection reference, we need an import statement
  let importStatement;
  if (isCrossCollection) {
    importStatement = `import { ${targetCollection.formattedName} } from "${targetCollection.formattedName}.slint";\n`;
    console.log(`Adding import: ${importStatement.trim()}`);
  }

  // Format the reference expression based on whether target has multiple modes
  let referenceExpr = '';

  // Parse the target path to get proper nested structure
  const pathParts = sanitizedRow.split('_');

  // Sanitize each part individually
  const sanitizedPathParts = pathParts.map(part => {
    // For parts that might already contain dots (like "neutral.600")
    if (part.includes('.')) {
      // Split by dots, sanitize each segment, then rejoin with dots
      return part.split('.')
        .map(segment => sanitizePropertyName(segment))
        .join('.');
    }
    return sanitizePropertyName(part);
  });

  const propertyPath = sanitizedPathParts.join('.');
  // Check if target collection has multiple modes
  if (targetCollection.modes.size > 1) {
    // Use property access syntax instead of function call
    referenceExpr = `${targetCollection.formattedName}.${propertyPath}.${sanitizedMode}`;

    // If this is a cross-collection reference, we need an import for the mode enum too
    if (isCrossCollection) {
      importStatement = `import { ${targetCollection.formattedName}, ${targetCollection.formattedName}Mode } from "${targetCollection.formattedName}.slint";\n`;
    }
  } else {
    // For collections without modes, just use direct property access
    referenceExpr = `${targetCollection.formattedName}.${propertyPath}`;

    if (isCrossCollection) {
      importStatement = `import { ${targetCollection.formattedName} } from "${targetCollection.formattedName}.slint";\n`;
    }
  }

  console.log(`Created reference expression: ${referenceExpr}`);

  return {
    value: referenceExpr,
    importStatement: importStatement
  };
}

interface VariableNode {
  name: string;
  type?: string;
  valuesByMode?: Map<string, { value: string, refId?: string, comment?: string }>;
  children: Map<string, VariableNode>;
}

// For Figma Plugin - Export function with hierarchical structure
// Export each collection to a separate virtual file
export async function exportFigmaVariablesToSeparateFiles(): Promise<Array<{ name: string, content: string }>> {
  console.log("Starting variable export...");

  try {
    // Get collections asynchronously
    const variableCollections = await figma.variables.getLocalVariableCollectionsAsync();

    // Array to store all exported files
    const exportedFiles: Array<{ name: string, content: string }> = [];

    // First, initialize the collection structure for ALL collections
    const collectionStructure = new Map<string, {
      name: string,
      formattedName: string,
      modes: Set<string>,
      variables: Map<string, Map<string, { value: string, type: string, refId?: string, comment?: string }>>
    }>();

    // Build a global map of variable paths
    const variablePathsById = new Map<string, { collection: string, row: string }>();

    // Initialize structure for all collections first
    for (const collection of variableCollections) {
      const collectionName = formatPropertyName(collection.name);
      const formattedCollectionName = formatStructName(collection.name);

      // Initialize the collection structure
      collectionStructure.set(collectionName, {
        name: collection.name,
        formattedName: formattedCollectionName,
        modes: new Set<string>(),
        variables: new Map()
      });

      // Add modes to collection
      collection.modes.forEach(mode => {
        const sanitizedMode = sanitizeModeForEnum(formatPropertyName(mode.name));
        collectionStructure.get(collectionName)!.modes.add(sanitizedMode);
      });
    }

    for (const collection of variableCollections) {
      const collectionName = formatPropertyName(collection.name);

      // Process variables in batches
      const batchSize = 5;
      for (let i = 0; i < collection.variableIds.length; i += batchSize) {
        const batch = collection.variableIds.slice(i, i + batchSize);
        const batchPromises = batch.map(id => figma.variables.getVariableByIdAsync(id));
        const batchResults = await Promise.all(batchPromises);

        for (const variable of batchResults) {
          if (!variable) continue;
          if (!variable.valuesByMode || Object.keys(variable.valuesByMode).length === 0) continue;

          // Use extractHierarchy to break up variable names
          const nameParts = extractHierarchy(variable.name);

          // For flat structure (existing code)
          const propertyName = nameParts.length > 0 ?
            nameParts[nameParts.length - 1] :
            formatPropertyName(variable.name);

          const path = nameParts.length > 1 ?
            nameParts.slice(0, -1).join('_') :
            '';

          const rowName = path ? `${path}_${propertyName}` : propertyName;
          const sanitizedRowName = sanitizeRowName(rowName);

          // Initialize row in variables map
          if (!collectionStructure.get(collectionName)!.variables.has(sanitizedRowName)) {
            collectionStructure.get(collectionName)!.variables.set(
              sanitizedRowName,
              new Map<string, { value: string, type: string, refId?: string }>()
            );
          }

          // Process values for each mode
          for (const [modeId, value] of Object.entries(variable.valuesByMode)) {
            const modeInfo = collection.modes.find(m => m.modeId === modeId);
            // console.log(`Variable ${variable.name} (${variable.id}) has value type: ${typeof value} value: ${JSON.stringify(value)}`);
            if (!modeInfo) continue;

            const modeName = sanitizeModeForEnum(formatPropertyName(modeInfo.name));

            // Format value and track references
            let formattedValue = '';
            let refId: string | undefined;

            // Process different variable types (COLOR, FLOAT, STRING, BOOLEAN)
            if (variable.resolvedType === 'COLOR') {
              if (typeof value === 'object' && value && 'r' in value) {
                formattedValue = convertColor(value);
              } else if (typeof value === 'object' && value && 'type' in value && value.type === 'VARIABLE_ALIAS') {
                refId = value.id;
                formattedValue = `@ref:${value.id}`;
              }
            } else if (variable.resolvedType === 'FLOAT') {
              if (typeof value === 'number') {
                formattedValue = `${value}px`;
              } else if (typeof value === 'object' && value && 'type' in value && value.type === 'VARIABLE_ALIAS') {
                refId = value.id;
                formattedValue = `@ref:${value.id}`;
              } else {
                console.warn(`Unexpected FLOAT value type: ${typeof value} for ${variable.name}`);
                formattedValue = "0px";
              }
            } else if (variable.resolvedType === 'STRING') {
              if (typeof value === 'string') {
                formattedValue = `"${value}"`;
              } else if (typeof value === 'object' && value && 'type' in value && value.type === 'VARIABLE_ALIAS') {
                refId = value.id;
                formattedValue = `@ref:${value.id}`;
              } else {
                console.warn(`Unexpected STRING value type: ${typeof value} for ${variable.name}`);
                formattedValue = `""`;
              }
            } else if (variable.resolvedType === 'BOOLEAN') {
              if (typeof value === 'boolean') {
                formattedValue = value ? 'true' : 'false';
              } else if (typeof value === 'object' && value && 'type' in value && value.type === 'VARIABLE_ALIAS') {
                refId = value.id;
                formattedValue = `@ref:${value.id}`;
              } else {
                console.warn(`Unexpected BOOLEAN value type: ${typeof value} for ${variable.name}`);
                formattedValue = 'false';
              }
              console.log(`Final formatted value stored for ${sanitizedRowName}.${modeName}: ${formattedValue}`);
            }

            collectionStructure.get(collectionName)!.variables.get(sanitizedRowName)!.set(
              modeName,
              {
                value: formattedValue,
                type: variable.resolvedType,
                refId: refId,
                comment: undefined  // Add comment property with undefined default value
              }
            );
          }

          // Store the path for each variable ID
          variablePathsById.set(variable.id, {
            collection: collectionName,
            row: sanitizedRowName
          });
        }

        // Force GC between batches
        await new Promise(resolve => setTimeout(resolve, 0));
      }
    }

    // Create a Set to track required imports across all collections
    const requiredImports = new Set<string>();

    // FINALLY process references after all collections are initialized
    for (const collection of variableCollections) {
      const collectionName = formatPropertyName(collection.name);

      for (const [rowName, columns] of collectionStructure.get(collectionName)!.variables.entries()) {
        for (const [colName, data] of columns.entries()) {
          if (data.refId) {
            const refResult = createReferenceExpression(
              data.refId,
              colName,
              variablePathsById,
              collectionStructure,
              collectionName,
              rowName // Pass current path to detect circular references
            );

            if (refResult.value) {
              // Update to handle circular references
              const valueToStore = refResult.value;
              const updatedValue = {
                value: valueToStore,
                type: data.type,
                refId: refResult.isCircular ? undefined : data.refId, // Remove refId if resolved circular ref
                comment: refResult.comment // Add this line to preserve the comment
              };

              collectionStructure.get(collectionName)!.variables.get(rowName)!.set(
                colName,
                updatedValue
              );
            } else {
              // Fallback...
            }

            // When processing references:
            if (refResult.importStatement && !refResult.isCircular) {
              requiredImports.add(refResult.importStatement);
            }
          }
        }
      }
    }

    for (const [collectionName, collectionData] of collectionStructure.entries()) {

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
        name: 'root',
        children: new Map()
      };

      // Process each variable to build the tree
      for (const [varName, modes] of collectionData.variables.entries()) {
        // Split the path by underscores to get the hierarchy
        const parts = varName.split('_');

        // Navigate the tree and create nodes as needed
        let currentNode = variableTree;

        // Process all parts except the last one (which is the property name)
        for (let i = 0; i < parts.length - 1; i++) {
          const part = parts[i];

          if (!currentNode.children.has(part)) {
            currentNode.children.set(part, {
              name: part,
              children: new Map()
            });
          }

          currentNode = currentNode.children.get(part)!;
        }

        // The last part is the property name
        const propertyName = sanitizePropertyName(parts[parts.length - 1]);

        // Create the leaf node with the value
        if (!currentNode.children.has(propertyName)) {
          // Create a new Map for valuesByMode
          const valuesByMode = new Map<string, { value: string, refId?: string, comment?: string }>();

          // Get the type from the first mode (or default to 'COLOR' if undefined)
          const firstModeValue = modes.values().next().value;
          const type = firstModeValue?.type || 'COLOR';

          // Process each mode's value
          for (const [modeName, valueData] of modes.entries()) {
            valuesByMode.set(modeName, {
              value: valueData.value,
              refId: valueData.refId,
              comment: valueData.comment // Add this to preserve comments
            });
          }

          // Add the node to the tree
          currentNode.children.set(propertyName, {
            name: propertyName,
            type: type,
            valuesByMode: valuesByMode,
            children: new Map()
          });
        }
      }

      // Recursively generate code from the tree structure
      // Replace your generateStructCode function with this version:

      function generateStructsAndInstances(variableTree: VariableNode, collectionName: string): {
        structs: string,
        instances: string
      } {
        // First pass: Generate all struct type definitions
        const structDefinitions: string[] = [];

        // Check if variableTree is valid
        if (!variableTree || !variableTree.children) {
          console.error("Invalid variable tree");
          return { structs: "", instances: "" };
        }
        // Recursive function to collect struct types
        function collectStructTypes(node: VariableNode, path: string[] = []) {
          if (node.name === 'root') {
            // First pass: Generate structs for all multi-mode leaf nodes
            collectMultiModeStructs(node, collectionData, structDefinitions);

            // Second pass: Process regular structs
            for (const [childName, childNode] of node.children.entries()) {
              collectStructTypes(childNode, [childName]);
            }
            return;
          }

          const currentPath = [...path];
          const typeName = currentPath.join('_');

          // Only generate struct for nodes with children
          if (node.children.size > 0) {
            // Process child nodes first (recursive definition from deepest to shallowest)
            for (const [childName, childNode] of node.children.entries()) {
              if (childNode.children.size > 0) {
                collectStructTypes(childNode, [...currentPath, childName]);
              }
            }

            // THEN define this struct (after its children are defined)
            let structDef = `struct ${typeName} {\n`;

            // Add fields for direct properties (leaf nodes)
            for (const [childName, childNode] of node.children.entries()) {
              const sanitizedChildName = sanitizePropertyName(childName);
              if (childNode.valuesByMode) {
                if (collectionData.modes.size > 1) {
                  // Multi-mode property - reference the GENERIC mode struct instead
                  const slintType = getSlintType(childNode.type || 'COLOR');
                  const modeCount = collectionData.modes.size;
                  const modeStructName = `mode${modeCount}_${slintType}`;
                  structDef += `    ${sanitizedChildName}: ${modeStructName},\n`;
                } else {
                  // Single mode property (unchanged)
                  const slintType = getSlintType(childNode.type || 'COLOR');
                  structDef += `    ${childName}: ${slintType},\n`;
                }
              } else if (childNode.children.size > 0) {
                // Reference to another struct
                const childPath = [...currentPath, childName];
                structDef += `    ${childName}: ${childPath.join('_')},\n`;
              }
            }

            structDef += `}\n\n`;
            structDefinitions.push(structDef);
          }
        }
        // Collect all struct definitions
        collectStructTypes(variableTree);

        // Second pass: Generate property instances
        const instances: string[] = [];

        // Recursive function to generate property instances
        function generateInstance(node: VariableNode, indent: string = '    '): string {
          if (node.name === 'root') {
            let result = '';

            // Process direct properties of root
            for (const [childName, childNode] of node.children.entries()) {
              const sanitizedChildName = sanitizePropertyName(childName);

              if (childNode.children.size > 0) {
                // Nested struct instance - use the defined struct type
                result += `${indent}out property <${sanitizedChildName}> ${sanitizedChildName}: {\n`;
                result += generateInstance(childNode, indent + '    ');
                result += `${indent}};\n\n`;
              } else if (childNode.valuesByMode) {
                // Direct value property
                result += generateProperty(childNode, indent);
              }
            }
            return result;
          }

          let result = '';

          // Process children
          for (const [childName, childNode] of node.children.entries()) {
            const sanitizedChildName = sanitizePropertyName(childName);

            if (childNode.children.size > 0) {
              // This is a nested struct - ensure it's fully populated
              result += `${indent}${sanitizedChildName}: {\n`;
              result += generateInstance(childNode, indent + '    ');
              result += `${indent}},\n\n`;
            } else if (childNode.valuesByMode) {
              if (collectionData.modes.size <= 1) {
                // Single mode - direct property with value
                const firstMode = childNode.valuesByMode.values().next().value;

                // Use the already resolved value directly instead of re-processing it
                const valueToUse = firstMode?.value;


                // Only check for @ref: prefixes (unresolved references)
                if (!valueToUse) {
                  console.error(`No value for nested property ${sanitizedChildName}`);
                  result += `${indent}${sanitizedChildName}: ${childNode.type === 'COLOR' ? '#808080' : '0px'},\n`;
                } else if (valueToUse.startsWith('@ref:')) {
                  console.warn(`Unresolved reference for nested property ${sanitizedChildName}: ${valueToUse}`);
                  result += `${indent}${sanitizedChildName}: ${childNode.type === 'COLOR' ? '#808080' : '0px'},\n`;
                } else {
                  // Use the value directly without re-formatting
                  result += `${indent}${sanitizedChildName}: ${valueToUse},\n`;
                }
              } else {
                // Multi-mode - create nested object with mode properties
                result += `${indent}${sanitizedChildName}: {\n`;
                for (const [modeName, data] of childNode.valuesByMode.entries()) {
                  if (data.comment) {
                    result += `${indent}    // ${data.comment}\n`;
                  }
                  result += `${indent}    ${modeName}: ${data.value},\n`;
                }
                result += `${indent}},\n`;
              }
            }
          }

          return result;
        }
        // Generate property values
        // Replace the current generateProperty function
        function generateProperty(node: VariableNode, indent: string): string {
          if (!node.valuesByMode) return '';

          const slintType = getSlintType(node.type || 'COLOR');

          if (collectionData.modes.size > 1) {
            // For multi-mode, reference the generic mode struct type
            const modeCount = collectionData.modes.size;
            const modeStructName = `mode${modeCount}_${slintType}`;

            // Use the mode struct reference instead of inline struct definition
            let result = `${indent}out property <${modeStructName}> ${node.name}: {\n`;

            // Add values directly (not repeating property names)
            for (const [modeName, data] of node.valuesByMode.entries()) {
              if (data.comment) {
                result += `${indent}    // ${data.comment}\n`;
              }
              result += `${indent}    ${modeName}: ${data.value},\n`;
            }

            result += `${indent}};\n\n`;
            return result;
          } else {
            // Single mode - completely revised extraction method
            console.log(`Processing single-mode for ${node.name} with ${node.valuesByMode.size} entries`);

            // Directly get the first (and only) mode entry
            const firstModeEntry = Array.from(node.valuesByMode.entries())[0];

            if (!firstModeEntry) {
              console.error(`No mode entries for ${node.name}`);
              return `${indent}out property <${slintType}> ${node.name}: ${slintType === 'brush' ? '#808080' : '0px'};\n`;
            }

            const [modeName, modeData] = firstModeEntry;
            console.log(`Mode ${modeName} has value: ${modeData.value}`);

            // Single mode - revised check for references
            if (!modeData.value) {
              console.error(`No value for ${node.name}`);
              return `${indent}out property <${slintType}> ${node.name}: ${slintType === 'brush' ? '#808080' : '0px'};\n`;
            }

            // At this point, references should be resolved - if still @ref: then resolution failed
            if (modeData.value.startsWith('@ref:')) {
              console.warn(`Unresolved reference for ${node.name}: ${modeData.value}`);
              return `${indent}out property <${slintType}> ${node.name}: ${slintType === 'brush' ? '#808080' : '0px'};\n`;
            }

            // Use the actual value which should now be fully resolved
            return `${indent}out property <${slintType}> ${node.name}: ${modeData.value};\n`;
          }
        }
        return {
          structs: structDefinitions.join(''),
          instances: generateInstance(variableTree, '    ')
        };
      }
      // Get structures and instances
      const { structs, instances } = generateStructsAndInstances(variableTree, collectionData.formattedName);

      // Generate the scheme structs (only for multi-mode collections)
      let schemeStruct = '';
      let schemeModeStruct = '';
      let schemeInstance = '';
      let currentSchemeInstance = '';

      if (collectionData.modes.size > 1) {
        const schemeResult = generateSchemeStructs(variableTree, collectionData);
        schemeStruct = schemeResult.schemeStruct;
        schemeModeStruct = schemeResult.schemeModeStruct;
        schemeInstance = schemeResult.schemeInstance;
        currentSchemeInstance = schemeResult.currentSchemeInstance;
      }

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
          if (instances.includes(`${targetCollection}.`) ||
            instances.includes(`${targetCollection}(`)) {
            content += importStmt;
          }
        }
      }

      // Add a blank line after imports if any were added
      if (content.includes('import ')) {
        content += '\n';
      }

      // Add the mode enum if needed
      if (collectionData.modes.size > 1) {
        const sanitizedName = sanitizeEnumName(collectionData.formattedName);
        content += `export enum ${sanitizedName}Mode {\n`;
        for (const mode of collectionData.modes) {
          content += `    ${sanitizeModeForEnum(mode)},\n`;
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
        content: content
      });

      console.log(`Generated file for collection: ${collectionData.name}`);
    }

    for (const file of exportedFiles) {
      // Check if there are any unresolved references left
      if (file.content.includes('@ref:')) {
        console.warn(`Found unresolved references in ${file.name}`);

        // Replace unresolved references with appropriate defaults based on context
        file.content = file.content.replace(/(@ref:VariableID:[0-9:]+)/g, (match, reference) => {
          console.warn(`  Replacing unresolved reference: ${reference}`);

          // Look at surrounding context to determine appropriate replacement
          if (file.content.includes(`brush,\n`) && file.content.includes(reference)) {
            return '#808080'; // Default color
          } else if (file.content.includes(`length,\n`) && file.content.includes(reference)) {
            return '0px';     // Default length
          } else if (file.content.includes(`string,\n`) && file.content.includes(reference)) {
            return '""';      // Default string
          } else if (file.content.includes(`bool,\n`) && file.content.includes(reference)) {
            return 'false';   // Default boolean
          } else {
            return '#808080'; // Default fallback
          }
        });
      }
    }
    return exportedFiles;
  } catch (error) {
    console.error("Error in exportFigmaVariablesToSeparateFiles:", error);
    // Return an error file
    return [{
      name: 'error.slint',
      content: `// Error generating variables: ${error}`
    }];
  }
}

function generateSchemeStructs(variableTree: VariableNode, collectionData: { name: string, formattedName: string, modes: Set<string> }): {
  schemeStruct: string,
  schemeModeStruct: string,
  schemeInstance: string,
  currentSchemeInstance: string
} {
  // Helper function to count leaf descendant nodes
  function findLeafDescendants(node: VariableNode): number {
    let count = 0;

    for (const [childName, childNode] of node.children.entries()) {
      if (childNode.valuesByMode) {
        count += 1;
      } else if (childNode.children.size > 0) {
        // Recursively count leaf descendants in nested nodes
        count += findLeafDescendants(childNode);
      }
    }

    return count;
  }

  // Track all scheme structs we need to create
  const schemeStructs = new Map<string, { fields: Map<string, string> }>();

  // Track the final variables mapping (can be nested now)
  const schemeVariables = new Map<string, { type: string, path: string[], isStruct: boolean }>();
  const schemeName = `${formatStructName(collectionData.name)}Scheme`;
  const schemeModeName = `${formatStructName(collectionData.name)}SchemeMode`;
  let schemeStruct = `struct ${schemeName} {\n`;
  let schemeModeStruct = '';

  // 1. First pass: Identify all structs needed for scheme representation
  function collectSchemeStructs(node: VariableNode, path: string[] = [], schemePrefix = 'scheme_') {
    for (const [childName, childNode] of node.children.entries()) {
      const currentPath = [...path, childName];

      if (childNode.children.size > 0) {
        // This is a nested node - we may need a scheme struct
        const hasLeafDescendants = findLeafDescendants(childNode);

        if (hasLeafDescendants > 0) {
          // Create a struct name like scheme_alert, scheme_sad_sub, etc.
          const structName = schemePrefix + currentPath.join('_');
          schemeStructs.set(structName, { fields: new Map() });

          // Register this in our variables
          schemeVariables.set(currentPath.join('_'), {
            type: structName,
            path: currentPath,
            isStruct: true
          });

          // First, recurse to handle any nested structs this might contain
          collectSchemeStructs(childNode, currentPath, schemePrefix);

          // Then, find immediate leaf children for this struct
          for (const [gcName, gcNode] of childNode.children.entries()) {
            if (gcNode.valuesByMode) {
              // Add as a field to our struct
              const fieldType = getSlintType(gcNode.type || 'COLOR');
              schemeStructs.get(structName)!.fields.set(gcName, fieldType);
            }
          }
        }
      } else if (childNode.valuesByMode) {
        // Leaf node - add to scheme variables
        schemeVariables.set(currentPath.join('_'), {
          type: getSlintType(childNode.type || 'COLOR'),
          path: currentPath,
          isStruct: false
        });
      }
    }
  }

  // Collect all scheme structs
  collectSchemeStructs(variableTree);
  // Create a map of parent-child relationships for structs
  const childStructMap = new Map<string, Set<string>>();

  // Build the relationship map for nested structs
  for (const [varName, info] of schemeVariables.entries()) {
    if (info.isStruct && info.path.length > 1) {
      // For each nested path like "sad_sub_foo", create entries for parents
      for (let i = 1; i < info.path.length; i++) {
        // Get parent path (e.g., "sad" for "sad_sub")
        const parentPath = info.path.slice(0, i).join('_');
        // Get full child path (e.g., "sad_sub")
        const childPath = info.path.slice(0, i + 1).join('_');

        // Add this child to its parent's children
        if (!childStructMap.has(parentPath)) {
          childStructMap.set(parentPath, new Set<string>());
        }
        childStructMap.get(parentPath)!.add(childPath);
      }
    }
  }
  // 2. Generate the scheme struct definitions with proper nesting
  let allSchemeStructs = '';

  // First, sort structs by depth (number of underscores) to ensure proper declaration order
  const structEntries = Array.from(schemeStructs.entries());
  structEntries.sort((a, b) => {
    // Count underscores to determine nesting depth
    const depthA = (a[0].match(/_/g) || []).length;
    const depthB = (b[0].match(/_/g) || []).length;

    // Sort from deepest to shallowest
    return depthB - depthA;
  });

  // Now generate all scheme structs with their fields, starting from deepest
  for (const [structName, structInfo] of structEntries) {
    allSchemeStructs += `struct ${structName} {\n`;

    // Add regular leaf fields first
    for (const [fieldName, fieldType] of structInfo.fields.entries()) {
      allSchemeStructs += `    ${fieldName}: ${fieldType},\n`;
    }

    // Extract the path without the scheme_ prefix
    const structPath = structName.replace('scheme_', '');

    // Check if this struct has direct child structs and add them as fields
    if (childStructMap.has(structPath)) {
      const children = childStructMap.get(structPath)!;

      for (const childPath of children) {
        // Get just the last segment of the path as the field name
        const parts = childPath.split('_');
        const fieldName = parts[parts.length - 1];
        const childStructName = 'scheme_' + childPath;

        // Only add if the child struct actually exists
        if (schemeStructs.has(childStructName)) {
          allSchemeStructs += `    ${fieldName}: ${childStructName},\n`;
        }
      }
    }

    allSchemeStructs += `}\n\n`;
  }
  schemeStruct = `struct ${schemeName} {\n`;
  for (const [varName, info] of schemeVariables.entries()) {
    if (info.path.length === 1) {
      // This is a top-level variable or struct
      const fieldName = info.path[0];
      schemeStruct += `    ${fieldName}: ${info.isStruct ? 'scheme_' + varName : info.type},\n`;
    }
  }
  schemeStruct += `}\n\n`;

  // 4. Generate the mode struct with each mode referencing the main scheme
  schemeModeStruct = `struct ${schemeModeName} {\n`;

  // Add each mode as a field that references the main scheme struct
  for (const mode of collectionData.modes) {
    schemeModeStruct += `    ${mode}: ${schemeName},\n`;
  }
  schemeModeStruct += `    }\n`;

  // 5. Generate the instance initialization for the mode property
  let schemeInstance = `    out property <${schemeModeName}> mode: {\n`;

  for (const mode of collectionData.modes) {
    schemeInstance += `        ${mode}: {\n`;

    // Function to add hierarchical values
    function addHierarchicalValues(node: VariableNode = variableTree, path: string[] = [], currentIndent: string = '            ') {
      for (const [childName, childNode] of node.children.entries()) {
        const currentPath = [...path, childName];

        if (childNode.children.size > 0) {
          // This is a struct node
          schemeInstance += `${currentIndent}${childName}: {\n`;
          // Recursively add its children
          addHierarchicalValues(childNode, currentPath, currentIndent + '    ');
          schemeInstance += `${currentIndent}},\n`;
        } else if (childNode.valuesByMode) {
          // This is a leaf value
          schemeInstance += `${currentIndent}${childName}: ${collectionData.formattedName}.${currentPath.join('.')}.${mode},\n`;
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
  let currentSchemeInstance = `    in-out property <${sanitizeEnumName(collectionData.formattedName)}Mode> current-scheme: ${[...collectionData.modes][0]};\n`;
  // Add the current-mode property that dynamically selects based on the enum
  currentSchemeInstance += `    out property <${schemeName}> current-mode: {\n`;
  currentSchemeInstance += `        // Dynamic mode selector based on enum value\n`;

  // Create a switch-like structure for all available modes
  const modeArray = [...collectionData.modes];
  for (let i = 0; i < modeArray.length; i++) {
    const mode = modeArray[i];
    currentSchemeInstance += `        if (current-scheme == ${sanitizeEnumName(collectionData.formattedName)}Mode.${mode}) { return root.mode.${mode}; }\n`;
  }

  // Add fallback to the first mode if somehow none matched
  currentSchemeInstance += `        // Fallback to first mode\n`;
  currentSchemeInstance += `        return root.mode.${modeArray[0]};\n`;
  currentSchemeInstance += `    };\n\n`;

  // Now add the current property that references current-mode
  currentSchemeInstance += `    out property <${schemeName}> current: {\n`;

  // Add properties in the same structure as the scheme
  function addCurrentValues(node: VariableNode = variableTree, path: string[] = [], currentIndent: string = '        ') {
    for (const [childName, childNode] of node.children.entries()) {
      const currentPath = [...path, childName];

      if (childNode.children.size > 0) {
        // This is a nested struct
        currentSchemeInstance += `${currentIndent}${childName}: {\n`;
        addCurrentValues(childNode, currentPath, currentIndent + '    ');
        currentSchemeInstance += `${currentIndent}},\n`;
      } else if (childNode.valuesByMode) {
        // This is a leaf value - reference current-mode
        currentSchemeInstance += `${currentIndent}${childName}: current-mode.${currentPath.join('.')},\n`;
      }
    }
  }

  // Build the current structure
  addCurrentValues();

  currentSchemeInstance += `    };\n`;

  return {
    schemeStruct: allSchemeStructs + schemeStruct,
    schemeModeStruct: schemeModeStruct,
    schemeInstance: schemeInstance,
    currentSchemeInstance: currentSchemeInstance
  };
}


function collectMultiModeStructs(node: VariableNode, collectionData: { modes: Set<string> }, structDefinitions: string[]) {
  if (collectionData.modes.size <= 1) return;

  // Define all Slint types we want to support
  const allSlintTypes = ['brush', 'length', 'string', 'bool'];

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
        const slintType = getSlintType(childNode.type || 'COLOR');
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