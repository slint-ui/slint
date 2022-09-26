// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { Widget } from "@lumino/widgets";

import { BindingTextProvider, Element, Property, PropertyQuery } from "./lsp_integration";

export class PropertiesWidget extends Widget {
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

  constructor() {
    super({ node: PropertiesWidget.createNode() });
    this.setFlag(Widget.Flag.DisallowLayout);
    this.addClass("content");
    this.addClass("properties-editor".toLowerCase());
    this.title.label = "Properties";
    this.title.closable = false;
    this.title.caption = `Element Properties`;

    this.set_header(null);
  }

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

  private populate_table(binding_text_provider: BindingTextProvider, properties: Property[]) {
    const table = this.tableNode;

    table.innerHTML = "";

    for (const p of properties) {
      const row = document.createElement("tr");
      row.className = "property";
      if (p.declared_at == null) {
        row.classList.add("builtin");
      }
      if (p.defined_at == null) {
        row.classList.add("undefined");
      }

      const name_field = document.createElement("td");
      name_field.className = "name-column";
      name_field.innerText = p.name;
      row.appendChild(name_field);

      const type_field = document.createElement("td");
      type_field.className = "type-column";
      type_field.innerText = p.type_name;
      row.appendChild(type_field);

      const value_field = document.createElement("td");
      value_field.className = "value-column";
      if (p.defined_at != null) {
        value_field.innerText = binding_text_provider.binding_text(
          p.defined_at
        );
      } else {
        value_field.innerText = "";
      }
      row.appendChild(value_field);

      table.appendChild(row);
    }
  }

  set_properties(binding_text_provider: BindingTextProvider, properties: PropertyQuery) {
    this.set_header(properties.element);
    this.populate_table(binding_text_provider, properties.properties);
  }
}
