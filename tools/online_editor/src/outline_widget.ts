// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import { MonacoLanguageClient } from "monaco-languageclient";

import {
  DocumentSymbolRequest,
  DocumentSymbolParams,
  DocumentSymbol,
  SymbolInformation,
} from "vscode-languageserver-protocol";

const SYMBOL_KIND_MAP = [
  "kind-unknown",
  "kind-file",
  "kind-module",
  "kind-namespace",
  "kind-package",
  "kind-class",
  "kind-method",
  "kind-property",
  "kind-field",
  "kind-constructor",
  "kind-enum",
  "kind-interface",
  "kind-function",
  "kind-variable",
  "kind-constant",
  "kind-string",
  "kind-number",
  "kind-boolean",
  "kind-array",
  "kind-object",
  "kind-key",
  "kind-null",
  "kind-enum-member",
  "kind-struct",
  "kind-event",
  "kind-operator",
  "kind-type-parameter",
];

function set_data(
  data: DocumentSymbol[],
  indent: number,
  table: HTMLTableElement,
) {
  for (const d of data) {
    const row = document.createElement("tr");
    row.className = "outline-element";
    if (d.deprecated || (d.tags != null && 1 in d.tags)) {
      row.classList.add("deprecated");
    }
    if (d.kind >= SYMBOL_KIND_MAP.length || d.kind < 1) {
      row.classList.add(SYMBOL_KIND_MAP[0]);
    } else {
      row.classList.add(SYMBOL_KIND_MAP[d.kind]);
    }
    row.classList.add("indent-" + indent);

    const cell = document.createElement("td");
    cell.innerText = d.name;

    row.appendChild(cell);
    table.appendChild(row);

    if (d.children != null) {
      set_data(d.children, indent + 1, table);
    }
  }
}

export class OutlineWidget extends Widget {
  #callback: () => [MonacoLanguageClient | undefined, string | undefined];
  #intervalId = -1;

  static createNode(): HTMLElement {
    const node = document.createElement("div");
    const content = document.createElement("div");
    node.appendChild(content);
    return node;
  }

  constructor(
    callback: () => [MonacoLanguageClient | undefined, string | undefined],
  ) {
    super({ node: OutlineWidget.createNode() });
    this.#callback = callback;
    this.setFlag(Widget.Flag.DisallowLayout);
    this.addClass("content");
    this.addClass("outline");
    this.title.label = "Document Outline";
    this.title.closable = true;
    this.title.caption = `Document Outline`;

    this.#intervalId = window.setInterval(() => {
      const [client, uri] = this.#callback();
      if (client != null && uri != null) {
        client
          .sendRequest(DocumentSymbolRequest.type, {
            textDocument: { uri: uri },
          } as DocumentSymbolParams)
          .then((r: DocumentSymbol[] | SymbolInformation[] | null) =>
            this.update_data(r),
          );
      } else {
        if (uri == null) {
          // No document is open
          this.clear_data();
        } else {
          this.set_error("Language server not available");
        }
      }
    }, 5000);
  }

  protected get contentNode(): HTMLDivElement {
    return this.node.getElementsByTagName("div")[0] as HTMLDivElement;
  }

  protected update_data(data: DocumentSymbol[] | SymbolInformation[] | null) {
    if (data == null) {
      this.set_error("No data received");
      return;
    }
    if (data.length > 0 && "location" in data[0]) {
      // location is a required key in SymbolInformation that does not exist in DocumentSymbol
      this.set_error("Invalid data format received");
      return;
    }
    const table = document.createElement("table");
    table.className = "outline-table";

    set_data(data as DocumentSymbol[], 0, table);

    this.clear_data();
    this.contentNode.appendChild(table);
  }

  protected clear_data() {
    this.contentNode.innerText = "";
  }

  protected set_error(message: string) {
    this.contentNode.innerHTML = '<div class="error">' + message + "</div>";
  }

  protected onCloseRequest(msg: Message): void {
    if (this.#intervalId !== -1) {
      clearInterval(this.#intervalId);
      this.#intervalId = -1;
    }
    super.onCloseRequest(msg);
    this.dispose();
  }
}
