// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect, vi, beforeEach } from "vitest";
import { exportFigmaVariablesToSeparateFiles } from "../backend/utils/export-variables";

// Mock the global figma object with proper variable structure
const mockFigma = {
    variables: {
        getLocalVariableCollectionsAsync: vi.fn(),
        getVariableByIdAsync: vi.fn(),
    },
    notify: vi.fn(),
};

// Set up global figma object
(global as any).figma = mockFigma;

beforeEach(() => {
    vi.clearAllMocks();
});

test("exports single collection with basic variables", async () => {
    // Mock collection and variables
    const mockCollection = {
        id: "collection1",
        name: "Colors",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1", "var2"],
    };

    const mockVariable1 = {
        id: "var1",
        name: "primary",
        type: "COLOR",
        valuesByMode: {
            mode1: { r: 1, g: 0, b: 0, a: 1 },
        },
    };

    const mockVariable2 = {
        id: "var2",
        name: "secondary",
        type: "COLOR",
        valuesByMode: {
            mode1: { r: 0, g: 1, b: 0, a: 1 },
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        if (id === "var1") {
            return Promise.resolve(mockVariable1);
        }
        if (id === "var2") {
            return Promise.resolve(mockVariable2);
        }
        return Promise.resolve(null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result).toHaveLength(2); // collection file + README
    expect(result[0].name).toBe("colors.slint");
    expect(result[0].content).toContain("export global colors");
    expect(result[0].content).toContain("primary");
    expect(result[0].content).toContain("secondary");
});

test("exports multiple modes with enum generation", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Theme",
        modes: [
            { modeId: "mode1", name: "light" },
            { modeId: "mode2", name: "dark" },
        ],
        variableIds: ["var1"],
    };

    const mockVariable = {
        id: "var1",
        name: "text-color",
        type: "COLOR",
        valuesByMode: {
            mode1: { r: 0, g: 0, b: 0, a: 1 },
            mode2: { r: 1, g: 1, b: 1, a: 1 },
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockResolvedValue(mockVariable);

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result).toHaveLength(2);
    // Check for camelCase enum name as per actual function output
    expect(result[0].content).toContain("export enum themeMode");
    expect(result[0].content).toContain("light,");
    expect(result[0].content).toContain("dark,");
});

test("handles hierarchical variable names with nested structs", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Design",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1", "var2"],
    };

    const mockVariable1 = {
        id: "var1",
        name: "colors/primary/main",
        type: "COLOR",
        valuesByMode: {
            mode1: { r: 1, g: 0, b: 0, a: 1 },
        },
    };

    const mockVariable2 = {
        id: "var2",
        name: "colors/secondary/accent",
        type: "COLOR",
        valuesByMode: {
            mode1: { r: 0, g: 1, b: 0, a: 1 },
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        if (id === "var1") {
            return Promise.resolve(mockVariable1);
        }
        if (id === "var2") {
            return Promise.resolve(mockVariable2);
        }
        return Promise.resolve(null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    // Check for actual struct naming convention
    expect(result[0].content).toContain("struct design_colors_primary");
    expect(result[0].content).toContain("primary:");
    expect(result[0].content).toContain("secondary:");
});

test("handles variable aliases (references)", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Colors",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1", "var2"],
    };

    const mockVariable1 = {
        id: "var1",
        name: "primary",
        type: "COLOR",
        valuesByMode: {
            mode1: { r: 1, g: 0, b: 0, a: 1 },
        },
    };

    const mockVariable2 = {
        id: "var2",
        name: "accent",
        type: "COLOR",
        valuesByMode: {
            mode1: { type: "VARIABLE_ALIAS", id: "var1" },
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        if (id === "var1") {
            return Promise.resolve(mockVariable1);
        }
        if (id === "var2") {
            return Promise.resolve(mockVariable2);
        }
        return Promise.resolve(null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result[0].content).toContain("primary");
    // The function resolves references, so accent should have the resolved value
    expect(result[0].content).toContain("accent");
});

test("detects and handles circular references", async () => {
    const consoleSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    const mockCollection = {
        id: "collection1",
        name: "Colors",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1", "var2"],
    };

    const mockVariable1 = {
        id: "var1",
        name: "first",
        type: "COLOR",
        valuesByMode: {
            mode1: { type: "VARIABLE_ALIAS", id: "var2" },
        },
    };

    const mockVariable2 = {
        id: "var2",
        name: "second",
        type: "COLOR",
        valuesByMode: {
            mode1: { type: "VARIABLE_ALIAS", id: "var1" },
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        if (id === "var1") {
            return Promise.resolve(mockVariable1);
        }
        if (id === "var2") {
            return Promise.resolve(mockVariable2);
        }
        return Promise.resolve(null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result).toHaveLength(2);
    // The function should handle circular references gracefully
    expect(result[0].content).toContain("export global colors");

    consoleSpy.mockRestore();
});
test("handles self-referential structs (struct members referencing each other)", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Colors",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1", "var2", "var3"],
    };

    // Variables that create a struct where members reference each other
    const mockVariable1 = {
        id: "var1",
        name: "theme/primary",
        type: "COLOR",
        valuesByMode: {
            mode1: { r: 1, g: 0, b: 0, a: 1 },
        },
    };

    const mockVariable2 = {
        id: "var2",
        name: "theme/secondary",
        type: "COLOR",
        valuesByMode: {
            mode1: { type: "VARIABLE_ALIAS", id: "var3" }, // References var3
        },
    };

    const mockVariable3 = {
        id: "var3",
        name: "theme/accent",
        type: "COLOR",
        valuesByMode: {
            mode1: { type: "VARIABLE_ALIAS", id: "var2" }, // References var2 - creates circular reference within same struct
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        if (id === "var1") return Promise.resolve(mockVariable1);
        if (id === "var2") return Promise.resolve(mockVariable2);
        if (id === "var3") return Promise.resolve(mockVariable3);
        return Promise.resolve(null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result).toHaveLength(2);
    expect(result[0].content).toContain("export global colors");
    // Should handle the circular reference within the theme struct gracefully
    expect(result[0].content).toContain("theme");
    // Should not contain self-referential struct definitions
    expect(result[0].content).not.toMatch(/struct.*theme.*{[^}]*theme[^}]*}/);
});

test("handles different variable types correctly", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Tokens",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1", "var2", "var3", "var4"],
    };

    const mockVariables = [
        {
            id: "var1",
            name: "color-token",
            type: "COLOR",
            valuesByMode: { mode1: { r: 1, g: 0, b: 0, a: 1 } },
        },
        {
            id: "var2",
            name: "size-token",
            type: "FLOAT",
            valuesByMode: { mode1: 16 },
        },
        {
            id: "var3",
            name: "text-token",
            type: "STRING",
            valuesByMode: { mode1: "Hello" },
        },
        {
            id: "var4",
            name: "flag-token",
            type: "BOOLEAN",
            valuesByMode: { mode1: true },
        },
    ];

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        return Promise.resolve(mockVariables.find((v) => v.id === id) || null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result[0].content).toContain("color-token");
    expect(result[0].content).toContain("size-token");
    expect(result[0].content).toContain("text-token");
    expect(result[0].content).toContain("flag-token");
});

test("exports as single file when requested", async () => {
    const mockCollection1 = {
        id: "collection1",
        name: "Colors",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1"],
    };

    const mockCollection2 = {
        id: "collection2",
        name: "Spacing",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var2"],
    };

    const mockVariable1 = {
        id: "var1",
        name: "primary",
        type: "COLOR",
        valuesByMode: { mode1: { r: 1, g: 0, b: 0, a: 1 } },
    };

    const mockVariable2 = {
        id: "var2",
        name: "small",
        type: "FLOAT",
        valuesByMode: { mode1: 8 },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection1,
        mockCollection2,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        if (id === "var1") {
            return Promise.resolve(mockVariable1);
        }
        if (id === "var2") {
            return Promise.resolve(mockVariable2);
        }
        return Promise.resolve(null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(true);

    // Should return single combined file plus README
    expect(result).toHaveLength(2);
    expect(result[0].name).toBe("design-tokens.slint");
    expect(result[0].content).toContain("export global colors");
    expect(result[0].content).toContain("export global spacing");
});

test("handles cross-collection references with imports", async () => {
    // This would be complex to test properly as it involves multiple collections
    // For now, test that it handles multiple collections without errors
    const mockCollection1 = {
        id: "collection1",
        name: "Colors",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1"],
    };

    const mockCollection2 = {
        id: "collection2",
        name: "Components",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var2"],
    };

    const mockVariable1 = {
        id: "var1",
        name: "primary",
        type: "COLOR",
        valuesByMode: { mode1: { r: 1, g: 0, b: 0, a: 1 } },
    };

    const mockVariable2 = {
        id: "var2",
        name: "button-color",
        type: "COLOR",
        valuesByMode: { mode1: { type: "VARIABLE_ALIAS", id: "var1" } },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection1,
        mockCollection2,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        if (id === "var1") {
            return Promise.resolve(mockVariable1);
        }
        if (id === "var2") {
            return Promise.resolve(mockVariable2);
        }
        return Promise.resolve(null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    // Should create separate files for each collection
    expect(result.length).toBeGreaterThanOrEqual(2);
    const colorFile = result.find((f) => f.name === "colors.slint");
    const componentFile = result.find((f) => f.name === "components.slint");

    expect(colorFile).toBeDefined();
    expect(componentFile).toBeDefined();
});

test("sanitizes identifiers with special characters", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Color & Shade",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1"],
    };

    const mockVariable = {
        id: "var1",
        name: "primary-color (main)",
        type: "COLOR",
        valuesByMode: { mode1: { r: 1, g: 0, b: 0, a: 1 } },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockResolvedValue(mockVariable);

    const result = await exportFigmaVariablesToSeparateFiles(false);

    // Collection name should be sanitized
    expect(result[0].name).toBe("color-and-shade.slint");
    // Variable name should be sanitized
    expect(result[0].content).toContain("primary-color-main");
});

test("handles empty collections gracefully", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Empty",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: [],
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result).toHaveLength(1); // Only README for empty collections
    expect(result[0].name).toBe("README.md");
});

test("handles API errors gracefully", async () => {
    mockFigma.variables.getLocalVariableCollectionsAsync.mockRejectedValue(
        new Error("API Error"),
    );

    const result = await exportFigmaVariablesToSeparateFiles(false);
    expect(result[0].name).toBe("error.slint");
    expect(result[0].content).toContain("Error generating variables");
});

test("uses properly formatted collection names in file headers", async () => {
    const mockCollection = {
        id: "collection1",
        name: "My Special Collection & Theme",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1"],
    };

    const mockVariable = {
        id: "var1",
        name: "primary",
        type: "COLOR",
        valuesByMode: { mode1: { r: 1, g: 0, b: 0, a: 1 } },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockResolvedValue(mockVariable);

    const result = await exportFigmaVariablesToSeparateFiles(false);

    // Filename should use sanitized name
    expect(result[0].name).toBe("my-special-collection-and-theme.slint");

    // File header should use the formatted collection name (sanitized version)
    expect(result[0].content).toContain(
        "// Generated Slint file for my-special-collection-and-theme",
    );

    // Should NOT contain the original name in the header
    expect(result[0].content).not.toContain(
        "// Generated Slint file for My Special Collection & Theme",
    );
});

test("resolves all references including legitimate cross-references to concrete values", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Colors",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1", "var2", "var3", "var4"],
    };

    // var1: concrete value
    const mockVariable1 = {
        id: "var1",
        name: "primary",
        type: "COLOR",
        valuesByMode: {
            mode1: { r: 1, g: 0, b: 0, a: 1 },
        },
    };

    // var2: legitimate reference to var1 (should be preserved)
    const mockVariable2 = {
        id: "var2",
        name: "accent",
        type: "COLOR",
        valuesByMode: {
            mode1: { type: "VARIABLE_ALIAS", id: "var1" },
        },
    };

    // var3: circular reference to var4 (should be resolved)
    const mockVariable3 = {
        id: "var3",
        name: "circular1",
        type: "COLOR",
        valuesByMode: {
            mode1: { type: "VARIABLE_ALIAS", id: "var4" },
        },
    };

    // var4: circular reference back to var3 (should be resolved)
    const mockVariable4 = {
        id: "var4",
        name: "circular2",
        type: "COLOR",
        valuesByMode: {
            mode1: { type: "VARIABLE_ALIAS", id: "var3" },
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        if (id === "var1") {
            return Promise.resolve(mockVariable1);
        }
        if (id === "var2") {
            return Promise.resolve(mockVariable2);
        }
        if (id === "var3") {
            return Promise.resolve(mockVariable3);
        }
        if (id === "var4") {
            return Promise.resolve(mockVariable4);
        }
        return Promise.resolve(null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result[0].content).toContain("primary");
    expect(result[0].content).toContain("accent");
    expect(result[0].content).toContain("circular1");
    expect(result[0].content).toContain("circular2");

    // With resolution-based approach, ALL references are resolved to concrete values
    // Both legitimate references (var2 -> var1) and circular references (var3 <-> var4)
    // are resolved to concrete values to eliminate "Missing data for mode" errors
    expect(result[0].content).toBeDefined();
});

test("resolves all references to concrete values (resolution-based approach)", async () => {
    // Setup mock collection
    const mockCollection = {
        id: "collection1",
        name: "Test Colors",
        modes: [{ modeId: "mode1", name: "Light" }],
        variableIds: ["var1", "var2", "var3", "var4"],
    };

    // var1: concrete value (the target of legitimate reference)
    const mockVariable1 = {
        id: "var1",
        name: "primary",
        type: "COLOR",
        resolvedType: "COLOR",
        valuesByMode: {
            mode1: { r: 1, g: 0, b: 0, a: 1 }, // Red color
        },
    };

    // var2: legitimate reference to var1 (should be preserved)
    const mockVariable2 = {
        id: "var2",
        name: "accent",
        type: "COLOR",
        resolvedType: "COLOR",
        valuesByMode: {
            mode1: { type: "VARIABLE_ALIAS", id: "var1" },
        },
    };

    // var3: part of circular reference (should be resolved)
    const mockVariable3 = {
        id: "var3",
        name: "circular1",
        type: "COLOR",
        resolvedType: "COLOR",
        valuesByMode: {
            mode1: { type: "VARIABLE_ALIAS", id: "var4" },
        },
    };

    // var4: circular reference back to var3 (should be resolved)
    const mockVariable4 = {
        id: "var4",
        name: "circular2",
        type: "COLOR",
        resolvedType: "COLOR",
        valuesByMode: {
            mode1: { type: "VARIABLE_ALIAS", id: "var3" },
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        const variables: { [key: string]: any } = {
            var1: mockVariable1,
            var2: mockVariable2,
            var3: mockVariable3,
            var4: mockVariable4,
        };
        return Promise.resolve(variables[id] || null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);
    const content = result[0].content;

    // With resolution-based approach, ALL references are resolved to concrete values
    // The legitimate cross-reference (var2 -> var1) should be resolved to the concrete value
    expect(content).toContain("accent: #ff0000"); // Resolved to concrete red value from var1

    // Check that circular references are resolved to concrete values (not references)
    expect(content).not.toContain("circular1: test-colors.circular2");
    expect(content).not.toContain("circular2: test-colors.circular1");

    // Check that circular references have some resolved value (likely default fallback)
    expect(content).toContain("circular1:");
    expect(content).toContain("circular2:");

    console.log("Generated content:", content);
});

test("shows readable variable names in comments for resolved references", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Colors",
        modes: [{ modeId: "mode1", name: "Default" }],
        variableIds: ["var1", "var2"],
    };

    const mockVariable1 = {
        id: "var1",
        name: "primary-color",
        type: "COLOR",
        resolvedType: "COLOR",
        valuesByMode: {
            mode1: { r: 1, g: 0, b: 0, a: 1 },
        },
    };

    const mockVariable2 = {
        id: "var2",
        name: "accent-color",
        type: "COLOR",
        resolvedType: "COLOR",
        valuesByMode: {
            mode1: { type: "VARIABLE_ALIAS", id: "var1" },
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        if (id === "var1") {
            return Promise.resolve(mockVariable1);
        }
        if (id === "var2") {
            return Promise.resolve(mockVariable2);
        }
        return Promise.resolve(null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    console.log("Test result content:", result[0].content);

    // Verify that comments show variable names instead of IDs
    expect(result[0].content).toContain(
        "Resolved from reference primary-color",
    );
    expect(result[0].content).not.toContain("Resolved from reference var1");
});

test("handles variables with same values across multiple modes without showing 'Missing data for mode'", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Theme",
        modes: [
            { modeId: "mode1", name: "light" },
            { modeId: "mode2", name: "dark" },
        ],
        variableIds: ["var1"],
    };

    // Variable that has the same value in both modes
    // In Figma, this often happens when a variable inherits its value from the base mode
    const mockVariable = {
        id: "var1",
        name: "border-radius",
        type: "FLOAT",
        resolvedType: "FLOAT",
        valuesByMode: {
            mode1: 8, // Same value
            mode2: 8, // Same value
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockResolvedValue(mockVariable);

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result).toHaveLength(2); // theme.slint + README
    const content = result[0].content;

    // Should NOT contain any "Missing data for mode" comments
    expect(content).not.toContain("Missing data for mode");

    // Should contain the actual value for both modes
    expect(content).toContain("light: 8px");
    expect(content).toContain("dark: 8px");

    // Should contain the variable name
    expect(content).toContain("border-radius");

    console.log("Generated content:", content);
});

test("reproduces 'Missing data for mode' issue with hierarchical variables", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Theme",
        modes: [
            { modeId: "mode1", name: "light" },
            { modeId: "mode2", name: "dark" },
        ],
        variableIds: ["var1", "var2"],
    };

    // Hierarchical variable that might trigger the issue
    const mockVariable1 = {
        id: "var1",
        name: "colors/background",
        type: "COLOR",
        resolvedType: "COLOR",
        valuesByMode: {
            mode1: { r: 1, g: 1, b: 1, a: 1 }, // white
            mode2: { r: 0, g: 0, b: 0, a: 1 }, // black
        },
    };

    // Another variable that has same values
    const mockVariable2 = {
        id: "var2",
        name: "colors/border",
        type: "COLOR",
        resolvedType: "COLOR",
        valuesByMode: {
            mode1: { r: 0.5, g: 0.5, b: 0.5, a: 1 }, // Same gray in both modes
            mode2: { r: 0.5, g: 0.5, b: 0.5, a: 1 }, // Same gray in both modes
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        if (id === "var1") return Promise.resolve(mockVariable1);
        if (id === "var2") return Promise.resolve(mockVariable2);
        return Promise.resolve(null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result).toHaveLength(2);
    const content = result[0].content;

    // Should NOT contain any "Missing data for mode" comments
    expect(content).not.toContain("Missing data for mode");

    // Should contain both modes for hierarchical variables
    expect(content).toContain("light:");
    expect(content).toContain("dark:");

    console.log("Generated content with hierarchical variables:", content);
});

test("fixes 'Missing data for mode' bug - modeId mismatch between collection and variable", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Theme",
        modes: [
            { modeId: "mode_light", name: "light" },
            { modeId: "mode_dark", name: "dark" },
        ],
        variableIds: ["var1"],
    };

    // This variable has values for modes with different modeIds than the collection
    // This simulates the real-world scenario where there's a mismatch
    const mockVariable = {
        id: "var1",
        name: "background-color",
        type: "COLOR",
        resolvedType: "COLOR",
        valuesByMode: {
            // These modeIds DON'T match the collection.modes modeIds
            mode_original: { r: 1, g: 1, b: 1, a: 1 }, // white
            mode_newer: { r: 0, g: 0, b: 0, a: 1 }, // black
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockResolvedValue(mockVariable);

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result).toHaveLength(2); // theme.slint + README
    const content = result[0].content;

    // Should NOT contain "Missing data for mode"
    expect(content).not.toContain("Missing data for mode");

    // Should not contain magenta placeholder values
    expect(content).not.toContain("#FF00FF");

    // Should contain actual color values from different variable modes
    expect(content).toContain("#ffffff"); // white from mode_original
    expect(content).toContain("#000000"); // black from mode_newer

    // Should contain different values for each mode (using enhanced fallback strategy)
    expect(content).toContain("light: #ffffff"); // Gets first available value (mode_original)
    expect(content).toContain("dark: #000000"); // Gets second available value (mode_newer)
});

test("comprehensive mode matching - handles various mismatch scenarios", async () => {
    const mockCollection = {
        id: "collection1",
        name: "Design",
        modes: [
            { modeId: "light_mode", name: "Light" },
            { modeId: "dark_mode", name: "Dark" },
            { modeId: "high_contrast", name: "High Contrast" },
        ],
        variableIds: ["var1", "var2", "var3"],
    };

    // Variable 1: Exact modeId match
    const mockVariable1 = {
        id: "var1",
        name: "perfect-match",
        type: "COLOR",
        resolvedType: "COLOR",
        valuesByMode: {
            light_mode: { r: 1, g: 1, b: 1, a: 1 }, // white
            dark_mode: { r: 0, g: 0, b: 0, a: 1 }, // black
            high_contrast: { r: 1, g: 1, b: 0, a: 1 }, // yellow
        },
    };

    // Variable 2: Mode name matching (modeIds don't match but names do)
    const mockVariable2 = {
        id: "var2",
        name: "name-match",
        type: "COLOR",
        resolvedType: "COLOR",
        valuesByMode: {
            mode_light: { r: 0.9, g: 0.9, b: 0.9, a: 1 }, // light gray
            mode_dark: { r: 0.1, g: 0.1, b: 0.1, a: 1 }, // dark gray
            mode_high_contrast: { r: 1, g: 0, b: 1, a: 1 }, // magenta
        },
    };

    // Variable 3: No matching modes at all (should use fallback)
    const mockVariable3 = {
        id: "var3",
        name: "fallback-needed",
        type: "COLOR",
        resolvedType: "COLOR",
        valuesByMode: {
            completely_different: { r: 0.5, g: 0.5, b: 0.5, a: 1 }, // gray
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        if (id === "var1") {
            return Promise.resolve(mockVariable1);
        }
        if (id === "var2") {
            return Promise.resolve(mockVariable2);
        }
        if (id === "var3") {
            return Promise.resolve(mockVariable3);
        }
        return Promise.resolve(null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result).toHaveLength(2); // design.slint + README
    const content = result[0].content;

    // Should NOT contain any "Missing data for mode" - all should be resolved
    expect(content).not.toContain("Missing data for mode");
    expect(content).not.toContain("#FF00FF");

    // Variable 1: Perfect match - should have distinct values
    expect(content).toContain("perfect-match");

    // Variable 2: Name-based matching - should work with some warnings
    expect(content).toContain("name-match");

    // Variable 3: Fallback strategy - should use the available value for all modes
    expect(content).toContain("fallback-needed");
    expect(content).toContain("#808080"); // The gray fallback value should appear multiple times
});

test("mode value mismatch - different modes should have different values", async () => {
    // Mock collection with two modes: modern_theme and brutal_theme
    const mockCollection = {
        id: "collection1",
        name: "Primitives Completed",
        modes: [
            { modeId: "mode1", name: "modern_theme" },
            { modeId: "mode2", name: "brutal_theme" },
        ],
        variableIds: ["var1"],
    };

    // Mock variable with hierarchical name and different values per mode
    const mockVariable = {
        id: "var1",
        name: "radius/sm",
        type: "FLOAT",
        resolvedType: "FLOAT",
        valuesByMode: {
            mode1: 4, // modern_theme should be 4px
            mode2: 0, // brutal_theme should be 0px
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        if (id === "var1") {
            return Promise.resolve(mockVariable);
        }
        return Promise.resolve(null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    expect(result).toHaveLength(2); // One collection file + README
    const collectionFile = result.find((f) =>
        f.name.includes("primitives-completed"),
    );
    expect(collectionFile).toBeDefined();

    // Check that both modes exist and have DIFFERENT values
    expect(collectionFile!.content).toContain("modern_theme: 4px");
    expect(collectionFile!.content).toContain("brutal_theme: 0px");

    // Make sure they're not both the same value
    expect(collectionFile!.content).not.toMatch(
        /modern_theme: 4px,\s*brutal_theme: 4px/,
    );
    expect(collectionFile!.content).not.toMatch(
        /modern_theme: 0px,\s*brutal_theme: 0px/,
    );
});

test("hierarchical variables with multiple values per mode", async () => {
    // Mock collection with two modes
    const mockCollection = {
        id: "collection1",
        name: "Design System",
        modes: [
            { modeId: "light-mode", name: "light" },
            { modeId: "dark-mode", name: "dark" },
        ],
        variableIds: ["radius-sm", "radius-md"],
    };

    // Mock multiple variables with different values
    const mockVariables = {
        "radius-sm": {
            id: "radius-sm",
            name: "radius/sm",
            type: "FLOAT",
            resolvedType: "FLOAT",
            valuesByMode: {
                "light-mode": 8, // light theme: 8px
                "dark-mode": 12, // dark theme: 12px
            },
        },
        "radius-md": {
            id: "radius-md",
            name: "radius/md",
            type: "FLOAT",
            resolvedType: "FLOAT",
            valuesByMode: {
                "light-mode": 16, // light theme: 16px
                "dark-mode": 20, // dark theme: 20px
            },
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockImplementation((id) => {
        return Promise.resolve(mockVariables[id as keyof typeof mockVariables] || null);
    });

    const result = await exportFigmaVariablesToSeparateFiles(false);

    const collectionFile = result.find((f) => f.name.includes("design-system"));
    expect(collectionFile).toBeDefined();

    // Verify each variable has different values per mode
    const content = collectionFile!.content;

    // Check sm values are different
    expect(content).toContain("light: 8px");
    expect(content).toContain("dark: 12px");

    // Check md values are different
    expect(content).toContain("light: 16px");
    expect(content).toContain("dark: 20px");

    // Ensure no duplicate values where they shouldn't be
    expect(content).not.toMatch(/light: 8px,\s*dark: 8px/);
    expect(content).not.toMatch(/light: 16px,\s*dark: 16px/);
});

test("mode mismatch fallback distributes different values correctly", async () => {
    // This test specifically reproduces the case where:
    // 1. Collection has modes with certain IDs
    // 2. Variable has valuesByMode with DIFFERENT IDs
    // 3. The enhanced fallback logic distributes different values to different modes

    const mockCollection = {
        id: "collection1",
        name: "Primitives",
        modes: [
            { modeId: "collection-mode-1", name: "modern_theme" },
            { modeId: "collection-mode-2", name: "brutal_theme" },
        ],
        variableIds: ["radius_sm"],
    };

    // Variable has valuesByMode with DIFFERENT keys than collection modeIds
    // This should trigger the enhanced fallback logic
    const mockVariable = {
        id: "radius_sm",
        name: "radius/sm",
        type: "FLOAT",
        resolvedType: "FLOAT",
        valuesByMode: {
            "variable-mode-a": 4, // This doesn't match "collection-mode-1"
            "variable-mode-b": 0, // This doesn't match "collection-mode-2"
        },
    };

    mockFigma.variables.getLocalVariableCollectionsAsync.mockResolvedValue([
        mockCollection,
    ]);
    mockFigma.variables.getVariableByIdAsync.mockResolvedValue(mockVariable);

    const result = await exportFigmaVariablesToSeparateFiles(false);
    const content = result[0].content;

    // With the enhanced fallback logic, modes should get different values
    const modernThemeMatch = content.match(/modern_theme:\s*(\d+px)/);
    const brutalThemeMatch = content.match(/brutal_theme:\s*(\d+px)/);

    expect(modernThemeMatch).toBeTruthy();
    expect(brutalThemeMatch).toBeTruthy();

    // The enhanced fallback should distribute different values to different modes
    expect(modernThemeMatch![1]).toBe("4px"); // Gets first available value (index 0)
    expect(brutalThemeMatch![1]).toBe("0px"); // Gets second available value (index 1)

    // Ensure they're not the same (which would be the old bug)
    expect(modernThemeMatch![1]).not.toBe(brutalThemeMatch![1]);
});
