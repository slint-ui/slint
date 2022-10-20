// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import { MonacoLanguageClient } from "monaco-languageclient";
import { GotoPositionCallback, TextPosition, TextRange } from "./text";

import {
  DocumentSymbolRequest,
  DocumentSymbolParams,
  DocumentSymbol,
  SymbolInformation,
} from "vscode-languageserver-protocol";

import { SymbolTag, SymbolKind } from "vscode-languageserver-types";

const SYMBOL_KIND_MAP = new Map<SymbolKind, string>([
  [SymbolKind.File, "kind-file"],
  [SymbolKind.Module, "kind-module"],
  [SymbolKind.Namespace, "kind-namespace"],
  [SymbolKind.Package, "kind-package"],
  [SymbolKind.Class, "kind-class"],
  [SymbolKind.Method, "kind-method"],
  [SymbolKind.Property, "kind-property"],
  [SymbolKind.Field, "kind-field"],
  [SymbolKind.Constructor, "kind-constructor"],
  [SymbolKind.Enum, "kind-enum"],
  [SymbolKind.Interface, "kind-interface"],
  [SymbolKind.Function, "kind-function"],
  [SymbolKind.Variable, "kind-variable"],
  [SymbolKind.Constant, "kind-constant"],
  [SymbolKind.String, "kind-string"],
  [SymbolKind.Number, "kind-number"],
  [SymbolKind.Boolean, "kind-boolean"],
  [SymbolKind.Array, "kind-array"],
  [SymbolKind.Object, "kind-object"],
  [SymbolKind.Key, "kind-key"],
  [SymbolKind.Null, "kind-null"],
  [SymbolKind.EnumMember, "kind-enum-member"],
  [SymbolKind.Struct, "kind-struct"],
  [SymbolKind.Event, "kind-event"],
  [SymbolKind.Operator, "kind-operator"],
  [SymbolKind.TypeParameter, "kind-type-parameter"],
]);

function set_data(
  data: DocumentSymbol[],
  parent: HTMLUListElement,
  uri: string,
  goto_position: GotoPositionCallback,
) {
  for (const d of data) {
    const row = document.createElement("li");
    row.className = "outline-element";
    // the deprecated flag is deprecated, so cast to any so that the check
    // works even if deprecated gets removed
    if (
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (d as any).deprecated ||
      SymbolTag.Deprecated in (d.tags ?? [])
    ) {
      row.classList.add("deprecated");
    }
    row.classList.add(SYMBOL_KIND_MAP.get(d.kind) ?? "kind-unknown");

    const span = document.createElement("span");
    span.innerText = d.name;
    span.addEventListener("click", () =>
      goto_position(uri, {
        startLineNumber: d.selectionRange.start.line + 1,
        startColumn: d.selectionRange.start.character + 1,
        endLineNumber: d.selectionRange.end.line + 1,
        endColumn: d.selectionRange.end.character + 1,
      } as TextRange),
    );

    row.appendChild(span);

    if (d.children != null) {
      const children_parent = document.createElement("ul");
      set_data(d.children, children_parent, uri, goto_position);
      row.appendChild(children_parent);
    }

    parent.appendChild(row);
  }
}

export class OutlineWidget extends Widget {
  #callback: () => [MonacoLanguageClient | undefined, string | undefined];
  #intervalId = -1;
  #onGotoPosition = (uri: string, position: TextPosition | TextRange) => {
    console.log("Goto Position ignored:", uri, position);
  };

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
            this.update_data(uri, r),
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

  set on_goto_position(callback: GotoPositionCallback) {
    this.#onGotoPosition = callback;
  }

  protected get contentNode(): HTMLDivElement {
    return this.node.getElementsByTagName("div")[0] as HTMLDivElement;
  }

  protected update_data(
    uri: string,
    data: DocumentSymbol[] | SymbolInformation[] | null,
  ) {
    if (data == null) {
      this.set_error("No data received");
      return;
    }
    if (data.length > 0 && "location" in data[0]) {
      // location is a required key in SymbolInformation that does not exist in DocumentSymbol
      this.set_error("Invalid data format received");
      return;
    }
    const content = document.createElement("ul");
    content.className = "outline-tree";

    set_data(data as DocumentSymbol[], content, uri, this.#onGotoPosition);

    this.clear_data();
    this.contentNode.appendChild(content);
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
