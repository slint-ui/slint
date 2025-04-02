// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
// cSpell: ignore codegen

import { listenTS, updateUI } from "./utils/code-utils.js";
import { generateSlintSnippet } from "./utils/property-parsing.js";
import { exportFigmaVariablesToSeparateFiles } from "./utils/export-variables.js"
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

j("exportToFiles", async () => {
    try {
      const exportedFiles = await exportFigmaVariablesToSeparateFiles();
      console.log(`Exported ${exportedFiles.length} collection files`);
      
      // Send to UI for downloading
      figma.ui.postMessage({
        type: 'exportedFiles',
        files: exportedFiles
      });
      
      figma.notify(`${exportedFiles.length} collection files ready for download!`);
    } catch (error) {
      console.error("Error exporting to files:", error);
      figma.notify("Failed to export to files", { error: true });
    }
  });
function j(messageType: string, callback: () => Promise<void>) {
    figma.ui.on('message', async (msg) => {
        if (msg.type === messageType) {
            try {
                await callback();
            } catch (error) {
                console.error(`Error in ${messageType} handler:`, error);
                figma.notify(`Error handling ${messageType}`, { error: true });
            }
        }
    });
}

// Define state variables outside any function (at module level)
const variableMonitoring: {
    initialized: boolean,
    lastSnapshot: string | null,
    lastChange: number,
    lastEventTime: number
} = {
    initialized: false,
    lastSnapshot: null,
    lastChange: 0,
    lastEventTime: 0
  };
  
  // Keep the DEBOUNCE_INTERVAL as a constant
  const DEBOUNCE_INTERVAL = 3000; // 3 seconds
  
  // Replace your monitorVariableChanges handler
  j("monitorVariableChanges", async () => {
    console.log("Setting up variable change monitoring in plugin");
    
    // Set up event listeners for variable changes
    if (figma.variables && typeof (figma.variables as any).onVariableValueChange === 'function') {
      console.log("Setting up onVariableValueChange listener");
      
      (figma.variables as any).onVariableValueChange((event: { variableId: string }) => {
        const now = Date.now();
        
        // Only process if enough time has passed since last event
        if (now - variableMonitoring.lastEventTime > DEBOUNCE_INTERVAL) {
          variableMonitoring.lastEventTime = now;
          console.log("Variable value changed:", event);
          
          figma.ui.postMessage({
            type: "variableChanged",
            data: { 
              variableId: event.variableId,
              timestamp: now
            }
          });
        }
      });
    }
    
    // Collection changes
    if (figma.variables && typeof (figma.variables as any).onVariableCollectionChange === 'function') {
      console.log("Setting up onVariableCollectionChange listener");
      
      (figma.variables as any).onVariableCollectionChange((event: { variableCollectionId: string }) => {
        const now = Date.now();
        
        // Only process if enough time has passed since last event
        if (now - variableMonitoring.lastEventTime > DEBOUNCE_INTERVAL) {
          variableMonitoring.lastEventTime = now;
          console.log("Variable collection changed:", event);
          
          figma.ui.postMessage({
            type: "variableCollectionChanged",
            data: {
              collectionId: event.variableCollectionId,
              timestamp: now
            }
          });
        }
      });
    }
    
    // Confirm setup to UI
    figma.ui.postMessage({
      type: "variableMonitoringActive",
      timestamp: Date.now()
    });
  });
  
  // Replace your checkVariableChanges handler
  j("checkVariableChanges", async () => {
    try {
      // Use the async version as required
      const collections = await figma.variables.getLocalVariableCollectionsAsync();
      
      // Create a compact representation of the current state
      const collectionData = collections.map(c => ({
        id: c.id,
        name: c.name,
        modeCount: c.modes.length, // simpler representation
        variableCount: c.variableIds.length
      }));
      
      const currentSnapshot = JSON.stringify(collectionData);
      
      // First run special case
      if (!variableMonitoring.initialized) {
        variableMonitoring.lastSnapshot = currentSnapshot;
        variableMonitoring.initialized = true;
        console.log("Variable monitoring initialized with baseline snapshot");
        return;
      }
      
      // Compare with stored snapshot
      const now = Date.now();
      const timeSinceLastChange = now - variableMonitoring.lastChange;
      const hasChanged = variableMonitoring.lastSnapshot !== currentSnapshot;
      
      // Update reference data when changed
      if (hasChanged) {
        variableMonitoring.lastSnapshot = currentSnapshot;
        variableMonitoring.lastChange = now;
      }
      
      // Only notify if there's an actual change AND enough time has passed
      if (hasChanged && timeSinceLastChange > 5000) {
        console.log("Real variable changes detected in collections");
        figma.ui.postMessage({
          type: "documentSnapshot",
          timestamp: now,
          collectionsCount: collections.length,
          hasChanges: true
        });
      } else {
        // Silent no-change indicator
      }
    } catch (error) {      
      // Notify UI of the error
      figma.ui.postMessage({
        type: "documentSnapshot",
        timestamp: Date.now(),
        error: String(error)
      });
    }
  });
