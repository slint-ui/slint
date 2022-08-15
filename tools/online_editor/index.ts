// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import { slint_language } from "./highlighting";

import "monaco-editor/esm/vs/editor/editor.all.js";
import "monaco-editor/esm/vs/editor/standalone/browser/accessibilityHelp/accessibilityHelp.js";
import "monaco-editor/esm/vs/editor/standalone/browser/iPadShowKeyboard/iPadShowKeyboard.js";
import "monaco-editor/esm/vs/editor/standalone/browser/inspectTokens/inspectTokens.js";
import "monaco-editor/esm/vs/editor/standalone/browser/quickAccess/standaloneHelpQuickAccess.js";
import "monaco-editor/esm/vs/editor/standalone/browser/quickAccess/standaloneGotoLineQuickAccess.js";
import "monaco-editor/esm/vs/editor/standalone/browser/quickAccess/standaloneGotoSymbolQuickAccess.js";
import "monaco-editor/esm/vs/editor/standalone/browser/quickAccess/standaloneCommandsQuickAccess.js";
import "monaco-editor/esm/vs/editor/standalone/browser/referenceSearch/standaloneReferenceSearch.js";

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";

(self as any).MonacoEnvironment = {
  getWorker(_: any, label: any) {
    return new Worker(
      new URL("monaco-editor/esm/vs/editor/editor.worker", import.meta.url),
      { type: "module" }
    );
  },
};

import slint_init, * as slint from "@preview/slint_wasm_interpreter.js";

(async function () {
  await slint_init();

  monaco.languages.register({
    id: "slint",
  });
  monaco.languages.onLanguage("slint", () => {
    monaco.languages.setMonarchTokensProvider("slint", slint_language);
  });
  const editor_element = document.getElementById("editor");
  if (!editor_element) {
    return;
  }
  const editor = monaco.editor.create(editor_element, {
    language: "slint",
  });
  let base_url = "";

  interface ModelAndViewState {
    model: monaco.editor.ITextModel;
    view_state: monaco.editor.ICodeEditorViewState | null;
  }

  /// Index by url. Inline documents will use the empty string.
  const editor_documents: Map<string, ModelAndViewState> = new Map();

  let hello_world = `import { Button, VerticalBox } from "std-widgets.slint";
export Demo := Window {
    VerticalBox {
        Text {
            text: "Hello World!";
            font-size: 24px;
        }
        Image {
            source: @image-url("https://slint-ui.com/logo/slint-logo-full-light.svg");
            height: 100px;
        }
        Button { text: "OK!"; }
    }
}
`;

  function load_from_url(url: string) {
    clearTabs();
    fetch(url).then((x) =>
      x.text().then((y) => {
        base_url = url;
        let model = createMainModel(y, url);
        addTab(model, url);
      })
    );
  }

  let select = <HTMLInputElement>document.getElementById("select_combo");
  function select_combo_changed() {
    if (select.value) {
      let tag = "master";
      {
        let found;
        if (
          (found = window.location.pathname.match(/releases\/([^\/]*)\/editor/))
        ) {
          tag = "v" + found[1];
        }
      }
      load_from_url(
        `https://raw.githubusercontent.com/slint-ui/slint/${tag}/${select.value}`
      );
    } else {
      clearTabs();
      base_url = "";
      let model = createMainModel(hello_world, "");
      addTab(model);
    }
  }
  select.onchange = select_combo_changed;

  let style_combo = <HTMLInputElement>document.getElementById("style_combo");
  if (style_combo) {
    style_combo.onchange = update_preview;
  }

  let compile_button = <HTMLButtonElement>(
    document.getElementById("compile_button")
  );
  compile_button.onclick = function () {
    update_preview();
  };

  let auto_compile = <HTMLInputElement>document.getElementById("auto_compile");
  auto_compile.onchange = function () {
    if (auto_compile.checked) {
      update_preview();
    }
  };

  function tabTitleFromURL(url: string): string {
    if (url === "") {
      return "unnamed.slint";
    }
    try {
      let parsed_url = new URL(url);
      let path = parsed_url.pathname;
      return path.substring(path.lastIndexOf("/") + 1);
    } catch (e) {
      return url;
    }
  }

  function maybe_update_preview_automatically() {
    if (auto_compile.checked) {
      if (keystroke_timeout_handle) {
        clearTimeout(keystroke_timeout_handle);
      }
      keystroke_timeout_handle = setTimeout(update_preview, 500);
    }
  }

  function createMainModel(
    source: string,
    url: string
  ): monaco.editor.ITextModel {
    let model = monaco.editor.createModel(source, "slint");
    model.onDidChangeContent(function () {
      let permalink = <HTMLAnchorElement>document.getElementById("permalink");
      let params = new URLSearchParams();
      params.set("snippet", editor.getModel()?.getValue() || "");
      const this_url = new URL(window.location.toString());
      this_url.search = params.toString();
      permalink.href = this_url.toString();
      maybe_update_preview_automatically();
    });
    editor_documents.set(url, { model, view_state: null });
    update_preview();
    return model;
  }

  function clearTabs() {
    let tab_bar = document.getElementById("tabs") as HTMLUListElement;
    tab_bar.innerHTML = "";
    editor_documents.clear();
  }

  function addTab(model: monaco.editor.ITextModel, url: string = "") {
    let tab_bar = document.getElementById("tabs") as HTMLUListElement;
    let tab = document.createElement("li");
    tab.setAttribute("class", "nav-item");
    tab.dataset["url"] = url;
    tab.innerHTML = `<span class="nav-link">${tabTitleFromURL(url)}</span>`;
    tab_bar.appendChild(tab);
    tab.addEventListener("click", (e) => {
      e.preventDefault();
      setCurrentTab(url);
    });
    if (tab_bar.childElementCount == 1) {
      setCurrentTab(url);
    }
  }

  function setCurrentTab(url: string) {
    let current_tab = document.querySelector(
      `#tabs li[class~="nav-item"] span[class~="nav-link"][class~="active"]`
    );
    if (current_tab != undefined) {
      current_tab.className = "nav-link";

      let url = current_tab.parentElement?.dataset.url;
      if (url != undefined) {
        const model_and_state = editor_documents.get(url);
        if (model_and_state !== undefined) {
          model_and_state.view_state = editor.saveViewState();
          editor_documents.set(url, model_and_state);
        }
      }
    }
    let new_current = document.querySelector(
      `#tabs li[class~="nav-item"][data-url="${url}"] span[class~="nav-link"]`
    );
    if (new_current != undefined) {
      new_current.className = "nav-link active";
    }
    let model_and_state = editor_documents.get(url);
    if (model_and_state != undefined) {
      editor.setModel(model_and_state.model);
      if (model_and_state.view_state != null) {
        editor.restoreViewState(model_and_state.view_state);
      }
      editor.focus();
    }
  }

  function update_preview() {
    let main_model_and_state = editor_documents.get(base_url);
    if (main_model_and_state === undefined) {
      return;
    }
    let source = main_model_and_state.model.getValue();
    let div = document.getElementById("preview") as HTMLDivElement;
    setTimeout(function () {
      render_or_error(source, base_url, div);
    }, 1);
  }

  async function render_or_error(
    source: string,
    base_url: string,
    div: HTMLDivElement
  ) {
    let style =
      (<HTMLInputElement>document.getElementById("style_combo"))?.value ??
      "fluent";

    let canvas_id = "canvas_" + Math.random().toString(36).substr(2, 9);
    let canvas = document.createElement("canvas");
    canvas.width = 800;
    canvas.height = 600;
    canvas.id = canvas_id;
    div.innerHTML = "";
    div.appendChild(canvas);
    var markers = [];
    let { component, diagnostics, error_string } =
      await slint.compile_from_string_with_style(
        source,
        base_url,
        style,
        async (url: string): Promise<string> => {
          let model_and_state = editor_documents.get(url);
          if (model_and_state === undefined) {
            const response = await fetch(url);
            let doc = await response.text();
            let model = monaco.editor.createModel(doc, "slint");
            model.onDidChangeContent(function () {
              maybe_update_preview_automatically();
            });
            editor_documents.set(url, { model, view_state: null });
            addTab(model, url);
            return doc;
          }
          return model_and_state.model.getValue();
        }
      );

    if (error_string != "") {
      let text = document.createTextNode(error_string);
      let p = document.createElement("pre");
      p.appendChild(text);
      div.innerHTML =
        "<pre style='color: red; background-color:#fee; margin:0'>" +
        p.innerHTML +
        "</pre>";
    }

    markers = diagnostics.map(function (x) {
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
    const model = editor.getModel();
    if (model !== null) {
      monaco.editor.setModelMarkers(model, "slint", markers);
    }

    if (component !== undefined) {
      component.run(canvas_id);
    }
  }

  let keystroke_timeout_handle: number;

  const params = new URLSearchParams(window.location.search);
  const code = params.get("snippet");
  const load_url = params.get("load_url");

  if (code) {
    clearTabs();
    let model = createMainModel(code, "");
    addTab(model);
  } else if (load_url) {
    load_from_url(load_url);
  } else {
    clearTabs();
    base_url = "";
    let model = createMainModel(hello_world, "");
    addTab(model);
  }
})();
