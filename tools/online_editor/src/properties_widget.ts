// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { GotoPositionCallback } from "./text";
import { LspPosition, LspURI } from "./lsp_integration";

import { PropertyQuery, PropertiesView } from "./shared/properties";
import {
    change_property,
    query_properties,
    remove_binding,
} from "./properties_client";

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import { BaseLanguageClient } from "vscode-languageclient";

export class PropertiesWidget extends Widget {
    #language_client: BaseLanguageClient | null = null;

    #onGotoPosition: GotoPositionCallback = (_u, _p) => {
        return;
    };
    #propertiesView: PropertiesView;

    constructor() {
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
                    this.#language_client,
                    doc,
                    element,
                    property_name,
                    value,
                    dry_run,
                );
            },
            "fa fa-trash-o",
            (doc, element, property_name) => {
                return remove_binding(
                    this.#language_client,
                    doc,
                    element,
                    property_name,
                );
            },
            "fa fa-plus-square-o",
        );

        this.#propertiesView.property_clicked = (uri, _, p) => {
            if (p.defined_at != null) {
                this.#onGotoPosition(uri, p.defined_at.expression_range);
            }
        };
    }

    dispose(): void {
        super.dispose();
    }

    set_language_client(client: BaseLanguageClient | null) {
        if (client != null) {
            this.#language_client = client;
        }
    }

    position_changed(uri: LspURI, version: number, position: LspPosition) {
        query_properties(this.#language_client, uri, position)
            .then((r: PropertyQuery) => {
                if (r.source_version < version) {
                    setTimeout(() => {
                        this.position_changed(uri, version, position);
                    }, 100);
                    return;
                }
                this.#propertiesView.set_properties(r);
            })
            .catch(() => {
                this.#propertiesView.current_data_uri = uri.toString();
                this.#propertiesView.show_welcome("Data not yet available.");
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
