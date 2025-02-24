// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { listenTS, updateUI } from "./utils/code-utils.js";
import { generateSlintSnippet } from "./utils/property-parsing.js";

if (figma.editorType === "dev" && figma.mode === "codegen") {
    figma.codegen.on("generate", async ({ node }) => {
        const slintSnippet = generateSlintSnippet(node);
        return [
            {
                title: "Slint Code: " + node.name,
                language: "CSS",
                code: slintSnippet,
            },
        ];
    });
}

console.log("Figma plugin:", figma.editorType, figma.mode);

if (figma.editorType === "figma" && figma.mode === "default") {
    figma.showUI(__html__, {
        themeColors: true,
        width: 400,
        height: 320,
    });
    updateUI();
}

listenTS("copyToClipboard", () => {
    figma.notify("Copied!");
});

figma.on("selectionchange", () => {
    if (figma.editorType === "figma" && figma.mode === "default") {
        console.log("selection changed")
        updateUI();
    }
});
