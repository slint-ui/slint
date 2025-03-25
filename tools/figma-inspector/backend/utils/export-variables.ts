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

// Helper to format variable name for Slint (convert to kebab-case)
function formatVariableName(name: string): string {
  return name
    .replace(/([a-z])([A-Z])/g, '$1-$2')
    .replace(/\s+/g, '-')
    .toLowerCase();
}

// Extract hierarchy from variable name (e.g. "colors/primary/base" → ["colors", "primary", "base"])
function extractHierarchy(name: string): string[] {
  // Split by common hierarchy separators
  const parts = name.split(/\/|\.|:|--|-(?=[a-z])/);
  return parts.map(part => formatVariableName(part));
}

// For Figma Plugin - Export function with hierarchical structure
export async function exportFigmaVariablesToSlint(): Promise<string> {
  try {
    // Get collections asynchronously
    const variableCollections = await figma.variables.getLocalVariableCollectionsAsync();
    console.log("Collections count:", variableCollections.length);
    
    // Define interface for the hierarchy structure
    interface HierarchyNode {
      [key: string]: HierarchyNode | { id: string, type: string, isLeaf: boolean } | any;
    }
    
    // Create hierarchical structure - using a plain object instead of Map
    const hierarchyMap: HierarchyNode = {};
    const variableMap = new Map<string, {
      name: string,
      path: string[],
      value: any,
      type: string,
      resolvedVariable: any
    }>();

    // First pass: analyze structure and build hierarchy
    for (const collection of variableCollections) {
      console.log(`Collection: ${collection.name}, Variables: ${collection.variableIds.length}`);
      
      // Process in smaller batches to prevent memory issues
      const batchSize = 10;
      for (let i = 0; i < collection.variableIds.length; i += batchSize) {
        const batch = collection.variableIds.slice(i, i + batchSize);
        const batchPromises = batch.map(id => figma.variables.getVariableByIdAsync(id));
        const batchResults = await Promise.all(batchPromises);
        
        for (const variable of batchResults) {
          if (!variable) continue;
          
          // Log the first few variables to understand structure
          if (i < 2) {
            console.log(`  Variable: ${variable.name}, Type: ${variable.resolvedType}`);
          }
          
          const modeId = collection.defaultModeId;
          const value = variable.valuesByMode[modeId];
          
          // Detect hierarchy from variable name
          const nameParts = extractHierarchy(variable.name);
          
          // Build full path including collection
          const collectionName = formatVariableName(collection.name);
          const fullPath = [collectionName, ...nameParts];
          const formattedName = fullPath.join('-');
          
          // Save the variable info
          variableMap.set(variable.id, {
            name: formattedName,
            path: fullPath, 
            value,
            type: variable.resolvedType,
            resolvedVariable: variable
          });
          
          // Build hierarchy tree
          let currentLevel = hierarchyMap;
          for (let i = 0; i < fullPath.length; i++) {
            const part = fullPath[i];
            
            if (i === fullPath.length - 1) {
              // Leaf node (actual variable)
              currentLevel[part] = {
                id: variable.id,
                type: variable.resolvedType,
                isLeaf: true
              };
            } else {
              // Branch node (structural)
              if (!currentLevel[part]) {
                currentLevel[part] = {};
              }
              currentLevel = currentLevel[part];
            }
          }
        }
      }
    }
    
    // Log the resulting hierarchy for debugging
    console.log("Hierarchy structure:", JSON.stringify(hierarchyMap, null, 2).slice(0, 500) + "...");
    
    // Generate Slint code based on hierarchical structure
    let slintCode = `// Generated from Figma variables\n\n`;
    
    // Helper function to generate struct code for a branch in the hierarchy
    function generateStructForBranch(branch: any, path: string[] = []): string {
      let code = '';
      const structName = path.length === 0 ? 'DesignTokens' : path[path.length - 1];
      
      // Start struct definition
      code += `export struct ${structName} {\n`;
      
      // Group variables by type
      const colorVars = [];
      const sizeVars = [];
      const textVars = [];
      const subStructs = [];
      
      // Type guard to check if a value is a leaf node
      function isLeafNode(value: any): value is { id: string, type: string, isLeaf: boolean } {
        return value && typeof value === 'object' && 'isLeaf' in value && value.isLeaf === true;
      }
      
      // Process all children
      for (const [key, value] of Object.entries(branch)) {
        if (isLeafNode(value)) {
          const varData = variableMap.get(value.id);
          if (!varData) continue;
          
          if (value.type === 'COLOR') {
            colorVars.push({ key, varData });
          } else if (value.type === 'FLOAT') {
            sizeVars.push({ key, varData });
          } else if (value.type === 'STRING') {
            textVars.push({ key, varData });
          }
        } else {
          // It's a sub-struct
          subStructs.push({ key, value });
        }
      }
      
      // Add color properties
      if (colorVars.length > 0) {
        code += `    // Color variables\n`;
        for (const { key, varData } of colorVars) {
          const variable = varData.resolvedVariable;
          const modeId = variable.variableCollectionId ? 
            (variableCollections.find(c => c.id === variable.variableCollectionId)?.defaultModeId || '') : '';
          
          if (!modeId) continue;
          
          const { value } = varData;
          if (typeof value === 'object' && value && value.type === 'VARIABLE_ALIAS') {
            const targetVar = variableMap.get(value.id);
            if (targetVar) {
              code += `    out property <color> ${key}: @${targetVar.name};\n`;
            }
          } else {
            try {
              const resolvedValue = variable.valuesByMode[modeId];
              if (resolvedValue && typeof resolvedValue === 'object' && 'r' in resolvedValue) {
                code += `    out property <color> ${key}: ${convertColor(resolvedValue)};\n`;
              }
            } catch (e) {
              code += `    out property <color> ${key}: #000000; // Failed to resolve\n`;
            }
          }
        }
        code += '\n';
      }
      
      // Add size properties
      if (sizeVars.length > 0) {
        code += `    // Size variables\n`;
        for (const { key, varData } of sizeVars) {
          const variable = varData.resolvedVariable;
          const modeId = variable.variableCollectionId ? 
            (variableCollections.find(c => c.id === variable.variableCollectionId)?.defaultModeId || '') : '';
          
          if (!modeId) continue;
          
          try {
            const resolvedValue = variable.valuesByMode[modeId];
            if (typeof resolvedValue === 'number') {
              code += `    out property <length> ${key}: ${resolvedValue}px;\n`;
            }
          } catch (e) {
            code += `    out property <length> ${key}: 0px; // Failed to resolve\n`;
          }
        }
        code += '\n';
      }
      
      // Add text properties
      if (textVars.length > 0) {
        code += `    // Text variables\n`;
        for (const { key, varData } of textVars) {
          const variable = varData.resolvedVariable;
          const modeId = variable.variableCollectionId ? 
            (variableCollections.find(c => c.id === variable.variableCollectionId)?.defaultModeId || '') : '';
          
          if (!modeId) continue;
          
          try {
            const resolvedValue = variable.valuesByMode[modeId];
            if (typeof resolvedValue === 'string') {
              code += `    out property <string> ${key}: "${resolvedValue}";\n`;
            }
          } catch (e) {
            code += `    out property <string> ${key}: ""; // Failed to resolve\n`;
          }
        }
        code += '\n';
      }
      
      // Add sub-structs
      const generatedSubStructs = [];
      for (const { key, value } of subStructs) {
        const subStructCode = generateStructForBranch(value, [...path, key]);
        code += `    out property <${key}> ${key};\n`;
        // Store the sub-struct code to be added after this struct
        generatedSubStructs.push(subStructCode);
      }
      
      // Close struct definition
      code += `}\n\n`;
      
      // Add sub-struct definitions
      for (const subStructCode of generatedSubStructs) {
        code += subStructCode;
      }
      
      return code;
    }
    
    // Generate the main design tokens struct and all nested structs
    slintCode += generateStructForBranch(hierarchyMap);
    
    // Add a global for easy access
    slintCode += `export global Tokens {\n`;
    slintCode += `    out property <DesignTokens> design;\n`;
    slintCode += `}\n`;
    
    return slintCode;
  } catch (error) {
    console.error("Error in exportFigmaVariablesToSlint:", error);
    return `// Error generating variables: ${error}`;
  }
}