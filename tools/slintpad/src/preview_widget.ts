// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore bindgen lumino

import type { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import type { Previewer, Lsp, ResourceUrlMapperFunction } from "./lsp";

const canvas_id = "canvas";

export class PreviewWidget extends Widget {
    #previewer: Previewer | null = null;

    static createNode(): HTMLElement {
        const node = document.createElement("div");
        node.className = "preview-container";

        const canvas = document.createElement("canvas");

        canvas.id = canvas_id;
        canvas.className = canvas_id;
        canvas.style.width = "100%";
        canvas.style.height = "100%";
        canvas.style.outline = "none";
        canvas.style.touchAction = "none";
        node.appendChild(canvas);

        return node;
    }

    constructor(
        lsp: Lsp,
        resource_url_mapper: ResourceUrlMapperFunction,
        style: string,
    ) {
        super({ node: PreviewWidget.createNode() });

        this.setFlag(Widget.Flag.DisallowLayout);
        this.addClass("content");
        this.addClass("preview");
        this.title.label = "Preview";
        this.title.caption = `Slint Viewer`;
        this.title.closable = true;

        lsp.previewer(resource_url_mapper, style).then((p) => {
            this.#previewer = p;

            // Give the UI some time to wire up the canvas so it can be found
            // when searching the document.
            this.#previewer.show_ui().then(() => {
                console.info("SlintPad: started");
                const canvas = document.getElementById(
                    canvas_id,
                ) as HTMLElement;
                canvas.style.width = "100%";
                canvas.style.height = "100%";
            });
        });
    }

    public current_style(): string {
        if (this.#previewer) {
            return this.#previewer.current_style();
        } else {
            return "";
        }
    }

    protected onResize(msg: Widget.ResizeMessage): void {
        super.onResize(msg);

        const canvas = document.getElementById(canvas_id) as HTMLCanvasElement;
        canvas.style.width = "100%";
        canvas.style.height = "100%";
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
