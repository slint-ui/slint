// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore bindgen lumino winit

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import { Previewer, Lsp } from "./lsp";

export class PreviewWidget extends Widget {
    #previewer: Previewer | null = null;

    static createNode(): HTMLElement {
        const node = document.createElement("div");
        node.className = "preview-editor";

        const canvas_id = "canvas";
        const canvas = document.createElement("canvas");
        node.appendChild(canvas);

        canvas.id = canvas_id;
        canvas.className = canvas_id;

        return node;
    }

    constructor(lsp: Lsp, _internal_url_prefix: string) {
        super({ node: PreviewWidget.createNode() });

        this.setFlag(Widget.Flag.DisallowLayout);
        this.addClass("content");
        this.addClass("preview");
        this.title.label = "Preview";
        this.title.caption = `Slint Viewer`;
        this.title.closable = true;

        lsp.previewer().then((p) => {
            this.#previewer = p;

            // Give the UI some time to wire up the canvas so it can be found
            // when searching the document.
            this.#previewer
                .show_ui(this.node.clientWidth, this.node.clientHeight)
                .then(() => {
                    console.info("UI should be up!");
                });
        });
    }

    protected onResize(msg: Widget.ResizeMessage): void {
        super.onResize(msg);
        this.#previewer?.resize_ui(
            this.node.clientWidth,
            this.node.clientHeight,
        );
    }

    protected onAfterShow(msg: Message): void {
        super.onAfterShow(msg);
        this.#previewer?.resize_ui(
            this.node.clientWidth,
            this.node.clientHeight,
        );
    }

    protected onCloseRequest(msg: Message): void {
        super.onCloseRequest(msg);
        this.dispose();
    }

    protected get contentNode(): HTMLDivElement {
        return this.node.getElementsByClassName(
            "preview-container",
        )[0] as HTMLDivElement;
    }

    dispose() {
        super.dispose();
    }
}
