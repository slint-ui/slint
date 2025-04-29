import { create } from "zustand";
import { listenTS } from "./bolt-utils";

interface StoreState {
    title: string;
    slintSnippet: string;
    setTitle: (title: string) => void;
    initializeEventListeners: () => void;
}

export const useInspectorStore = create<StoreState>()((set) => ({
    title: "",
    slintSnippet: "",
    setTitle: (title) => set({ title }),
    initializeEventListeners: () => {

        listenTS("updatePropertiesCallback", (res) => {
            set({ title: res.title, slintSnippet: res.slintSnippet || "" });
        });
    },
}));