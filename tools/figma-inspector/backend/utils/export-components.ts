interface ComponentPropertyDefinitions {
    type: ComponentPropertyType;
    defaultValue: string | boolean;
    variantOptions?: string[];
    preferredValues?: InstanceSwapPreferredValue[];
    boundVariables?: { [key: string]: any };
}

interface VariantInfo {
    name: string;
    id: string;
    variantProperties: {
        [key: string]: ComponentPropertyDefinitions;  
    };
    variants: Array<{
        name: string;
        id: string;
        propertyValues: { [key: string]: string | boolean | number };
    }>;
}

interface SlintComponent {
    componentName: string;
    enums: { [key: string]: string[] };
    variants: Array<{
        name: string;
        properties: { [key: string]: string };
    }>;
}
const usedNames = new Map<string, number>();  

export function exportComponentSet(): string | null {
    async function processFigmaFile() {
        const componentSets: VariantInfo[] = [];


        function getComponentSetInfo(node: ComponentSetNode): VariantInfo {
            // Parse variants from component names like "Style=Normal, State=Enabled, Type=Regular"
            const variants = node.children.map(variant => {
                const variantProps = variant.name.split(", ").reduce((acc, part) => {
                    const [key, value] = part.split("=");
                    acc[key] = value;
                    return acc;
                }, {} as { [key: string]: string });
        
                console.log("Parsed variant:", variant.name, "->", variantProps);
                
                return {
                    name: variant.name,
                    id: variant.id,
                    propertyValues: variantProps
                };
            });
        
            const variantInfo: VariantInfo = {
                name: node.name,
                id: node.id,
                variantProperties: node.componentPropertyDefinitions || {},
                variants: variants
            };
            
            console.log("Final VariantInfo:", JSON.stringify(variantInfo, null, 2));
            return variantInfo;
        }

        function traverse(node: SceneNode) {
            if (node.type === "COMPONENT_SET") {
                componentSets.push(getComponentSetInfo(node));
            }

            if ("children" in node) {
                for (const child of node.children) {
                    traverse(child);
                }
            }
        }

        const selection = figma.currentPage.selection;
        if (selection.length > 0) {
            selection.forEach(traverse);
            const slintComponents = componentSets.map(convertToSlintFormat);
            const slintCode = slintComponents.map(generateSlintCode).join('\n');
            console.log("Generated Slint Code:", slintCode);
            return slintCode;
        } else {
            console.log("Please select a component set to export");
            return null;
        }
    }

    processFigmaFile();
    return null;
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
    // Sanitize the component name
    const componentName = sanitizeIdentifier(componentSet.name, 'component');
    
    // Extract enums from variantProperties with namespaced names
    const enums: { [key: string]: string[] } = {};
    for (const [key, value] of Object.entries(componentSet.variantProperties)) {
        if (value.type === "VARIANT" && value.variantOptions) {
            // Create namespaced enum name: ComponentName_VariantName
            const enumName = `${componentName}_${toUpperCamelCase(key)}`;
            enums[enumName] = value.variantOptions;
        }
    }

    return {
        componentName,
        enums,
        variants: componentSet.variants.map(v => ({
            name: v.name,
            properties: Object.fromEntries(
                Object.entries(v.propertyValues).map(([key, value]) => [
                    toLowerDashed(key),
                    String(value)
                ])
            )
        }))
    };
}

function generateSlintCode(slintComponent: SlintComponent): string {
    let code = '';
    
    // Generate enums before component
    for (const [enumName, values] of Object.entries(slintComponent.enums)) {
        code += `export enum ${enumName} {\n`;
        values.forEach(value => {
            code += `    ${value},\n`;
        });
        code += `}\n\n`;
    }
    
    // Generate component
    code += `export component ${slintComponent.componentName} {\n`;
    
    // Add properties for each enum using the namespaced enum type
    Object.keys(slintComponent.enums).forEach(enumName => {
        const propertyName = toLowerDashed(enumName.split('_')[1]); // Get the property name part
        code += `    in property <${enumName}> ${propertyName};\n`;
    });

    code += '}\n';
    
    return code;
}