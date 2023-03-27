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
      <h1>Welcome to SlintPad</h1>

      <a href="https://slint-ui.com/" target="_blank"><img src="https://slint-ui.com/logo/slint-logo-simple-light.svg"></a>
      </center>

      <p>Slint is a toolkit to efficiently develop fluid graphical user interfaces for
      any display: embedded devices and desktop applications. It comes with a custom markup language for user
      interfaces. This language is easy to learn, to read and write, and provides a powerful way to describe
      graphical elements. For more details, check out the <a href="https://slint-ui.com/docs/slint" target="_blank">Slint Language Documentation</a>.</p>

      <p>Use SlintPad to quickly try out Slint code snippets, with auto-completion, code navigation, and live-preview.</p>
      <p>The same features are also available in the <a href="https://marketplace.visualstudio.com/items?itemName=Slint.slint" target="_blank">Visual Studio Code extension</a>,
      which runs in your local VS code installation as well as in the <a href="https://vscode.dev/" target="_blank">Visual Studio Code for the Web</a>.</p>

      <p>SlintPad is licensed under the GNU GPLv3. The source code is located in our <a href="https://github.com/slint-ui/slint/tree/master/tools/online_editor" target="_blank">GitHub repository</a>.
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
