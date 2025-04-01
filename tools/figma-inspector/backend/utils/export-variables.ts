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
  const parts = name.split(/\/|\.|:|--|-(?=[a-z])/);
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
  // For collections with multiple modes, use function call with mode parameter
  referenceExpr = `${targetCollection.formattedName}.${propertyPath}(${targetCollection.formattedName}Mode.${sanitizedMode})`;
  
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

      // Add all required imports
      for (const importStmt of requiredImports) {
        content += importStmt;
      }

      // Add a blank line after imports if there are any
      if (requiredImports.size > 0) {
        content += '\n';
      }

      // Create a ModeEnum if we have more than one mode
      if (collectionData.modes.size > 1) {
        content += `export enum ${collectionData.formattedName}Mode {\n`;

        // Add all modes to the enum
        for (const mode of collectionData.modes) {
          content += `    ${mode},\n`;
        }
        content += `}\n\n`;
      }

      // Generate global singleton
      content += `export global ${collectionData.formattedName} {\n`;

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
      function generateStructCode(node: VariableNode, indent: string = ''): string {
        let structCode = '';

        // For leaf nodes (actual variables)
        if (node.valuesByMode) {
          const slintType = getSlintType(node.type || 'COLOR');

          // If we have multiple modes, generate a function
          if (collectionData.modes.size > 1) {
            structCode += `${indent}export function ${node.name}(mode: ${collectionData.formattedName}Mode) -> ${slintType} {\n`;
            structCode += `${indent}    if (mode == ${collectionData.formattedName}Mode.`;

            // Add switch-like logic for each mode
            let isFirst = true;
            for (const [modeName, data] of node.valuesByMode.entries()) {
              if (!isFirst) {
                structCode += `${indent}    } else if (mode == ${collectionData.formattedName}Mode.`;
              }
              structCode += `${modeName}) {\n`;
              structCode += `${indent}        return ${data.value};\n`;
              isFirst = false;
            }

            // Close the if-else and function
            structCode += `${indent}    } else {\n`;
            const defaultFormatted = formatValueForSlint(
              node.type || 'COLOR',
              node.valuesByMode.values().next().value?.value,
              true
            );
            structCode += `${indent}        return ${defaultFormatted.value};\n`;
            structCode += `${indent}    }\n`;
            structCode += `${indent}}\n\n`;
          } else {
            const defaultFormatted = formatValueForSlint(
              node.type || 'COLOR',
              node.valuesByMode.values().next().value?.value,
              true
            );
            structCode += `${indent}export ${node.name}: ${slintType} = ${defaultFormatted.value};\n`;
          }
          return structCode;
        }

        // Skip empty nodes
        if (node.children.size === 0) return '';

        // For non-leaf nodes with children (nested structs)
        if (node.name !== 'root') {
          structCode += `${indent}export ${node.name}: {\n`;
        }

        // Process all children
        for (const child of node.children.values()) {
          structCode += generateStructCode(child, node.name !== 'root' ? indent + '    ' : indent);
        }

        // Close the struct
        if (node.name !== 'root') {
          structCode += `${indent}}\n\n`;
        }

        return structCode;
      }

      // Generate code for all the nested structures
      content += generateStructCode(variableTree, '    ');

      // Close the global
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
 * Gets the appropriate Slint type for a Figma variable type
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


// Improved reference resolution function with better debugging and more flexible mode matching
function resolveReference(
  referenceId: string,
  modeName: string,
  variableValuesById: Map<string, Map<string, { value: string, type: string }>>,
  visited: Set<string>
): { value: string, type: string } | null {
  // Check for circular references
  if (visited.has(referenceId)) {
    console.warn('Circular reference detected:', referenceId);
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
  console.log(`Available modes for this reference:`, Array.from(targetValues.keys()));

  // Get the value for this exact mode
  let modeValue = targetValues.get(modeName);

  // If exact mode not found, try alternative mode matching strategies
  if (!modeValue) {
    console.log(`Mode "${modeName}" not found directly, trying alternatives...`);

    // Strategy 1: Try case-insensitive matching
    for (const [availableMode, value] of targetValues.entries()) {
      if (availableMode.toLowerCase() === modeName.toLowerCase()) {
        console.log(`Found matching mode with different case: ${availableMode}`);
        modeValue = value;
        break;
      }
    }

    // Strategy 2: If "light" or "dark" are in the name, try variations
    if (!modeValue) {
      if (modeName.includes('light')) {
        for (const [availableMode, value] of targetValues.entries()) {
          if (availableMode.includes('light')) {
            console.log(`Found alternative light mode: ${availableMode}`);
            modeValue = value;
            break;
          }
        }
      } else if (modeName.includes('dark')) {
        for (const [availableMode, value] of targetValues.entries()) {
          if (availableMode.includes('dark')) {
            console.log(`Found alternative dark mode: ${availableMode}`);
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
  if (modeValue.value.startsWith('@ref:')) {
    console.log(`Found nested reference: ${modeValue.value}`);
    const nestedRefId = modeValue.value.substring(5); // Remove '@ref:' prefix
    return resolveReference(nestedRefId, modeName, variableValuesById, visited);
  }

  // Return the resolved value
  console.log(`Successfully resolved reference to: ${modeValue.value}`);
  return modeValue;
}

// Process a single collection for a specific mode - memory efficient approach
async function processCollectionForMode(
  collection: any,
  modeName: string,
  callback: (name: string, data: any) => void
): Promise<void> {
  // Process variables in smaller batches
  const batchSize = 10;

  for (let i = 0; i < collection.variableIds.length; i += batchSize) {
    const batch = collection.variableIds.slice(i, i + batchSize);
    interface VariableAlias {
      type: 'VARIABLE_ALIAS';
      id: string;
    }

    type VariableValue = RGB | RGBA | VariableAlias | number | string;

    interface FigmaVariable {
      name: string;
      resolvedType: 'COLOR' | 'FLOAT' | 'STRING';
      valuesByMode: Record<string, VariableValue>;
    }

    const batchPromises: Promise<FigmaVariable | null>[] = batch.map((id: string) =>
      figma.variables.getVariableByIdAsync(id)
    );
    const batchResults = await Promise.all(batchPromises);

    for (const variable of batchResults) {
      if (!variable) continue;

      // Skip variables without values for all modes
      if (!variable.valuesByMode || Object.keys(variable.valuesByMode).length === 0) continue;

      // Find the mode ID for this mode name
      interface VariableCollectionMode {
        name: string;
        modeId: string;
      }

      const modeInfo: VariableCollectionMode | undefined = collection.modes.find((m: VariableCollectionMode) => m.name.toLowerCase() === modeName.toLowerCase());
      if (!modeInfo) continue;

      const modeId = modeInfo.modeId;

      // Skip if there's no value for this mode
      if (!variable.valuesByMode[modeId]) continue;

      const value = variable.valuesByMode[modeId];

      // Use extractHierarchy to break up hierarchical names
      const nameParts = extractHierarchy(variable.name);

      // Format the last part as the property name
      const propertyName = nameParts.length > 0 ?
        formatPropertyName(nameParts[nameParts.length - 1]) :
        formatPropertyName(variable.name);

      // Format the value based on type
      let formattedValue = '';
      if (variable.resolvedType === 'COLOR') {
        if (typeof value === 'object' && value && 'r' in value) {
          formattedValue = convertColor(value);
        } else if (typeof value === 'object' && value && value.type === 'VARIABLE_ALIAS') {
          // For references, we'll handle this later - store as a specially formatted string
          formattedValue = `@ref:${value.id}`;
        }
      } else if (variable.resolvedType === 'FLOAT') {
        formattedValue = `${value}px`;
      } else if (variable.resolvedType === 'STRING') {
        formattedValue = `"${value}"`;
      }

      // Create a hierarchical path for this variable
      // Start with collection name, then add all parts except the last one
      const path = [formatPropertyName(collection.name)];
      for (let i = 0; i < nameParts.length - 1; i++) {
        path.push(formatPropertyName(nameParts[i]));
      }

      // Join with underscores instead of slashes
      const fullPath = path.join('_');
      // Add the last part (property name)
      const fullName = fullPath ? `${fullPath}_${propertyName}` : propertyName;

      callback(fullName, {
        value: formattedValue,
        type: variable.resolvedType,
      });
    }

    // Force a micro-task to allow garbage collection
    await new Promise(resolve => setTimeout(resolve, 0));
  }
}

