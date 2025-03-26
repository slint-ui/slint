// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
// cSpell: ignore codegen

import { listenTS, updateUI } from "./utils/code-utils.js";
import { generateSlintSnippet } from "./utils/property-parsing.js";
import { exportFigmaVariablesToSlint } from "./utils/export-variables.js"
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
listenTS("exportAll", async ({ result }) => {
    if (result) {
        try {
            // Call the async function and await its result
            const slintCode = await exportFigmaVariablesToSlint();
            // console.clear();
            console.log("slint\n\n", slintCode);
            
            // Send the code to the UI for clipboard functionality
            figma.ui.postMessage({
                type: 'copyToClipboard',
                text: slintCode
            });
            
            // Log for debugging purpose
            console.log("Slint variables exported successfully");
            
            figma.notify("Slint variables exported to clipboard!");
        } catch (error) {
            console.error("Error exporting variables:", error);
            figma.notify("Failed to export variables", { error: true });
        }
    }
});
