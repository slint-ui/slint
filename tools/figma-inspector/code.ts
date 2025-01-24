// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

function getStatus(selectionCount: number) {
    if (selectionCount === 0) {
        return "Please select a layer";
    }
    if (selectionCount > 1) {
        return "Please select only one layer";
    }
    return "Slint properties:";
}

type StyleObject = {
    [key: string]: string;
};

const itemsToKeep = [
    "color",
    "font-family",
    "font-size",
    "font-weight",
    "width",
    "height",
    "fill",
    "opacity",
    "border-radius",
    "fill",
    "stroke-width",
    "stroke",
];

function transformStyle(styleObj: StyleObject): string {
    const filteredEntries = Object.entries(styleObj)
        .filter(([key]) => itemsToKeep.includes(key))
        .map(([key, value]) => {
            let finalKey = key;
            let finalValue = value;

            switch (key) {
                case "fill":
                    finalKey = "background";
                    break;
                case "stroke":
                    finalKey = "border-color";
                    break;
                case "stroke-width":
                    finalKey = "border-width";
                    break;
                case "font-family":
                    finalValue = `"${value}"`;
                    break;
            }

            if (value.includes("linear-gradient")) {
                return `${finalKey}: @${finalValue}`;
            }

            return `${finalKey}: ${finalValue}`;
        });

    return filteredEntries.length > 0 ? `${filteredEntries.join(";\n")};` : "";
}

async function updateUI() {
    const title = getStatus(figma.currentPage.selection.length);
    let slintProperties = "";

    if (figma.currentPage.selection.length === 1) {
        const cssProperties =
            await figma.currentPage.selection[0].getCSSAsync();
        slintProperties = transformStyle(cssProperties);
        console.log(cssProperties);
    }

    figma.ui.postMessage({ title, slintProperties });
}

// This shows the HTML page in "ui.html".
figma.showUI(__html__, { width: 400, height: 320, themeColors: true });

// init
updateUI();

figma.on("selectionchange", () => {
    updateUI();
});

// Logic to react to UI events
figma.ui.onmessage = async (msg: { type: string; count: number }) => {
    if (msg.type === "copy") {
        figma.notify("Copied to clipboard");
    }
};
