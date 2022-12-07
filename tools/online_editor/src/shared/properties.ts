// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import {
    Diagnostic,
    OptionalVersionedTextDocumentIdentifier,
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
    expression_value: string;
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

export interface PropertyQuery {
    source_uri: string;
    source_version: number;
    element: Element | null;
    properties: Property[];
}

export interface SetBindingResponse {
    diagnostics: Diagnostic[];
}

export type PropertyClickedCallback = (
    _uri: LspURI,
    _v: number,
    _p: Property,
) => void;

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

export type BindingEditor = (
    _doc: OptionalVersionedTextDocumentIdentifier,
    _element: LspRange,
    _property_name: string,
    _new_value: string,
    _dry_run: boolean,
) => Promise<SetBindingResponse>;

export type BindingRemover = (
    _doc: OptionalVersionedTextDocumentIdentifier,
    _element: LspRange,
    _property_name: string,
) => Promise<boolean>;

export class PropertiesView {
    #current_data_uri = "";
    #current_data_version = -10000;
    #current_element_range: LspRange | null = null;
    #binding_editor: BindingEditor;
    #binding_remover: BindingRemover;
    #remover_icon_class: string;
    #adder_icon_class: string;

    get current_data_uri() {
        return this.#current_data_uri;
    }
    set current_data_uri(uri: string) {
        this.#current_data_uri = uri;
    }
    get current_data_version() {
        return this.#current_data_version;
    }

    static createNode(): HTMLElement {
        const node = document.createElement("div");
        const content = document.createElement("div");
        node.appendChild(content);

        const welcome = document.createElement("div");
        welcome.className = "welcome-page";
        welcome.style.display = "none";
        content.appendChild(welcome);

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

    constructor(
        node: HTMLElement,
        binding_editor: BindingEditor,
        remover_icon_class: string,
        binding_remover: BindingRemover,
        adder_icon_class: string,
    ) {
        this.#binding_editor = binding_editor;
        this.#remover_icon_class = remover_icon_class;
        this.#adder_icon_class = adder_icon_class;
        this.#binding_remover = binding_remover;
        this.node = node;
        this.show_welcome("Waiting for data from Slint LSP");
    }

    node: HTMLElement;
    /// Callback called when the property is clicked
    property_clicked: PropertyClickedCallback = () => {
        return;
    };

    protected get contentNode(): HTMLDivElement {
        return this.node.getElementsByTagName("div")[0] as HTMLDivElement;
    }

    protected get welcomeNode(): HTMLDivElement {
        return this.contentNode.getElementsByTagName(
            "div",
        )[0] as HTMLDivElement;
    }
    protected get headerNode(): HTMLDivElement {
        return this.contentNode.getElementsByTagName(
            "div",
        )[1] as HTMLDivElement;
    }
    protected get tableNode(): HTMLTableElement {
        return this.contentNode.getElementsByTagName(
            "table",
        )[0] as HTMLTableElement;
    }

    protected get elementTypeNode(): HTMLDivElement {
        return this.headerNode.getElementsByTagName("div")[0] as HTMLDivElement;
    }
    protected get elementIdNode(): HTMLDivElement {
        return this.headerNode.getElementsByTagName("div")[1] as HTMLDivElement;
    }

    private set_header(element: Element) {
        this.elementTypeNode.innerText = element.type_name;
        this.elementIdNode.innerText = element.id;
    }

    private populate_table(
        element_range: LspRange | null | undefined,
        properties: Property[],
        uri: LspURI,
        version: number,
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
                group_cell.setAttribute("colspan", "3");
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
                this.property_clicked(uri, version, p);
            };

            const name_field = document.createElement("td");
            name_field.className = "name-column";
            name_field.innerText = p.name;
            name_field.addEventListener("click", goto_property);
            row.appendChild(name_field);

            const value_field = document.createElement("td");
            value_field.className = "value-column";
            value_field.classList.add(type_class_for_typename(p.type_name));
            value_field.setAttribute("title", p.type_name);
            const extra_field = document.createElement("td");
            extra_field.className = "extra-column";
            extra_field.setAttribute("title", p.type_name);
            const delete_button = document.createElement("button");
            delete_button.style.display = "hidden";
            extra_field.appendChild(delete_button);
            const input = document.createElement("input");
            input.type = "text";
            value_field.appendChild(input);

            if (p.defined_at != null) {
                const code_text = p.defined_at.expression_value;
                input.value = code_text;
                const changed_class = "value-changed";
                const error_class = "value-has-error";
                const warn_class = "value-has-warning";

                input.addEventListener("focus", () => {
                    if (FOCUSING) {
                        FOCUSING = false;
                    } else {
                        FOCUSING = true;
                        goto_property();
                        input.focus();
                    }
                });
                input.addEventListener("input", () => {
                    const current_text = input.value;
                    if (current_text != code_text) {
                        input.classList.add(changed_class);
                    } else {
                        input.classList.remove(changed_class);
                    }
                    input.classList.remove(error_class);
                    input.classList.remove(warn_class);
                    if (current_text != code_text && element_range != null) {
                        this.#binding_editor(
                            { uri: uri, version: version },
                            element_range,
                            p.name,
                            current_text,
                            true,
                        ).then((r: SetBindingResponse) => {
                            const diagnostics = r.diagnostics;
                            let has_error = false;
                            let has_warning = false;
                            for (const d of diagnostics) {
                                if (d.severity == 1) {
                                    has_error = true;
                                } else if (d.severity == 2) {
                                    has_warning = true;
                                }
                            }

                            if (has_error) {
                                input.classList.add(error_class);
                            } else if (has_warning) {
                                input.classList.add(warn_class);
                            }
                        });
                    }
                });
                input.addEventListener("change", () => {
                    const current_text = input.value;
                    if (current_text != code_text && element_range != null) {
                        this.#binding_editor(
                            { uri: uri, version: version },
                            element_range,
                            p.name,
                            current_text,
                            false,
                        );
                    }
                });

                delete_button.className = this.#remover_icon_class;
                if (element_range != null) {
                    delete_button.addEventListener("click", (event) => {
                        event.stopPropagation();
                        this.#binding_remover(
                            { uri: uri, version: version },
                            element_range,
                            p.name,
                        );
                    });
                } else {
                    delete_button.disabled = true;
                }
            } else {
                input.disabled = true;

                delete_button.className = this.#adder_icon_class;
            }
            row.appendChild(value_field);
            row.appendChild(extra_field);

            table.appendChild(row);
        }
    }

    show_welcome(content: string) {
        this.headerNode.style.display = "none";
        this.tableNode.style.display = "none";
        this.welcomeNode.style.display = "block";
        this.welcomeNode.innerHTML = content;
    }

    hide_welcome() {
        this.headerNode.style.display = "block";
        this.tableNode.style.display = "block";
        this.welcomeNode.style.display = "none";
        this.welcomeNode.innerHTML = "";
    }

    set_properties(properties: PropertyQuery) {
        if (
            JSON.stringify(properties.element?.range) ==
                JSON.stringify(this.#current_element_range) &&
            properties.source_uri == this.#current_data_uri &&
            properties.source_version == this.#current_data_version
        ) {
            return;
        }

        if (properties.element == null) {
            this.show_welcome("No element selected");
        } else {
            this.hide_welcome();
            this.set_header(properties.element);
            this.populate_table(
                properties.element?.range,
                properties.properties,
                properties.source_uri,
                properties.source_version,
            );
        }

        // Update info about current data:
        this.#current_element_range = properties.element?.range ?? null;
        this.#current_data_uri = properties.source_uri;
        this.#current_data_version = properties.source_version;
    }
}
