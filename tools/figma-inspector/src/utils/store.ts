// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { create } from "zustand";
import { dispatchTS, listenTS } from "./bolt-utils";
import { writeTextToClipboard } from "./utils.js";

interface StoreState {
    title: string;
    slintSnippet: string;
    useVariables: boolean;
    setTitle: (title: string) => void;
    initializeEventListeners: () => void;
    copyToClipboard: () => Promise<void>;
    setUseVariables: (useVariables: boolean) => void;
}

export const useInspectorStore = create<StoreState>()((set, get) => ({
    title: "",
    slintSnippet: "",
    useVariables: false,

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
}));
