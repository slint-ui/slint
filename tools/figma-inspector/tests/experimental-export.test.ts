// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import {
    sanitizeSlintPropertyName,
    generateVariableValue,
    indent2,
} from "../backend/utils/experimental-export";
import type {
    CollectionId,
    VariableCollectionSU,
    VariableId,
    VariableSU,
} from "../shared/custom-figma-types.js";

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

test("should round float values to one decimal place", () => {
    const variable = {
        name: "test-float",
        resolvedType: "FLOAT",
        scopes: ["OPACITY"],
    } as any;
    const variablesMap = new Map<VariableId, VariableSU>();
    const collectionName = "test-collection";
    const collectionsMap = new Map<CollectionId, VariableCollectionSU>([
        [
            "collection-1" as CollectionId,
            {
                id: "collection-1" as CollectionId,
                name: "Colors",
                modes: [],
                defaultModeId: "mode1",
                variables: new Map(),
            } as unknown as VariableCollectionSU,
        ],
    ]);

    expect(
        generateVariableValue(
            variable,
            0.89099,
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe(`${indent2}test-float: 0.9,\n`);
    expect(
        generateVariableValue(
            variable,
            1.003,
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe(`${indent2}test-float: 1.0,\n`);
    expect(
        generateVariableValue(
            variable,
            2.567,
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe(`${indent2}test-float: 2.6,\n`);
    expect(
        generateVariableValue(
            variable,
            3.0,
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe(`${indent2}test-float: 3.0,\n`);
});

test("should handle other types correctly", () => {
    const variablesMap = new Map<VariableId, VariableSU>();
    const collectionName = "test-collection";
    const collectionsMap = new Map<CollectionId, VariableCollectionSU>([
        [
            "collection-1" as CollectionId,
            {
                id: "collection-1" as CollectionId,
                name: "Colors",
                modes: [],
                defaultModeId: "mode1",
                variables: new Map(),
            } as unknown as VariableCollectionSU,
        ],
    ]);

    // Test string
    const stringVar = {
        name: "test-string",
        resolvedType: "STRING",
    } as any;
    expect(
        generateVariableValue(
            stringVar,
            "hello",
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe(`${indent2}test-string: "hello",\n`);

    // Test boolean
    const boolVar = {
        name: "test-bool",
        resolvedType: "BOOLEAN",
    } as any;
    expect(
        generateVariableValue(
            boolVar,
            true,
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe(`${indent2}test-bool: true,\n`);

    // Test length
    const lengthVar = {
        name: "test-length",
        resolvedType: "FLOAT",
        scopes: ["ALL_SCOPES"],
    } as any;
    expect(
        generateVariableValue(
            lengthVar,
            42,
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe(`${indent2}test-length: 42px,\n`);

    // Test brush
    const brushVar = {
        name: "test-brush",
        resolvedType: "COLOR",
    } as any;
    expect(
        generateVariableValue(
            brushVar,
            "invalid-data",
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe("// unable to convert test-brush to brush,\n");

    // Test RGB object conversion
    expect(
        generateVariableValue(
            brushVar,
            { r: 1, g: 0, b: 0, a: 1 },
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe(`${indent2}test-brush: #ff0000,\n`);
    expect(
        generateVariableValue(
            brushVar,
            { r: 0, g: 1, b: 0, a: 1 },
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe(`${indent2}test-brush: #00ff00,\n`);
    expect(
        generateVariableValue(
            brushVar,
            { r: 0, g: 0, b: 1, a: 1 },
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe(`${indent2}test-brush: #0000ff,\n`);
    expect(
        generateVariableValue(
            brushVar,
            { r: 0.5, g: 0.5, b: 0.5, a: 1 },
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe(`${indent2}test-brush: #808080,\n`);
});

test("should handle variable aliases", () => {
    const variable = {
        name: "test-var",
        resolvedType: "COLOR",
    } as any;
    const variablesMap = new Map<VariableId, VariableSU>([
        [
            "var-id-1" as VariableId,
            {
                id: "var-id-1" as VariableId,
                name: "primary",
                resolvedType: "COLOR",
                variableCollectionId: "collection-1" as CollectionId,
                valuesByMode: {},
                scopes: [],
            } as unknown as VariableSU,
        ],
        [
            "var-id-2" as VariableId,
            {
                id: "var-id-2" as VariableId,
                name: "secondary",
                resolvedType: "COLOR",
                variableCollectionId: "collection-1" as CollectionId,
                valuesByMode: {},
                scopes: [],
            } as unknown as VariableSU,
        ],
    ]);
    const collectionName = "test-collection";
    const collectionsMap = new Map<CollectionId, VariableCollectionSU>([
        [
            "collection-1" as CollectionId,
            {
                id: "collection-1" as CollectionId,
                name: "Colors",
                modes: [],
                defaultModeId: "mode1",
                variables: new Map(),
            } as unknown as VariableCollectionSU,
        ],
    ]);

    // Test direct reference
    expect(
        generateVariableValue(
            variable,
            { type: "VARIABLE_ALIAS", id: "var-id-1" },
            collectionName,
            variablesMap,
            collectionsMap,
        ),
    ).toBe(`${indent2}test-var: Colors.collection.primary,\n`);
});
