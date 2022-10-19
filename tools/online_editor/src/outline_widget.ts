// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

export class OutlineWidget extends Widget {
  static createNode(): HTMLElement {
    const node = document.createElement("div");
    const content = document.createElement("div");
    node.appendChild(content);
    return node;
  }

  constructor() {
    super({ node: OutlineWidget.createNode() });
    this.setFlag(Widget.Flag.DisallowLayout);
    this.addClass("content");
    this.addClass("outline".toLowerCase());
    this.title.label = "Document Outline";
    this.title.closable = true;
    this.title.caption = `Document Outline`;
  }

  protected onCloseRequest(msg: Message): void {
    super.onCloseRequest(msg);
    this.dispose();
  }
}
