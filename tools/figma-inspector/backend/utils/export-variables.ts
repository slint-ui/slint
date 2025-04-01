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
  sourceColumnName: string,
  variablePathsById: Map<string, { collection: string, row: string }>,
  collectionStructure: Map<string, any>
): string | null {
  // Get the target variable path
  const targetPath = variablePathsById.get(referenceId);
  if (!targetPath) {
    console.warn(`Reference path not found for ID: ${referenceId}`);
    return null;
  }

  // Get the target collection
  const targetCollection = collectionStructure.get(targetPath.collection);
  if (!targetCollection) {
    console.warn(`Collection not found: ${targetPath.collection}`);
    return null;
  }

  // Get all modes from target collection
  const targetModes = [...targetCollection.modes];
  if (targetModes.length === 0) {
    console.warn(`No modes found in target collection: ${targetPath.collection}`);
    return null;
  }

  // First try: exact match with sanitized names
  let targetColumnName = targetModes.find(mode =>
    sanitizeModeForEnum(mode) === sanitizeModeForEnum(sourceColumnName)
  );

  // Second try: direct match without sanitization
  if (!targetColumnName) {
    targetColumnName = targetModes.find(mode => mode === sourceColumnName);
  }

  // Third try: match the collection's first mode
  if (!targetColumnName) {
    targetColumnName = targetModes[0];
    console.log(`Using default mode ${targetColumnName} for reference to ${referenceId}`);
  }

  // Sanitize both row and column names
  const sanitizedRow = sanitizeRowName(targetPath.row);
  const sanitizedColumn = sanitizeModeForEnum(targetColumnName);

  // Format the reference expression
  return `${targetCollection.formattedName}.${sanitizedRow}-${sanitizedColumn}`;
}

interface VariableNode {
  name: string;
  type?: string;
  valuesByMode?: Map<string, { value: string, refId?: string }>;
  children: Map<string, VariableNode>;
}
// For Figma Plugin - Export function with hierarchical structure

// Export each collection to a separate virtual file
export async function exportFigmaVariablesToSeparateFiles(): Promise<Array<{name: string, content: string}>> {
  try {
    // Get collections asynchronously
    const variableCollections = await figma.variables.getLocalVariableCollectionsAsync();
    
    // Array to store all exported files
    const exportedFiles: Array<{name: string, content: string}> = [];
    
    // Process each collection
    for (const collection of variableCollections) {
      // Skip empty collections
      if (!collection.variableIds || collection.variableIds.length === 0) continue;
      
      const collectionName = formatPropertyName(collection.name);
      const formattedCollectionName = formatStructName(collection.name);
      
      // Initialize code output for this collection
      let slintCode = `// Generated from Figma collection: ${collection.name}\n\n`;
      
      // Collection-specific data structures
      const collectionStructure = new Map<string, {
        name: string,
        formattedName: string,
        modes: Set<string>,
        variables: Map<string, Map<string, { value: string, type: string, refId?: string }>>
      }>();
      
      // Initialize the collection in our structure
      collectionStructure.set(collectionName, {
        name: collection.name,
        formattedName: formattedCollectionName,
        modes: new Set<string>(),
        variables: new Map<string, Map<string, { value: string, type: string, refId?: string }>>()
      });
      
      // Add modes to collection
      collection.modes.forEach(mode => {
        const sanitizedMode = sanitizeModeForEnum(formatPropertyName(mode.name));
        collectionStructure.get(collectionName)!.modes.add(sanitizedMode);
      });
      
      // Maps for references that can be scoped to this collection
      const variableValuesById = new Map<string, Map<string, { value: string, type: string }>>();
      const variableNameById = new Map<string, string>();
      const variablePathsById = new Map<string, { collection: string, row: string }>();
      
      // Process variables in batches
      const batchSize = 5;
      for (let i = 0; i < collection.variableIds.length; i += batchSize) {
        const batch = collection.variableIds.slice(i, i + batchSize);
        const batchPromises = batch.map(id => figma.variables.getVariableByIdAsync(id));
        const batchResults = await Promise.all(batchPromises);
        
        for (const variable of batchResults) {
          if (!variable) continue;
          if (!variable.valuesByMode || Object.keys(variable.valuesByMode).length === 0) continue;
          
          // Store variable name by ID
          variableNameById.set(variable.id, variable.name);
          
          // Initialize variable in valuesByID map
          if (!variableValuesById.has(variable.id)) {
            variableValuesById.set(variable.id, new Map<string, { value: string, type: string }>());
          }
          
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
          
          // Store the path to this variable for reference lookup
          variablePathsById.set(variable.id, {
            collection: collectionName,
            row: sanitizedRowName
          });
          
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
            
            // Store values for this variable
            variableValuesById.get(variable.id)!.set(
              modeName,
              { value: formattedValue, type: variable.resolvedType }
            );
            
            collectionStructure.get(collectionName)!.variables.get(sanitizedRowName)!.set(
              modeName,
              {
                value: formattedValue,
                type: variable.resolvedType,
                refId: refId
              }
            );
          }
        }
        
        // Force GC between batches
        await new Promise(resolve => setTimeout(resolve, 0));
      }
      
      // Preserve references - second pass
      for (const [rowName, columns] of collectionStructure.get(collectionName)!.variables.entries()) {
        for (const [colName, data] of columns.entries()) {
          if (data.refId) {
            const refExpression = createReferenceExpression(
              data.refId,
              colName,
              variablePathsById,
              collectionStructure
            );
            
            if (refExpression) {
              collectionStructure.get(collectionName)!.variables.get(rowName)!.set(
                colName,
                {
                  value: refExpression,
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
                         '""',
                  type: data.type
                }
              );
            }
          }
        }
      }
      
      // Build hierarchical variable structure
      const rootNode: VariableNode = {
        name: collectionName,
        children: new Map<string, VariableNode>()
      };
      
      // Convert flat variables to hierarchical structure
      for (const [rowName, columns] of collectionStructure.get(collectionName)!.variables.entries()) {
        // Determine variable type
        let varType = 'color';
        for (const [, data] of columns.entries()) {
          varType = data.type === 'COLOR' ? 'color' : 
                   data.type === 'FLOAT' ? 'length' : 
                   data.type === 'BOOLEAN' ? 'bool' : 'string';
          break;
        }
        
        // Split by underscores for hierarchy
        const nameParts = rowName.split('_');
        
        // Build the tree
        let currentNode = rootNode;
        for (let i = 0; i < nameParts.length - 1; i++) {
          const part = nameParts[i];
          if (!currentNode.children.has(part)) {
            currentNode.children.set(part, {
              name: part,
              children: new Map<string, VariableNode>()
            });
          }
          currentNode = currentNode.children.get(part)!;
        }
        
        // Add the leaf node
        const leafName = nameParts[nameParts.length - 1];
        if (!currentNode.children.has(leafName)) {
          currentNode.children.set(leafName, {
            name: leafName,
            type: varType,
            valuesByMode: new Map(),
            children: new Map<string, VariableNode>()
          });
        }
        
        // Add values for each mode
        const leafNode = currentNode.children.get(leafName)!;
        for (const [modeName, data] of columns.entries()) {
          if (!leafNode.valuesByMode) {
            leafNode.valuesByMode = new Map();
          }
          leafNode.valuesByMode.set(modeName, {
            value: data.value,
            refId: data.refId
          });
        }
      }
      
      // Get collection info for code generation
      const collectionData = collectionStructure.get(collectionName)!;
      
      // Only generate if there are variables
      if (collectionData.variables.size === 0) continue;
      
      // Convert modes to an array for consistent indexing
      const modes = [...collectionData.modes];
      
      // 1. Generate enum for columns (modes)
      slintCode += `// ${collectionData.name} Modes\n`;
      slintCode += `export enum ${collectionData.formattedName}Column {\n`;
      modes.forEach(mode => {
        slintCode += `    ${sanitizeModeForEnum(mode)},\n`;
      });
      slintCode += `}\n\n`;
      
// 2. Generate struct definitions for hierarchical structure - TOPOLOGICAL SORT VERSION
function generateStructDefinitions(rootNode: VariableNode, rootPath: string[]): string {
  let result = '';
  
  // First collect all structs and their dependencies
  interface StructInfo {
    name: string;
    path: string[];
    dependencies: string[]; // Array of struct names this struct depends on
    code: string;          // The struct definition code
  }
  
  const structs: Map<string, StructInfo> = new Map();
  
  // Recursive function to collect all structs
  function collectStructs(node: VariableNode, path: string[] = []) {
    // Skip leaf nodes and the root
    if (node.valuesByMode || path.length === 0) {
      return;
    }
    
    // Create struct name from path
    const structName = path.map(p => formatStructName(p)).join('_');
    
    // Skip if already processed
    if (structs.has(structName)) {
      return;
    }
    
    // Start building the struct code
    let structCode = `// ${path.join('/')} structure\n`;
    structCode += `struct ${structName} {\n`;
    
    // Track dependencies
    const dependencies: string[] = [];
    
    // Add properties for children
    for (const [childName, childNode] of node.children.entries()) {
      if (childNode.valuesByMode) {
        // This is a variable (leaf node)
        structCode += `    ${childName}: ${childNode.type || 'color'},\n`;
      } else {
        // This is a nested struct - add as dependency
        const childStructName = [...path, childName].map(p => formatStructName(p)).join('_');
        structCode += `    ${childName}: ${childStructName},\n`;
        dependencies.push(childStructName);
      }
    }
    
    structCode += `}\n\n`;
    
    // Store this struct's info
    structs.set(structName, {
      name: structName,
      path: path,
      dependencies: dependencies,
      code: structCode
    });
    
    // Process all child structs
    for (const [childName, childNode] of node.children.entries()) {
      if (!childNode.valuesByMode) {
        collectStructs(childNode, [...path, childName]);
      }
    }
  }
  
  // Collect all structs starting from the root path
  collectStructs(rootNode, rootPath);
  
  // Perform a topological sort to ensure dependencies are defined first
  const visited = new Set<string>();
  const temp = new Set<string>();
  const sorted: string[] = [];
  
  function visit(structName: string) {
    // Skip if already processed
    if (visited.has(structName)) return;
    
    // Check for circular dependencies
    if (temp.has(structName)) {
      console.warn(`Circular dependency detected for struct: ${structName}`);
      return;
    }
    
    // Mark as being processed
    temp.add(structName);
    
    // Visit all dependencies first
    const struct = structs.get(structName);
    if (struct) {
      for (const dependency of struct.dependencies) {
        visit(dependency);
      }
    }
    
    // Mark as processed
    temp.delete(structName);
    visited.add(structName);
    
    // Add to sorted list
    sorted.push(structName);
  }
  
  // Visit all structs to create a topologically sorted list
  for (const structName of structs.keys()) {
    if (!visited.has(structName)) {
      visit(structName);
    }
  }
  
  // Generate the struct definitions in topological order (dependencies first)
  for (const structName of sorted) {
    const struct = structs.get(structName);
    if (struct) {
      result += struct.code;
    }
  }
  
  return result;
}      
      // Generate all required structs
      const structDefinitions = generateStructDefinitions(rootNode, [formattedCollectionName]);
      slintCode += structDefinitions;
      
      // 3. Start generating the global
      slintCode += `// ${collectionData.name} Variables\n`;
      slintCode += `export global ${collectionData.formattedName} {\n`;
      
      // Current column property
      slintCode += `    in-out property <${collectionData.formattedName}Column> current-column: ${modes[0] || 'light'};\n\n`;
      
      // 4. Add hierarchical properties with initializers
      function generateHierarchicalProperties(node: VariableNode, indent: string = "    ", path: string[] = []): string {
        let result = '';
        
        // Skip the root node
        if (path.length === 0) {
          // Process top-level properties only
          for (const [childName, childNode] of node.children.entries()) {
            if (!childNode.valuesByMode) {
              // This is a struct node - FIX: use correct struct name with full path
              // Include the collection prefix in the struct name
              const structName = [formattedCollectionName, childName].map(p => formatStructName(p)).join('_');
              
              result += `${indent}out property <${structName}> ${childName}: {\n`;
              
              // Initialize nested properties
              result += generateHierarchicalProperties(childNode, indent + "    ", [...path, childName]);
              
              result += `${indent}};\n\n`;
            }
          }
          return result;
        }
        
        // Process properties of non-root nodes
        for (const [childName, childNode] of node.children.entries()) {
          if (childNode.valuesByMode) {
            // This is a leaf node
            const defaultMode = modes[0];
            const defaultValue = childNode.valuesByMode.get(defaultMode)?.value || 
                               (childNode.type === 'color' ? '#000000' : 
                                childNode.type === 'length' ? '0px' : 
                                childNode.type === 'bool' ? 'false' : '""');
            
            result += `${indent}${childName}: ${defaultValue},\n`;
          } else {
            // This is a nested struct
            result += `${indent}${childName}: {\n`;
            
            // Process nested properties
            result += generateHierarchicalProperties(childNode, indent + "    ", [...path, childName]);
            
            result += `${indent}},\n`;
          }
        }
        
        return result;
      }
            
      // Add hierarchical properties
      slintCode += generateHierarchicalProperties(rootNode);
      
      // 5. Determine types for variables (for flat structure)
      const variableTypes = new Map<string, string>();
      for (const [rowName, columns] of collectionData.variables.entries()) {
        for (const [, data] of columns.entries()) {
          if (!variableTypes.has(rowName)) {
            variableTypes.set(rowName,
              data.type === 'COLOR' ? 'color' :
              data.type === 'FLOAT' ? 'length' :
              data.type === 'BOOLEAN' ? 'bool' :
              'string'
            );
          }
          break;
        }
      }
      
      // 6. Add individual cell properties (flat structure)
      slintCode += `    // Individual cell values\n`;
      for (const [rowName, columns] of collectionData.variables.entries()) {
        const rowType = variableTypes.get(rowName) || 'color';
        
        for (const [colName, data] of columns.entries()) {
          let valueExpression = data.value;
          
          // Fix for empty or invalid values
          if (valueExpression === undefined || valueExpression === null || valueExpression === '') {
            if (data.type === 'STRING') {
              valueExpression = `"default"`;
            } else if (data.type === 'BOOLEAN') {
              valueExpression = 'false';
            } else if (data.type === 'FLOAT') {
              valueExpression = '0px';
            } else if (data.type === 'COLOR') {
              valueExpression = '#808080';
            }
          }
          
          // Add comments for references
          if (data.refId) {
            const refName = variableNameById.get(data.refId) || data.refId;
            valueExpression = `${valueExpression} /* Reference to ${refName} */`;
          }
          
          slintCode += `    out property <${rowType}> ${rowName}-${sanitizeModeForEnum(colName)}: ${valueExpression};\n`;
        }
      }
      
      // 7. Generate row accessor functions (flat structure)
      slintCode += `\n    // Row accessor functions\n`;
      for (const [rowName, columns] of collectionData.variables.entries()) {
        const rowType = variableTypes.get(rowName) || 'color';
        
        slintCode += `    function ${rowName}(column: ${collectionData.formattedName}Column) -> ${rowType} {\n`;
        slintCode += `        if (`;
        
        let isFirst = true;
        for (const [colName] of columns.entries()) {
          if (!isFirst) slintCode += `} else if (`;
          slintCode += `column == ${collectionData.formattedName}Column.${sanitizeModeForEnum(colName)}`;
          if (isFirst) isFirst = false;
          
          slintCode += `) {\n`;
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
      
      // 8. Generate current value properties (flat structure)
      slintCode += `\n    // Current values based on current-column\n`;
      for (const [rowName] of collectionData.variables.entries()) {
        const rowType = variableTypes.get(rowName) || 'color';
        slintCode += `    out property <${rowType}> current-${rowName}: ${rowName}(self.current-column);\n`;
      }
      
      // 9. Generate hierarchical accessors
      function generateHierarchicalAccessors(node: VariableNode, path: string[] = []): string {
        let result = '';
        
        // Skip the root node
        if (path.length === 0) {
          for (const [childName, childNode] of node.children.entries()) {
            result += generateHierarchicalAccessors(childNode, [childName]);
          }
          return result;
        }
        
        // For leaf nodes (actual variables)
        if (node.valuesByMode) {
          const functionName = path.join('_');
          
          result += `\n    function ${functionName}(column: ${collectionData.formattedName}Column) -> ${node.type || 'color'} {\n`;
          result += `        if (`;
          
          // Add conditions for each mode
          let isFirst = true;
          for (const mode of modes) {
            if (!node.valuesByMode.has(mode)) continue;
            
            if (!isFirst) result += `} else if (`;
            result += `column == ${collectionData.formattedName}Column.${mode}`;
            isFirst = false;
            
            result += `) {\n`;
            result += `            return ${node.valuesByMode.get(mode)!.value};\n`;
            result += `        `;
          }
          
          // Default case
          result += `} else {\n`;
          result += `            return ${node.valuesByMode.get(modes[0])?.value || '#000000'};\n`;
          result += `        }\n`;
          result += `    }\n`;
        } else {
          // For struct nodes, process children
          for (const [childName, childNode] of node.children.entries()) {
            result += generateHierarchicalAccessors(childNode, [...path, childName]);
          }
        }
        
        return result;
      }
      
      // Add hierarchical accessors
      slintCode += `\n    // Hierarchical accessors${generateHierarchicalAccessors(rootNode)}`;
      
      // Close the global
      slintCode += `}\n\n`;
      
      // Add this collection to our exported files
      exportedFiles.push({
        name: `${formatStructName(collection.name)}.slint`,
        content: slintCode
      });
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


// Helper function to resolve variable references
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

