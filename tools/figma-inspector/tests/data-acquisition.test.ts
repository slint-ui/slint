// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { test, expect, vi, beforeEach } from "vitest";
import { acquireFigmaVariableData } from "../backend/utils/figma-data-acquisition";

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

test("data acquisition works with simple mock", async () => {
    // Simple mock with no variables
    const mockCollection = {
        id: "collection1",
        name: "Test",
        defaultModeId: "mode1",
        hiddenFromPublishing: false,
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: [],
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);

    const result = await acquireFigmaVariableData();

    expect(result.collections).toHaveLength(1);
    expect(result.collections[0].name).toBe("Test");
    expect(result.variables.size).toBe(0);
});
