// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore bindgen lumino winit

import { Message } from "@lumino/messaging";
import { Widget } from "@lumino/widgets";

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";

import slint_init, * as slint from "@preview/slint_wasm_interpreter.js";

const ensure_slint_wasm_bindgen_glue_initialized: Promise<slint.InitOutput> =
    slint_init();

let is_event_loop_running = false;

export class PreviewWidget extends Widget {
    #instance: slint.WrappedInstance | null;
    #canvas_id: string;
    #canvas: HTMLCanvasElement | null = null;
    #canvas_observer: MutationObserver | null = null;
    #ensure_attached_to_dom: Promise<void>;
    #resolve_attached_to_dom: () => void;
    #zoom_level = 100;

    static createNode(): HTMLElement {
        const node = document.createElement("div");

        const menu_area = document.createElement("div");
        menu_area.className = "menu-area";
        node.appendChild(menu_area);

        const content = document.createElement("div");
        content.className = "preview-container";

        const scroll = document.createElement("div");
        scroll.className = "preview-scrolled-area";
        scroll.style.overflow = "hidden";
        scroll.style.position = "relative";
        content.appendChild(scroll);

        const error_area = document.createElement("div");
        error_area.className = "error-area";
        node.appendChild(error_area);

        node.appendChild(content);

        return node;
    }

    constructor() {
        super({ node: PreviewWidget.createNode() });
        this.setFlag(Widget.Flag.DisallowLayout);
        this.addClass("content");
        this.addClass("preview");
        this.title.label = "Preview";
        this.title.caption = `Slint Viewer`;
        this.title.closable = true;

        this.#canvas_id = "";
        this.#instance = null;
        this.#resolve_attached_to_dom = () => {
            // dummy, to be replaced with resolution function provided to promise
            // executor.
        };
        this.#ensure_attached_to_dom = new Promise((resolve) => {
            this.#resolve_attached_to_dom = resolve;
        });

        this.populate_menu();
    }

    private populate_menu() {
        const menu = this.menuNode;

        const zoom_in = document.createElement("button");
        zoom_in.innerHTML = '<i class="fa fa-search-minus"></i>';

        const zoom_level = document.createElement("input");
        zoom_level.type = "number";
        zoom_level.max = "1600";
        zoom_level.min = "25";
        zoom_level.value = this.#zoom_level.toString();

        const zoom_out = document.createElement("button");
        zoom_out.innerHTML = '<i class="fa fa-search-plus"></i>';

        const set_zoom_level = (level: number) => {
            this.#zoom_level = level;
            const canvas = this.canvasNode;
            if (canvas != null) {
                canvas.style.scale = (level / 100).toString();
            }
            if (+zoom_level.value != level) {
                zoom_level.value = level.toString();
            }
        };

        zoom_in.addEventListener("click", () => {
            let next_level = +zoom_level.max;
            const current_level = +zoom_level.value;
            const smallest_level = +zoom_level.min;

            while (next_level > smallest_level && next_level >= current_level) {
                next_level = Math.ceil(next_level / 2);
            }
            set_zoom_level(next_level);
        });

        zoom_out.addEventListener("click", () => {
            let next_level = +zoom_level.min;
            const current_level = +zoom_level.value;
            const biggest_level = +zoom_level.max;

            while (next_level < biggest_level && next_level <= current_level) {
                next_level = Math.ceil(next_level * 2);
            }
            set_zoom_level(next_level);
        });

        zoom_level.addEventListener("change", () => {
            set_zoom_level(+zoom_level.value);
        });

        menu.appendChild(zoom_in);
        menu.appendChild(zoom_level);
        menu.appendChild(zoom_out);
    }

    protected onCloseRequest(msg: Message): void {
        super.onCloseRequest(msg);
        this.dispose();
    }

    protected update_scroll_size() {
        // I use style.scale to zoom the canvas, which can be GPU accelerated
        // and should be fast. Unfortunately that only scales at render-time,
        // _not_ at layout time. So scrolling breaks as it calculates the scroll
        // area based on the canvas size without scaling applied!
        //
        // So we have a scrollNode as the actual scroll area and watch the canvas
        // for style changes, triggering this function.
        //
        // This resizes the scrollNode to be scale_factor * canvas size + padding
        // and places the canvas into the middle- This makes scrolling work
        // properly: The scroll area size is calculated based on the scrollNode,
        // which has enough room around the canvas for it to be rendered in
        // zoomed state.
        if (this.#canvas == null || this.#zoom_level < 0) {
            return;
        }

        const padding = 50;
        const canvas_style = document.defaultView?.getComputedStyle(
            this.#canvas,
        );
        const parent_style = document.defaultView?.getComputedStyle(
            this.contentNode,
        );

        if (canvas_style == null || parent_style == null) {
            return;
        }

        const raw_canvas_scale = parseFloat(canvas_style.scale);
        const raw_canvas_width = parseInt(canvas_style.width, 10);
        const raw_canvas_height = parseInt(canvas_style.height, 10);
        const canvas_width = Math.ceil(raw_canvas_width * raw_canvas_scale);
        const canvas_height = Math.ceil(raw_canvas_height * raw_canvas_scale);
        const width = Math.max(
            parseInt(parent_style.width, 10),
            canvas_width + 2 * padding,
        );
        const height = Math.max(
            parseInt(parent_style.height, 10),
            canvas_height + 2 * padding,
        );
        const left = Math.ceil((width - raw_canvas_width) / 2) + "px";
        const top = Math.ceil((height - raw_canvas_height) / 2) + "px";

        const zl = this.#zoom_level;
        this.#zoom_level = -1;
        this.#canvas.style.left = left;
        this.#canvas.style.top = top;
        this.scrollNode.style.width = width + "px";
        this.scrollNode.style.height = height + "px";
        this.#zoom_level = zl;
    }

    protected get canvas_id(): string {
        if (this.#canvas_id === "") {
            this.#canvas_id =
                "canvas_" + Math.random().toString(36).slice(2, 11);

            this.#canvas = document.createElement("canvas");
            this.#canvas.width = 800;
            this.#canvas.height = 600;
            this.#canvas.id = this.#canvas_id;
            this.#canvas.className = "slint-preview";
            this.#canvas.style.scale = (this.#zoom_level / 100).toString();
            this.#canvas.style.padding = "0px";
            this.#canvas.style.margin = "0px";
            this.#canvas.style.position = "absolute";
            this.#canvas.style.imageRendering = "pixelated";

            this.scrollNode.appendChild(this.#canvas);

            const config = { attributes: true };

            const update_scroll_size = () => {
                this.update_scroll_size();
            };

            update_scroll_size();

            // Callback function to execute when mutations are observed
            this.#canvas_observer = new MutationObserver((mutationList) => {
                for (const mutation of mutationList) {
                    if (
                        mutation.type === "attributes" &&
                        mutation.attributeName === "style"
                    ) {
                        update_scroll_size();
                    }
                }
            });
            this.#canvas_observer.observe(this.#canvas, config);
        }

        return this.#canvas_id;
    }

    protected get contentNode(): HTMLDivElement {
        return this.node.getElementsByClassName(
            "preview-container",
        )[0] as HTMLDivElement;
    }

    protected get scrollNode(): HTMLDivElement {
        return this.node.getElementsByClassName(
            "preview-scrolled-area",
        )[0] as HTMLDivElement;
    }

    protected get canvasNode(): HTMLCanvasElement | null {
        return this.contentNode.getElementsByClassName(
            "slint-preview",
        )[0] as HTMLCanvasElement;
    }
    protected get menuNode(): HTMLDivElement {
        return this.node.getElementsByClassName(
            "menu-area",
        )[0] as HTMLDivElement;
    }

    protected get errorNode(): HTMLDivElement {
        return this.node.getElementsByClassName(
            "error-area",
        )[0] as HTMLDivElement;
    }

    dispose() {
        super.dispose();
        this.#canvas_observer?.disconnect();
    }

    protected onAfterAttach(msg: Message): void {
        super.onAfterAttach(msg);
        this.#resolve_attached_to_dom();
    }

    protected onResize(_msg: Message): void {
        if (this.isAttached) {
            this.update_scroll_size();
        }
    }

    public async render(
        style: string,
        source: string,
        base_url: string,
        load_callback: (_url: string) => Promise<string>,
    ): Promise<monaco.editor.IMarkerData[]> {
        await ensure_slint_wasm_bindgen_glue_initialized;

        const { component, diagnostics, error_string } =
            await slint.compile_from_string_with_style(
                source,
                base_url,
                style,
                load_callback,
            );

        const error_area = this.errorNode;

        error_area.innerHTML = "";

        if (error_string != "") {
            const text = document.createTextNode(error_string);
            const p = document.createElement("p");
            p.className = "error-message";
            p.appendChild(text);
            error_area.appendChild(p);

            error_area.style.display = "block";
        } else {
            error_area.style.display = "none";
        }

        const markers = diagnostics.map(function (x) {
            return {
                severity: 3 - x.level,
                message: x.message,
                source: x.fileName,
                startLineNumber: x.lineNumber,
                startColumn: x.columnNumber,
                endLineNumber: x.lineNumber,
                endColumn: -1,
            };
        });

        if (component != null) {
            // It's not enough for the canvas element to exist, in order to extract a webgl rendering
            // context, the element needs to be attached to the window's dom.
            await this.#ensure_attached_to_dom;
            if (this.#instance == null) {
                this.#instance = component.create(this.canvas_id);
                this.#instance.show();
                try {
                    if (!is_event_loop_running) {
                        slint.run_event_loop();
                        // this will trigger a JS exception, so this line will never be reached!
                    }
                } catch (e) {
                    // The winit event loop, when targeting wasm, throws a JavaScript exception to break out of
                    // Rust without running any destructors. Don't rethrow the exception but swallow it, as
                    // this is no error and we truly want to resolve the promise of this function by returning
                    // the model markers.
                    is_event_loop_running = true; // Assume the winit caused the exception and that the event loop is up now
                }
            } else {
                this.#instance = component.create_with_existing_window(
                    this.#instance,
                );
            }
        }

        return markers;
    }
}
