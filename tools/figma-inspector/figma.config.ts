// Copyright © Hyper Brew LLC
// SPDX-License-Identifier: MIT

import type { FigmaConfig, PluginManifest } from "vite-figma-plugin/lib/types";
import { version } from "./package.json";

export const manifest: PluginManifest = {
    name: "Figma to Slint",
    id: "slint.figma.plugin",
    api: "1.0.0",
    main: "code.js",
    ui: "index.html",
    editorType: ["dev"],
    capabilities: ["codegen", "vscode"],
    codegenLanguages: [
        {"label": "Slint", "value": "slint"},
    ],
    codegenPreferences: [
    ],
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
