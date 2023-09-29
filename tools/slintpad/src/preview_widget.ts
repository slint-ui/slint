// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore bindgen lumino

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import { Previewer, Lsp, ResourceUrlMapperFunction } from "./lsp";

export class PreviewWidget extends Widget {
    #previewer: Previewer | null = null;

    static createNode(): HTMLElement {
        const node = document.createElement("div");
        node.className = "preview-container";

        const canvas_id = "canvas";
        const canvas = document.createElement("canvas");

        canvas.id = canvas_id;
        canvas.className = canvas_id;

        canvas.dataset.slintAutoResizeToPreferred = "true";

        node.appendChild(canvas);

        return node;
    }

    constructor(lsp: Lsp, resource_url_mapper: ResourceUrlMapperFunction) {
        super({ node: PreviewWidget.createNode() });

        this.setFlag(Widget.Flag.DisallowLayout);
        this.addClass("content");
        this.addClass("preview");
        this.title.label = "Preview";
        this.title.caption = `Slint Viewer`;
        this.title.closable = true;

        lsp.previewer(resource_url_mapper).then((p) => {
            this.#previewer = p;

            // Give the UI some time to wire up the canvas so it can be found
            // when searching the document.
            this.#previewer.show_ui().then(() => {
                console.info("UI should be up!");
            });
        });
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
}
