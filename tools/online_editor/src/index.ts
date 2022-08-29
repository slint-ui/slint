// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore mimetypes

import { slint_language } from "./highlighting";

import "monaco-editor/esm/vs/editor/editor.all.js";
import "monaco-editor/esm/vs/editor/standalone/browser/accessibilityHelp/accessibilityHelp.js";
import "monaco-editor/esm/vs/editor/standalone/browser/iPadShowKeyboard/iPadShowKeyboard.js";
import "monaco-editor/esm/vs/editor/standalone/browser/inspectTokens/inspectTokens.js";
import "monaco-editor/esm/vs/editor/standalone/browser/quickAccess/standaloneCommandsQuickAccess.js";
import "monaco-editor/esm/vs/editor/standalone/browser/quickAccess/standaloneGotoLineQuickAccess.js";
import "monaco-editor/esm/vs/editor/standalone/browser/quickAccess/standaloneGotoSymbolQuickAccess.js";
import "monaco-editor/esm/vs/editor/standalone/browser/quickAccess/standaloneHelpQuickAccess.js";
import "monaco-editor/esm/vs/editor/standalone/browser/referenceSearch/standaloneReferenceSearch.js";

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";

import {
  MonacoLanguageClient,
  CloseAction,
  ErrorAction,
  MonacoServices,
  MessageTransports,
  Message,
  RequestMessage,
  ResponseMessage,
} from "monaco-languageclient";
import {
  BrowserMessageReader,
  BrowserMessageWriter,
} from "vscode-languageserver-protocol/browser";

import { FilterProxyReader } from "./proxy";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
(self as any).MonacoEnvironment = {
  getWorker(_: unknown, _label: unknown) {
    return new Worker(new URL("worker/monaco_worker.mjs", import.meta.url), {
      type: "module",
    });
  },
};

import slint_init, * as slint from "@preview/slint_wasm_interpreter.js";

(async function () {
  await slint_init();

  monaco.languages.register({
    id: "slint",
    extensions: [".slint"],
    aliases: ["Slint", "slint"],
    mimetypes: ["application/slint"],
  });
  monaco.languages.onLanguage("slint", () => {
    monaco.languages.setMonarchTokensProvider("slint", slint_language);
  });
  const editor_element = document.getElementById("editor");
  if (editor_element == null) {
    console.error("No editor id found!");
    return;
  }
  const editor = monaco.editor.create(editor_element, {
    cursorBlinking: "smooth",
    cursorSurroundingLines: 2,
    glyphMargin: true,
    language: "slint",
    lightbulb: { enabled: true },
  });
  MonacoServices.install();

  let base_url = "";
  let event_loop_started = false;

  interface ModelAndViewState {
    model: monaco.editor.ITextModel;
    view_state: monaco.editor.ICodeEditorViewState | null;
  }

  /// Index by url. Inline documents will use the empty string.
  const editor_documents: Map<string, ModelAndViewState> = new Map();

  const hello_world = `import { Button, VerticalBox } from "std-widgets.slint";
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
        const model = createMainModel(y, url);
        addTab(model, url);
      })
    );
  }

  const select = document.getElementById("select_combo") as HTMLInputElement;
  if (select == null) {
    console.error("No select_combo id found!");
    return;
  }
  function select_combo_changed() {
    if (select.value) {
      let tag = "master";
      {
        let found;
        if (
          (found = window.location.pathname.match(/releases\/([^/]*)\/editor/))
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
      const model = createMainModel(hello_world, "");
      addTab(model);
    }
  }
  select.onchange = select_combo_changed;

  const style_combo = document.getElementById(
    "style_combo"
  ) as HTMLInputElement;
  if (style_combo) {
    style_combo.onchange = update_preview;
  }

  const compile_button = document.getElementById(
    "compile_button"
  ) as HTMLButtonElement;
  compile_button.onclick = function () {
    update_preview();
  };

  const auto_compile = document.getElementById(
    "auto_compile"
  ) as HTMLInputElement;
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
      const parsed_url = new URL(url);
      const path = parsed_url.pathname;
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
    const model = monaco.editor.createModel(
      source,
      "slint",
      monaco.Uri.parse(url)
    );
    model.onDidChangeContent(function () {
      const permalink = document.getElementById(
        "permalink"
      ) as HTMLAnchorElement;
      const params = new URLSearchParams();
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
    const tab_bar = document.getElementById("tabs") as HTMLUListElement;
    tab_bar.innerHTML = "";
    editor_documents.clear();
  }

  function addTab(_model: monaco.editor.ITextModel, url = "") {
    const tab_bar = document.getElementById("tabs") as HTMLUListElement;
    const tab = document.createElement("li");
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
    const current_tab = document.querySelector(
      `#tabs li[class~="nav-item"] span[class~="nav-link"][class~="active"]`
    );
    if (current_tab != null) {
      current_tab.className = "nav-link";

      const url = current_tab.parentElement?.dataset.url;
      if (url != null) {
        const model_and_state = editor_documents.get(url);
        if (model_and_state !== undefined) {
          model_and_state.view_state = editor.saveViewState();
          editor_documents.set(url, model_and_state);
        }
      }
    }
    const new_current = document.querySelector(
      `#tabs li[class~="nav-item"][data-url="${url}"] span[class~="nav-link"]`
    );
    if (new_current != undefined) {
      new_current.className = "nav-link active";
    }
    const model_and_state = editor_documents.get(url);
    if (model_and_state != null) {
      editor.setModel(model_and_state.model);
      if (model_and_state.view_state != null) {
        editor.restoreViewState(model_and_state.view_state);
      }
    }
    editor.focus();
  }

  function update_preview() {
    const main_model_and_state = editor_documents.get(base_url);
    if (main_model_and_state != null) {
      const source = main_model_and_state.model.getValue();
      const div = document.getElementById("preview") as HTMLDivElement;
      setTimeout(function () {
        render_or_error(source, base_url, div);
      }, 1);
    }
  }

  async function read_from_url(url: string): Promise<string> {
    let model_and_state = editor_documents.get(url);
    if (model_and_state === undefined) {
      const response = await fetch(url);
      const doc = await response.text();

      // Did this get added in the meantime?
      model_and_state = editor_documents.get(url);
      if (model_and_state === undefined) {
        const model = monaco.editor.createModel(
          doc,
          "slint",
          monaco.Uri.parse(url)
        );
        model.onDidChangeContent(function () {
          maybe_update_preview_automatically();
        });
        editor_documents.set(url, { model, view_state: null });
        addTab(model, url);
        return doc;
      }
    }
    return model_and_state.model.getValue();
  }

  async function render_or_error(
    source: string,
    base_url: string,
    div: HTMLDivElement
  ) {
    const style =
      (document.getElementById("style_combo") as HTMLInputElement)?.value ??
      "fluent";

    const canvas_id = "canvas_" + Math.random().toString(36).slice(2, 11);
    const canvas = document.createElement("canvas");
    canvas.width = 800;
    canvas.height = 600;
    canvas.id = canvas_id;
    div.innerHTML = "";
    div.appendChild(canvas);
    let markers = [];
    const { component, diagnostics, error_string } =
      await slint.compile_from_string_with_style(
        source,
        base_url,
        style,
        read_from_url
      );

    if (error_string != "") {
      const text = document.createTextNode(error_string);
      const p = document.createElement("pre");
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
    if (model != null) {
      monaco.editor.setModelMarkers(model, "slint", markers);
    }

    if (component !== undefined) {
      const instance = component.create(canvas_id);
      instance.show();
      if (!event_loop_started) {
        event_loop_started = true;
        slint.run_event_loop();
      }
    }
  }

  let keystroke_timeout_handle: number;

  const params = new URLSearchParams(window.location.search);
  const code = params.get("snippet");
  const load_url = params.get("load_url");

  if (code) {
    clearTabs();
    const model = createMainModel(code, "");
    addTab(model);
  } else if (load_url) {
    load_from_url(load_url);
  } else {
    clearTabs();
    base_url = "";
    const model = createMainModel(hello_world, "");
    addTab(model);
  }

  function createLanguageClient(
    transports: MessageTransports
  ): MonacoLanguageClient {
    return new MonacoLanguageClient({
      name: "Slint Language Client",
      clientOptions: {
        // use a language id as a document selector
        documentSelector: [{ language: "slint" }],
        // disable the default error handler
        errorHandler: {
          error: () => ({ action: ErrorAction.Continue }),
          closed: () => ({ action: CloseAction.DoNotRestart }),
        },
      },
      // create a language client connection to the server running in the web worker
      connectionProvider: {
        get: (_encoding: string) => {
          return Promise.resolve(transports);
        },
      },
    });
  }

  const lsp_worker = new Worker(
    new URL("worker/lsp_worker.ts", import.meta.url),
    {
      type: "module",
    }
  );

  lsp_worker.onmessage = (m) => {
    // We cannot start sending messages to the client before we start listening which
    // the server only does in a future after the wasm is loaded.
    if (m.data === "OK") {
      const writer = new BrowserMessageWriter(lsp_worker);

      const reader = new FilterProxyReader(
        new BrowserMessageReader(lsp_worker),
        (data: Message) => {
          if ((data as RequestMessage).method == "slint/load_file") {
            const request = data as RequestMessage;
            const url = (request.params as string[])[0];

            read_from_url(url).then((contents) => {
              writer.write({
                jsonrpc: request.jsonrpc,
                id: request.id,
                result: contents,
                error: undefined,
              } as ResponseMessage);
            });

            return true;
          }
          return false;
        }
      );

      const languageClient = createLanguageClient({ reader, writer });

      languageClient.start();

      reader.onClose(() => languageClient.stop());
    }
  };
})();
