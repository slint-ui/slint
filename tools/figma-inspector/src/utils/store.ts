import { create } from "zustand";
import { dispatchTS, listenTS } from "./bolt-utils";
import { writeTextToClipboard } from "./utils.js";

interface StoreState {
    title: string;
    slintSnippet: string;
    setTitle: (title: string) => void;
    initializeEventListeners: () => void;
    copyToClipboard: () => Promise<void>;
}

export const useInspectorStore = create<StoreState>()((set, get) => ({
    title: "",
    slintSnippet: "",
    setTitle: (title) => set({ title }),
    initializeEventListeners: () => {
        listenTS("updatePropertiesCallback", (res) => {
            set({ title: res.title, slintSnippet: res.slintSnippet || "" });
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
}));
