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

    expect(result[0].content).toContain("color_token");
    expect(result[0].content).toContain("size_token");
    expect(result[0].content).toContain("text_token");
    expect(result[0].content).toContain("flag_token");
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
    expect(result[0].content).toContain("primary_color_main");
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
