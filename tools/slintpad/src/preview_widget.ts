// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore bindgen lumino winit

import type { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import type {
    Previewer,
    Lsp,
    ResourceUrlMapperFunction,
    InvokeSlintpadCallback,
} from "./lsp";

const canvas_id = "canvas";

export class PreviewWidget extends Widget {
    #previewer: Previewer | null = null;

    // Resize handling: shrinking the preview never flickers (winit reuses the
    // larger WebGL surface), so we let it resize live. Growing needs a bigger
    // surface, which winit reallocates and clears on every pointer move -> that
    // is the flicker. So while the pane is growing we hold the canvas at its
    // current pixel size (no reallocation, and no CSS scaling so nothing is
    // distorted), then restore live sizing once the resize settles. See onResize.
    #resizeSettleTimer: ReturnType<typeof setTimeout> | null = null;
    #held = false;
    #heldWidth = 0;
    #heldHeight = 0;

    static createNode(): HTMLElement {
        const node = document.createElement("div");
        node.className = "preview-container";
        // Clip the canvas while it is held at a fixed pixel size and the pane
        // shrinks around it.
        node.style.overflow = "hidden";

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
        slintpad_callback: InvokeSlintpadCallback,
    ) {
        super({ node: PreviewWidget.createNode() });

        this.setFlag(Widget.Flag.DisallowLayout);
        this.addClass("content");
        this.addClass("preview");
        this.title.label = "Preview";
        this.title.caption = "Slint Viewer";
        this.title.closable = true;

        void lsp
            .previewer(resource_url_mapper, style, slintpad_callback)
            .then((p) => {
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
        }
        return "";
    }

    protected onResize(msg: Widget.ResizeMessage): void {
        super.onResize(msg);

        const canvas = document.getElementById(
            canvas_id,
        ) as HTMLCanvasElement | null;
        if (canvas === null) {
            return;
        }

        // Target size of the preview pane after this resize step. Lumino may
        // report an unknown size (-1); fall back to measuring the node.
        const rect = this.node.getBoundingClientRect();
        const width = msg.width >= 0 ? msg.width : rect.width;
        const height = msg.height >= 0 ? msg.height : rect.height;
        if (width <= 0 || height <= 0) {
            return;
        }

        // Current on-screen size of the canvas (its allocated WebGL surface, in
        // CSS pixels); while held, that is the size we pinned it to.
        const canvas_rect = canvas.getBoundingClientRect();
        const current_width = this.#held ? this.#heldWidth : canvas_rect.width;
        const current_height = this.#held
            ? this.#heldHeight
            : canvas_rect.height;

        const growing =
            width > current_width + 0.5 || height > current_height + 0.5;

        if (growing && current_width > 1 && current_height > 1) {
            // Growing forces winit to reallocate and clear a bigger WebGL
            // surface on every pointer move, which is the flicker. Hold the
            // canvas at its current pixel size instead: the surface is not
            // reallocated, and nothing is scaled so nothing is distorted. The
            // pane grows around it (clipped by the container) until the resize
            // settles.
            if (!this.#held) {
                this.#heldWidth = Math.max(1, Math.round(current_width));
                this.#heldHeight = Math.max(1, Math.round(current_height));
                canvas.style.width = `${this.#heldWidth}px`;
                canvas.style.height = `${this.#heldHeight}px`;
                this.#held = true;
            }
        } else if (this.#held) {
            // Shrinking back within the held size: resume live sizing. Shrinking
            // reuses the existing surface, so it stays crisp and does not flicker.
            this.#restore_live_size(canvas);
        }

        // On settle, restore live sizing so winit reallocates once and renders a
        // single crisp frame at the final size.
        if (this.#resizeSettleTimer !== null) {
            clearTimeout(this.#resizeSettleTimer);
        }
        this.#resizeSettleTimer = setTimeout(() => {
            this.#resizeSettleTimer = null;
            const c = document.getElementById(
                canvas_id,
            ) as HTMLCanvasElement | null;
            if (c !== null) {
                this.#restore_live_size(c);
            }
        }, 150);
    }

    #restore_live_size(canvas: HTMLCanvasElement): void {
        this.#held = false;
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
