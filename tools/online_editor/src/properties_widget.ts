// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { GotoPositionCallback, ReplaceTextFunction, TextRange } from "./text";
import { lsp_range_to_editor_range } from "./lsp_integration";

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import { PropertyQuery, DefinitionPosition } from "./shared/properties";

import { PropertiesView } from "./shared/properties";

function editor_definition_range(
    uri: string,
    def_pos: DefinitionPosition | null,
): TextRange | null {
    if (def_pos == null) {
        return null;
    }
    return lsp_range_to_editor_range(uri, def_pos.expression_range);
}

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
            const expression_range = editor_definition_range(uri, p.defined_at);
            if (expression_range != null) {
                this.#onGotoPosition(uri, expression_range);
            }
        };
        this.#propertiesView.change_property = (
            uri,
            p,
            current_text,
            code_text,
        ) => {
            const expression_range = editor_definition_range(uri, p.defined_at);
            if (expression_range != null) {
                this.replace_property_value(
                    uri,
                    expression_range,
                    current_text,
                    (old_text) => old_text == code_text,
                );
            }
        };
    }

    private replace_property_value(
        uri: string,
        range: TextRange,
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
