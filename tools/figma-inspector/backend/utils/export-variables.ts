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


// Helper to format struct/global name for Slint (PascalCase) with sanitization
function formatStructName(name: string): string {
  // Handle names starting with "." - remove the dot
  let sanitizedName = name.startsWith('.') ? name.substring(1) : name;

  // If that made it empty, use a default
  if (!sanitizedName || sanitizedName.trim() === '') {
    sanitizedName = 'DefaultCollection';
  }

  // First, replace problematic characters with spaces before splitting
  sanitizedName = sanitizedName.replace(/[&+]/g, ' ');

  // Then continue with normal PascalCase conversion
  return sanitizedName
    .split(/[-_\s\/]/)
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
  currentCollection: string = ""
): { value: string | null, importStatement?: string } {
  console.log(`Creating reference for ID: ${referenceId}, mode: ${sourceModeName}, collection: ${currentCollection}`);

  // Get the target variable path
  const targetPath = variablePathsById.get(referenceId);
  if (!targetPath) {
    console.warn(`Reference path not found for ID: ${referenceId}`,
      "Available IDs:", Array.from(variablePathsById.keys()).join(", ").substring(0, 100) + "...");
    return { value: null };
  }
  console.log(`Indexed ${variablePathsById.size} variable paths`);
  console.log(`Found path for reference: collection=${targetPath.collection}, row=${targetPath.row}`);

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
  const propertyPath = pathParts.join('.');

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
  valuesByMode?: Map<string, { value: string, refId?: string }>;
  children: Map<string, VariableNode>;
}

// For Figma Plugin - Export function with hierarchical structure

// Export each collection to a separate virtual file
export async function exportFigmaVariablesToSeparateFiles(): Promise<Array<{ name: string, content: string }>> {
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
      variables: Map<string, Map<string, { value: string, type: string, refId?: string }>>
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

    // THEN process the variables for each collection
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
            }

            collectionStructure.get(collectionName)!.variables.get(sanitizedRowName)!.set(
              modeName,
              {
                value: formattedValue,
                type: variable.resolvedType,
                refId: refId
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
              variablePathsById, // Use the populated map!
              collectionStructure,
              collectionName // Pass current collection name
            );

            if (refResult.value) {
              collectionStructure.get(collectionName)!.variables.get(rowName)!.set(
                colName,
                {
                  value: refResult.value,
                  type: data.type,
                  refId: data.refId
                }
              );
            } else {
              console.warn(`Couldn't create reference expression for: ${data.refId} for ${rowName}-${colName}`);
              collectionStructure.get(collectionName)!.variables.get(rowName)!.set(
                colName,
                {
                  value: data.type === 'COLOR' ? '#808080' :
                    data.type === 'FLOAT' ? '0px' :
                      data.type === 'BOOLEAN' ? 'false' :
                        data.type === 'STRING' ? '""' : '',
                  type: data.type
                }
              );
            }

            // When processing references:
            if (refResult.importStatement) {
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
        const propertyName = parts[parts.length - 1];

        // Create the leaf node with the value
        if (!currentNode.children.has(propertyName)) {
          // Create a new Map for valuesByMode
          const valuesByMode = new Map<string, { value: string, refId?: string }>();

          // Get the type from the first mode (or default to 'COLOR' if undefined)
          const firstModeValue = modes.values().next().value;
          const type = firstModeValue?.type || 'COLOR';

          // Process each mode's value
          for (const [modeName, valueData] of modes.entries()) {
            valuesByMode.set(modeName, {
              value: valueData.value,
              refId: valueData.refId
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
              if (childNode.valuesByMode) {
                if (collectionData.modes.size > 1) {
                  // Multi-mode property - reference the GENERIC mode struct instead
                  const slintType = getSlintType(childNode.type || 'COLOR');
                  const modeCount = collectionData.modes.size;
                  const modeStructName = `mode${modeCount}_${slintType}`;
                  structDef += `    ${childName}: ${modeStructName},\n`;
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
              if (childNode.children.size > 0) {
                // Nested struct instance - use the defined struct type
                result += `${indent}out property <${childName}> ${childName}: {\n`;
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
            if (childNode.valuesByMode) {
              if (collectionData.modes.size <= 1) {
                // Single mode - direct property with value (unchanged)
                const firstMode = childNode.valuesByMode.values().next().value;
                const formattedValue = formatValueForSlint(
                  childNode.type || 'COLOR',
                  firstMode?.value,
                  true
                ).value;
                result += `${indent}${childName}: ${formattedValue},\n`;
              } else {
                // Multi-mode - create nested object with mode properties
                result += `${indent}${childName}: {\n`;
                for (const [modeName, data] of childNode.valuesByMode.entries()) {
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
              result += `${indent}    ${modeName}: ${data.value},\n`;
            }

            result += `${indent}};\n\n`;
            return result;
          } else {
            // Single mode - direct property (unchanged)
            const defaultFormatted = formatValueForSlint(
              node.type || 'COLOR',
              node.valuesByMode.values().next().value?.value,
              true
            );
            return `${indent}out property <${slintType}> ${node.name}: ${defaultFormatted.value};\n`;
          }
        }
        return {
          structs: structDefinitions.join(''),
          instances: generateInstance(variableTree, '    ')
        };
      }
      // Get structures and instances first
      const { structs, instances } = generateStructsAndInstances(variableTree, collectionData.formattedName);

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
        content += `export enum ${collectionData.formattedName}Mode {\n`;
        for (const mode of collectionData.modes) {
          content += `    ${mode},\n`;
        }
        content += `}\n\n`;
      }

      // Add structs and global content
      content += structs;
      content += `export global ${collectionData.formattedName} {\n`;
      content += instances;
      content += `}\n`;

      // Add file to exported files
      exportedFiles.push({
        name: `${collectionData.formattedName}.slint`,
        content: content
      });

      console.log(`Generated file for collection: ${collectionData.name}`);
    }

    console.log(`Exported ${exportedFiles.length} collection files`);
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



function collectMultiModeStructs(node: VariableNode, collectionData: { modes: Set<string> }, structDefinitions: string[], path: string[] = []) {
  if (collectionData.modes.size <= 1) return;

  // Create maps to track shared mode structs we've created
  const modeTypeMap = new Map<string, string>();

  // First pass - find all unique mode count + type combinations
  function findUniqueTypeConfigs(node: VariableNode) {
    for (const [childName, childNode] of node.children.entries()) {
      if (childNode.valuesByMode && childNode.valuesByMode.size > 0) {
        const slintType = getSlintType(childNode.type || 'COLOR');
        const key = `${collectionData.modes.size}_${slintType}`;
        modeTypeMap.set(key, slintType);
      } else if (childNode.children.size > 0) {
        findUniqueTypeConfigs(childNode);
      }
    }
  }

  // First find all unique type configurations
  findUniqueTypeConfigs(node);

  // Then generate the shared structs
  for (const [key, slintType] of modeTypeMap.entries()) {
    const [count, type] = key.split('_');
    const structName = `mode${count}_${type}`;

    let structDef = `struct ${structName} {\n`;
    for (const mode of collectionData.modes) {
      structDef += `    ${mode}: ${slintType},\n`;
    }
    structDef += `}\n\n`;

    structDefinitions.push(structDef);
  }
}

