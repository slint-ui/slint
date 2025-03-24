import { generateRectangleSnippet, generateTextSnippet, generateSlintSnippet } from './property-parsing';

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
    type?: string;  // Add type
    enums: { [key: string]: string[] };
    variants: Array<{
        name: string;
        properties: { [key: string]: string };
        style?: ComponentStyle;
        children: SlintComponent[];
    }>;
    style: ComponentStyle;
    properties?: { [key: string]: any };  // Add properties
    children: SlintComponent[];
}

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
    // console.clear();
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
/*
function findCommonChildren(variants: ReadonlyArray<ComponentNode>): SceneNode[] {
    // Debug the incoming variants first
    console.log('Processing variants:', variants.map(v => ({
        name: v.name,
        childCount: v.children?.length,
        children: v.children?.map(c => `${c.name} (${c.type})`)
    })));

    if (!variants.length || !variants[0].children?.length) {
        return [];
    }

    // Get first variant's children
    const firstVariantChildren = variants[0].children;
    
    // Look for children that exist in ALL variants
    const commonChildren = firstVariantChildren.filter(child => {
        const isCommon = variants.every(variant => 
            variant.children?.some(variantChild => 
                variantChild.name === child.name && 
                variantChild.type === child.type
            )
        );
        
        console.log(`Child ${child.name} (${child.type}) is common:`, isCommon);
        return isCommon;
    });

    console.log('Found common children:', commonChildren.map(c => c.name));
    return commonChildren;
}
*/

/*
function handleComponentSet(node: ComponentSetNode): VariantInfo {
    // Find common children first
    const commonChildren = findCommonChildren(node.children.filter((child): child is ComponentNode => child.type === "COMPONENT"));

    // Get variant property definitions
    const variantProperties: VariantInfo['variantProperties'] = {};
    if (node.componentPropertyDefinitions) {
        Object.entries(node.componentPropertyDefinitions).forEach(([key, def]) => {
            variantProperties[key] = {
                type: def.type,
                defaultValue: def.defaultValue,
                variantOptions: def.variantOptions
            };
        });
    }

    // Process variants
    const variants = node.children.map(variant => {
        const propertyValues: { [key: string]: string | boolean | number } = {};
        
        // Extract variant name properties
        const variantParts = variant.name.split(', ');
        variantParts.forEach(part => {
            const [key, value] = part.split('=');
            if (key && value) {
                const sanitizedKey = key.trim();
                const enumName = `${node.name}_${sanitizedKey}`;
                propertyValues[sanitizedKey] = value.trim();
            }
        });
        
        // Extract component properties
        if ('componentProperties' in variant) {
            Object.entries(variant.componentProperties).forEach(([key, prop]) => {
                if ('value' in prop) {
                    propertyValues[key] = prop.value;
                }
            });
        }

        // Get variant's style info but exclude position properties
        const snippet = generateSlintSnippet({
            ...variant,
            x: undefined,
            y: undefined
        });
        const style = parseSnippetToStyle(snippet || '');

        // Filter to only non-common children for states
        const stateChildren = ('children' in variant) ? 
            variant.children
                .filter(child => !commonChildren.some(common => 
                    common.name === child.name && common.type === child.type
                ))
                .map(child => ({
                    name: child.name,
                    id: child.id,
                    variantProperties: {},
                    variants: [{
                        name: child.name,
                        id: child.id,
                        propertyValues: {},
                        style: parseSnippetToStyle(generateSlintSnippet(child) || '')
                    }],
                    style: parseSnippetToStyle(generateSlintSnippet(child) || '')
                })) : [];

        return {
            name: variant.name,
            id: variant.id,
            propertyValues,
            style,
            children: stateChildren
        };
    });

    return {
        name: node.name,
        id: node.id,
        variantProperties,
        variants,
        style: {},
        children: commonChildren.map(child => ({
            name: child.name,
            id: child.id,
            variantProperties: {},
            variants: [],
            style: parseSnippetToStyle(generateSlintSnippet(child) || '')
        }))
    };
}
*/

function parseSnippetToStyle(snippet: string): ComponentStyle {
    const style: ComponentStyle = {};
    const lines = snippet.split('\n');
    let currentObject: any = style;
    
    lines.forEach(line => {
        const match = line.match(/^\s*([a-z-]+):\s*(.+);$/);
        if (match) {
            const [, key, value] = match;
            
            // Handle specific properties
            if (key === 'text') {
                currentObject[key] = value.replace(/"/g, '');
            } else if (key === 'color' || key === 'background') {
                currentObject[key] = value;
            } else if (value.endsWith('px')) {
                currentObject[key] = Number(value.replace('px', ''));
            } else {
                currentObject[key] = value;
            }
        }
    });

    return style;
}

// Add this new function for string sanitization
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

    // Convert common children (preserve original names and types)
    const commonChildren = componentSet.children?.map(child => ({
        componentName: sanitizeIdentifier(child.name, 'component'),
        type: child.type === 'TEXT' ? 'Text' : 'Rectangle', // Convert Figma types to Slint types
        style: child.style || {},
        variants: [], 
        enums: {},
        children: []
    })) || [];

    // Map variants, excluding common children
    const variants = componentSet.variants.map(v => ({
        name: v.name,
        properties: Object.fromEntries(
            Object.entries(v.propertyValues || {}).map(([key, value]) => [
                key,
                String(value)
            ])
        ),
        style: v.style || {},
        children: v.children
            ?.filter(child => !componentSet.children?.some(c => c.name === child.name))
            .map(child => ({
                componentName: sanitizeIdentifier(child.name, 'component'),
                type: child.type || 'Rectangle',
                style: child.style || {},
                variants: [],
                enums: {},
                children: []
            })) || []
    }));

    return {
        componentName,
        enums,
        variants,
        style: componentSet.style || {},
        children: commonChildren
    };
}
// helper function to output hex
function rgbToHex(r: number, g: number, b: number): string {
    const toHex = (n: number) => Math.round(n * 255).toString(16).padStart(2, '0');
    return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

function generateSlintCode(slintComponent: SlintComponent): string {
    let code = '';
    
    // Generate enums from variant properties
    for (const [enumName, values] of Object.entries(slintComponent.enums)) {
        code += `export enum ${enumName} {\n`;
        values.forEach(value => {
            code += `    ${value},\n`;
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
    if (slintComponent.style.width !== undefined) {
        code += `        width: ${slintComponent.style.width}px;\n`;
    }
    if (slintComponent.style.height !== undefined) {
        code += `        height: ${slintComponent.style.height}px;\n`;
    }

    // Add common children
    slintComponent.children?.forEach(child => {
        code += `        ${toLowerDashed(child.componentName)} := ${child.type} {\n`;
        // Add style properties
        if (child.style) {
            Object.entries(child.style).forEach(([key, value]) => {
                if (value !== undefined && typeof value !== 'object') {
                    if (['width', 'height', "font-size", 'x', 'y'].includes(key)) {
                        code += `            ${key}: ${value}px;\n`;
                    } else if (key === 'text') {
                        code += `            ${key}: "${value}";\n`;
                    } else {
                        code += `            ${key}: ${value};\n`;
                    }
                }
            });
        }
        code += `        }\n`;
    });

    code += `    }\n\n`;

    // Add states for variant-specific properties
    code += `    states [\n`;
    slintComponent.variants.forEach(variant => {
        const conditions = Object.entries(variant.properties)
            .map(([key, value]) => {
                const enumName = `${slintComponent.componentName}_${toUpperCamelCase(key)}`;
                return `${toLowerDashed(key)} == ${enumName}.${value}`;
            })
            .join(' && ');

        const variantId = variant.name
            .replace(/,\s*/g, '_')
            .replace(/[^a-zA-Z0-9]/g, '_')
            .toLowerCase();

        code += `        ${variantId} when ${conditions}: {\n`;
        if (variant.style) {
            Object.entries(variant.style).forEach(([key, value]) => {
                if (value !== undefined && typeof value !== 'object') {
                    if (['width', 'height', "font-size", 'x', 'y'].includes(key)) {
                        code += `            base-rect.${toLowerDashed(key)}: ${value}px;\n`;
                    } else {
                        code += `            base-rect.${toLowerDashed(key)}: ${value};\n`;
                    }
                }
            });
        }
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
    isCommon?: boolean;
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
                isCommon: false
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
            isCommon: !hasPropertyDifferences
        };
    });
}