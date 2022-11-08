// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import {
    Position as LspPosition,
    Range as LspRange,
    URI as LspURI,
} from "vscode-languageserver-types";

export interface DeclarationPosition {
    uri: LspURI;
    start_position: LspPosition;
}

export interface DefinitionPosition {
    property_definition_range: LspRange;
    expression_range: LspRange;
}

export interface Property {
    name: string;
    group: string;
    type_name: string;
    declared_at: DeclarationPosition | null;
    defined_at: DefinitionPosition | null;
}

export interface Element {
    id: string;
    type_name: string;
    range: LspRange | null;
}

export interface BindingTextProvider {
    binding_text(_location: DefinitionPosition): string;
}

export interface PropertyQuery {
    source_uri: string;
    element: Element | null;
    properties: Property[];
}

const TYPE_PATTERN = /^[a-z-]+$/i;

let FOCUSING = false;

function type_class_for_typename(name: string): string {
    if (name === "callback" || name.slice(0, 9) === "callback(") {
        return "type-callback";
    }
    if (name.slice(0, 5) === "enum ") {
        return "type-enum";
    }
    if (name.slice(0, 9) === "function(") {
        return "type-function";
    }
    if (name === "element ref") {
        return "type-element-ref";
    }
    if (TYPE_PATTERN.test(name)) {
        return "type-" + name;
    }
    return "type-unknown";
}

export class PropertiesView {

    static createNode(): HTMLElement {
        const node = document.createElement("div");
        const content = document.createElement("div");
        node.appendChild(content);

        const header = document.createElement("div");
        header.className = "element-header";
        const element_type = document.createElement("div");
        element_type.className = "element-type";
        const element_id = document.createElement("div");
        element_id.className = "element-id";
        header.appendChild(element_type);
        header.appendChild(element_id);

        const table = document.createElement("table");
        table.className = "properties-table";

        content.appendChild(header);
        content.appendChild(table);

        return node;
    }

    constructor(node: HTMLElement) {
        this.node = node;
        this.set_header(null);
    }

    node: HTMLElement;
    /// Callback called when the property is clicked
    property_clicked: (_uri: LspURI, _p: Property) => void = (_) => { /**/ };
    /// Callback called when the property is modified. The old value is given so it can be checked
    change_property: (_uri: LspURI, _p: Property, _new_value: string, _old_value: string) => void = (_) => { /**/ };

    protected get contentNode(): HTMLDivElement {
        return this.node.getElementsByTagName("div")[0] as HTMLDivElement;
    }
    protected get headerNode(): HTMLDivElement {
        return this.contentNode.getElementsByTagName("div")[0] as HTMLDivElement;
    }
    protected get elementTypeNode(): HTMLDivElement {
        return this.headerNode.getElementsByTagName("div")[0] as HTMLDivElement;
    }
    protected get elementIdNode(): HTMLDivElement {
        return this.headerNode.getElementsByTagName("div")[1] as HTMLDivElement;
    }
    protected get tableNode(): HTMLTableElement {
        return this.contentNode.getElementsByTagName(
            "table",
        )[0] as HTMLTableElement;
    }

    private set_header(element: Element | null) {
        if (element == null) {
            this.elementTypeNode.innerText = "<Unknown>";
            this.elementIdNode.innerText = "";
        } else {
            this.elementTypeNode.innerText = element.type_name;
            this.elementIdNode.innerText = element.id;
        }
    }

    private populate_table(
        binding_text_provider: BindingTextProvider,
        properties: Property[],
        uri: LspURI,
    ) {
        const table = this.tableNode;

        let current_group = "";

        table.innerHTML = "";

        for (const p of properties) {
            if (p.group !== current_group) {
                const group_header = document.createElement("tr");
                group_header.className = "group-header";

                const group_cell = document.createElement("td");
                group_cell.innerText = p.group;
                group_cell.setAttribute("colspan", "2");
                current_group = p.group;

                group_header.appendChild(group_cell);
                table.appendChild(group_header);
            }
            const row = document.createElement("tr");
            row.className = "property";
            if (p.declared_at == null) {
                row.classList.add("builtin");
            }
            if (p.defined_at == null) {
                row.classList.add("undefined");
            }

            const goto_property = () => {
                this.property_clicked(uri, p)
            }

            const name_field = document.createElement("td");
            name_field.className = "name-column";
            name_field.innerText = p.name;
            name_field.addEventListener("click", goto_property);
            row.appendChild(name_field);

            const value_field = document.createElement("td");
            value_field.className = "value-column";
            value_field.classList.add(type_class_for_typename(p.type_name));
            value_field.setAttribute("title", p.type_name);
            const input = document.createElement("input");
            input.type = "text";
            if (p.defined_at != null) {
                const code_text = binding_text_provider.binding_text(p.defined_at);
                input.value = code_text;
                const changed_class = "value-changed";
                input.addEventListener("focus", (_) => {
                    if (FOCUSING) {
                        FOCUSING = false;
                    } else {
                        FOCUSING = true;
                        goto_property();
                        input.focus();
                    }
                });
                input.addEventListener("input", (_) => {
                    const current_text = input.value;
                    if (current_text != code_text) {
                        input.classList.add(changed_class);
                    } else {
                        input.classList.remove(changed_class);
                    }
                });
                input.addEventListener("change", (_) => {
                    const current_text = input.value;
                    if (current_text != code_text) {
                        this.change_property(uri, p, current_text, code_text);
                    }
                });
            } else {
                input.disabled = true;
            }
            value_field.appendChild(input);
            row.appendChild(value_field);

            table.appendChild(row);
        }
    }

    set_properties(
        binding_text_provider: BindingTextProvider,
        properties: PropertyQuery,
    ) {
        this.set_header(properties.element);
        this.populate_table(
            binding_text_provider,
            properties.properties,
            properties.source_uri,
        );
    }
}
