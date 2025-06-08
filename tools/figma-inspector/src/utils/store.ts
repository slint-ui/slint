// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { create } from "zustand";
import { dispatchTS, listenTS } from "./bolt-utils";
import { downloadZipFile, writeTextToClipboard } from "./utils.js";

export enum ExportType {
    SeparateFiles,
    SingleFile,
}

interface StoreState {
    title: string;
    slintSnippet: string;
    useVariables: boolean;
    exportsAreCurrent: boolean;
    exportedFiles: Array<{ name: string; content: string }>;
    devMode: boolean;
    isExporting: boolean;
    setTitle: (title: string) => void;
    initializeEventListeners: () => void;
    copyToClipboard: () => void;
    setUseVariables: (useVariables: boolean) => void;
    setExportsAreCurrent: (exportsAreCurrent: boolean) => void;
    exportFilesHandler: (
        fileName: string,
        files: Array<{ name: string; content: string }>,
    ) => Promise<void>;
    exportFiles: (singleOrMultiple: ExportType) => void;
    getTestData: () => void;
    startVariableCheckInterval: () => void;
    resizeWindow: (width: number, height: number) => void;
    setDevMode: (devMode: boolean) => void;
    simpleExport: () => void;
}

export const useInspectorStore = create<StoreState>()((set, get) => ({
    // Default store values
    title: "",
    slintSnippet: "",
    useVariables: false,
    exportsAreCurrent: false,
    exportedFiles: [],
    exportAsSingleFile: false,
    devMode: false,
    isExporting: false,

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

        listenTS("nodeChanged", () => {
            dispatchTS("generateSnippetRequest", {
                useVariables: get().useVariables,
            });
        });

        listenTS("exportedFiles", (res) => {
            get().exportFilesHandler(res.zipFilename, res.files);
        });

        listenTS("saveTextFile", (res) => {
            set({ isExporting: false });
            saveTextFile(res.filename, res.content);
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

    exportFiles: (singleOrMultiple) => {
        set({ exportedFiles: [], exportsAreCurrent: false });
        dispatchTS("exportToFiles", {
            exportAsSingleFile:
                singleOrMultiple === ExportType.SingleFile ? true : false,
        });
    },

    exportFilesHandler: async (fileName, files) => {
        if (files && Array.isArray(files) && files.length > 0) {
            set({ exportedFiles: files, exportsAreCurrent: true });

            await downloadZipFile(fileName, files);
            set({ isExporting: false });
        } else {
            console.error("Invalid or empty files data received:", files);
            set({
                exportedFiles: [],
                exportsAreCurrent: false,
                isExporting: false,
            }); // Mark as not current if export failed to produce files
        }
    },

    getTestData: () => {
        set({ isExporting: true });
        dispatchTS("getTestData", {});
    },

    startVariableCheckInterval: () => {
        setInterval(() => {
            dispatchTS("checkVariableChanges", {});
        }, 5000);
    },

    resizeWindow: (width: number, height: number) => {
        dispatchTS("resizeWindow", { width, height });
    },

    setDevMode: (devMode) => {
        set({ devMode });
    },

    simpleExport: () => {
        set({ isExporting: true });
        dispatchTS("createSlintExport", {});
    },
}));

function saveTextFile(filename: string, text: string) {
    const element = document.createElement("a");
    element.setAttribute(
        "href",
        "data:text/plain;charset=utf-8," + encodeURIComponent(text),
    );
    element.setAttribute("download", filename);
    element.style.display = "none";
    document.body.appendChild(element);
    element.click();
    document.body.removeChild(element);
}
