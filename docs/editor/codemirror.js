// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { EditorState } from '@codemirror/state';
import { highlightSelectionMatches } from '@codemirror/search';
import { indentWithTab, history, defaultKeymap, historyKeymap } from '@codemirror/commands';
import { foldGutter, indentOnInput, indentUnit, bracketMatching, foldKeymap, syntaxHighlighting, defaultHighlightStyle } from '@codemirror/language';
import { closeBrackets, autocompletion, closeBracketsKeymap, completionKeymap } from '@codemirror/autocomplete';
import { lineNumbers, highlightActiveLineGutter, highlightSpecialChars, drawSelection, dropCursor, rectangularSelection, crosshairCursor, highlightActiveLine, keymap, EditorView } from '@codemirror/view';

// Theme
import { dracula } from '@uiw/codemirror-theme-dracula';

// Language
import { javascript } from '@codemirror/lang-javascript';
import { python } from '@codemirror/lang-python';
import { rust } from '@codemirror/lang-rust';
import { cpp } from '@codemirror/lang-cpp';

// Initialize CodeMirror based on the language passed as a data attribute
window.initCodeMirror = function (editorDiv, language, content) {
    new EditorView({
        state: EditorState.create({
            doc: content,
            extensions: [
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
                python(),
                javascript(),
                rust(),
                cpp(),
                dracula,
            ],
        }),
        parent: editorDiv
    });
};

document.addEventListener("DOMContentLoaded", function () {
    // Find all the divs that need a CodeMirror editor
    document.querySelectorAll('.codemirror-editor').forEach(function (editorDiv) {
        const editorContent = editorDiv.querySelector('.codemirror-content');
        const language = editorDiv.getAttribute('data-lang');
        const content = editorContent.textContent.trim();
        window.initCodeMirror(editorDiv, language, content);
    });
});
