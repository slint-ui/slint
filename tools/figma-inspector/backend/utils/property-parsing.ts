// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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

export function getBorderRadius(node: SceneNode): string | null {
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

export function getBorderWidthAndColor(sceneNode: SceneNode): string[] | null {
    const properties: string[] = [];
    if (
        !("strokes" in sceneNode) ||
        !Array.isArray(sceneNode.strokes) ||
        sceneNode.strokes.length === 0
    ) {
        return null;
    }
    if (
        "strokeWeight" in sceneNode &&
        typeof sceneNode.strokeWeight === "number"
    ) {
        const borderWidth = roundNumber(sceneNode.strokeWeight);
        if (borderWidth) {
            properties.push(`${indentation}border-width: ${borderWidth}px;`);
        }
    }
    const brush = getBrush(sceneNode.strokes[0]);
    if (brush) {
        properties.push(`${indentation}border-color: ${brush};`);
    }
    return properties;
}

export function getBrush(fill: {
    type: string;
    opacity: number;
    color?: { r: number; g: number; b: number };
    gradientStops?: Array<{
        color: { r: number; g: number; b: number; a: number };
        position: number;
    }>;
    gradientTransform?: number[][];
}): string | null {
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

export function generateSlintSnippet(sceneNode: SceneNode): string | null {
    const nodeType = sceneNode.type;

    switch (nodeType) {
        case "FRAME": {
            return generateRectangleSnippet(sceneNode);
        }
        case "RECTANGLE": {
            return generateRectangleSnippet(sceneNode);
        }
        case "TEXT": {
            return generateTextSnippet(sceneNode);
        }
        default: {
            return generateUnsupportedNodeSnippet(sceneNode);
        }
    }
    return null;
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

export function generateRectangleSnippet(sceneNode: SceneNode): string {
    const properties: string[] = [];

    rectangleProperties.forEach((property) => {
        switch (property) {
            case "width":
                const normalizedWidth = roundNumber(sceneNode.width);
                if (normalizedWidth) {
                    properties.push(
                        `${indentation}width: ${normalizedWidth}px;`,
                    );
                }
                break;
            case "height":
                const normalizedHeight = roundNumber(sceneNode.height);
                if (normalizedHeight) {
                    properties.push(
                        `${indentation}height: ${normalizedHeight}px;`,
                    );
                }
                break;
            case "fill":
                if (
                    "fills" in sceneNode &&
                    Array.isArray(sceneNode.fills) &&
                    sceneNode.fills.length > 0
                ) {
                    const brush = getBrush(sceneNode.fills[0]);
                    if (brush) {
                        properties.push(`${indentation}background: ${brush};`);
                    }
                }
                break;
            case "opacity":
                if ("opacity" in sceneNode && sceneNode.opacity !== 1) {
                    const opacity = sceneNode.opacity;
                    properties.push(
                        `${indentation}opacity: ${Math.round(opacity * 100)}%;`,
                    );
                }
                break;
            case "border-radius":
                const borderRadius = getBorderRadius(sceneNode);
                if (borderRadius !== null) {
                    properties.push(borderRadius);
                }
                break;
            case "border-width":
                const borderWidthAndColor = getBorderWidthAndColor(sceneNode);
                if (borderWidthAndColor !== null) {
                    properties.push(...borderWidthAndColor);
                }
                break;
        }
    });

    return `Rectangle {\n${properties.join("\n")}\n}`;
}

export function generateTextSnippet(sceneNode: SceneNode): string {
    const properties: string[] = [];
    textProperties.forEach((property) => {
        switch (property) {
            case "text":
                if ("characters" in sceneNode) {
                    const characters = sceneNode.characters;
                    properties.push(`${indentation}text: "${characters}";`);
                }
                break;
            case "fill":
                if (
                    "fills" in sceneNode &&
                    Array.isArray(sceneNode.fills) &&
                    sceneNode.fills.length > 0
                ) {
                    const brush = getBrush(sceneNode.fills[0]);
                    if (brush) {
                        properties.push(`${indentation}color: ${brush};`);
                    }
                }
                break;
            case "font-family":
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
                if (
                    "fontSize" in sceneNode &&
                    typeof sceneNode.fontSize === "number"
                ) {
                    const fontSize = roundNumber(sceneNode.fontSize);
                    if (fontSize) {
                        properties.push(
                            `${indentation}font-size: ${fontSize}px;`,
                        );
                    }
                }
                break;
            case "font-weight":
                if (
                    "fontWeight" in sceneNode &&
                    typeof sceneNode.fontWeight === "number"
                ) {
                    const fontWeight = sceneNode.fontWeight;
                    properties.push(
                        `${indentation}font-weight: ${fontWeight};`,
                    );
                }
                break;
        }
    });

    return `Text {\n${properties.join("\n")}\n}`;
}
