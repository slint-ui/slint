// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { GotoPositionCallback } from "./text";
import { LspPosition, LspURI } from "./lsp_integration";

import { PropertyQuery, PropertiesView } from "./shared/properties";
import { change_property, query_properties } from "./properties_client";

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import {
    BaseLanguageClient,
    HandleWorkDoneProgressSignature,
    ProgressToken,
    WorkDoneProgressBegin,
    WorkDoneProgressEnd,
    WorkDoneProgressReport,
} from "vscode-languageclient";

function extract_uri_from_progress_message(input: string): string {
    const start = input.indexOf(": ");
    const end = input.lastIndexOf("@");
    return input.slice(start + 2, end);
}

export class PropertiesWidget extends Widget {
    #language_client: BaseLanguageClient | null = null;

    #onGotoPosition: GotoPositionCallback = (_u, _p) => {
        return;
    };
    #propertiesView: PropertiesView;
    #current_position: LspPosition | null = null;

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

    dispose(): void {
        super.dispose();
    }

    set_language_client(client: BaseLanguageClient | null) {
        if (client != null) {
            this.#language_client = client;
            const work = client.middleware.handleWorkDoneProgress;
            const new_work = (
                token: ProgressToken,
                params:
                    | WorkDoneProgressBegin
                    | WorkDoneProgressReport
                    | WorkDoneProgressEnd,
                next: HandleWorkDoneProgressSignature,
            ) => {
                if (params.kind === "begin") {
                    this.data_stale(
                        extract_uri_from_progress_message(params.message || ""),
                    );
                } else if (params.kind === "end") {
                    this.data_valid(
                        extract_uri_from_progress_message(params.message || ""),
                    );
                }
                if (work != null) {
                    work(token, params, next);
                }
            };
            this.#language_client.middleware.handleWorkDoneProgress = new_work;
        }
    }

    data_stale(uri: string) {
        if (uri == this.#propertiesView.current_data_uri) {
            this.#propertiesView.show_welcome("Refreshing data");
        }
    }

    data_valid(uri: string) {
        if (
            uri == this.#propertiesView.current_data_uri &&
            this.#current_position
        ) {
            query_properties(this.#language_client, uri, this.#current_position)
                .then((r: PropertyQuery) => {
                    this.#propertiesView.set_properties(r);
                })
                .catch(() => {
                    this.#propertiesView.current_data_uri = uri.toString();
                    this.#propertiesView.show_welcome("loading...");
                });
        }
    }

    position_changed(uri: LspURI, _: number, position: LspPosition) {
        this.#current_position = position;
        query_properties(this.#language_client, uri, position)
            .then((r: PropertyQuery) => {
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
