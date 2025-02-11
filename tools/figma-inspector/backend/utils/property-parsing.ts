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

type StyleObject = {
    [key: string]: string;
};

export async function getSlintSnippet(): Promise<string> {
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
                const borderRadius = getBorderRadius();
                if (borderRadius !== null) {
                    return borderRadius;
                }
            }

            if (value.includes("linear-gradient")) {
                return `  ${finalKey}: @${finalValue};`;
            }

            return `  ${finalKey}: ${finalValue};`;
        });

    return filteredEntries.length > 0 ? `${filteredEntries.join("\n")}` : "";
}

function rgbToHex({ r, g, b }) {
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

function getBorderRadius(): string | null {
    const node = figma.currentPage.selection[0];
    console.log("node", node);

    if (!("cornerRadius" in node)) {
        return null;
    }

    const cornerRadius = node.cornerRadius;

    // Single border value
    if (typeof cornerRadius === "number") {
        return `  border-radius: ${cornerRadius}px;`;
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
        return `  ${corner.slint}: ${node[corner.prop]}px;`;
    });

    return radiusStrings.length > 0 ? radiusStrings.join("\n") : null;
}
