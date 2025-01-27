// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import {
    getStore,
    setStore,
    listenTS,
    dispatchTS,
    getStatus,
    updateUI,
} from "./utils/code-utils";

figma.showUI(__html__, {
    themeColors: true,
    width: 400,
    height: 320,
});

listenTS("copyToClipboard", () => {
    figma.notify("Copied!");
});

figma.on("selectionchange", () => {
    updateUI();
});

// init
updateUI();
