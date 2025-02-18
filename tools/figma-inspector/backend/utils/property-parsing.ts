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

export const indentation = "  ";

type StyleObject = {
    [key: string]: string;
};

export async function getSlintSnippet(): Promise<string> {
    console.info("ID:", figma.currentPage.selection[0].id);
    generateSlintSnippet(figma.currentPage.selection[0]);
    const cssProperties = await figma.currentPage.selection[0].getCSSAsync();
    const slintProperties = transformStyle(cssProperties);

    let elementName = "Rectangle";
    const node = figma.currentPage.selection[0].type;
    if (node === "TEXT") {
        elementName = "Text";
    }

    return `${elementName} {\n${slintProperties}\n}`;
}

function transformStyle(styleObj: StyleObject): string {
    const filteredEntries = Object.entries(styleObj)
        .filter(([key]) => itemsToKeep.includes(key))
        .map(([key, value]) => {
            let finalKey = key;
            let finalValue = value;

            switch (key) {
                case "fill":
                    finalKey = "background";
                    break;
                case "stroke":
                    finalKey = "border-color";
                    break;
                case "stroke-width":
                    finalKey = "border-width";
                    break;
                case "font-family":
                    finalValue = `"${value}"`;
                    break;
            }

            if (key === "color") {
                return `  ${finalKey}: ${getColor(figma.currentPage.selection[0])};`;
            }
            if (key === "border-radius") {
                const borderRadius = getBorderRadius(
                    figma.currentPage.selection[0],
                );
                if (borderRadius !== null) {
                    return borderRadius;
                }
            }

            if (value.includes("linear-gradient")) {
                return `${indentation}${finalKey}: @${finalValue};`;
            }

            return `${indentation}${finalKey}: ${finalValue};`;
        });

    return filteredEntries.length > 0 ? `${filteredEntries.join("\n")}` : "";
}

export function rgbToHex({ r, g, b }) {
    const red = Math.round(r * 255);
    const green = Math.round(g * 255);
    const blue = Math.round(b * 255);

    return (
        "#" +
        [red, green, blue].map((x) => x.toString(16).padStart(2, "0")).join("")
    );
}

// Manually get the color for now as the CSS API returns figma variables which for now is not supported.
function getColor(node: SceneNode): string | null {
    if ("fills" in node && Array.isArray(node.fills) && node.fills.length > 0) {
        const fillColor = node.fills[0].color;
        return rgbToHex(fillColor);
    }

    return null;
}

export function getBorderRadius(node: SceneNode): string | null {
    if (!("cornerRadius" in node)) {
        return null;
    }

    const roundRadius = (value: number) => {
        return Number(value.toFixed(3));
    };

    const cornerRadius = node.cornerRadius;

    // Single values will be a number, multi border values will be a Symbol.
    if (typeof cornerRadius === "number") {
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

export function generateSlintSnippet(node: SceneNode): string | null {
    // console.time("generateSlintSnippet");
    // Determine the type of node
    const nodeType = node.type;
    console.info("Node type:", nodeType);

    switch (nodeType) {
        
        case "FRAME": {
            // Not handled. It's a type of layout node in Figma.
            break;
        }
        default: {
            console.log("Unknown node type:", nodeType);
        }
            
    }
    // console.timeEnd("generateSlintSnippet");
    return null;
}
