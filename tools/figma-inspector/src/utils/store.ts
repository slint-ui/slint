// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { create } from "zustand";
import { dispatchTS, listenTS } from "./bolt-utils";
import { downloadZipFile, writeTextToClipboard } from "./utils.js";

interface StoreState {
    title: string;
    slintSnippet: string;
    useVariables: boolean;
    exportsAreCurrent: boolean;
    exportedFiles: Array<{ name: string; content: string }>;
    exportAsSingleFile: boolean;
    menuOpen: boolean;
    setTitle: (title: string) => void;
    initializeEventListeners: () => void;
    copyToClipboard: () => void;
    setUseVariables: (useVariables: boolean) => void;
    setExportsAreCurrent: (exportsAreCurrent: boolean) => void;
    setExportAsSingleFile: (exportAsSingleFile: boolean) => void;
    exportFilesHandler: (
        files: Array<{ name: string; content: string }>,
    ) => Promise<void>;
    setMenuOpen: (menuOpen: boolean) => void;
    toggleMenu: () => void;
    exportFiles: () => void;
    startVariableCheckInterval: () => void;
}

export const useInspectorStore = create<StoreState>()((set, get) => ({
    // Default store values
    title: "",
    slintSnippet: "",
    useVariables: false,
    exportsAreCurrent: false,
    exportedFiles: [],
    exportAsSingleFile: false,
    menuOpen: false,

    setTitle: (title) => set({ title }),

    initializeEventListeners: () => {
        listenTS("updatePropertiesCallback", (res) => {
            set({ title: res.title, slintSnippet: res.slintSnippet || "" });
        });

        listenTS("selectionChangedInFigma", () => {
            dispatchTS("generateSnippetRequest", {
                useVariables: get().useVariables,
            });
        });

        listenTS("exportedFiles", (res) => {
            get().exportFilesHandler(res.files);
        });

        // On first run check to see if anything is currently selected and show a snippet.
        dispatchTS("generateSnippetRequest", {
            useVariables: get().useVariables,
        });

        get().startVariableCheckInterval();
    },

    copyToClipboard: () => {
        const success = writeTextToClipboard(get().slintSnippet);

        if (success) {
            dispatchTS("copyToClipboard", {
                result: true,
            });
        } else {
            dispatchTS("copyToClipboard", {
                result: false,
            });
        }
    },

    setUseVariables: (useVariables) => {
        set({ useVariables });
        dispatchTS("generateSnippetRequest", { useVariables });
    },

    setExportsAreCurrent: (exportsAreCurrent) => {
        set({ exportsAreCurrent });
    },

    setExportAsSingleFile: (exportAsSingleFile) => {
        set({ exportAsSingleFile });
    },

    setMenuOpen: (menuOpen) => {
        set({ menuOpen });
    },

    toggleMenu: () => {
        set({ menuOpen: !get().menuOpen });
    },

    exportFiles: () => {
        set({ exportedFiles: [], exportsAreCurrent: false, menuOpen: false });
        dispatchTS("exportToFiles", {
            exportAsSingleFile: get().exportAsSingleFile,
        });
    },

    exportFilesHandler: async (files) => {
        if (files && Array.isArray(files) && files.length > 0) {
            set({ exportedFiles: files, exportsAreCurrent: true });

            await downloadZipFile(files);
        } else {
            console.error("Invalid or empty files data received:", files);
            set({ exportedFiles: [], exportsAreCurrent: false }); // Mark as not current if export failed to produce files
        }
    },
    startVariableCheckInterval: () => {
       setInterval(() => {
            dispatchTS("checkVariableChanges", {});
        }, 5000);
    },
}));
