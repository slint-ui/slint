// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

export class WelcomeWidget extends Widget {
  static createNode(): HTMLElement {
    const node = document.createElement("div");
    const content = document.createElement("div");
    content.innerHTML = `
      <div>
      <center>
      <h1>Welcome to the Slint Online Editor</h1>

      <a href="https://slint-ui.com/"><img src="https://slint-ui.com/logo/slint-logo-simple-light.svg"></a>
      </center>

      <p>Slint is a GUI toolkit to develop native graphical user interface for embedded and desktop platform.
      It does that using a domain specific language to describe the interface.
      More information on <a href="https://slint-ui.com/">our Homepage</a></p>

      <p>This Online Editor is allowing to quickly try out Slint code snippets and see the preview.
      It also enables feature like auto-completion, smart navigation, and such.</p>
      <p>Similar features are also available in <a href="https://marketplace.visualstudio.com/items?itemName=Slint.slint">our Visual Studio Code extension</a>
      (also available as a web extension for the <a href="https://vscode.dev/">web version of vscode</a>)</p>

      <p>This Online Editor is licensed under the GNU GPLv3. <a href="https://github.com/slint-ui/slint/tree/master/tools/online_editor">Sources</a>.
      </div>
      `;
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
