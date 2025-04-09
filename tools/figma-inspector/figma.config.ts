// Copyright © Hyper Brew LLC
// SPDX-License-Identifier: MIT
// cSpell: ignore codegen prefs

import type { FigmaConfig, PluginManifest } from "vite-figma-plugin/lib/types";
import { version } from "./package.json";

export const manifest: PluginManifest = {
    name: "Figma to Slint",
    id: "1474418299182276871",
    api: "1.0.0",
    main: "code.js",
    ui: "index.html",
    editorType: ["figma", "dev"],
    capabilities: ["codegen", "vscode"],
    codegenLanguages: [{ label: "Slint", value: "slint" }],
    codegenPreferences: [],
    networkAccess: {
        allowedDomains: ["https://cdnjs.cloudflare.com"],
    },
    documentAccess: "dynamic-page",
};

const extraPrefs = {
    copyZipAssets: ["public-zip/*"],
};

export const config: FigmaConfig = {
    manifest,
    version,
    ...extraPrefs,
};
