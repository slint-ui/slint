// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
import {
    formatStructName,
    extractHierarchy,
    sanitizePropertyName,
} from "./export-variables";

export const indentation = "    ";
const rectangleProperties = [
    "width",
    "height",
    "fill",
    "opacity",
    "border-radius",
    "border-width",
    "border-color",
];

const textProperties = [
    "x",
    "y",
    "text",
    "fill",
    "font-family",
    "font-size",
    "font-weight",
];

const unsupportedNodeProperties = ["x", "y", "width", "height", "opacity"];

export type RGBAColor = {
    r: number;
    g: number;
    b: number;
    a: number;
};

export function rgbToHex(rgba: RGBAColor): string {
    const red = Math.round(rgba.r * 255);
    const green = Math.round(rgba.g * 255);
    const blue = Math.round(rgba.b * 255);
    const alpha = Math.round(rgba.a * 255);

    const values = rgba.a < 1 ? [red, green, blue, alpha] : [red, green, blue];
    return "#" + values.map((x) => x.toString(16).padStart(2, "0")).join("");
}

export function generateRadialGradient(fill: {
    opacity: number;
    gradientStops: Array<{
        color: { r: number; g: number; b: number; a: number };
        position: number;
    }>;
    gradientTransform: number[][];
}): string {
    if (!fill.gradientStops || fill.gradientStops.length < 2) {
        return "";
    }

    const stops = fill.gradientStops
        .map((stop) => {
            const { r, g, b, a } = stop.color;
            const hexColor = rgbToHex({ r, g, b, a });
            const position = Math.round(stop.position * 100);

            return `${hexColor} ${position}%`;
        })
        .join(", ");

    return `@radial-gradient(circle, ${stops})`;
}

export function generateLinearGradient(fill: {
    opacity: number;
    gradientStops: Array<{ color: RGBAColor; position: number }>;
    gradientTransform: number[][];
}): string {
    if (!fill.gradientStops || fill.gradientStops.length < 2) {
        return "";
    }

    const [a, b] = fill.gradientTransform[0];
    const angle = (90 + Math.round(Math.atan2(b, a) * (180 / Math.PI))) % 360;

    const stops = fill.gradientStops
        .map((stop) => {
            const { r, g, b, a } = stop.color;
            const hexColor = rgbToHex({ r, g, b, a });
            const position = Math.round(stop.position * 100);

            return `${hexColor} ${position}%`;
        })
        .join(", ");

    return `@linear-gradient(${angle}deg, ${stops})`;
}

function roundNumber(value: number): number | null {
    if (value === 0) {
        return null;
    }
    return Number(value.toFixed(3));
}

export async function getBorderRadius(node: SceneNode, useVariables:boolean): Promise<string | null> {
    if ("boundVariables" in node) {
        const boundVars = (node as any).boundVariables;

        // --- Remove [0] when accessing cornerRadius ID ---
        const boundCornerRadiusId = boundVars?.cornerRadius?.id;
        if (boundCornerRadiusId && useVariables) {
            const path = await getVariablePathString(boundCornerRadiusId);
            if (path) {
                console.log(
                    `[getBorderRadius] Using variable path for cornerRadius: ${path}`,
                );
                return `${indentation}border-radius: ${path};`;
            }
            console.warn(
                `[getBorderRadius] Failed to get path for bound cornerRadius ID: ${boundCornerRadiusId}`,
            );
        }

        // --- Remove [0] when accessing individual corner IDs ---
        const cornerBindings = [
            {
                prop: "topLeftRadius",
                slint: "border-top-left-radius",
                id: boundVars?.topLeftRadius?.id,
            },
            {
                prop: "topRightRadius",
                slint: "border-top-right-radius",
                id: boundVars?.topRightRadius?.id,
            },
            {
                prop: "bottomLeftRadius",
                slint: "border-bottom-left-radius",
                id: boundVars?.bottomLeftRadius?.id,
            },
            {
                prop: "bottomRightRadius",
                slint: "border-bottom-right-radius",
                id: boundVars?.bottomRightRadius?.id,
            },
        ] as const;

        const boundIndividualCorners = cornerBindings.filter((c) => c.id);

        if (boundIndividualCorners.length > 0) {
            // --- Check if all bound corners use the SAME variable ID ---
            const allSameId = boundIndividualCorners.every(
                (c) => c.id === boundIndividualCorners[0].id,
            );

            if (allSameId && boundIndividualCorners.length === 4) {
                // All 4 corners bound to the same variable -> use shorthand border-radius
                const path = await getVariablePathString(
                    boundIndividualCorners[0].id,
                );
                if (path) {
                    console.log(
                        `[getBorderRadius] Using variable path for uniform border-radius (all corners same): ${path}`,
                    );
                    return `${indentation}border-radius: ${path};`;
                }
                console.warn(
                    `[getBorderRadius] Failed to get path for uniform bound corner ID: ${boundIndividualCorners[0].id}`,
                );
                // Fall through to numeric fallback if path fails
            } else {
                // Different variables or not all corners bound -> use individual properties
                const radiusStrings: string[] = [];
                for (const corner of boundIndividualCorners) {
                    const path = await getVariablePathString(corner.id);
                    if (path) {
                        console.log(
                            `[getBorderRadius] Using variable path for ${corner.prop}: ${path}`,
                        );
                        radiusStrings.push(
                            `${indentation}${corner.slint}: ${path};`,
                        );
                    } else {
                        console.warn(
                            `[getBorderRadius] Failed to get path for bound ${corner.prop} ID: ${corner.id}`,
                        );
                        // Fall through to numeric fallback if path fails
                    }
                }
                if (radiusStrings.length > 0) {
                    return radiusStrings.join("\n");
                }
            }
        }
    }
    // First check if node has cornerRadius property
    if (node === null || !("cornerRadius" in node) || node.cornerRadius === 0) {
        return null;
    }

    const roundRadius = (value: number) => {
        return Number(value.toFixed(3));
    };

    const cornerRadius = node.cornerRadius;

    if (typeof cornerRadius === "number") {
        return `${indentation}border-radius: ${roundRadius(cornerRadius)}px;`;
    }

    // Create type guard for corner properties
    type NodeWithCorners = {
        topLeftRadius?: number | symbol;
        topRightRadius?: number | symbol;
        bottomLeftRadius?: number | symbol;
        bottomRightRadius?: number | symbol;
    };

    // Check if node has the corner properties
    const hasCornerProperties = (
        node: SceneNode,
    ): node is SceneNode & NodeWithCorners => {
        return (
            "topLeftRadius" in node ||
            "topRightRadius" in node ||
            "bottomLeftRadius" in node ||
            "bottomRightRadius" in node
        );
    };

    if (!hasCornerProperties(node)) {
        return null;
    }

    const corners = [
        { prop: "topLeftRadius", slint: "border-top-left-radius" },
        { prop: "topRightRadius", slint: "border-top-right-radius" },
        { prop: "bottomLeftRadius", slint: "border-bottom-left-radius" },
        { prop: "bottomRightRadius", slint: "border-bottom-right-radius" },
    ] as const;

    const validCorners = corners.filter((corner) => {
        const value = node[corner.prop as keyof typeof node];
        return typeof value === "number" && value > 0;
    });

    const radiusStrings = validCorners.map((corner) => {
        const value = node[corner.prop as keyof typeof node] as number;
        return `${indentation}${corner.slint}: ${roundRadius(value)}px;`;
    });

    return radiusStrings.length > 0 ? radiusStrings.join("\n") : null;
}

export async function getBorderWidthAndColor(
    sceneNode: SceneNode,
    useVariables: boolean,
): Promise<string[] | null> {
    const properties: string[] = [];
    if (
        !("strokes" in sceneNode) ||
        !Array.isArray(sceneNode.strokes) ||
        sceneNode.strokes.length === 0
    ) {
        return null;
    }

    const firstStroke = sceneNode.strokes[0];

    // Border Width (check variable binding)
    const boundWidthVarId = firstStroke.boundVariables?.strokeWeight?.id;
    let borderWidthValue: string | null = null;

    if (boundWidthVarId && useVariables) {
        borderWidthValue = await getVariablePathString(boundWidthVarId);
    }
    // Fallback or if not bound
    if (
        !borderWidthValue &&
        "strokeWeight" in sceneNode &&
        typeof sceneNode.strokeWeight === "number"
    ) {
        const width = roundNumber(sceneNode.strokeWeight);
        if (width) {
            borderWidthValue = `${width}px`;
        }
    }
    if (borderWidthValue) {
        properties.push(`${indentation}border-width: ${borderWidthValue};`);
    }

    // Border Color (check variable binding)
    const boundColorVarId = firstStroke.boundVariables?.color?.id;
    let borderColorValue: string | null = null;

    if (boundColorVarId && useVariables) {
        borderColorValue = await getVariablePathString(boundColorVarId);
    }
    // Fallback or if not bound
    if (!borderColorValue) {
        borderColorValue = await getBrush(firstStroke); // Use existing function for resolved color
    }

    if (borderColorValue) {
        properties.push(`${indentation}border-color: ${borderColorValue};`);
    }

    return properties.length > 0 ? properties : null;
}

export async function getBrush(fill: {
    type: string;
    opacity: number;
    color?: { r: number; g: number; b: number };
    gradientStops?: Array<{
        color: { r: number; g: number; b: number; a: number };
        position: number;
    }>;
    gradientTransform?: number[][];
}): Promise<string | null> {
    switch (fill.type) {
        case "SOLID": {
            if (!fill.color) {
                console.log("Missing fill colors for solid color value");
                return "";
            }
            return rgbToHex({ ...fill.color, a: fill.opacity });
        }
        case "GRADIENT_LINEAR": {
            if (!fill.gradientStops || !fill.gradientTransform) {
                console.log("Missing gradient stops for linear gradient");
                return "";
            }
            return generateLinearGradient({
                opacity: fill.opacity,
                gradientStops: fill.gradientStops,
                gradientTransform: fill.gradientTransform,
            });
        }
        case "GRADIENT_RADIAL": {
            if (!fill.gradientStops || !fill.gradientTransform) {
                console.log("Missing gradient stops for radial gradient");
                return "";
            }
            return generateRadialGradient({
                opacity: fill.opacity,
                gradientStops: fill.gradientStops,
                gradientTransform: fill.gradientTransform,
            });
        }
        default: {
            console.log("Unknown fill type:", fill.type);
            return null;
        }
    }
}

async function getVariablePathString(
    variableId: string,
): Promise<string | null> {
    console.log(
        `[getVariablePathString] Fetching details for ID: ${variableId}`,
    );
    try {
        const variable = await figma.variables.getVariableByIdAsync(variableId);
        if (variable) {
            console.log(
                `[getVariablePathString] Fetched variable: ${variable.name}`,
            );
            const collection =
                await figma.variables.getVariableCollectionByIdAsync(
                    variable.variableCollectionId,
                );
            if (collection) {
                console.log(
                    `[getVariablePathString] Fetched collection: ${collection.name}`,
                );
                const globalName = formatStructName(collection.name); // e.g., "color"
                const pathParts = extractHierarchy(variable.name); // e.g., ["Text", "Neutral", "Default"]
                const slintPath = pathParts.map(sanitizePropertyName).join("."); // e.g., "text.neutral.default"

                // --- Adjust placement of .current ---
                let resultPath = "";
                if (collection.modes.length > 1) {
                    // Add .current after the global name
                    resultPath = `${globalName}.current.${slintPath}`;
                } else {
                    // No .current needed
                    resultPath = `${globalName}.${slintPath}`;
                }
                // --- End adjustment ---

                console.log(
                    `[getVariablePathString] Constructed path: ${resultPath}`,
                );
                return resultPath;
            } else {
                console.warn(
                    `[getVariablePathString] Collection not found for variable ID: ${variableId}`,
                );
            }
        } else {
            console.warn(
                `[getVariablePathString] Variable not found for ID: ${variableId}`,
            );
        }
    } catch (err) {
        console.error(
            `[getVariablePathString] Error fetching details for ${variableId}:`,
            err,
        );
    }
    console.log(`[getVariablePathString] Returning null for ID: ${variableId}`);
    return null;
}

export async function generateSlintSnippet(
    sceneNode: SceneNode,
    useVariables: boolean,
): Promise<string | null> {
    // Return Promise
    const nodeType = sceneNode.type;

    switch (nodeType) {
        case "FRAME":
            return await generateRectangleSnippet(sceneNode, useVariables); // Await result
        case "RECTANGLE":
        case "COMPONENT": // Add Component type
        case "INSTANCE": // Add Instance type
            return await generateRectangleSnippet(sceneNode, useVariables); // Await result
        case "TEXT":
            return await generateTextSnippet(sceneNode, useVariables); // Await result
        default:
            // Keep unsupported sync for now, or make async if needed
            return generateUnsupportedNodeSnippet(sceneNode);
    }
    // return null; // Should be unreachable if default handles all cases
}

export function generateUnsupportedNodeSnippet(sceneNode: SceneNode): string {
    const properties: string[] = [];
    const nodeType = sceneNode.type;

    unsupportedNodeProperties.forEach((property) => {
        switch (property) {
            case "x":
                if ("x" in sceneNode && typeof sceneNode.x === "number") {
                    const x = roundNumber(sceneNode.x);
                    if (x) {
                        properties.push(`${indentation}x: ${x}px;`);
                    }
                }
                break;
            case "y":
                if ("y" in sceneNode && typeof sceneNode.y === "number") {
                    const y = roundNumber(sceneNode.y);
                    if (y) {
                        properties.push(`${indentation}y: ${y}px;`);
                    }
                }
                break;
            case "width":
                if (
                    "width" in sceneNode &&
                    typeof sceneNode.width === "number"
                ) {
                    const width = roundNumber(sceneNode.width);
                    if (width) {
                        properties.push(`${indentation}width: ${width}px;`);
                    }
                }
                break;
            case "height":
                if (
                    "height" in sceneNode &&
                    typeof sceneNode.height === "number"
                ) {
                    const height = roundNumber(sceneNode.height);
                    if (height) {
                        properties.push(`${indentation}height: ${height}px;`);
                    }
                }
                break;
            case "opacity":
                if (
                    "opacity" in sceneNode &&
                    typeof sceneNode.opacity === "number"
                ) {
                    const opacity = sceneNode.opacity;
                    if (opacity !== 1) {
                        properties.push(
                            `${indentation}opacity: ${Math.round(opacity * 100)}%;`,
                        );
                    }
                }
                break;
        }
    });

    return `//Unsupported type: ${nodeType}\nRectangle {\n${properties.join("\n")}\n}`;
}

export async function generateRectangleSnippet(
    sceneNode: SceneNode,
    useVariables: boolean,
): Promise<string> {
    const properties: string[] = [];
    if ("boundVariables" in sceneNode) {
        console.log(
            "[generateRectangleSnippet] Inspecting sceneNode.boundVariables:",
            JSON.stringify((sceneNode as any).boundVariables, null, 2),
        );
    } else {
        console.log(
            "[generateRectangleSnippet] sceneNode has no boundVariables property.",
        );
    }
    for (const property of rectangleProperties) {
        // --- Add try...catch around each property's logic ---
        try {
            switch (property) {
                case "width":
                    const boundWidthVarId = (sceneNode as any).boundVariables
                        ?.width?.id;
                    let widthValue: string | null = null;
                    if (boundWidthVarId && useVariables) {
                        widthValue =
                            await getVariablePathString(boundWidthVarId);
                    }
                    if (!widthValue && "width" in sceneNode) {
                        const normalizedWidth = roundNumber(sceneNode.width);
                        if (normalizedWidth) {
                            widthValue = `${normalizedWidth}px`;
                        }
                    }
                    if (widthValue) {
                        properties.push(`${indentation}width: ${widthValue};`);
                    }
                    break;
                case "height":
                    const boundHeightVarId = (sceneNode as any).boundVariables
                        ?.height?.id;
                    let heightValue: string | null = null;
                    if (boundHeightVarId  && useVariables) {
                        heightValue =
                            await getVariablePathString(boundHeightVarId);
                    }
                    if (!heightValue && "height" in sceneNode) {
                        const normalizedHeight = roundNumber(sceneNode.height);
                        if (normalizedHeight) {
                            heightValue = `${normalizedHeight}px`;
                        }
                    }
                    if (heightValue) {
                        properties.push(
                            `${indentation}height: ${heightValue};`,
                        );
                    }
                    break;
                case "fill":
                    if (
                        "fills" in sceneNode &&
                        Array.isArray(sceneNode.fills) &&
                        sceneNode.fills.length > 0
                    ) {
                        const firstFill = sceneNode.fills[0];
                        if (firstFill.type === "SOLID") {
                            const boundVarId =
                                firstFill.boundVariables?.color?.id;
                            let fillValue: string | null = null;
                            if (boundVarId && useVariables) {
                                fillValue =
                                    await getVariablePathString(boundVarId);
                            }
                            if (!fillValue) {
                                fillValue = await getBrush(firstFill);
                            }
                            if (fillValue) {
                                properties.push(
                                    `${indentation}background: ${fillValue};`,
                                );
                            }
                        } else {
                            const brush = await getBrush(firstFill);
                            if (brush) {
                                properties.push(
                                    `${indentation}background: ${brush};`,
                                );
                            }
                        }
                    }
                    break;
                case "opacity":
                    if ("opacity" in sceneNode && sceneNode.opacity !== 1) {
                        properties.push(
                            `${indentation}opacity: ${Math.round(sceneNode.opacity * 100)}%;`,
                        );
                    }
                    break;
                case "border-radius":
                    // --- Ensure this uses await and the new async getBorderRadius ---
                    const borderRadiusProp = await getBorderRadius(sceneNode, useVariables); // Use await
                    if (borderRadiusProp !== null) {
                        properties.push(borderRadiusProp);
                        // Use new log message
                        console.log(
                            `[generateRectangleSnippet] Added border-radius property: ${borderRadiusProp.includes("\n") ? "\n" + borderRadiusProp : borderRadiusProp}`,
                        );
                    } else {
                        console.log(
                            "[generateRectangleSnippet] No border-radius property added.",
                        );
                    }
                    break; // --- End border-radius case ---

                case "border-width": // Handled below
                    break;
                case "border-color":
                    const borderWidthAndColor =
                        await getBorderWidthAndColor(sceneNode, useVariables);
                    if (borderWidthAndColor !== null) {
                        properties.push(...borderWidthAndColor);
                    }
                    break;
            }
        } catch (err) {
            // --- Log error specific to this property ---
            console.error(
                `[generateRectangleSnippet] Error processing property "${property}":`,
                err,
            );
            // Optionally add a comment to the snippet indicating the error
            properties.push(
                `${indentation}// Error processing ${property}: ${err instanceof Error ? err.message : err}`,
            );
        }
        // --- End try...catch ---
    }

    return `Rectangle {\n${properties.join("\n")}\n}`;
}
export async function generateTextSnippet(
    sceneNode: SceneNode,
    useVariables: boolean,
): Promise<string> {
    const properties: string[] = [];
    if ("boundVariables" in sceneNode) {
        console.log(
            "[generateTextSnippet] Inspecting sceneNode.boundVariables:",
            JSON.stringify((sceneNode as any).boundVariables, null, 2),
        );
    } else {
        console.log(
            "[generateTextSnippet] sceneNode has no boundVariables property.",
        );
    }

    for (const property of textProperties) {
        try {
            switch (property) {
                // --- Add case for x ---
                case "x":
                    const boundXVarId = (sceneNode as any).boundVariables?.x
                        ?.id; // Assume direct object binding
                    let xValue: string | null = null;
                    if (boundXVarId && useVariables) {
                        xValue = await getVariablePathString(boundXVarId);
                        console.log(
                            `[generateTextSnippet] x: Using variable path: ${xValue}`,
                        );
                    }
                    if (
                        !xValue &&
                        "x" in sceneNode &&
                        typeof sceneNode.x === "number"
                    ) {
                        const x = roundNumber(sceneNode.x);
                        if (x !== null) {
                            // roundNumber returns null for 0
                            xValue = `${x}px`;
                            console.log(
                                `[generateTextSnippet] x: Using numeric value: ${xValue}`,
                            );
                        }
                    }
                    if (xValue) {
                        properties.push(`${indentation}x: ${xValue};`);
                    }
                    break;
                // --- Add case for y ---
                case "y":
                    const boundYVarId = (sceneNode as any).boundVariables?.y
                        ?.id; // Assume direct object binding
                    let yValue: string | null = null;
                    if (boundYVarId && useVariables) {
                        yValue = await getVariablePathString(boundYVarId);
                        console.log(
                            `[generateTextSnippet] y: Using variable path: ${yValue}`,
                        );
                    }
                    if (
                        !yValue &&
                        "y" in sceneNode &&
                        typeof sceneNode.y === "number"
                    ) {
                        const y = roundNumber(sceneNode.y);
                        if (y !== null) {
                            // roundNumber returns null for 0
                            yValue = `${y}px`;
                            console.log(
                                `[generateTextSnippet] y: Using numeric value: ${yValue}`,
                            );
                        }
                    }
                    if (yValue) {
                        properties.push(`${indentation}y: ${yValue};`);
                    }
                    break;
                case "text":
                    // Assuming 'characters' binding is also an array if it exists
                    const boundCharsVarId = (sceneNode as any).boundVariables
                        ?.characters?.[0]?.id;
                    let textValue: string | null = null;
                    if (boundCharsVarId  && useVariables) {
                        textValue =
                            await getVariablePathString(boundCharsVarId);
                    }
                    if (!textValue && "characters" in sceneNode) {
                        textValue = `"${sceneNode.characters}"`;
                    }
                    if (textValue) {
                        properties.push(`${indentation}text: ${textValue};`);
                    }
                    break;
                case "fill":
                    if (
                        "fills" in sceneNode &&
                        Array.isArray(sceneNode.fills) &&
                        sceneNode.fills.length > 0
                    ) {
                        const firstFill = sceneNode.fills[0];
                        if (firstFill.type === "SOLID") {
                            // Access ID via array index [0]
                            const boundVarId = (sceneNode as any).boundVariables
                                ?.fills?.[0]?.id;
                            let fillValue: string | null = null;
                            if (boundVarId && useVariables) {
                                fillValue =
                                    await getVariablePathString(boundVarId);
                            }
                            if (!fillValue) {
                                fillValue = await getBrush(firstFill);
                            }
                            if (fillValue) {
                                properties.push(
                                    `${indentation}color: ${fillValue};`,
                                );
                            }
                        } else {
                            const brush = await getBrush(firstFill);
                            if (brush) {
                                properties.push(
                                    `${indentation}color: ${brush};`,
                                );
                            }
                        }
                    }
                    break;
                case "font-family":
                    // Keep using resolved family name. Variable structure for FontName is complex.
                    if ("fontName" in sceneNode) {
                        const fontName = sceneNode.fontName;
                        if (typeof fontName !== "symbol" && fontName) {
                            properties.push(
                                `${indentation}font-family: "${fontName.family}";`,
                            );
                        }
                    }
                    break;
                case "font-size":
                    // --- Access ID via array index [0] ---
                    const boundSizeVarId = (sceneNode as any).boundVariables
                        ?.fontSize?.[0]?.id;
                    console.log(
                        `[generateTextSnippet] font-size: Found bound variable ID? ${boundSizeVarId ?? "No"}`,
                    );
                    let sizeValue: string | null = null;
                    if (boundSizeVarId && useVariables) {
                        sizeValue = await getVariablePathString(boundSizeVarId);
                        console.log(
                            `[generateTextSnippet] font-size: getVariablePathString returned: ${sizeValue ?? "null"}`,
                        );
                    }
                    if (
                        !sizeValue &&
                        "fontSize" in sceneNode &&
                        typeof sceneNode.fontSize === "number"
                    ) {
                        const fontSize = roundNumber(sceneNode.fontSize);
                        if (fontSize) {
                            sizeValue = `${fontSize}px`;
                            console.log(
                                `[generateTextSnippet] font-size: Using fallback value: ${sizeValue}`,
                            );
                        }
                    }
                    if (sizeValue) {
                        properties.push(
                            `${indentation}font-size: ${sizeValue};`,
                        );
                        console.log(
                            `[generateTextSnippet] font-size: Added property: ${sizeValue}`,
                        );
                    }
                    break;
                case "font-weight":
                    const boundWeightVarId = (sceneNode as any).boundVariables
                        ?.fontWeight?.[0]?.id; // Still use [0] based on Text node structure
                    console.log(
                        `[generateTextSnippet] font-weight: Found bound variable ID? ${boundWeightVarId ?? "No"}`,
                    );
                    let weightValue: string | number | null = null;
                    let isVariable = false; // Flag to track if value is from a variable

                    if (boundWeightVarId && useVariables) {
                        const path =
                            await getVariablePathString(boundWeightVarId);
                        if (path) {
                            weightValue = path;
                            isVariable = true; // Set flag
                            console.log(
                                `[generateTextSnippet] font-weight: getVariablePathString returned: ${weightValue}`,
                            );
                        } else {
                            console.warn(
                                `[generateTextSnippet] font-weight: getVariablePathString returned null for ID ${boundWeightVarId}`,
                            );
                        }
                    }

                    // Fallback if not bound or variable path failed
                    if (
                        weightValue === null && // Use strict null check
                        "fontWeight" in sceneNode &&
                        typeof sceneNode.fontWeight === "number"
                    ) {
                        weightValue = sceneNode.fontWeight;
                        console.log(
                            `[generateTextSnippet] font-weight: Using fallback value: ${weightValue}`,
                        );
                    }

                    if (weightValue !== null) {
                        // --- Append '/ 1px' if it's a variable path (string) ---
                        const finalWeightValue = isVariable
                            ? `${weightValue} / 1px`
                            : weightValue;
                        // --- End modification ---

                        properties.push(
                            `${indentation}font-weight: ${finalWeightValue};`,
                        );
                        console.log(
                            `[generateTextSnippet] font-weight: Added property: ${finalWeightValue}`,
                        );
                    }
                    break;
            }
        } catch (err) {
            console.error(
                `[generateTextSnippet] Error processing property "${property}":`,
                err,
            );
            properties.push(
                `${indentation}// Error processing ${property}: ${err instanceof Error ? err.message : err}`,
            );
        }
    }

    console.log(
        `[generateTextSnippet] Finished processing properties for node. Snippet content:\nText {\n${properties.join("\n")}\n}`,
    );
    return `Text {\n${properties.join("\n")}\n}`;
}
