import { generateRectangleSnippet, generateTextSnippet, generateSlintSnippet, generateComponentProperties, PropertyHandler} from './property-parsing';

interface VariantInfo {
    name: string;
    id: string;
    type?: string;  // Add this
    variantProperties: {
        [key: string]: {
            type: ComponentPropertyType;
            defaultValue: string | boolean;
            preferredValues?: InstanceSwapPreferredValue[];
            variantOptions?: string[];
            boundVariables?: { [key: string]: VariableAlias };
        };
    };
    variants: Array<{
        name: string;
        id: string;
        type?: string;  // Add this
        propertyValues: { [key: string]: string | boolean | number };
        style?: ComponentStyle;
        children?: VariantInfo[];
    }>;
    style?: ComponentStyle;
    children?: VariantInfo[];
}

interface AutoLayout {
    direction: "HORIZONTAL" | "VERTICAL" | "NONE";
    spacing?: number;
    padding?: {
        top: number;
        right: number;
        bottom: number;
        left: number;
    };
    alignment?: string;
    crossAxisAlignment?: string;
}

interface ComponentStyle {
    width?: number;
    height?: number;
    background?: string;
    borderWidth?: number;
    borderColor?: string;
    padding?: { top: number; right: number; bottom: number; left: number };
    layout?: {
        direction: "HORIZONTAL" | "VERTICAL" | "NONE";
        spacing?: number;
        alignment?: string;
        crossAxisAlignment?: string;
    };
}

interface SlintComponent {
    componentName: string;
    type?: string;
    enums: { [key: string]: string[] };
    variants: Array<{
        name: string;
        properties: { [key: string]: string };
        style?: ComponentStyle;
        children: SlintComponent[];
    }>;
    style: ComponentStyle;
    properties?: { [key: string]: any };
    isCommon?: boolean; 
    children: SlintComponent[];
}

/////////////////////////////////////////
// HELPERS

function mapNodeTypeToSlintType(nodeType: string): string {
    switch (nodeType) {
        case "TEXT":
            return "Text";
        case "RECTANGLE":
        case "FRAME":
        case "COMPONENT":
        case "INSTANCE":
        case "BOOLEAN_OPERATION":
            return "Rectangle";
        default:
            return "Rectangle"; // Default fallback
    }
}

function sanitizeEnumValue(value: string): string {
    return toUpperCamelCase(value);
}

function rgbToHex(r: number, g: number, b: number): string {
    const toHex = (n: number) => Math.round(n * 255).toString(16).padStart(2, '0');
    return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

function processChildrenRecursively(node: VariantInfo): SlintComponent[] {
    if (!node.children || node.children.length === 0) {
        return [];
    }
    
    return node.children.map(child => {
        // Process this child's children recursively
        const childrenOfChild = processChildrenRecursively(child);
        
        return {
            componentName: sanitizeIdentifier(child.name, 'component'),
            type: mapNodeTypeToSlintType(child.type),
            style: child.style || {},
            isCommon: true, // Since these are children of a common component
            variants: [],
            enums: {},
            children: childrenOfChild
        };
    });
}

function generateNestedChildren(children: SlintComponent[], indentLevel: number): string {
    let code = '';
    const indent = ' '.repeat(indentLevel * 4);
    
    children.forEach(child => {
        const childName = toLowerDashed(child.componentName);
        code += `${indent}${childName} := ${child.type} {\n`;
        code += generateComponentProperties(child.type, child.style, indentLevel + 1);
        
        // Recursive call for deeper nesting
        if (child.children && child.children.length > 0) {
            code += generateNestedChildren(child.children, indentLevel + 1);
        }
        
        code += `\n${indent}}\n`;
    });
    
    return code;
}
/////////////////////////////////////////

export function exportComponentSet(): void {
    const selectedNodes = figma.currentPage.selection;

    if (selectedNodes.length === 0) {
        figma.notify("Please select a component set");
        return;
    }

    const componentSets = selectedNodes.filter(
        node => node.type === "COMPONENT_SET"
    ) as ComponentSetNode[];


    if (componentSets.length === 0) {
        figma.notify("No component sets selected");
        return;
    }

    const slintComponents = componentSets.map(node => {
        const variantInfo = getComponentSetInfo(node);
        console.log("Variant info:", variantInfo);  // Add this
        return convertToSlintFormat(variantInfo);
    });

    // console.log("Generated components:", slintComponents);  // Add this
    const slintCode = slintComponents.map(generateSlintCode).join("\n\n");
    console.clear();
    console.log(slintCode);  
    figma.ui.postMessage({ type: "exportComplete", code: slintCode });
}

const usedNames = new Map<string, number>();  

function getComponentSetInfo(node: ComponentSetNode): VariantInfo {
    
    // Get variant property definitions first
    const variantProperties: VariantInfo['variantProperties'] = {};
    if (node.componentPropertyDefinitions) {
        Object.entries(node.componentPropertyDefinitions).forEach(([key, def]) => {
            // console.log("Property definition:", key, def);
            variantProperties[key] = {
                type: def.type,
                defaultValue: def.defaultValue,
                variantOptions: def.variantOptions
            };
        });
    }

    const variants = node.children.map(variant => {
        const propertyValues: { [key: string]: string | boolean | number } = {};
        const variantParts = variant.name.split(', ');
        variantParts.forEach(part => {
            const [key, value] = part.split('=');
            if (key && value) {
                // Transform the key to match our enum naming
                const sanitizedKey = key.trim();
                const enumName = `${node.name}_${sanitizedKey}`;
                propertyValues[sanitizedKey] = value.trim();
            }
        });
        
        // Extract variant properties
        if ('componentProperties' in variant) {
            Object.entries(variant.componentProperties).forEach(([key, prop]) => {
                // Debug log
                // console.log("Property:", key, prop);
                if ('value' in prop) {
                    propertyValues[key] = prop.value;
                }
            });
        }

        // Get variant's style and structure
        const snippet = generateSlintSnippet(variant);
        const style = parseSnippetToStyle(snippet || '');

        // Process children recursively
        const children = ('children' in variant) ? 
            variant.children
                .filter(child => 'type' in child)
                .map(child => ({
                    name: child.name,
                    id: child.id,
                    type: child.type,
                    variantProperties: {},
                    variants: [{
                        name: child.name,
                        id: child.id,
                        type: child.type,
                        propertyValues: {},
                        style: parseSnippetToStyle(generateSlintSnippet(child) || '')
                    }],
                    style: parseSnippetToStyle(generateSlintSnippet(child) || '')
                })) : [];

        return { name: variant.name, id: variant.id, propertyValues, style, children };
    });
    // Get common style
    const baseStyle = parseSnippetToStyle(generateSlintSnippet(node) || '');

    // Find common children (same structure across all variants)
    const commonChildren = variants[0]?.children?.filter(child => 
        variants.every(v => v.children?.some(c => c.name === child.name))
    ) || [];

    return {
        name: node.name,
        id: node.id,
        variantProperties,
        variants,
        style: baseStyle,
        children: commonChildren
    };
}
// Update parseSnippetToStyle to capture more properties
function parseSnippetToStyle(snippet: string): ComponentStyle {
    const style: ComponentStyle = {};
    
    // Extract all properties from the snippet
    const lines = snippet.split('\n');
    lines.forEach(line => {
        // Match any property in the form "property: value;"
        const match = line.match(/^\s*([a-z-]+):\s*(.+?);$/);
        if (match) {
            const [, key, value] = match;
            
            // Handle different property types
            if (key === 'border-radius') {
                style[key] = parseFloat(value);
            } else if (key === 'opacity') {
                // Convert "50%" to 0.5
                style[key] = parseInt(value) / 100;
            } else if (key === 'fill' || key === 'background') {
                style.background = value;
            } else if (key === 'border-width') {
                style[key] = parseFloat(value);
            } else if (key === 'border-color') {
                style[key] = value;
            } else {
                // Default parsing for other properties
                style[key] = PropertyHandler.parse(key, value);
            }
        }
    });

    // Check for fills separately to ensure they're captured
    if (snippet.includes("fill:") && !style.background) {
        const fillMatch = snippet.match(/fill:\s*rgba?\(([^)]+)\)/);
        if (fillMatch && fillMatch[1]) {
            const colors = fillMatch[1].split(',').map(n => parseFloat(n.trim()));
            if (colors.length >= 3) {
                style.background = rgbToHex(colors[0], colors[1], colors[2]);
            }
        }
    }

    return style;
}


// Sanitizers
function toUpperCamelCase(str: string): string {
    return str
        .split(/[-_\s]+/)
        .map(word => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
        .join('');
}

function toLowerDashed(str: string): string {
    return str
        .replace(/([a-z])([A-Z])/g, '$1-$2')
        .toLowerCase()
        .replace(/[^a-z0-9-]/g, '-')
        .replace(/-+/g, '-')
        .replace(/^-|-$/g, '');
}

function sanitizeIdentifier(name: string, type: 'component' | 'property' = 'property'): string {
    const baseName = type === 'component' 
        ? toUpperCamelCase(name)
        : toLowerDashed(name);
    
    // Ensure valid identifier
    const safeName = type === 'component'
        ? baseName.replace(/^[^A-Z]/, 'T$&')
        : baseName.replace(/^[^a-z]/, 'p-$&');
    
    // Handle duplicates
    if (usedNames.has(safeName)) {
        const count = usedNames.get(safeName)! + 1;
        usedNames.set(safeName, count);
        return `${safeName}${count}`;
    } else {
        usedNames.set(safeName, 1);
        return safeName;
    }
}

// Update the isValidId function signature
function isValidId(id: string, properties: Set<string>): boolean {
    // List of Slint reserved words and common property names to avoid
    const reservedWords = new Set([
        'text', 'width', 'height', 'x', 'y', 'background', 'color',
        'font-size', 'font-family', 'border-width', 'border-color',
        'border-radius', 'opacity', 'visible', 'enabled', 'parent',
        'children', 'layout', 'callback', 'property'
    ]);
    
    return !reservedWords.has(id) && !properties.has(id);
}

function convertToSlintFormat(componentSet: VariantInfo): SlintComponent {
    const componentName = sanitizeIdentifier(componentSet.name, 'component');
    
    // Extract enums from variant properties
    const enums: { [key: string]: string[] } = {};
    for (const [key, def] of Object.entries(componentSet.variantProperties)) {
        if (def.type === "VARIANT" && def.variantOptions) {
            const enumName = `${componentName}_${toUpperCamelCase(key)}`;
            enums[enumName] = def.variantOptions;
        }
    }

    // Get all unique children across variants
    const allChildren = new Set<string>();
    componentSet.variants.forEach(v => {
        v.children?.forEach(child => {
            allChildren.add(child.name);
        });
    });

    // Convert common children - WITH RECURSIVE PROCESSING
    const commonChildren = componentSet.children?.map(child => {
        // Process this child's children recursively, if it has any
        const childrenOfChild = processChildrenRecursively(child);
        
        return {
            componentName: sanitizeIdentifier(child.name, 'component'),
            type: mapNodeTypeToSlintType(child.type),
            style: child.style || {},
            isCommon: true,
            variants: [], 
            enums: {},
            children: childrenOfChild // Store recursive children
        };
    }) || [];

    // Add variant-specific children with visibility:false
    const variantChildren = Array.from(allChildren)
    .filter(name => !componentSet.children?.some(c => c.name === name))
    .map(name => {
        const child = componentSet.variants[0].children?.find(c => c.name === name);
        return {
            componentName: sanitizeIdentifier(name, 'component'),
            type: mapNodeTypeToSlintType(child?.type || 'RECTANGLE'), // Use our mapping function
            style: {
                ...child?.style,
                visible: false
            },
            isCommon: false,
            variants: [],
            enums: {},
            children: []
        };
    });
        
    // Combine all children
    const allProcessedChildren = [...commonChildren, ...variantChildren];

    return {
        componentName,
        enums,
        variants: componentSet.variants.map(v => {
            // Find child components in this variant
            const variantChildren = v.children?.map(child => {
                const matchingChild = allProcessedChildren.find(
                    c => c.componentName === sanitizeIdentifier(child.name, 'component')
                );
                return {
                    componentName: matchingChild?.componentName || sanitizeIdentifier(child.name, 'component'),
                    type: matchingChild?.type || mapNodeTypeToSlintType(child.type),
                    style: child.style,
                    enums: {},
                    variants: [],
                    children: []
                };
            }) || [];
    
            return {
                name: v.name,
                properties: Object.fromEntries(
                    Object.entries(v.propertyValues || {}).map(([key, value]) => [
                        key,
                        String(value)
                    ])
                ),
                style: v.style || {},
                children: variantChildren // Use actual children, not empty array
            };
        }),
        style: componentSet.style || {},
        children: allProcessedChildren
    };
}



export function generateStateProperties(
    type: string,
    style: ComponentStyle,
    prefix: string,
    indentLevel: number = 1,
    baseStyle: ComponentStyle = {}
): string {
    // Clean component name
    const componentName = prefix.split('.').pop() || '';
    const cleanComponentName = PropertyHandler.cleanComponentName(componentName);
    
    // Create a new style object with manually detected changes
    const changedStyle: ComponentStyle = {};
    
    // For each property in the variant style
    for (const [key, value] of Object.entries(style)) {
        // Skip x/y for base-rect only
        if (prefix === 'base-rect' && (key === 'x' || key === 'y')) {
            continue;
        }
        
        // Explicitly check if values differ
        if (baseStyle[key] !== value) {
            // Deep comparison for objects
            if (typeof value === 'object' && typeof baseStyle[key] === 'object') {
                if (JSON.stringify(value) !== JSON.stringify(baseStyle[key])) {
                    changedStyle[key] = value;
                }
            } else {
                changedStyle[key] = value;
            }
        }
    }
    
    // Generate the property code
    const indent = ' '.repeat(indentLevel * 4);
    let result = '';
    
    for (const [key, value] of Object.entries(changedStyle)) {
        const formattedValue = PropertyHandler.format(key, value);
        result += `${indent}${cleanComponentName}.${key}: ${formattedValue};\n`;
    }
    
    return result;
}

function generateSlintCode(slintComponent: SlintComponent): string {
    let code = '';
    
    // Generate enums from variant properties
    for (const [enumName, values] of Object.entries(slintComponent.enums)) {
        code += `export enum ${enumName} {\n`;
        values.forEach(value => {
            code += `    ${sanitizeEnumValue(value)},\n`;
        });
        code += `}\n\n`;
    }
    
    // Generate component with properties
    code += `export component ${slintComponent.componentName} {\n`;
    
    // Add enum properties
    Object.keys(slintComponent.enums).forEach(enumName => {
        const propertyName = toLowerDashed(enumName.split('_')[1]);
        code += `    in property <${enumName}> ${propertyName};\n`;
    });
    code += '\n';

    // Base component with common elements
    code += `    base-rect := Rectangle {\n`;
    code += generateComponentProperties('Rectangle', slintComponent.style, 2);
    code += '\n';

    // Add common children with proper indentation - WITH RECURSION
    slintComponent.children?.forEach(child => {
        const childName = toLowerDashed(child.componentName);
        code += `        ${childName} := ${child.type} {\n`;
        code += generateComponentProperties(child.type, child.style, 3);
        
        // Add nested children recursively
        if (child.children && child.children.length > 0) {
            code += generateNestedChildren(child.children, 4); // Increase indent level
        }
        
        // Add newline before closing brace
        code += `\n        }\n`;
    });
    code += `    }\n\n`;

    // Add states for variant-specific properties
    code += `    states [\n`;
    slintComponent.variants.forEach(variant => {
        const conditions = Object.entries(variant.properties)
        .map(([key, value]) => {
            const enumName = `${slintComponent.componentName}_${toUpperCamelCase(key)}`;
            // Apply same capitalization to enum value as in definition
            return `${toLowerDashed(key)} == ${enumName}.${sanitizeEnumValue(String(value))}`;
        })
        .join(' && ');

        const variantId = variant.name
            .replace(/,\s*/g, '_')
            .replace(/[^a-zA-Z0-9]/g, '_')
            .toLowerCase();

        code += `        ${variantId} when ${conditions}: {\n`;
        
        // Add visibility state for non-common children
        slintComponent.children?.forEach(child => {
            const variantHasChild = variant.children?.some(vc => {
                // Compare by removing numbers from names
                const cleanName = vc.componentName.replace(/\d+$/, '');
                const childCleanName = child.componentName.replace(/\d+$/, '');
                return childCleanName === cleanName;
            });
            
            if (!child.isCommon && variantHasChild) {
                const cleanChildName = toLowerDashed(child.componentName).replace(/\d+$/, '');
                code += `            ${cleanChildName}.visible: true;\n`; // Remove base-rect prefix
            }
        });

        // Base rectangle properties
        if (variant.style) {
            code += generateStateProperties('Rectangle', variant.style, 'base-rect', 3, slintComponent.style);
            if (code) code += '\n';
        }

        // Add child component states
        variant.children?.forEach(variantChild => {
            // Find the matching child in the component by name (without numbers)
            const child = slintComponent.children?.find(c => {
                const cleanName = c.componentName.replace(/\d+$/, '');
                const variantCleanName = variantChild.componentName.replace(/\d+$/, '');
                return cleanName === variantCleanName;
            });
            
            // Apply style properties from this variant's version of the child
            if (variantChild.style && child) {
                // Use clean component naming
                const cleanChildName = child.componentName.replace(/\d+$/, '');
                const childPrefix = toLowerDashed(cleanChildName);
                
                console.log(`Adding styles for ${childPrefix} in variant ${variant.name}`);
                console.log(`Style:`, variantChild.style);
                
                const childStateProps = generateStateProperties(
                    variantChild.type || 'Rectangle', 
                    variantChild.style, 
                    childPrefix,  // Use clean name
                    3,
                    child.style   // Compare against base style
                );
                
                if (childStateProps) {
                    code += childStateProps;
                }
            }
        });
        code += `        }\n`;
    });
        code += `    ]\n`;

    code += `}\n`;
    return code;
}

interface ChildInfo {
    type: string;
    name: string;
    style?: ComponentStyle;
    properties?: { [key: string]: any };
    isCommon: boolean;
    variants: Array<{
        variantName: string;  // Which variant this child appears in
        visible: boolean;     // Visibility in this variant
        style?: ComponentStyle;
    }>;
}


function getUniqueChildren(
    currentVariant: { 
        name: string, 
        properties: { [key: string]: string }, 
        style?: ComponentStyle,
        children?: SlintComponent[] 
    },
    allVariants: Array<typeof currentVariant>
): ChildInfo[] {
    if (!currentVariant.children) {
        return [];
    }

    return currentVariant.children.map(child => {
        // Check if this child exists in other variants
        const childInOtherVariants = allVariants
            .filter(v => v !== currentVariant)
            .map(v => v.children?.find(c => c.componentName === child.componentName))
            .filter(Boolean);

        // If child doesn't exist in all variants, it's unique
        if (childInOtherVariants.length !== allVariants.length - 1) {
            return {
                type: child.type || 'Rectangle',
                name: child.componentName,
                style: child.style,
                properties: child.properties,
                isCommon: false,
                variants: [{  // Add missing variants array
                    variantName: currentVariant.name,
                    visible: true,
                    style: child.style
                }]
            };
        }

        // If child exists but has different properties, it's unique
        const hasPropertyDifferences = childInOtherVariants.some(otherChild => {
            // Compare style properties
            return JSON.stringify(child.style) !== JSON.stringify(otherChild?.style) ||
                   JSON.stringify(child.properties) !== JSON.stringify(otherChild?.properties);
        });

        return {
            type: child.type || 'Rectangle',
            name: child.componentName,
            style: child.style,
            properties: child.properties,
            isCommon: !hasPropertyDifferences,
            variants: [{  // Add missing variants array
                variantName: currentVariant.name,
                visible: true,
                style: child.style
            }]
        };
    });
}