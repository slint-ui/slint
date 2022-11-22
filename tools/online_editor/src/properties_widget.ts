// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { GotoPositionCallback, ReplaceTextFunction } from "./text";
import { LspRange } from "./lsp_integration";

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import { PropertyQuery } from "./shared/properties";

import { PropertiesView } from "./shared/properties";

export class PropertiesWidget extends Widget {
    #onGotoPosition: GotoPositionCallback = (_u, _p) => {
        return;
    };
    #replaceText: ReplaceTextFunction = (_u, _r, _t, _v) => {
        return true;
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

        this.#propertiesView = new PropertiesView(node);

        this.#propertiesView.property_clicked = (uri, p) => {
            if (p.defined_at != null) {
                this.#onGotoPosition(uri, p.defined_at.expression_range);
            }
        };
        this.#propertiesView.change_property = (
            uri,
            p,
            current_text,
            code_text,
        ) => {
            if (p.defined_at != null) {
                this.replace_property_value(
                    uri,
                    p.defined_at.expression_range,
                    current_text,
                    (old_text) => old_text == code_text,
                );
            }
        };
    }

    private replace_property_value(
        uri: string,
        range: LspRange,
        new_value: string,
        validator: (_old: string) => boolean,
    ): boolean {
        return this.#replaceText(uri, range, new_value, validator);
    }

    protected onCloseRequest(msg: Message): void {
        super.onCloseRequest(msg);
        this.dispose();
    }

    set on_goto_position(callback: GotoPositionCallback) {
        this.#onGotoPosition = callback;
    }

    set replace_text_function(fn: ReplaceTextFunction) {
        this.#replaceText = fn;
    }

    set_properties(properties: PropertyQuery) {
        this.#propertiesView.set_properties(properties);
    }
}
