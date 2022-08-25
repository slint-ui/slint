// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino mimetypes printerdemo

import { BoxLayout, TabBar, Title, Widget } from "@lumino/widgets";
import { Message } from "@lumino/messaging";

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
} from "monaco-languageclient";

import {
  BrowserMessageReader,
  BrowserMessageWriter,
} from "vscode-languageserver-protocol/browser";

interface ModelAndViewState {
  model: monaco.editor.ITextModel;
  view_state: monaco.editor.ICodeEditorViewState | null;
}

const hello_world = `import { Button, VerticalBox } from "std-widgets.slint";
export Demo := Window {
    VerticalBox {
        alignment: start;
        Text {
            text: "Hello World!";
            font-size: 24px;
            horizontal-alignment: center;
        }
        Image {
            source: @image-url("https://slint-ui.com/logo/slint-logo-full-light.svg");
            height: 100px;
        }
        HorizontalLayout { alignment: center; Button { text: "OK!"; } }
    }
}
`;

function createModel(source: string): monaco.editor.ITextModel {
  return monaco.editor.createModel(source, "slint");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
(self as any).MonacoEnvironment = {
  getWorker(_: unknown, _label: unknown) {
    return new Worker(new URL("worker/monaco_worker.mjs", import.meta.url), {
      type: "module",
    });
  },
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

class EditorPaneWidget extends Widget {
  private _auto_compile = true;
  private _style = "fluent";
  private _editor_documents: Map<string, ModelAndViewState>;
  private _editor: monaco.editor.IStandaloneCodeEditor | undefined;
  private _keystroke_timeout_handle: number | undefined;
  private _base_url: string | undefined;

  private _onRenderRequest?: (
    _style: string,
    _source: string,
    _url: string,
    _fetch: (_url: string) => Promise<string>
  ) => Promise<monaco.editor.IMarkerData[]>;

  private _onModelRemoved?: (_url: string) => void;
  private _onModelAdded?: (_url: string) => void;
  private _onModelSelected?: (_url: string) => void;
  private _onModelsCleared?: () => void;

  static createNode(): HTMLElement {
    const node = document.createElement("div");
    const content = document.createElement("div");
    node.appendChild(content);

    return node;
  }

  constructor() {
    const node = EditorPaneWidget.createNode();

    super({ node: node });
    this._editor_documents = new Map();
    this.setFlag(Widget.Flag.DisallowLayout);
    this.addClass("content");
    this.addClass("editor");
    this.title.label = "Editor";
    this.title.closable = false;
    this.title.caption = `Slint code editor`;

    this.setup_editor(this.contentNode);
  }

  protected get contentNode(): HTMLDivElement {
    return this.node.getElementsByTagName("div")[0] as HTMLDivElement;
  }

  get current_editor_content(): string {
    return this._editor?.getModel()?.getValue() || "";
  }

  compile() {
    this.update_preview();
  }

  set auto_compile(value: boolean) {
    this._auto_compile = value;
  }

  get auto_compile() {
    return this._auto_compile;
  }

  set style(value: string) {
    this._style = value;
    this.update_preview();
  }

  get style() {
    return this._style;
  }

  public clear_models() {
    console.log("Clearing all models!");
    this._editor_documents.clear();
    if (this._onModelsCleared != null) {
      this._onModelsCleared();
    }
  }

  private resize_editor() {
    if (this._editor != null) {
      const width = this.contentNode.offsetWidth;
      const height = this.contentNode.offsetHeight;
      this._editor.layout({ width, height });
    }
  }

  public add_model(url: string, model: monaco.editor.ITextModel) {
    console.log("Adding model for url", url);
    if (this._editor_documents.get(url) != null) {
      console.log("  => Already known!");
      return; // already know that URL
    }
    model.onDidChangeContent(() => {
      this.maybe_update_preview_automatically();
    });
    this._editor_documents.set(url, { model, view_state: null });
    if (this._onModelAdded != null) {
      this._onModelAdded(url);
    }
    if (this._editor_documents.size === 1) {
      this._base_url = url;
      this.set_model(url);
    }
    this.update_preview();
  }

  public remove_model(url: string) {
    console.log("Removing model for url", url);
    this._editor_documents.delete(url);
    if (this._onModelRemoved != null) {
      this._onModelRemoved(url);
    }
  }

  public set_model(url: string): boolean {
    console.log("Setting model for url", url);
    const model_and_state = this._editor_documents.get(url);
    if (model_and_state != null && this._editor != null) {
      this._editor.setModel(model_and_state.model);
      if (model_and_state.view_state != null) {
        this._editor.restoreViewState(model_and_state.view_state);
      }
      this._editor.focus();
      if (this._onModelSelected != null) {
        this._onModelSelected(url);
      }
      return true;
    }
    return false;
  }

  protected onResize(_msg: Message): void {
    if (this.isAttached) {
      this.resize_editor();
    }
  }

  protected update_preview() {
    const base_url = this._base_url != null ? this._base_url : "";
    const main_model_and_state = this._editor_documents.get(base_url);

    if (main_model_and_state != null) {
      const source = main_model_and_state.model.getValue();

      setTimeout(() => {
        if (this._onRenderRequest != null) {
          this._onRenderRequest(
            this._style,
            source,
            base_url,
            (url: string) => {
              return this.fetch_url_content(url);
            }
          ).then((markers: monaco.editor.IMarkerData[]) => {
            if (this._editor != null) {
              const model = this._editor.getModel();
              if (model != null) {
                monaco.editor.setModelMarkers(model, "slint", markers);
              }
            }
          });
        }
      }, 1);
    }
  }

  protected maybe_update_preview_automatically() {
    if (this.auto_compile) {
      console.log("Refreshing auto_compile timeout.");
      if (this._keystroke_timeout_handle != null) {
        clearTimeout(this._keystroke_timeout_handle);
      }
      this._keystroke_timeout_handle = setTimeout(() => {
        this.update_preview();
      }, 500);
    }
  }

  private async setup_editor(container: HTMLDivElement): Promise<void> {
    container.className += "edit-area";

    monaco.languages.register({
      id: "slint",
      extensions: [".slint"],
      aliases: ["Slint", "slint"],
      mimetypes: ["application/slint"],
    });
    monaco.languages.onLanguage("slint", () => {
      monaco.languages.setMonarchTokensProvider("slint", slint_language);
    });

    this._editor = monaco.editor.create(container, {
      language: "slint",
      glyphMargin: true,
      lightbulb: {
        enabled: true,
      },
    });
    MonacoServices.install();

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
      { type: "module" }
    );
    let worker_is_set_up = false;
    lsp_worker.onmessage = (m) => {
      // We cannot start sending messages to the client before we start listening which
      // the server only does in a future after the wasm is loaded.
      if (m.data === "OK") {
        const reader = new BrowserMessageReader(lsp_worker);
        const writer = new BrowserMessageWriter(lsp_worker);

        const languageClient = createLanguageClient({ reader, writer });

        languageClient.start();

        reader.onClose(() => languageClient.stop());

        worker_is_set_up = true;
      }
    };

    // Wait a bit for the worker to come up... leaving the loader in place;-)
    for (let i = 0; i < 10; ++i) {
      if (!worker_is_set_up) {
        await new Promise((f) => setTimeout(f, 1000));
      } else {
        break;
      }
    }
  }

  set onRenderRequest(
    request: (
      _style: string,
      _source: string,
      _url: string,
      _fetch: (_url: string) => Promise<string>
    ) => Promise<monaco.editor.IMarkerData[]>
  ) {
    this._onRenderRequest = request;
  }

  set onModelsCleared(f: () => void) {
    this._onModelsCleared = f;
  }

  set onModelAdded(f: (_url: string) => void) {
    this._onModelAdded = f;
  }
  set onModelRemoved(f: (_url: string) => void) {
    this._onModelRemoved = f;
  }
  set onModelSelected(f: (_url: string) => void) {
    this._onModelSelected = f;
  }

  protected async fetch_url_content(url: string): Promise<string> {
    console.log("Fetching ", url);

    let model_and_state = this._editor_documents.get(url);
    if (model_and_state != null) {
      console.log("*** Fetch shortcut!");
      return model_and_state.model.getValue();
    }

    const response = await fetch(url);
    const doc = await response.text();

    model_and_state = this._editor_documents.get(url);
    if (model_and_state != null) {
      console.log("*** Not storing again...");
      return model_and_state.model.getValue();
    }

    const model = createModel(doc);
    this.add_model(url, model);
    return doc;
  }
}

export class EditorWidget extends Widget {
  private tab_bar: TabBar<Widget>;
  private editor: EditorPaneWidget;
  private tab_map: Map<string, Title<Widget>>;

  private static createNode(): HTMLDivElement {
    const node = document.createElement("div");
    const content = document.createElement("ul");
    node.appendChild(content);

    return node;
  }

  constructor() {
    super({ node: EditorWidget.createNode() });
    this.title.label = "Editor";
    this.title.closable = false;
    this.title.caption = `Slint code editor`;
    this.tab_map = new Map();

    const layout = new BoxLayout({ spacing: 0 });

    this.tab_bar = new TabBar<Widget>({ name: "Open Documents Tab Bar" });
    layout.addWidget(this.tab_bar);

    this.editor = new EditorPaneWidget();
    layout.addWidget(this.editor);

    this.layout = layout;

    this.editor.onModelsCleared = () => {
      console.log("Clearing all Tabs");
      this.tab_bar.clearTabs();
      this.tab_map.clear();
    };
    this.editor.onModelAdded = (url: string) => {
      console.log("Adding Tab for", url);
      const title = this.tab_bar.addTab({
        owner: this,
        label: tabTitleFromURL(url),
      });
      this.tab_map.set(url, title);
    };
    this.editor.onModelRemoved = (url: string) => {
      console.log("Removing Tab for", url);
      const title = this.tab_map.get(url);
      if (title != null) {
        this.tab_bar.removeTab(title);
        this.tab_map.delete(url);
      }
    };
    this.editor.onModelSelected = (url: string) => {
      console.log("Selecting Tab for", url);
      const title = this.tab_map.get(url);
      if (title != null && this.tab_bar.currentTitle != title) {
        this.tab_bar.currentTitle = title;
      }
    };
    this.tab_bar.currentChanged.connect(
      (_: TabBar<Widget>, args: TabBar.ICurrentChangedArgs<Widget>) => {
        const title = args.currentTitle;

        for (const [url, value] of this.tab_map.entries()) {
          if (value === title) {
            this.editor.set_model(url);
          }
        }
      }
    );

    const params = new URLSearchParams(window.location.search);
    const code = params.get("snippet");
    const load_url = params.get("load_url");

    if (code) {
      this.editor.clear_models();
      this.editor.add_model("", createModel(code));
    } else if (load_url) {
      this.load_from_url(load_url);
    } else {
      this.editor.clear_models();
      this.editor.add_model("", createModel(hello_world));
    }
  }

  get current_editor_content(): string {
    return this.editor.current_editor_content;
  }

  compile() {
    this.editor.compile();
  }

  set auto_compile(value: boolean) {
    this.editor.auto_compile = value;
  }

  get auto_compile() {
    return this.editor.auto_compile;
  }

  set style(value: string) {
    this.editor.style = value;
  }

  get style() {
    return this.editor.style;
  }

  protected load_from_url(url: string) {
    this.editor.clear_models();
    fetch(url).then((x) =>
      x.text().then((y) => {
        const model = createModel(y);
        this.editor.add_model(url, model);
      })
    );
  }

  known_demos(): [string, string][] {
    return [
      ["", "Hello World!"],
      ["examples/gallery/gallery.slint", "Gallery"],
      ["examples/printerdemo/ui/printerdemo.slint", "Printer Demo"],
      ["examples/todo/ui/todo.slint", "Todo Demo"],
      ["examples/iot-dashboard/main.slint", "IOT Dashboard"],
    ];
  }

  set_demo(location: string) {
    if (location) {
      let tag = "master";
      {
        let found;
        if (
          (found = window.location.pathname.match(/releases\/([^/]*)\/editor/))
        ) {
          tag = "v" + found[1];
        }
      }
      this.load_from_url(
        `https://raw.githubusercontent.com/slint-ui/slint/${tag}/${location}`
      );
    } else {
      this.editor.clear_models();
      const model = createModel(hello_world);
      this.editor.add_model("", model);
    }
  }

  set onRenderRequest(
    request: (
      _style: string,
      _source: string,
      _url: string,
      _fetch: (_url: string) => Promise<string>
    ) => Promise<monaco.editor.IMarkerData[]>
  ) {
    this.editor.onRenderRequest = request;
  }
}
