// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { getBorderRadius, rgbToHex, indentation } from "./property-parsing";
import { expect, test } from "vitest";

const testJson = require("../../figma_output.json");

// Json node ID for various tests.
const testBorderRadius55px = "163:266";
const testBorderRadiusMultiValue = "163:267";

function findNodeById(obj: any, targetId: string): any {
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

// The JSON in the file for border radius is different to the API ojbect
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

test("converts rgb to hex #ffffff", () => {
    const color = rgbToHex({ r: 1, g: 1, b: 1 });
    expect(color).toBe("#ffffff");
});

test("converts rgb to hex floating #ffffff", () => {
    const color = rgbToHex({ r: 1.0, g: 1.0, b: 1.0 });
    expect(color).toBe("#ffffff");
});

test("converts rgb to hex #000000", () => {
    const color = rgbToHex({ r: 0, g: 0, b: 0 });
    expect(color).toBe("#000000");
});

test("converts rgb to hex floating #000000", () => {
    const color = rgbToHex({ r: 0.0, g: 0.0, b: 0.0 });
    expect(color).toBe("#000000");
});

test("Single border radius", () => {
    const jsonNode = findNodeById(testJson, testBorderRadius55px);
    const snippet = getBorderRadius(jsonNode);
    expect(snippet).toBe(`${indentation}border-radius: 55px;`);
});

test("Multiple border radius", () => {
    const jsonNode = findNodeById(testJson, testBorderRadiusMultiValue);
    const convertToApiJson = processCornerRadii(jsonNode);
    const snippet = getBorderRadius(convertToApiJson);
    const expectedSnippet = `${indentation}border-top-left-radius: 50px;\n${indentation}border-top-right-radius: 28px;\n${indentation}border-bottom-right-radius: 30.343290328979492px;`;
    expect(snippet).toBe(expectedSnippet);
});
