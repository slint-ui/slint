// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore bindgen lumino winit

import { Widget } from "@lumino/widgets";
import { Message } from "@lumino/messaging";

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";

import slint_init, * as slint from "@preview/slint_wasm_interpreter.js";

const ensure_slint_wasm_bindgen_glue_initialized: Promise<slint.InitOutput> =
  slint_init();

export class PreviewWidget extends Widget {
  #instance: slint.WrappedInstance | null;
  #canvas_id: string;
  #ensure_attached_to_dom: Promise<void>;
  #resolve_attached_to_dom: () => void;

  static createNode(): HTMLElement {
    const node = document.createElement("div");
    const content = document.createElement("div");
    content.className = "preview-container";

    const error_area = document.createElement("div");
    error_area.className = "error-area";
    content.appendChild(error_area);

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
  }

  protected get canvas_id(): string {
    if (this.#canvas_id === "") {
      this.#canvas_id = "canvas_" + Math.random().toString(36).slice(2, 11);
      const canvas = document.createElement("canvas");
      canvas.width = 800;
      canvas.height = 600;
      canvas.id = this.#canvas_id;
      canvas.className = "slint-preview";

      this.node.getElementsByTagName("div")[0].appendChild(canvas);
    }

    return this.#canvas_id;
  }

  protected get errorNode(): HTMLDivElement {
    return this.node.getElementsByTagName("div")[1] as HTMLDivElement;
  }

  dispose() {
    super.dispose();
  }

  protected onAfterAttach(msg: Message): void {
    super.onAfterAttach(msg);
    this.#resolve_attached_to_dom();
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
      if (this.#instance == null) {
        // It's not enough for the canvas element to exist, in order to extract a webgl rendering
        // context, the element needs to be attached to the window's dom.
        await this.#ensure_attached_to_dom;
        this.#instance = component.create(this.canvas_id);
        this.#instance.show();
        try {
          slint.run_event_loop();
        } catch (e) {
          // The winit event loop, when targeting wasm, throws a JavaScript exception to break out of
          // Rust without running any destructors. Don't rethrow the exception but swallow it, as
          // this is no error and we truly want to resolve the promise of this function by returning
          // the model markers.
        }
      } else {
        this.#instance = component.create_with_existing_window(this.#instance);
      }
    }

    return markers;
  }
}
