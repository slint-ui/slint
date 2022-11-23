// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { GotoPositionCallback } from "./text";
import { LspPosition, LspURI } from "./lsp_integration";

import { PropertiesView } from "slint-editor-shared/properties";
import {
    change_property,
    query_properties,
} from "slint-editor-shared/properties_client";

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import { BaseLanguageClient } from "vscode-languageclient";

export class PropertiesWidget extends Widget {
    #language_client_getter: () => BaseLanguageClient | null;
    #language_client: BaseLanguageClient | null = null;

    #onGotoPosition: GotoPositionCallback = (_u, _p) => {
        return;
    };
    #propertiesView: PropertiesView;

    constructor(language_client_getter: () => BaseLanguageClient | null) {
        const node = PropertiesView.createNode();
        super({ node: node });
        this.setFlag(Widget.Flag.DisallowLayout);
        this.addClass("content");
        this.addClass("properties-editor".toLowerCase());
        this.title.label = "Properties";
        this.title.closable = true;
        this.title.caption = `Element Properties`;

        this.#propertiesView = new PropertiesView(
            node,
            (doc, element, property_name, value, dry_run) => {
                return change_property(
                    this.language_client,
                    doc,
                    element,
                    property_name,
                    value,
                    dry_run,
                );
            },
        );

        this.#language_client_getter = language_client_getter;

        this.#propertiesView.property_clicked = (uri, _, p) => {
            if (p.defined_at != null) {
                this.#onGotoPosition(uri, p.defined_at.expression_range);
            }
        };
    }

    private get language_client(): BaseLanguageClient | null {
        if (this.#language_client == null) {
            this.#language_client = this.#language_client_getter();
        }
        return this.#language_client;
    }

    position_changed(uri: LspURI, _version: number, position: LspPosition) {
        query_properties(this.language_client, uri, position, (r) => {
            this.#propertiesView.set_properties(r);
        });
    }

    protected onCloseRequest(msg: Message): void {
        super.onCloseRequest(msg);
        this.dispose();
    }

    set on_goto_position(callback: GotoPositionCallback) {
        this.#onGotoPosition = callback;
    }
}
