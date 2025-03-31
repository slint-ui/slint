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

