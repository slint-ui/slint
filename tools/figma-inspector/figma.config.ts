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
    codegenLanguages: [{ label: "Typescript", value: "typescript" }],
    codegenPreferences: [
        {
            itemType: "select",
            propertyName: "useVariables",
            label: "Use Variables",
            options: [
                { label: "Yes", value: "true" },
                { label: "No", value: "false", isDefault: true },
            ],
            includedLanguages: ["typescript"],
        },
    ],
    networkAccess: {
        allowedDomains: ["none"],
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
