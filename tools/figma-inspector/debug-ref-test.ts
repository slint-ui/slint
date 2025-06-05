// Simple test to verify reference preservation
import { test } from "vitest";
import { exportFigmaVariablesToSeparateFiles } from "../backend/utils/export-variables";

// Mock the global figma object
const mockFigma = {
    variables: {
        getLocalVariableCollectionsAsync: () =>
            Promise.resolve([
                {
                    id: "collection1",
                    name: "TestColors",
                    modes: [{ modeId: "mode1", name: "Default" }],
                    variableIds: ["var1", "var2"],
                },
            ]),
        getVariableByIdAsync: (id: string) => {
            if (id === "var1") {
                return Promise.resolve({
                    id: "var1",
                    name: "primary",
                    type: "COLOR",
                    resolvedType: "COLOR",
                    valuesByMode: {
                        mode1: { r: 1, g: 0, b: 0, a: 1 },
                    },
                });
            }
            if (id === "var2") {
                return Promise.resolve({
                    id: "var2",
                    name: "accent",
                    type: "COLOR",
                    resolvedType: "COLOR",
                    valuesByMode: {
                        mode1: { type: "VARIABLE_ALIAS", id: "var1" },
                    },
                });
            }
            return Promise.resolve(null);
        },
    },
    notify: () => {},
};

(global as any).figma = mockFigma;

test("debug reference output", async () => {
    const result = await exportFigmaVariablesToSeparateFiles(false);
    console.log("Generated content:");
    console.log(result[0].content);
    console.log("---");

    // Check if the content contains reference indicators
    const hasReference =
        result[0].content.includes("primary") &&
        result[0].content.includes("accent");
    console.log("Contains both variables:", hasReference);
});
