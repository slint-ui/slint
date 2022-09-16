// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { Widget } from "@lumino/widgets";

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";

import slint_init, * as slint from "@preview/slint_wasm_interpreter.js";

let was_initialized = false;

async function setup_preview() {
  if (!was_initialized) {
    await slint_init();
  }
  was_initialized = true;
}

export class PreviewWidget extends Widget {
  #instance: slint.WrappedInstance | null;
  #canvas_id: string;

  static createNode(): HTMLElement {
    const node = document.createElement("div");
    const content = document.createElement("div");
    content.className = "preview-container";

    const canvas = document.createElement("canvas");
    canvas.width = 800;
    canvas.height = 600;
    content.appendChild(canvas);

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

    this.#canvas_id = "";
    this.#instance = null;
  }

  protected get canvas_id(): string {
    if (this.#canvas_id === "") {
      this.#canvas_id = "canvas_" + Math.random().toString(36).slice(2, 11);
      const canvas = document.createElement("canvas");
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

  public async render(
    style: string,
    source: string,
    base_url: string,
    load_callback: (_url: string) => Promise<string>,
  ): Promise<monaco.editor.IMarkerData[]> {
    await setup_preview();

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
        this.#instance = component.create(this.canvas_id);
        this.#instance.show();
        slint.run_event_loop();
      } else {
        this.#instance = component.create_with_existing_window(this.#instance);
      }
    }

    return markers;
  }
}
