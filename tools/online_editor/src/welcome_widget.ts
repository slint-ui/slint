// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

export class WelcomeWidget extends Widget {
  static createNode(): HTMLElement {
    const node = document.createElement("div");
    const content = document.createElement("div");
    const welcome_div = document.createElement("div");
    welcome_div.innerHTML = `
      <center>
      <h1>Welcome to the Slint Online Editor</h1>

      <p>We hope you enjoy working with the Slint UI description language!</p>


      <p>Feel free to visit <a href="https://slint-ui.com/">our Homepage</a> for more information.</p>
      </center>
      `;
    content.appendChild(welcome_div);
    node.appendChild(content);
    return node;
  }

  constructor() {
    super({ node: WelcomeWidget.createNode() });
    this.setFlag(Widget.Flag.DisallowLayout);
    this.addClass("content");
    this.addClass("welcome".toLowerCase());
    this.title.label = "Welcome";
    this.title.closable = true;
    this.title.caption = `Welcome to Slint`;
  }

  protected onCloseRequest(msg: Message): void {
    super.onCloseRequest(msg);
    this.dispose();
  }
}
