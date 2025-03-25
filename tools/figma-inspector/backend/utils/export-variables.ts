import * as fs from 'fs';
import * as path from 'path';

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

// For Figma Plugin - Export function (now async)
export async function exportFigmaVariablesToSlint(): Promise<string> {
  try {
    // Get collections asynchronously
    const variableCollections = await figma.variables.getLocalVariableCollectionsAsync();
    const variableMap = new Map<string, {name: string, value: any, type: string, resolvedVariable: any}>();
    
    // Initialize results for different variable types
    const colorVars = [];
    const sizeVars = [];
    const textVars = [];
    
    // Fetch all variables at once to avoid multiple API calls
    for (const collection of variableCollections) {
      // Get variables for this collection asynchronously (in batches if many)
      const variablePromises = [];
      
      // Process in smaller batches to prevent memory issues
      const batchSize = 20;
      for (let i = 0; i < collection.variableIds.length; i += batchSize) {
        const batch = collection.variableIds.slice(i, i + batchSize);
        const batchPromises = batch.map(id => figma.variables.getVariableByIdAsync(id));
        const batchResults = await Promise.all(batchPromises);
        
        for (const variable of batchResults) {
          if (!variable) continue;
          
          const modeId = collection.defaultModeId;
          const value = variable.valuesByMode[modeId];
          
          // Format variable name with collection prefix for uniqueness
          const formattedName = `${formatVariableName(collection.name)}-${formatVariableName(variable.name)}`;
          
          variableMap.set(variable.id, {
            name: formattedName,
            value,
            type: variable.resolvedType,
            resolvedVariable: variable
          });
          
          // Categorize by type
          if (variable.resolvedType === 'COLOR') {
            colorVars.push(variable);
          } else if (variable.resolvedType === 'FLOAT') {
            sizeVars.push(variable);
          } else if (variable.resolvedType === 'STRING') {
            textVars.push(variable);
          }
        }
      }
    }
    
    // Generate Slint code
    let slintCode = `// Generated from Figma variables\n\n`;
    
    // Add color variables
    slintCode += `export global Colors {\n`;
    slintCode += `    // Color variables\n`;
    
    for (const variable of colorVars) {
      const varData = variableMap.get(variable.id);
      if (!varData) continue;
      
      const { name, value } = varData;
      const modeId = variable.variableCollectionId ? 
        (variableCollections.find(c => c.id === variable.variableCollectionId)?.defaultModeId || '') : '';
      
      if (!modeId) continue;
      
      if (typeof value === 'object' && value && value.type === 'VARIABLE_ALIAS') {
        const targetVar = variableMap.get(value.id);
        if (targetVar) {
          slintCode += `    out property <color> ${name}: @${targetVar.name};\n`;
        }
      } else {
        try {
          const resolvedValue = variable.valuesByMode[modeId];
          if (resolvedValue && typeof resolvedValue === 'object' && 'r' in resolvedValue) {
            slintCode += `    out property <color> ${name}: ${convertColor(resolvedValue)};\n`;
          }
        } catch (e) {
          slintCode += `    out property <color> ${name}: #000000; // Failed to resolve\n`;
        }
      }
    }
    
    slintCode += `}\n\n`;
    
    // Add float/size variables
    slintCode += `export global Sizing {\n`;
    slintCode += `    // Size variables\n`;
    
    for (const variable of sizeVars) {
      const varData = variableMap.get(variable.id);
      if (!varData) continue;
      
      const { name } = varData;
      const modeId = variable.variableCollectionId ? 
        (variableCollections.find(c => c.id === variable.variableCollectionId)?.defaultModeId || '') : '';
      
      if (!modeId) continue;
      
      try {
        const resolvedValue = variable.valuesByMode[modeId];
        if (typeof resolvedValue === 'number') {
          slintCode += `    out property <length> ${name}: ${resolvedValue}px;\n`;
        }
      } catch (e) {
        slintCode += `    out property <length> ${name}: 0px; // Failed to resolve\n`;
      }
    }
    
    slintCode += `}\n\n`;
    
    // Add string variables
    slintCode += `export global Text {\n`;
    slintCode += `    // Text variables\n`;
    
    for (const variable of textVars) {
      const varData = variableMap.get(variable.id);
      if (!varData) continue;
      
      const { name } = varData;
      const modeId = variable.variableCollectionId ? 
        (variableCollections.find(c => c.id === variable.variableCollectionId)?.defaultModeId || '') : '';
      
      if (!modeId) continue;
      
      try {
        const resolvedValue = variable.valuesByMode[modeId];
        if (typeof resolvedValue === 'string') {
          slintCode += `    out property <string> ${name}: "${resolvedValue}";\n`;
        }
      } catch (e) {
        slintCode += `    out property <string> ${name}: ""; // Failed to resolve\n`;
      }
    }
    
    slintCode += `}\n`;
    
    return slintCode;
  } catch (error) {
    console.error("Error in exportFigmaVariablesToSlint:", error);
    return `// Error generating variables: ${error.message}`;
  }
}