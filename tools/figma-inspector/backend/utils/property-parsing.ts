// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

const itemsToKeep = [
    "color",
    "font-family",
    "font-size",
    "font-weight",
    "width",
    "height",
    "fill",
    "opacity",
    "border-radius",
    "stroke-width",
    "stroke",
];

export const indentation = "    ";

export function rgbToHex(fill: {
    opacity: number;
    color: { r: number; g: number; b: number };
}): string {
    const {
        color: { r, g, b },
    } = fill;

    const red = Math.round(r * 255);
    const green = Math.round(g * 255);
    const blue = Math.round(b * 255);

    return (
        "#" +
        [red, green, blue].map((x) => x.toString(16).padStart(2, "0")).join("")
    );
}


function roundNumber(value: number): number | null {
    if (value === 0) {
        return null;
    }
    return Number(value.toFixed(3));
};

export function getBorderRadius(node: SceneNode): string | null {
    if (node === null || !("cornerRadius" in node) || node.cornerRadius === 0) {
        return null;
    }

    const roundRadius = (value: number) => {
        return Number(value.toFixed(3));
    };

    const cornerRadius = node.cornerRadius;

    if (typeof cornerRadius === "number") {
        // Single values will be a number, multi border values will be a Symbol.
        return `${indentation}border-radius: ${roundRadius(cornerRadius)}px;`;
    }

    // Multiple border values
    const corners = [
        { prop: "topLeftRadius", slint: "border-top-left-radius" },
        { prop: "topRightRadius", slint: "border-top-right-radius" },
        { prop: "bottomLeftRadius", slint: "border-bottom-left-radius" },
        { prop: "bottomRightRadius", slint: "border-bottom-right-radius" },
    ];

    const validCorners = corners.filter(
        (corner) =>
            corner.prop in node &&
            typeof node[corner.prop] === "number" &&
            node[corner.prop] > 0,
    );

    const radiusStrings = validCorners.map((corner, index) => {
        return `${indentation}${corner.slint}: ${roundRadius(node[corner.prop])}px;`;
    });

    return radiusStrings.length > 0 ? radiusStrings.join("\n") : null;
}

export function generateSlintSnippet(sceneNode: SceneNode): string | null {
    console.log("node ID:", sceneNode.id);
    const nodeType = sceneNode.type;

    switch (nodeType) {
        case "FRAME": {
            // Not handled. It's a type of layout node in Figma.
            break;
        }
        case "RECTANGLE": {
            return generateRectangleSnippet(sceneNode);
        }
        default: {
            console.log("Unknown node type:", nodeType);
        }
    }
    return null;
}

const rectangleProperties = [
    "width",
    "height",
    "fill",
    "opacity",
    "border-radius",
    // "stroke-width",
    // "stroke",
];

export function generateRectangleSnippet(sceneNode: SceneNode): string {
    const properties: string[] = [];

    rectangleProperties.forEach((property) => {
        switch (property) {
            case "width":
                const normalizedWidth = roundNumber(sceneNode.width);
                if (normalizedWidth) {
                    properties.push(`${indentation}width: ${sceneNode.width}px;`);
                }
                break;
            case "height":
                const normalizedHeight = roundNumber(sceneNode.height);
                if (normalizedHeight) {
                    properties.push(`${indentation}height: ${sceneNode.height}px;`);
                }
                break;
            case "fill":
                if (
                    "fills" in sceneNode &&
                    Array.isArray(sceneNode.fills) &&
                    sceneNode.fills.length > 0
                ) {
                    const hexColor = rgbToHex(sceneNode.fills[0]);
                    properties.push(`${indentation}background: ${hexColor};`);

                }
                break;
            case "opacity":
                if ("opacity" in sceneNode && sceneNode.opacity !== 1) {
                    const opacity = sceneNode.opacity;
                    properties.push(`${indentation}opacity: ${opacity * 100}%;`);
                }
                break;
            case "border-radius":
                const borderRadius = getBorderRadius(sceneNode);
                if (borderRadius !== null) {
                    properties.push(borderRadius);
                }
                break;
        }
    });

    return `Rectangle {\n${properties.join("\n")}\n}`;
}
