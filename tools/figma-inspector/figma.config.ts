// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import type { FigmaConfig, PluginManifest } from "vite-figma-plugin/lib/types";
import { version } from "./package.json";

export const manifest: PluginManifest = {
    name: "Figma to Slint",
    id: "slint.figma.plugin",
    api: "1.0.0",
    main: "code.js",
    ui: "index.html",
    editorType: ["figma", "dev"],
    documentAccess: "dynamic-page",
    networkAccess: {
        allowedDomains: ["*"],
        reasoning: "For accessing remote assets",
    },
};

const extraPrefs = {
    copyZipAssets: ["public-zip/*"],
};

export const config: FigmaConfig = {
    manifest,
    version,
    ...extraPrefs,
};
