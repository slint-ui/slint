// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { GotoPositionCallback } from "./text";
import { LspPosition, LspURI } from "./lsp_integration";

import { PropertyQuery, PropertiesView } from "./shared/properties";
import { change_property, query_properties } from "./properties_client";

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
        );

        this.#propertiesView.property_clicked = (uri, _, p) => {
            if (p.defined_at != null) {
                this.#onGotoPosition(uri, p.defined_at.expression_range);
            }
        };
    }

    set_language_client(client: BaseLanguageClient | null) {
        this.#language_client = client ?? this.#language_client;
    }

    position_changed(uri: LspURI, version: number, position: LspPosition) {
        query_properties(this.#language_client, uri, position)
            .then((r: PropertyQuery) => {
                this.#propertiesView.set_properties(r);
            })
            .catch(() => {
                // Document has not loaded yet!
                setTimeout(
                    () => this.position_changed(uri, version, position),
                    1000,
                );
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
