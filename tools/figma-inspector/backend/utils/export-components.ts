interface VariantInfo {
    name: string;
    id: string;
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
        propertyValues: { [key: string]: string | boolean | number };
        style?: ComponentStyle;  // Add this
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
    enums: { [key: string]: string[] };
    variants: Array<{
        name: string;
        properties: { [key: string]: string };
        style?: ComponentStyle;  // Add this
    }>;
    style: ComponentStyle;
    children: SlintComponent[];
}

export function exportComponentSet(): void {
    const selectedNodes = figma.currentPage.selection;
    console.log("Selected nodes:", selectedNodes.length);  // debug

    if (selectedNodes.length === 0) {
        figma.notify("Please select a component set");
        return;
    }

    const componentSets = selectedNodes.filter(
        node => node.type === "COMPONENT_SET"
    ) as ComponentSetNode[];
    console.log("Component sets found:", componentSets.length);  // debug


    if (componentSets.length === 0) {
        figma.notify("No component sets selected");
        return;
    }

    const slintComponents = componentSets.map(node => {
        console.log("Processing component:", node.name);  // Add this
        const variantInfo = getComponentSetInfo(node);
        console.log("Variant info:", variantInfo);  // Add this
        return convertToSlintFormat(variantInfo);
    });

    console.log("Generated components:", slintComponents);  // Add this
    const slintCode = slintComponents.map(generateSlintCode).join("\n\n");
    console.log("Generated code:\n\n", slintCode);  
    figma.ui.postMessage({ type: "exportComplete", code: slintCode });
}

const usedNames = new Map<string, number>();  

function getComponentSetInfo(node: ComponentSetNode): VariantInfo {
    console.log("Processing component set:", node);
    
    // Get variant property definitions first
    const variantProperties: VariantInfo['variantProperties'] = {};
    if (node.componentPropertyDefinitions) {
        Object.entries(node.componentPropertyDefinitions).forEach(([key, def]) => {
            console.log("Property definition:", key, def);
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
        console.log("Processing variant:", variant);  // Debug log
        
        // Extract variant properties
        if ('componentProperties' in variant) {
            Object.entries(variant.componentProperties).forEach(([key, prop]) => {
                // Debug log
                console.log("Property:", key, prop);
                if ('value' in prop) {
                    propertyValues[key] = prop.value;
                }
            });
        }

        // Base style info (keep existing code)
        const style: ComponentStyle = {
            width: variant.width,
            height: variant.height
        };

        // Layout info
        if ('layoutMode' in variant) {
            style.layout = {
                direction: variant.layoutMode as "HORIZONTAL" | "VERTICAL" | "NONE",
                spacing: variant.itemSpacing,
                alignment: variant.primaryAxisAlignItems,
                crossAxisAlignment: variant.counterAxisAlignItems
            };

            // Add padding if present
            if ('paddingLeft' in variant) {
                style.padding = {
                    top: variant.paddingTop,
                    right: variant.paddingRight,
                    bottom: variant.paddingBottom,
                    left: variant.paddingLeft
                };
            }
        }

        // Process all children recursively
        const children = ('children' in variant) ? 
            variant.children
                .filter(child => 'type' in child) // Only process valid nodes
                .map(child => {
                    if (child.type === "COMPONENT_SET") {
                        return getComponentSetInfo(child);
                    } else if (child.type === "COMPONENT" || child.type === "INSTANCE") {
                        // Process any nested component
                        return {
                            name: child.name,
                            id: child.id,
                            variantProperties: {},
                            variants: [{
                                name: child.name,
                                id: child.id,
                                propertyValues: {}
                            }],
                            style: {
                                width: child.width,
                                height: child.height,
                                // Add layout info for child if present
                                layout: child.layoutMode ? {
                                    direction: child.layoutMode as "HORIZONTAL" | "VERTICAL" | "NONE",
                                    spacing: child.itemSpacing
                                } : undefined
                            }
                        };
                    }
                    return null;
                })
                .filter(Boolean) as VariantInfo[] 
            : [];

            return {
                name: variant.name,
                id: variant.id,
                propertyValues,
                style: {
                    width: variant.width,
                    height: variant.height,
                    // ...rest of style
                },
                children: []
            };
        });
    


    return {
        name: node.name,
        id: node.id,
        variantProperties,
        variants,
        style: variants[0]?.style,
        children: []
    };
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
    // Log the incoming data
    console.log("Converting to Slint format:", componentSet);
    
    const componentName = sanitizeIdentifier(componentSet.name, 'component');
    
    // Extract all variant properties into enums
    const enums: { [key: string]: string[] } = {};
    for (const [key, def] of Object.entries(componentSet.variantProperties)) {
        if (def.type === "VARIANT" && def.variantOptions) {
            const enumName = `${componentName}_${toUpperCamelCase(key)}`;
            enums[enumName] = def.variantOptions;
        }
    }

    // Map variants with their properties
    const variants = componentSet.variants.map(v => {
        console.log("Processing variant properties:", v.propertyValues);
        return {
            name: v.name,
            properties: Object.fromEntries(
                Object.entries(v.propertyValues || {}).map(([key, value]) => [
                    key,
                    String(value)
                ])
            ),
            style: v.style
        };
    });

    console.log("Generated variants:", variants);

    return {
        componentName,
        enums,
        variants,
        style: componentSet.style || {},
        children: componentSet.children?.map(child => 
            convertToSlintFormat(child)
        ) || []
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

    // Generate states for variants
    if (slintComponent.variants.length > 0) {
        code += `    states [\n`;
        slintComponent.variants.forEach(variant => {
            console.log("states:",variant.properties);
            const conditions = Object.entries(variant.properties)
                .map(([key, value]) => {
                    const enumName = `${slintComponent.componentName}_${toUpperCamelCase(key)}`;
                    return `${toLowerDashed(key)} == ${enumName}.${value}`;
                })
                .join(' && ');
            
            if (conditions) {
                code += `        ${variant.name} when ${conditions}: {\n`;
                if (variant.style) {
                    Object.entries(variant.style).forEach(([key, value]) => {
                        if (value !== undefined) {
                            switch(key) {
                                case 'width':
                                case 'height':
                                    code += `            ${key}: ${value}px;\n`;
                                    break;
                                case 'background':
                                    code += `            background: ${value};\n`;
                                    break;
                                case 'borderColor':
                                    code += `            border-color: ${value};\n`;
                                    break;
                                case 'borderWidth':
                                    code += `            border-width: ${value}px;\n`;
                                    break;
                            }
                        }
                    });
                }
                code += `        }\n`;
            }
        });
        code += `    ]\n\n`;
    }

    // Main Rectangle with base styling
    code += `    Rectangle {\n`;
    code += `        width: ${slintComponent.style.width}px;\n`;
    code += `        height: ${slintComponent.style.height}px;\n`;

    // Add layout if present
    if (slintComponent.style?.layout) {
        const layoutType = slintComponent.style.layout.direction === "HORIZONTAL" ? 
            "HorizontalLayout" : "VerticalLayout";
        code += `        ${layoutType} {\n`;
        
        // Add padding if present
        if (slintComponent.style.padding) {
            const p = slintComponent.style.padding;
            if (p.top === p.right && p.top === p.bottom && p.top === p.left) {
                code += `            padding: ${p.top}px;\n`;
            } else {
                if (p.top) code += `            padding-top: ${p.top}px;\n`;
                if (p.right) code += `            padding-right: ${p.right}px;\n`;
                if (p.bottom) code += `            padding-bottom: ${p.bottom}px;\n`;
                if (p.left) code += `            padding-left: ${p.left}px;\n`;
            }
        }

        // Add spacing
        if (slintComponent.style.layout.spacing) {
            code += `            spacing: ${slintComponent.style.layout.spacing}px;\n`;
        }

        // Add child components
        if (slintComponent.children?.length > 0) {
            slintComponent.children.forEach(child => {
                const childCode = generateSlintCode(child)
                    .split('\n')
                    .map(line => `            ${line}`)
                    .join('\n');
                code += childCode;
            });
        }

        
        code += `        }\n`;
    }
    // Generate states for variants
    if (slintComponent.variants.length > 0) {
        code += `    states [\n`;
        slintComponent.variants.forEach(variant => {
            // Parse the variant name to get the property combinations
            // e.g. "Style=Normal, State=Enabled, Type=Regular"
            const conditions = Object.entries(variant.properties)
                .map(([key, value]) => {
                    const enumName = `${slintComponent.componentName}_${toUpperCamelCase(key)}`;
                    return `${toLowerDashed(key)} == ${enumName}.${value}`;
                })
                .join(' && ');

            if (conditions) {
                code += `        variant-${variant.name} when ${conditions}: {\n`;
                // Add the variant's specific component structure here
                // This will be handled by child component generation
                code += `        }\n`;
            }
        });
        code += `    ]\n`;
    }

    // Now we need each variant's structure in the component
    slintComponent.variants.forEach(variant => {
        // TODO: Output each variant's complete component structure
        // This would include its Rectangle/Text/etc components and their children
    });
    
    code += `    }\n`;
    code += `}\n`;
    return code;
}