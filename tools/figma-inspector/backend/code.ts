// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
// cSpell: ignore codegen

import { listenTS, updateUI } from "./utils/code-utils.js";
import { generateSlintSnippet } from "./utils/property-parsing.js";
import { exportFigmaVariablesToSeparateFiles } from "./utils/export-variables.js";
if (figma.editorType === "dev" && figma.mode === "codegen") {
    figma.codegen.on("generate", async ({ node }) => {
        const slintSnippet = generateSlintSnippet(node);
        return slintSnippet
            ? [
                  {
                      title: "Slint Code: " + node.name,
                      language: "CSS",
                      code: slintSnippet,
                  },
              ]
            : [];
    });
}

if (figma.editorType === "figma" && figma.mode === "default") {
    figma.showUI(__html__, {
        themeColors: true,
        width: 400,
        height: 320,
    });
    updateUI();
}

listenTS("copyToClipboard", ({ result }) => {
    if (result) {
        figma.notify("Copied!");
    } else {
        figma.notify("Failed to copy");
    }
});

figma.on("selectionchange", () => {
    if (figma.editorType === "figma" && figma.mode === "default") {
        updateUI();
    }
});

listenTS("exportToFiles", async (payload: { exportAsSingleFile?: boolean }) => {
    console.log(
        "[Backend] Received 'exportToFiles' request. Payload:",
        payload,
    ); // Check this log
    const shouldExportAsSingleFile = payload?.exportAsSingleFile ?? false;
    console.log(
        `[Backend] Parsed shouldExportAsSingleFile: ${shouldExportAsSingleFile}`,
    ); // Check this log
    try {
        const files = await exportFigmaVariablesToSeparateFiles(
            shouldExportAsSingleFile,
        );
        console.log(`Exported ${files.length} collection files`);

        // Send to UI for downloading
        figma.ui.postMessage({
            type: "exportedFiles",
            files: files,
        });

        figma.notify(`${files.length} collection files ready for download!`);
    } catch (error) {
        console.error("Error exporting to files:", error);
        figma.notify("Failed to export to files", { error: true });
    }
});

// Define state variables outside any function (at module level)
const variableMonitoring: {
    initialized: boolean;
    lastSnapshot: string | null;
    lastChange: number;
    lastEventTime: number;
} = {
    initialized: false,
    lastSnapshot: null,
    lastChange: 0,
    lastEventTime: 0,
};

// Keep the DEBOUNCE_INTERVAL as a constant
const DEBOUNCE_INTERVAL = 3000; // 3 seconds

listenTS("monitorVariableChanges", async () => {
    console.log("[Backend] Received 'monitorVariableChanges' from UI."); // <-- Add Log

    // Confirm setup to UI
    console.log(
        "[Backend] Posting 'variableMonitoringActive' confirmation to UI.",
    ); // <-- Add Log
    figma.ui.postMessage({
        type: "variableMonitoringActive", // Keep this confirmation
        timestamp: Date.now(),
    });
});
listenTS("checkVariableChanges", async () => {
    await checkVariableChanges(); // Call the main async function
});

// Replace your checkVariableChanges handler
async function checkVariableChanges(isInitialRun = false) {
    console.log("[Backend] Running checkVariableChanges..."); // Log run
    try {
        const collections =
            await figma.variables.getLocalVariableCollectionsAsync();
        const detailedSnapshotData: Record<string, any> = {};
        let variableFetchError = false;

        for (const collection of collections) {
            detailedSnapshotData[collection.id] = {
                id: collection.id,
                name: collection.name,
                modes: collection.modes.map((m) => ({
                    id: m.modeId,
                    name: m.name,
                })),
                variables: {}, // Store variable details here
            };

            // Fetch details for each variable in the collection
            // NOTE: This can be slow for *very* large numbers of variables
            for (const variableId of collection.variableIds) {
                try {
                    const variable =
                        await figma.variables.getVariableByIdAsync(variableId);
                    if (variable) {
                        // Store relevant value data (e.g., valuesByMode)
                        detailedSnapshotData[collection.id].variables[
                            variable.id
                        ] = {
                            id: variable.id,
                            name: variable.name,
                            resolvedType: variable.resolvedType,
                            // Include valuesByMode to detect value changes
                            valuesByMode: variable.valuesByMode,
                        };
                    }
                } catch (err) {
                    console.error(
                        `[Backend] Error fetching variable ${variableId}:`,
                        err,
                    );
                    variableFetchError = true; // Mark that an error occurred
                    // Optionally add placeholder data or skip
                    detailedSnapshotData[collection.id].variables[variableId] =
                        { error: `Failed to fetch: ${err}` };
                }
            }
        }

        const currentSnapshot = JSON.stringify(detailedSnapshotData);
        const now = Date.now();

        // Handle initial run or forced update
        if (isInitialRun || !variableMonitoring.initialized) {
            variableMonitoring.lastSnapshot = currentSnapshot;
            variableMonitoring.initialized = true;
            variableMonitoring.lastChange = now; // Set initial timestamp
            console.log(
                "[Backend] Variable monitoring initialized/updated with detailed baseline snapshot.",
            );
            // Optionally notify UI that it's initialized, maybe reset its state
            figma.ui.postMessage({
                type: "snapshotInitialized",
                timestamp: now,
            });
            return; // Don't compare on the very first run
        }

        // Compare with the stored detailed snapshot
        const hasChanged = variableMonitoring.lastSnapshot !== currentSnapshot;

        if (hasChanged) {
            console.log(
                "[Backend] Detailed snapshot comparison detected changes.",
            ); // Log change detection
            variableMonitoring.lastSnapshot = currentSnapshot;
            variableMonitoring.lastChange = now;

            // Post a message indicating changes were found via snapshot
            figma.ui.postMessage({
                type: "documentSnapshot", // Use the existing type the UI listens for
                timestamp: now,
                hasChanges: true, // Indicate changes found
                details: variableFetchError
                    ? "Snapshot updated (some variable errors)"
                    : "Snapshot updated",
            });
        } else {
            console.log("[Backend] No changes detected in detailed snapshot."); // Log no change
        }
    } catch (error) {
        console.error("[Backend] Error during checkVariableChanges:", error);
        // Notify UI of the error
        figma.ui.postMessage({
            type: "documentSnapshot", // Use existing type
            timestamp: Date.now(),
            error: `Error checking variables: ${String(error)}`,
            hasChanges: false, // Indicate no confirmed change due to error
        });
    }
}
