// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { test, expect, vi, beforeEach } from "vitest";
import { exportFigmaVariablesToJson } from "../backend/utils/export-json";

// Mock the global figma object
const mockFigma = {
    variables: {
        getLocalVariableCollectionsAsync: vi.fn(),
        getVariableByIdAsync: vi.fn(),
    },
};

// Set up global figma object
(global as any).figma = mockFigma;

beforeEach(() => {
    vi.clearAllMocks();
});

test("exports basic collection using data acquisition layer", async () => {
    // Mock collection
    const mockCollection = {
        id: "collection1",
        name: "Colors",
        defaultModeId: "mode1",
        hiddenFromPublishing: false,
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1"],
    };

    // Mock variable
    const mockVariable = {
        id: "var1",
        name: "primary",
        variableCollectionId: "collection1",
        resolvedType: "COLOR",
        valuesByMode: {
            mode1: { r: 1, g: 0, b: 0, a: 1 },
        },
        hiddenFromPublishing: false,
        scopes: ["ALL_SCOPES"],
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockResolvedValue(mockVariable);

    const result = await exportFigmaVariablesToJson();

    expect(result.collections).toHaveLength(1);
    expect(result.collections[0].name).toBe("Colors");
    expect(result.collections[0].variables).toHaveLength(1);
    expect(result.collections[0].variables[0].name).toBe("primary");
});

test("handles empty collections", async () => {
    // Mock collection with no variables
    const mockCollection = {
        id: "collection1",
        name: "Empty",
        defaultModeId: "mode1",
        hiddenFromPublishing: false,
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: [],
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);

    const result = await exportFigmaVariablesToJson();

    expect(result.collections).toHaveLength(1);
    expect(result.collections[0].name).toBe("Empty");
    expect(result.collections[0].variables).toHaveLength(0);
});
