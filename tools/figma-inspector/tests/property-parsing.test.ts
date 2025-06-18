// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import {
    getBorderRadius,
    rgbToHex,
    indentation,
    generatePathNodeSnippet,
    getBorderWidthAndColor,
    generateTextSnippet,
} from "../backend/utils/property-parsing";
import { expect, test } from "vitest";

const testJson = require("./figma_output.json");

export function findNodeById(obj: any, targetId: string): any {
    if (Array.isArray(obj)) {
        for (const item of obj) {
            const result = findNodeById(item, targetId);
            if (result) {
                return result;
            }
        }
    } else if (typeof obj === "object" && obj !== null) {
        if (obj.id === targetId) {
            return obj;
        }

        for (const key in obj) {
            const result = findNodeById(obj[key], targetId);
            if (result) {
                return result;
            }
        }
    }
    return null;
}

function findNodeByName(obj: any, targetName: string): any {
    if (Array.isArray(obj)) {
        for (const item of obj) {
            const result = findNodeByName(item, targetName);
            if (result) {
                return result;
            }
        }
    } else if (typeof obj === "object" && obj !== null) {
        if (obj.name === targetName) {
            return obj;
        }

        for (const key in obj) {
            const result = findNodeByName(obj[key], targetName);
            if (result) {
                return result;
            }
        }
    }
    return null;
}

// The JSON in the file for border radius is different to the API object
// the runtime plugin uses. This converts the test JSON to match the API object.
// This isn't a great soloution, but test options are limited for Figma right now.
function processCornerRadii(json: any): any {
    if (json.rectangleCornerRadii && Array.isArray(json.rectangleCornerRadii)) {
        const [
            topLeftRadius,
            topRightRadius,
            bottomRightRadius,
            bottomLeftRadius,
        ] = json.rectangleCornerRadii;

        return {
            ...json,
            cornerRadius: Symbol(),
            topLeftRadius,
            topRightRadius,
            bottomRightRadius,
            bottomLeftRadius,
        };
    }

    return json;
}
function processVectorNode(json: any): any {
    // You can expand this as needed for your test expectations
    if (json.type === "VECTOR") {
        return {
            ...json,
            vectorPaths: json.vectorPaths || [],
            strokes: json.strokes || [],
            strokeWeight: json.strokeWeight ?? 1,
            exportAsync: async () => `<svg><path d="M10 10L90 90"/></svg>`,
        };
    }
    return json;
}
// Convert test JSON to match the API object.
function processTextNode(json: any): any {
    if (json.type === "TEXT" && json.style) {
        return {
            ...json,
            characters: json.characters,
            fontName: {
                family: json.style.fontFamily,
                style: json.style.fontStyle,
            },
            fontSize: json.style.fontSize,
            fontWeight: json.style.fontWeight,
        };
    }
    return json;
}

test("converts rgb to hex #ffffff", () => {
    const color = rgbToHex({ r: 1, g: 1, b: 1, a: 1 });
    expect(color).toBe("#ffffff");
});

test("converts rgb to hex floating #ffffff", () => {
    const color = rgbToHex({ r: 1.0, g: 1.0, b: 1.0, a: 1.0 });
    expect(color).toBe("#ffffff");
});

test("converts rgb to hex #000000", () => {
    const color = rgbToHex({ r: 0, g: 0, b: 0, a: 1 });
    expect(color).toBe("#000000");
});

test("converts rgb to hex floating #000000", () => {
    const color = rgbToHex({ r: 0.0, g: 0.0, b: 0.0, a: 1.0 });
    expect(color).toBe("#000000");
});

test(" No border radius", async () => {
    const jsonNode = findNodeByName(
        testJson,
        "rectangle no corner radius test",
    );
    expect(jsonNode).not.toBeNull();
    const snippet = await getBorderRadius(jsonNode, false);
    expect(snippet).toBe(null);
});

test("Single border radius", async () => {
    const jsonNode = findNodeByName(testJson, "border-test 1");
    expect(jsonNode).not.toBeNull();
    const snippet = await getBorderRadius(jsonNode, false);
    expect(snippet).toBe(`${indentation}border-radius: 55px;`);
});

test("Multiple border radius", async () => {
    const jsonNode = findNodeByName(testJson, "border-test 2");
    expect(jsonNode).not.toBeNull();
    const convertToApiJson = processCornerRadii(jsonNode);
    const snippet = await getBorderRadius(convertToApiJson, false);
    const expectedSnippet = `${indentation}border-top-left-radius: 50px;\n${indentation}border-top-right-radius: 28px;\n${indentation}border-bottom-right-radius: 30.343px;`;
    expect(snippet).toBe(expectedSnippet);
});

test("Border width and color", async () => {
    const jsonNode = findNodeByName(testJson, "stroke test 2");
    expect(jsonNode).not.toBeNull();
    const snippet = await getBorderWidthAndColor(jsonNode, false);
    const expectedSnippet = [
        `${indentation}border-width: 10.455px;`,
        `${indentation}border-color: #5c53dc;`,
    ];
    expect(snippet).toStrictEqual(expectedSnippet);
});

test("Text node", async () => {
    const jsonNode = findNodeByName(testJson, "Monthly");
    expect(jsonNode).not.toBeNull();
    const convertToApiJson = processTextNode(jsonNode);
    const snippet = await generateTextSnippet(convertToApiJson, false);
    const expectedSnippet = `monthly := Text {\n${indentation}text: "Monthly";\n${indentation}color: #896fff;\n${indentation}font-family: "Roboto";\n${indentation}font-size: 12px;\n${indentation}font-weight: 400;\n}`;
    expect(snippet).toBe(expectedSnippet);
});

test("Vector node", async () => {
    const jsonNode = findNodeByName(testJson, "vector test");
    expect(jsonNode).not.toBeNull();
    const convertToApiJson = processVectorNode(jsonNode);
    const snippet = await generatePathNodeSnippet(convertToApiJson, false);
    const expectedSnippet = `vector-test := Path {
${indentation}commands: "M10 10L90 90";
${indentation}fill: #2e5adf;
${indentation}stroke: #000000;
${indentation}stroke-width: 2.5px;
}`;
    expect(snippet).toBe(expectedSnippet);
});
