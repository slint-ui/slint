// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { EditorState } from "@codemirror/state";
import { highlightSelectionMatches } from "@codemirror/search";
import {
    indentWithTab,
    history,
    defaultKeymap,
    historyKeymap,
} from "@codemirror/commands";
import {
    foldGutter,
    indentOnInput,
    indentUnit,
    bracketMatching,
    foldKeymap,
    syntaxHighlighting,
    defaultHighlightStyle,
} from "@codemirror/language";
import {
    closeBrackets,
    autocompletion,
    closeBracketsKeymap,
    completionKeymap,
} from "@codemirror/autocomplete";
import {
    lineNumbers,
    highlightActiveLineGutter,
    highlightSpecialChars,
    drawSelection,
    rectangularSelection,
    crosshairCursor,
    highlightActiveLine,
    keymap,
    EditorView,
    showPanel,
} from "@codemirror/view";

// Theme
import { dracula } from "@uiw/codemirror-theme-dracula";

// Language
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { cpp } from "@codemirror/lang-cpp";
import { languageNameFacet } from "./language-facets";

const editor_url = "https://snapshots.slint.dev/master/editor/";
const wasm_url =
    "https://snapshots.slint.dev/master/wasm-interpreter/slint_wasm_interpreter.js";
let slint_wasm_module = null;
// keep them alive
var all_instances = new Array();

// Function to create the Copy button and add it to the panel
function createCopyButton(view) {
    const button = document.createElement("button");
    button.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" class="icon icon-tabler icon-tabler-copy" width="24" height="24" viewBox="0 0 24 24" stroke-width="1.5" stroke="#000000" fill="none" stroke-linecap="round" stroke-linejoin="round">
  <title>Copy</title>
  <path stroke="none" d="M0 0h24v24H0z" fill="none"/>
  <rect x="8" y="8" width="12" height="12" rx="2" />
  <path d="M16 8v-2a2 2 0 0 0 -2 -2h-8a2 2 0 0 0 -2 2v8a2 2 0 0 0 2 2h2" />
</svg>`;
    button.style.marginRight = "10px";

    button.onclick = () => {
        const content = view.state.doc.toString();
        navigator.clipboard.writeText(content).then(
            () => {
                alert("Content copied to clipboard!");
            },
            (err) => {
                console.error("Could not copy text: ", err);
            },
        );
    };

    return button;
}

// Function to create the Run/Preview button and add it to the panel
function createRunButton(view) {
    const button = document.createElement("button");
    button.innerHTML = `<svg width="24" height="24" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg">
    <title>Open in SlintPad</title>
<path d="M20.6632 55.828L48.7333 37.0309C48.7333 37.0309 50 36.3278 50 35.2182C50 33.7406 48.3981 33.2599 48.3981 33.2599L32.9557 27.355C32.4047 27.1462 31.6464 27.7312 32.3564 28.4728L37.4689 33.4165C37.4689 33.4165 38.889 34.765 38.889 35.6494C38.889 36.5338 38.017 37.322 38.017 37.322L19.4135 54.6909C18.7517 55.3089 19.6464 56.4294 20.6632 55.828Z" fill="#2379F4"/>
<path d="M43.3368 8.17339L15.2667 26.9677C15.2667 26.9677 14 27.6708 14 28.7804C14 30.258 15.6019 30.7387 15.6019 30.7387L31.0443 36.6464C31.5953 36.8524 32.3565 36.2674 31.6436 35.5286L26.5311 30.5684C26.5311 30.5684 25.111 29.2226 25.111 28.3355C25.111 27.4483 25.983 26.6628 25.983 26.6628L44.5752 9.30769C45.2483 8.68973 44.3565 7.56916 43.3368 8.17339Z" fill="#2379F4"/>
</svg>`;

    button.onclick = () => {
        const content = view.state.doc.toString();
        window.open(
            `${editor_url}?snippet=${encodeURIComponent(content)}`,
            "_blank",
        );
    };

    return button;
}

// Define the status panel with copy and run buttons
function statusPanel(view) {
    const dom = document.createElement("div");
    dom.className = "cm-status-panel";

    // Add the buttons to the panel
    const copyButton = createCopyButton(view);
    dom.appendChild(copyButton);

    const language = view.state.facet(languageNameFacet);
    if (language === "slint") {
        const runButton = createRunButton(view);
        dom.appendChild(runButton);
    }

    return {
        dom,
        update(_update) {
            // You can update the panel content based on editor state changes if needed
        },
    };
}

// Debounce function to limit how often updates are made
function debounce(func, wait) {
    let timeout;
    return (...args) => {
        clearTimeout(timeout);
        timeout = setTimeout(() => func.apply(this, args), wait);
    };
}

async function updateWasmPreview(previewContainer, content) {
    const { component, error_string } =
        await slint_wasm_module.compile_from_string(content, "");
    var error_div = previewContainer.parentNode.querySelector(".error-status");
    if (error_string !== "") {
        var text = document.createTextNode(error_string);
        var p = document.createElement("pre");
        p.appendChild(text);
        error_div.innerHTML =
            "<pre style='color: red; background-color:#fee; margin:0'>" +
            p.innerHTML +
            "</pre>";
    } else {
        error_div.innerHTML = "";
    }
    if (component !== undefined) {
        const canvas_id = previewContainer.getAttribute("data-canvas-id");
        const instance = await component.create(canvas_id);
        await instance.show();
        all_instances.push(instance);
    }
}

// Wrap updateWasmPreview in a debounce function (500ms delay)
const debouncedUpdateWasmPreview = debounce(updateWasmPreview, 500);

function initializePreviewContainers(previewContainer, _content) {
    const canvas_id = "canvas_" + Math.random().toString(36).substring(2, 9);
    const canvas = document.createElement("canvas");
    canvas.id = canvas_id;
    previewContainer.appendChild(canvas);
    previewContainer.setAttribute("data-canvas-id", `${canvas_id}`);
    const error_div = document.createElement("div");
    error_div.classList.add("error-status");
    previewContainer.parentNode.appendChild(error_div);
}

async function loadSlintWasmInterpreter(_editor) {
    try {
        if (slint_wasm_module) {
            return;
        }

        // Dynamically import the Slint WASM module
        slint_wasm_module = await import(wasm_url);
        await slint_wasm_module.default(); // Wait for WASM to initialize

        try {
            slint_wasm_module.run_event_loop(); // Run the event loop, which will trigger an exception
        } catch (e) {
            // Swallow the expected JavaScript exception that breaks out of Rust's event loop
        }

        return;
    } catch (error) {
        console.error(
            "Error during Slint WASM interpreter initialization:",
            error,
        );
        throw error; // Re-throw error to handle it in the calling context
    }
}

// Initialize CodeMirror based on the language passed as a data attribute
window.initCodeMirror = function (editorDiv, language, content) {
    // const editorDiv_id = editorDiv.getAttribute("id");

    const extensions = [
        lineNumbers(),
        highlightActiveLineGutter(),
        highlightSpecialChars(),
        history(),
        foldGutter(),
        drawSelection(),
        indentUnit.of("  "),
        EditorState.allowMultipleSelections.of(true),
        indentOnInput(),
        bracketMatching(),
        closeBrackets(),
        autocompletion(),
        rectangularSelection(),
        crosshairCursor(),
        highlightActiveLine(),
        highlightSelectionMatches(),
        keymap.of([
            indentWithTab,
            ...closeBracketsKeymap,
            ...defaultKeymap,
            ...historyKeymap,
            ...foldKeymap,
            ...completionKeymap,
        ]),
        syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
        languageNameFacet.of(language),
        dracula,
        showPanel.of(statusPanel),
    ];

    // Get the appropriate language extension
    let isReadOnly = true;
    let previewContainer;

    switch (language.toLowerCase()) {
        case "javascript":
            extensions.push(javascript());
            break;
        case "python":
            extensions.push(python());
            break;
        case "cpp":
            extensions.push(cpp());
            break;
        case "rust":
            extensions.push(rust());
            break;
        case "slint":
            isReadOnly = false;
            extensions.push(javascript());
            if (
                editorDiv.getAttribute("data-readonly") === "true" ||
                editorDiv.getAttribute("data-ignore") === "true"
            ) {
                break;
            }
            previewContainer = document.createElement("div");
            previewContainer.classList.add("preview-container");
            editorDiv.classList.add("show-preview");
            extensions.push(
                EditorView.updateListener.of((editor) => {
                    if (editor.docChanged) {
                        const newContent = editor.state.doc.toString();
                        debouncedUpdateWasmPreview(
                            previewContainer,
                            newContent,
                        );
                    }
                }),
            );
            break;
        default:
    }

    extensions.push(EditorView.editable.of(!isReadOnly));

    const editor = new EditorView({
        state: EditorState.create({
            doc: content,
            extensions: extensions,
        }),
        parent: editorDiv,
    });

    if (previewContainer) {
        editorDiv.append(previewContainer);
        loadSlintWasmInterpreter(editor)
            .then(async () => {
                initializePreviewContainers(previewContainer, content);
                await updateWasmPreview(previewContainer, content);
            })
            .catch((error) => {
                console.error("Error loading Slint WASM interpreter:", error);
            });
    }
};

document.addEventListener("DOMContentLoaded", () => {
    // Find all the divs that need a CodeMirror editor
    document
        .querySelectorAll(".codemirror-editor")
        .forEach(function (editorDiv) {
            const editorContent = editorDiv.querySelector(
                ".codemirror-content",
            );
            const language = editorDiv.getAttribute("data-lang");
            const content = editorContent.textContent.trim();
            window.initCodeMirror(editorDiv, language, content);
        });
});
