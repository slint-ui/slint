// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test, describe, it } from "vitest";
import {
    sanitizeSlintPropertyName,
    generateVariableValue,
    indent2,
} from "../backend/utils/experimental-export";

// sanitizeSlintPropertyName tests
test("keeps valid property name starting with letter", () => {
    const name = "validName";
    const result = sanitizeSlintPropertyName(name);
    expect(result).toBe(name);
});

test("keeps valid property name starting with underscore", () => {
    const name = "_validName";
    const result = sanitizeSlintPropertyName(name);
    expect(result).toBe(name);
});

test("adds underscore to name starting with number", () => {
    const result = sanitizeSlintPropertyName("123invalid");
    expect(result).toBe("_123invalid");
});

test("adds underscore to name starting with special character", () => {
    const result = sanitizeSlintPropertyName("@invalid");
    expect(result).toBe("invalid");
});

test("removes spaces from property name", () => {
    const result = sanitizeSlintPropertyName("my property name");
    expect(result).toBe("mypropertyname");
});

test("preserves hyphens in property name", () => {
    const result = sanitizeSlintPropertyName("my-property-name");
    expect(result).toBe("my-property-name");
});

test("removes em dashes and other special characters", () => {
    const result = sanitizeSlintPropertyName("my—property—name");
    expect(result).toBe("mypropertyname");
});

test("removes non-ASCII characters", () => {
    const result = sanitizeSlintPropertyName("my-π-property");
    expect(result).toBe("my--property");
});

test("handles multiple consecutive hyphens", () => {
    const result = sanitizeSlintPropertyName("my--property--name");
    expect(result).toBe("my--property--name");
});

test("handles mixed valid and invalid characters", () => {
    const result = sanitizeSlintPropertyName("my@#$%property&*()name");
    expect(result).toBe("mypropertyname");
});

test("handles property name with numbers in middle", () => {
    const result = sanitizeSlintPropertyName("property123name");
    expect(result).toBe("property123name");
});

test("handles property name with underscores in middle", () => {
    const result = sanitizeSlintPropertyName("property_name");
    expect(result).toBe("property_name");
});

test("handles property name with hyphens and underscores", () => {
    const result = sanitizeSlintPropertyName("property-name_with-mixed");
    expect(result).toBe("property-name_with-mixed");
});

test("handles property name starting with hyphen", () => {
    const result = sanitizeSlintPropertyName("-foo-bar");
    expect(result).toBe("_foo-bar");
});

test("handles empty string", () => {
    const result = sanitizeSlintPropertyName("");
    expect(result).toBe("_");
});

test("handles string with only invalid characters", () => {
    const result = sanitizeSlintPropertyName("@#$%^&*()");
    expect(result).toBe("_");
});

test("trims spaces from start and end", () => {
    const result = sanitizeSlintPropertyName("  my property name  ");
    expect(result).toBe("mypropertyname");
});

test("converts forward slashes to hyphens", () => {
    const result = sanitizeSlintPropertyName("my/property/name");
    expect(result).toBe("my-property-name");
});

test("removes consecutive duplicate words", () => {
    expect(sanitizeSlintPropertyName("text-text-foo")).toBe("text-foo");
    expect(sanitizeSlintPropertyName("surface-surface-primary")).toBe(
        "surface-primary",
    );
    expect(sanitizeSlintPropertyName("color-color-background")).toBe(
        "color-background",
    );
});

test("keeps non-consecutive duplicate words", () => {
    expect(sanitizeSlintPropertyName("text-foo-text")).toBe("text-foo-text");
    expect(sanitizeSlintPropertyName("surface-primary-surface")).toBe(
        "surface-primary-surface",
    );
    expect(sanitizeSlintPropertyName("color-background-color")).toBe(
        "color-background-color",
    );
});

test("handles multiple consecutive duplicates", () => {
    expect(sanitizeSlintPropertyName("text-text-text-foo")).toBe("text-foo");
    expect(sanitizeSlintPropertyName("surface-surface-surface-primary")).toBe(
        "surface-primary",
    );
});

// generateVariableValue tests
describe("generateVariableValue", () => {
    it("should round float values to one decimal place", () => {
        const variable = {
            name: "test-float",
            resolvedType: "FLOAT",
            scopes: ["OPACITY"],
        } as any;
        const variableRefMap = new Map<string, { path: string; variable: any }>();
        const collectionName = "test-collection";
        const sanitizedCollection = { variables: [] } as any;

        expect(generateVariableValue(variable, 0.89099, collectionName, sanitizedCollection, variableRefMap)).toBe(
            `${indent2}test-float: 0.9,\n`,
        );
        expect(generateVariableValue(variable, 1.003, collectionName, sanitizedCollection, variableRefMap)).toBe(
            `${indent2}test-float: 1.0,\n`,
        );
        expect(generateVariableValue(variable, 2.567, collectionName, sanitizedCollection, variableRefMap)).toBe(
            `${indent2}test-float: 2.6,\n`,
        );
        expect(generateVariableValue(variable, 3.0, collectionName, sanitizedCollection, variableRefMap)).toBe(
            `${indent2}test-float: 3.0,\n`,
        );
    });

    it("should handle other types correctly", () => {
        const variableRefMap = new Map<string, { path: string; variable: any }>();
        const collectionName = "test-collection";
        const sanitizedCollection = { variables: [] } as any;

        // Test string
        const stringVar = {
            name: "test-string",
            resolvedType: "STRING",
        } as any;
        expect(generateVariableValue(stringVar, "hello", collectionName, sanitizedCollection, variableRefMap)).toBe(
            `${indent2}test-string: "hello",\n`,
        );

        // Test boolean
        const boolVar = {
            name: "test-bool",
            resolvedType: "BOOLEAN",
        } as any;
        expect(generateVariableValue(boolVar, true, collectionName, sanitizedCollection, variableRefMap)).toBe(
            `${indent2}test-bool: true,\n`,
        );

        // Test length
        const lengthVar = {
            name: "test-length",
            resolvedType: "FLOAT",
            scopes: ["ALL_SCOPES"],
        } as any;
        expect(generateVariableValue(lengthVar, 42, collectionName, sanitizedCollection, variableRefMap)).toBe(
            `${indent2}test-length: 42px,\n`,
        );

        // Test brush
        const brushVar = {
            name: "test-brush",
            resolvedType: "COLOR",
        } as any;
        expect(
            generateVariableValue(brushVar, "invalid-data", collectionName, sanitizedCollection, variableRefMap),
        ).toBe("// unable to convert test-brush to brush,\n");

        // Test RGB object conversion
        expect(
            generateVariableValue(
                brushVar,
                { r: 1, g: 0, b: 0, a: 1 },
                collectionName,
                sanitizedCollection,
                variableRefMap,
            ),
        ).toBe(`${indent2}test-brush: #ff0000,\n`);
        expect(
            generateVariableValue(
                brushVar,
                { r: 0, g: 1, b: 0, a: 1 },
                collectionName,
                sanitizedCollection,
                variableRefMap,
            ),
        ).toBe(`${indent2}test-brush: #00ff00,\n`);
        expect(
            generateVariableValue(
                brushVar,
                { r: 0, g: 0, b: 1, a: 1 },
                collectionName,
                sanitizedCollection,
                variableRefMap,
            ),
        ).toBe(`${indent2}test-brush: #0000ff,\n`);
        expect(
            generateVariableValue(
                brushVar,
                { r: 0.5, g: 0.5, b: 0.5, a: 1 },
                collectionName,
                sanitizedCollection,
                variableRefMap,
            ),
        ).toBe(`${indent2}test-brush: #808080,\n`);
    });

    it("should handle variable aliases", () => {
        const variable = {
            name: "test-var",
            resolvedType: "COLOR",
        } as any;
        const variableRefMap = new Map<string, { path: string; variable: any }>([
            ["var-id-1", { path: "Colors.collection.primary", variable: { name: "primary", resolvedType: "COLOR" } }],
            ["var-id-2", { path: "Colors.collection.secondary", variable: { name: "secondary", resolvedType: "COLOR" } }],
        ]);
        const collectionName = "test-collection";
        const sanitizedCollection = { variables: [] } as any;

        // Test direct reference
        expect(
            generateVariableValue(
                variable,
                { type: "VARIABLE_ALIAS", id: "var-id-1" },
                collectionName,
                sanitizedCollection,
                variableRefMap,
            ),
        ).toBe(`${indent2}test-var: Colors.collection.primary,\n`);

        // Test reference to non-existent variable
        expect(
            generateVariableValue(
                variable,
                { type: "VARIABLE_ALIAS", id: "non-existent" },
                collectionName,
                sanitizedCollection,
                variableRefMap,
            ),
        ).toBe(
            `// Figma file is pointing at a deleted Variable "test-var"\n${indent2}test-var: #000000,\n`,
        );
    });
});
