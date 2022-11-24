// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import { MonacoLanguageClient } from "monaco-languageclient";
import { VersionedDocumentAndPosition, GotoPositionCallback } from "./text";
import { LspRange, LspPosition } from "./lsp_integration";

import {
    DocumentSymbolRequest,
    DocumentSymbolParams,
    DocumentSymbol,
    SymbolInformation,
} from "vscode-languageserver-protocol";

import { SymbolTag, SymbolKind } from "vscode-languageserver-types";

const SYMBOL_KIND_MAP = new Map<SymbolKind, string>([
    [SymbolKind.File, "kind-file"],
    [SymbolKind.Module, "kind-module"],
    [SymbolKind.Namespace, "kind-namespace"],
    [SymbolKind.Package, "kind-package"],
    [SymbolKind.Class, "kind-class"],
    [SymbolKind.Method, "kind-method"],
    [SymbolKind.Property, "kind-property"],
    [SymbolKind.Field, "kind-field"],
    [SymbolKind.Constructor, "kind-constructor"],
    [SymbolKind.Enum, "kind-enum"],
    [SymbolKind.Interface, "kind-interface"],
    [SymbolKind.Function, "kind-function"],
    [SymbolKind.Variable, "kind-variable"],
    [SymbolKind.Constant, "kind-constant"],
    [SymbolKind.String, "kind-string"],
    [SymbolKind.Number, "kind-number"],
    [SymbolKind.Boolean, "kind-boolean"],
    [SymbolKind.Array, "kind-array"],
    [SymbolKind.Object, "kind-object"],
    [SymbolKind.Key, "kind-key"],
    [SymbolKind.Null, "kind-null"],
    [SymbolKind.EnumMember, "kind-enum-member"],
    [SymbolKind.Struct, "kind-struct"],
    [SymbolKind.Event, "kind-event"],
    [SymbolKind.Operator, "kind-operator"],
    [SymbolKind.TypeParameter, "kind-type-parameter"],
]);

const ACTIVE_ELEMENT_CLASS = "active";

interface PositionData {
    range: LspRange;
    element: HTMLElement;
    children: PositionData[];
}

interface OutlineData {
    uri: string;
    version: number;
    data: PositionData[];
}

function set_data(
    data: DocumentSymbol[],
    parent: HTMLUListElement,
    uri: string,
    goto_position: GotoPositionCallback,
): PositionData[] {
    const pos_data = [];
    for (const d of data) {
        const row = document.createElement("li");
        row.className = "outline-element";
        // the deprecated flag is deprecated, so cast to any so that the check
        // works even if deprecated gets removed
        if (
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            (d as any).deprecated ||
            SymbolTag.Deprecated in (d.tags ?? [])
        ) {
            row.classList.add("deprecated");
        }
        row.classList.add(SYMBOL_KIND_MAP.get(d.kind) ?? "kind-unknown");

        const span = document.createElement("span");

        const current_pos_data = {
            range: d.range,
            element: span,
            children: [],
        } as PositionData;

        span.innerText = d.name;
        span.addEventListener("click", () =>
            goto_position(uri, d.selectionRange),
        );

        row.appendChild(span);

        if (d.children != null) {
            const children_parent = document.createElement("ul");
            current_pos_data.children = set_data(
                d.children,
                children_parent,
                uri,
                goto_position,
            );
            row.appendChild(children_parent);
        }

        pos_data.push(current_pos_data);
        parent.appendChild(row);
    }

    return pos_data;
}

function containsPosition(p: LspPosition, r: LspRange): boolean {
    if (
        p.line < r.start.line ||
        (p.line == r.start.line && p.character < r.start.character)
    ) {
        return false;
    }
    if (
        p.line > r.end.line ||
        (p.line == r.end.line && p.character >= r.end.character)
    ) {
        return false;
    }
    return true;
}

function deactivate_elements_and_find_to_activate(
    data: PositionData[],
    position: LspPosition,
): HTMLElement | null {
    let to_activate = null;
    for (const d of data) {
        d.element.classList.remove(ACTIVE_ELEMENT_CLASS);
        if (containsPosition(position, d.range)) {
            to_activate = d.element;
        }
        to_activate =
            deactivate_elements_and_find_to_activate(d.children, position) ??
            to_activate;
    }
    return to_activate;
}

export class OutlineWidget extends Widget {
    #language_client_getter: () => MonacoLanguageClient | null;
    #language_client: MonacoLanguageClient | null = null;
    #onGotoPosition: GotoPositionCallback = (_) => {
        return;
    };

    #outline: OutlineData | null = null;
    #cursor_position: VersionedDocumentAndPosition;
    #timer_id?: number = undefined;

    static createNode(): HTMLElement {
        const node = document.createElement("div");
        const content = document.createElement("div");
        node.appendChild(content);
        return node;
    }

    constructor(
        cursor_position: VersionedDocumentAndPosition,
        language_client_getter: () => MonacoLanguageClient | null,
    ) {
        super({ node: OutlineWidget.createNode() });
        this.#language_client_getter = language_client_getter;
        this.setFlag(Widget.Flag.DisallowLayout);
        this.addClass("content");
        this.addClass("outline");
        this.title.label = "Outline";
        this.title.closable = true;
        this.title.caption = `Document Outline`;

        this.position_changed(cursor_position);
        this.#cursor_position = cursor_position; // Actually also happens in position_changed...
    }

    set on_goto_position(callback: GotoPositionCallback) {
        this.#onGotoPosition = callback;
    }

    get language_client(): MonacoLanguageClient | null {
        if (this.#language_client == null) {
            const client = this.#language_client_getter();
            if (client != null) {
                this.#language_client = client;
            }
        }
        return this.#language_client;
    }

    query_symbols(uri: string, version: number) {
        const client = this.language_client;
        if (client == null) {
            return;
        }

        client
            .sendRequest(DocumentSymbolRequest.type, {
                textDocument: { uri: uri },
            } as DocumentSymbolParams)
            .then((r: DocumentSymbol[] | SymbolInformation[] | null) =>
                this.update_data(uri, version, r),
            );
    }

    position_changed(position: VersionedDocumentAndPosition) {
        const client = this.language_client;
        if (this.#timer_id != null) {
            clearTimeout(this.#timer_id);
            this.#timer_id = undefined;
        }

        if (client != null) {
            if (
                this.#outline == null ||
                position.uri != this.#outline.uri ||
                position.version != this.#outline.version
            ) {
                // Document has changed, and we have no new data yet!
                if (position.uri == "") {
                    this.clear_data();
                    this.set_error("No document");
                } else {
                    const version = position.version;
                    const uri = position.uri;
                    this.query_symbols(uri, version);
                }
            } else {
                deactivate_elements_and_find_to_activate(
                    this.#outline.data,
                    position.position,
                )?.classList.add(ACTIVE_ELEMENT_CLASS);
            }
        }

        this.#cursor_position = position;
    }

    protected get contentNode(): HTMLDivElement {
        return this.node.getElementsByTagName("div")[0] as HTMLDivElement;
    }

    protected update_data(
        uri: string,
        version: number,
        data: DocumentSymbol[] | SymbolInformation[] | null,
    ) {
        this.clear_data();

        if (data == null) {
            this.set_error("No data available yet");
            this.#timer_id = setTimeout(
                () => this.query_symbols(uri, version),
                1000,
            );
            return;
        }
        if (
            this.#outline != null &&
            this.#outline.uri == uri &&
            this.#outline.version >= version
        ) {
            // Data is outdated, ignore!
            return;
        }

        if (data.length > 0 && "location" in data[0]) {
            // location is a required key in SymbolInformation that does not exist in DocumentSymbol
            this.set_error("Invalid data format received");
            return;
        }
        const content = document.createElement("ul");
        content.className = "outline-tree";

        const pos_data = set_data(
            data as DocumentSymbol[],
            content,
            uri,
            this.#onGotoPosition,
        );

        this.#outline = { uri: uri, version: version, data: pos_data };
        this.position_changed(this.#cursor_position); // re-highlight the expected element:-)

        this.contentNode.appendChild(content);
    }

    protected clear_data() {
        this.contentNode.innerText = "";

        this.#outline = null;
    }

    protected set_error(message: string) {
        this.clear_data();
        this.contentNode.innerHTML = '<div class="error">' + message + "</div>";
    }

    protected onCloseRequest(msg: Message): void {
        super.onCloseRequest(msg);
        this.dispose();
    }
}
