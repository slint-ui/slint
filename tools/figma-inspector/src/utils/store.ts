// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { create } from "zustand";
import { dispatchTS, listenTS } from "./bolt-utils";
import { writeTextToClipboard } from "./utils.js";

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
    copyToClipboard: () => Promise<void>;
    setUseVariables: (useVariables: boolean) => void;
    setExportsAreCurrent: (exportsAreCurrent: boolean) => void;
    setExportedFiles: (
        exportedFiles: Array<{ name: string; content: string }>,
    ) => void;
    setExportAsSingleFile: (exportAsSingleFile: boolean) => void;
    setMenuOpen: (menuOpen: boolean) => void;
    toggleMenu: () => void;
    exportFiles: () => void;
}

export const useInspectorStore = create<StoreState>()((set, get) => ({
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
            dispatchTS("generateSnippetRequest", { useVariables: get().useVariables });
        });
    },

    copyToClipboard: async () => {
        try {
            writeTextToClipboard(get().slintSnippet);
            dispatchTS("copyToClipboard", {
                result: true,
            });
        } catch (error) {
            dispatchTS("copyToClipboard", {
                result: false,
            });
        }
    },

    setUseVariables: (useVariables) => {
        set({ useVariables })
        dispatchTS("generateSnippetRequest", { useVariables });
    },

    setExportsAreCurrent: (exportsAreCurrent) => {
        set({ exportsAreCurrent })
    },

    setExportedFiles: (exportedFiles) => {
        set({ exportedFiles })
    },

    setExportAsSingleFile: (exportAsSingleFile) => {
        set({ exportAsSingleFile })
    },

    setMenuOpen: (menuOpen) => {
        set({ menuOpen })
    },

    toggleMenu: () => {
        set({ menuOpen: !get().menuOpen })
    },

    exportFiles: () => {
        set({ exportedFiles: [], exportsAreCurrent: false, menuOpen: false });
        dispatchTS("exportToFiles", { exportAsSingleFile: get().exportAsSingleFile });
    },
}));
