// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { EditorView, basicSetup } from "codemirror";
import { EditorState, Compartment } from "@codemirror/state";
import { javascript } from "@codemirror/lang-javascript";
import type { FileMap, PlaygroundFile } from "./files";

type ChangeListener = (path: string) => void;

/** A non-file tab (e.g. "Logs") rendered alongside the file tabs. */
export interface VirtualTab {
    /** Unique key, e.g. "(logs)". Prefixed with parens to avoid colliding with file paths. */
    id: string;
    label: string;
    /**
     * The element to show in the editor area when this tab is active.
     * Hidden by default; the EditorUi flips `display` between this and the
     * CodeMirror editor as the user switches tabs.
     */
    element: HTMLElement;
}

/** Owns the tab strip and the CodeMirror editor. */
export class EditorUi {
    #files: FileMap;
    #virtualTabs: VirtualTab[] = [];
    /** Either a file path (string) or a virtual tab id (`(...)`). */
    #openTabs: string[] = [];
    #activeTab: string | null = null;
    #editor: EditorView;
    #langCompartment = new Compartment();
    #tabsEl: HTMLDivElement;
    #editorEl: HTMLDivElement;
    #onChange: ChangeListener;

    constructor(
        files: FileMap,
        elements: { tabs: HTMLDivElement; editor: HTMLDivElement },
        onChange: ChangeListener,
    ) {
        this.#files = files;
        this.#tabsEl = elements.tabs;
        this.#editorEl = elements.editor;
        this.#onChange = onChange;

        this.#editor = new EditorView({
            parent: this.#editorEl,
            state: this.#emptyState(),
        });
    }

    /**
     * Drop all open file tabs and load a fresh file map (virtual tabs are
     * preserved). Used when the user switches to another demo.
     */
    reset(files: FileMap): void {
        this.#files = files;
        this.#openTabs = this.#openTabs.filter((id) =>
            this.#virtualTabs.some((v) => v.id === id),
        );
        this.#activeTab = null;
        this.#editor.setState(this.#emptyState());
        this.#renderTabs();
    }

    /** Register a virtual tab; the tab strip is re-rendered. */
    addVirtualTab(tab: VirtualTab): void {
        this.#virtualTabs.push(tab);
        if (!this.#openTabs.includes(tab.id)) {
            this.#openTabs.push(tab.id);
        }
        this.#renderTabs();
    }

    /** Open a file as a tab and optionally activate it (default true). */
    openFile(path: string, activate = true): void {
        const file = this.#files.get(path);
        if (!file) return;
        if (!this.#openTabs.includes(path)) {
            this.#openTabs.push(path);
        }
        if (activate) {
            this.#activate(path);
        } else {
            this.#renderTabs();
        }
    }

    /** Make a tab active by id (file path or virtual id). */
    activate(id: string): void {
        this.#activate(id);
    }

    #activate(id: string): void {
        const virt = this.#virtualTabs.find((t) => t.id === id);
        const file = this.#files.get(id);
        if (!virt && !file) return;
        this.#activeTab = id;

        if (virt) {
            this.#editorEl.classList.add("hidden");
            for (const t of this.#virtualTabs) {
                t.element.classList.toggle("visible", t.id === id);
            }
        } else if (file) {
            for (const t of this.#virtualTabs) {
                t.element.classList.remove("visible");
            }
            this.#editorEl.classList.remove("hidden");
            this.#editor.setState(this.#stateFor(file));
            this.#editor.focus();
        }

        this.#renderTabs();
    }

    #stateFor(file: PlaygroundFile): EditorState {
        const language =
            file.language === "javascript" ? [javascript()] : [];
        return EditorState.create({
            doc: file.content,
            extensions: [
                basicSetup,
                this.#langCompartment.of(language),
                EditorView.updateListener.of((update) => {
                    if (
                        update.docChanged &&
                        this.#activeTab === file.relativePath
                    ) {
                        file.content = update.state.doc.toString();
                        file.dirty = true;
                        this.#renderTabs();
                        this.#onChange(file.relativePath);
                    }
                }),
                EditorView.theme({
                    "&": { height: "100%" },
                    ".cm-scroller": { fontFamily: "ui-monospace, monospace" },
                }),
            ],
        });
    }

    #emptyState(): EditorState {
        return EditorState.create({
            doc: "// Open a file in a tab to edit it.",
            extensions: [basicSetup, EditorState.readOnly.of(true)],
        });
    }

    #renderTabs(): void {
        this.#tabsEl.innerHTML = "";
        for (const id of this.#openTabs) {
            const tab = document.createElement("div");
            tab.className = "tab";
            if (id === this.#activeTab) tab.classList.add("active");

            const label = document.createElement("span");
            const virt = this.#virtualTabs.find((t) => t.id === id);
            if (virt) {
                tab.classList.add("virtual");
                label.textContent = virt.label;
                tab.title = virt.label;
            } else {
                const file = this.#files.get(id);
                if (!file) continue;
                if (file.dirty) tab.classList.add("dirty");
                tab.title = id;
                const slash = id.lastIndexOf("/");
                label.textContent = slash < 0 ? id : id.substring(slash + 1);
            }
            label.addEventListener("click", () => this.#activate(id));
            tab.appendChild(label);
            this.#tabsEl.appendChild(tab);
        }
    }
}
