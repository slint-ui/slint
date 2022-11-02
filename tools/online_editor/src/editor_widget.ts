// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino inmemory mimetypes printerdemo

import { slint_language } from "./highlighting";
import {
    PropertyQuery,
    BindingTextProvider,
    DefinitionPosition,
    lsp_range_to_editor_range,
} from "./lsp_integration";
import { FilterProxyReader } from "./proxy";
import {
    TextPosition,
    TextRange,
    PositionChangeCallback,
    DocumentAndTextPosition,
} from "./text";

import { BoxLayout, TabBar, Title, Widget } from "@lumino/widgets";
import { Message as LuminoMessage } from "@lumino/messaging";

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
    CloseAction,
    ErrorAction,
    Message,
    MessageTransports,
    MonacoLanguageClient,
    MonacoServices,
    RequestMessage,
    ResponseMessage,
} from "monaco-languageclient";

import {
    BrowserMessageReader,
    BrowserMessageWriter,
} from "vscode-languageserver-protocol/browser";

import { commands } from "vscode";
import { StandaloneServices, ICodeEditorService } from "vscode/services";

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

function createModel(
    source: string,
    uri?: monaco.Uri,
): monaco.editor.ITextModel {
    let model = null;
    if (uri != null) {
        model = monaco.editor.getModel(uri);
    }
    return model ?? monaco.editor.createModel(source, "slint", uri);
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
(self as any).MonacoEnvironment = {
    getWorker(_: unknown, _label: unknown) {
        return new Worker(
            new URL("worker/monaco_worker.mjs", import.meta.url),
            {
                type: "module",
            },
        );
    },
};

function tabTitleFromURL(url: monaco.Uri): string {
    if (url.scheme == "inmemory") {
        return "unnamed.slint";
    }
    try {
        const path = url.path;
        return path.substring(path.lastIndexOf("/") + 1);
    } catch (e) {
        return url.toString();
    }
}

type PropertyDataNotifier = (
    _binding_text_provider: BindingTextProvider,
    _p: PropertyQuery,
) => void;

class EditorPaneWidget extends Widget {
    auto_compile = true;
    #style = "fluent-light";
    #editor_view_states: Map<
        monaco.Uri,
        monaco.editor.ICodeEditorViewState | null | undefined
    >;
    #editor: monaco.editor.IStandaloneCodeEditor | null;
    #client?: MonacoLanguageClient;
    #keystroke_timeout_handle?: number;
    #base_url?: string;
    #edit_era: number;
    #disposables: monaco.IDisposable[] = [];
    #current_properties = "";

    onPositionChangeCallback: PositionChangeCallback = (
        _pos: DocumentAndTextPosition,
    ) => {
        return;
    };
    onNewPropertyData: PropertyDataNotifier | null = null;

    readonly editor_ready: Promise<void>;

    #onRenderRequest?: (
        _style: string,
        _source: string,
        _url: string,
        _fetch: (_url: string) => Promise<string>,
    ) => Promise<monaco.editor.IMarkerData[]>;

    #onModelRemoved?: (_url: monaco.Uri) => void;
    #onModelAdded?: (_url: monaco.Uri) => void;
    #onModelSelected?: (_url: monaco.Uri) => void;
    #onModelsCleared?: () => void;

    static createNode(): HTMLElement {
        const node = document.createElement("div");
        const content = document.createElement("div");
        node.appendChild(content);

        return node;
    }

    constructor() {
        const node = EditorPaneWidget.createNode();

        super({ node: node });
        this.#editor = null;

        this.#editor_view_states = new Map();
        this.setFlag(Widget.Flag.DisallowLayout);
        this.addClass("content");
        this.addClass("editor");
        this.title.label = "Editor";
        this.title.closable = false;
        this.title.caption = `Slint Code Editor`;

        this.#edit_era = 0;

        this.editor_ready = this.setup_editor(this.contentNode);

        monaco.editor.onDidCreateModel((model) =>
            this.add_model_listener(model),
        );
    }

    dispose() {
        this.#disposables.forEach((d: monaco.IDisposable) => d.dispose());
        this.#disposables = [];
        this.#editor?.dispose();
        this.#editor = null;
        super.dispose();
    }

    protected get contentNode(): HTMLDivElement {
        return this.node.getElementsByTagName("div")[0] as HTMLDivElement;
    }

    get current_editor_content(): string {
        return this.#editor?.getModel()?.getValue() || "";
    }

    get supported_actions(): string[] | undefined {
        return this.#editor?.getSupportedActions().map((a) => a.id);
    }

    get supported_commands(): Thenable<string[]> {
        return commands.getCommands();
    }

    get language_client(): MonacoLanguageClient | undefined {
        return this.#client;
    }

    get current_text_document_uri(): string | undefined {
        return this.#editor?.getModel()?.uri.toString();
    }

    goto_position(uri: string, position: TextPosition | TextRange) {
        const uri_ = monaco.Uri.parse(uri);
        let selection: monaco.Range;
        if (monaco.Range.isIRange(position)) {
            selection = monaco.Range.lift(position as TextRange);
        } else {
            selection = new monaco.Range(
                position.lineNumber,
                position.column,
                position.lineNumber,
                position.column,
            );
        }

        if (!this.set_model(uri_)) {
            return;
        }

        this.#editor?.setSelection(selection);
        this.#editor?.revealLine(selection.startLineNumber);
    }

    compile() {
        this.update_preview();
    }

    next_era() {
        this.#edit_era += 1;
    }

    set style(value: string) {
        this.#style = value;
        this.update_preview();
    }

    get style() {
        return this.#style;
    }

    public clear_models() {
        this.next_era();
        this.#editor_view_states.clear();
        monaco.editor.getModels().forEach((model) => model.dispose());
        this.#onModelsCleared?.();
    }

    private resize_editor() {
        if (this.#editor != null) {
            const width = this.contentNode.offsetWidth;
            const height = this.contentNode.offsetHeight;
            this.#editor.layout({ width, height });
        }
    }

    get position(): DocumentAndTextPosition {
        const uri = this.#editor?.getModel()?.uri.toString() ?? "";
        const position = this.#editor?.getPosition() ?? {
            lineNumber: 1,
            column: 1,
        };

        return { uri: uri, position: position };
    }

    private add_model_listener(model: monaco.editor.ITextModel) {
        const uri = model.uri;
        model.onDidChangeContent(() => {
            this.maybe_update_preview_automatically();
        });
        this.#editor_view_states.set(uri, null);
        this.#onModelAdded?.(uri);
        if (monaco.editor.getModels().length === 1) {
            this.#base_url = uri.toString();
            this.set_model(uri);
            this.update_preview();
        }
    }

    public remove_model(uri: monaco.Uri) {
        this.#editor_view_states.delete(uri);
        const model = monaco.editor.getModel(uri);
        if (model != null) {
            model.dispose();
            this.#onModelRemoved?.(uri);
        }
    }

    public set_model(uri: monaco.Uri): boolean {
        const current_model = this.#editor?.getModel();
        if (current_model != null) {
            this.#editor_view_states.set(uri, this.#editor?.saveViewState());
        }

        const state = this.#editor_view_states.get(uri);
        if (this.#editor != null) {
            this.#editor.setModel(monaco.editor.getModel(uri));
            if (state != null) {
                this.#editor.restoreViewState(state);
            }
            this.#editor.focus();
            this.#onModelSelected?.(uri);
            return true;
        }
        return false;
    }

    protected onResize(_msg: LuminoMessage): void {
        if (this.isAttached) {
            this.resize_editor();
        }
    }

    protected update_preview() {
        const model = this.#editor?.getModel();
        if (model != null) {
            const source = model.getValue();
            const era = this.#edit_era;

            setTimeout(() => {
                if (this.#onRenderRequest != null) {
                    this.#onRenderRequest(
                        this.#style,
                        source,
                        this.#base_url ?? "",
                        (url: string) => {
                            const uri = monaco.Uri.parse(url);
                            return this.fetch_url_content(era, uri);
                        },
                    ).then((markers: monaco.editor.IMarkerData[]) => {
                        if (this.#editor != null) {
                            const model = this.#editor.getModel();
                            if (model != null) {
                                monaco.editor.setModelMarkers(
                                    model,
                                    "slint",
                                    markers,
                                );
                            }
                        }
                    });
                }
            }, 1);
        }
    }

    protected maybe_update_preview_automatically() {
        if (this.auto_compile) {
            if (this.#keystroke_timeout_handle != null) {
                clearTimeout(this.#keystroke_timeout_handle);
            }
            this.#keystroke_timeout_handle = setTimeout(() => {
                this.update_preview();
            }, 500);
        }
    }

    private async setup_editor(container: HTMLDivElement): Promise<void> {
        container.classList.add("edit-area");

        monaco.languages.register({
            id: "slint",
            extensions: [".slint"],
            aliases: ["Slint", "slint"],
            mimetypes: ["application/slint"],
        });
        monaco.languages.onLanguage("slint", () => {
            monaco.languages.setMonarchTokensProvider("slint", slint_language);
        });
        MonacoServices.install();

        const code_editor_service = StandaloneServices.get(ICodeEditorService);
        this.#disposables.push(
            code_editor_service.registerCodeEditorOpenHandler(
                (
                    { resource, options },
                    source: monaco.editor.ICodeEditor | null,
                    _sideBySide?: boolean,
                ): Promise<monaco.editor.ICodeEditor | null> => {
                    if (editor == null) {
                        return Promise.resolve(editor);
                    }

                    if (!this.set_model(resource)) {
                        return Promise.resolve(null);
                    }

                    if (options != null && options.selection != undefined) {
                        editor.setSelection(options.selection as monaco.IRange);
                        editor.revealLine(options.selection.startLineNumber);
                    }

                    return Promise.resolve(source);
                },
            ),
        );

        const editor = monaco.editor.create(container, {
            language: "slint",
            glyphMargin: true,
            lightbulb: {
                enabled: true,
            },
        });

        this.#editor = editor;

        this.#disposables.push(
            editor.onDidChangeCursorPosition(
                (pos: monaco.editor.ICursorPositionChangedEvent) => {
                    const model = editor.getModel();
                    if (model != null) {
                        this.onPositionChangeCallback({
                            uri: model.uri.toString(),
                            position: pos.position,
                        });

                        // TODO: Move this into an onPositionChangeCallback?
                        const offset =
                            model.getOffsetAt(pos.position) -
                            model.getOffsetAt({
                                lineNumber: pos.position.lineNumber,
                                column: 0,
                            });
                        const uri = model.uri;

                        commands
                            .executeCommand(
                                "queryProperties",
                                uri.toString(),
                                pos.position.lineNumber - 1,
                                offset,
                            )
                            .then((r) => {
                                const result = r as PropertyQuery;
                                const result_str = JSON.stringify(result);
                                if (this.#current_properties != result_str) {
                                    this.#current_properties = result_str;
                                    this.onNewPropertyData?.(
                                        new ModelBindingTextProvider(model),
                                        result,
                                    );
                                }
                            });
                    }
                },
            ),
        );

        function createLanguageClient(
            transports: MessageTransports,
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
            { type: "module" },
        );

        const ensure_lsp_running = new Promise<void>(
            (resolve_lsp_worker_promise) => {
                lsp_worker.onmessage = (m) => {
                    // We cannot start sending messages to the client before we start listening which
                    // the server only does in a future after the wasm is loaded.
                    if (m.data === "OK") {
                        const reader = new FilterProxyReader(
                            new BrowserMessageReader(lsp_worker),
                            (data: Message) => {
                                if (
                                    (data as RequestMessage).method ==
                                    "slint/load_file"
                                ) {
                                    const request = data as RequestMessage;
                                    const url = (request.params as string[])[0];

                                    this.read_from_url(url).then(
                                        (text_contents) => {
                                            writer.write({
                                                jsonrpc: request.jsonrpc,
                                                id: request.id,
                                                result: text_contents,
                                                error: undefined,
                                            } as ResponseMessage);
                                        },
                                    );

                                    return true;
                                }
                                return false;
                            },
                        );
                        const writer = new BrowserMessageWriter(lsp_worker);

                        this.#client = createLanguageClient({ reader, writer });

                        this.#client.start();

                        reader.onClose(() => {
                            this.#client?.stop();
                            this.#client = undefined;
                        });

                        resolve_lsp_worker_promise();
                    }
                };
            },
        );

        await ensure_lsp_running;
    }

    set onRenderRequest(
        request: (
            _style: string,
            _source: string,
            _url: string,
            _fetch: (_url: string) => Promise<string>,
        ) => Promise<monaco.editor.IMarkerData[]>,
    ) {
        this.#onRenderRequest = request;
    }

    set onModelsCleared(f: () => void) {
        this.#onModelsCleared = f;
    }

    set onModelAdded(f: (_url: monaco.Uri) => void) {
        this.#onModelAdded = f;
    }
    set onModelRemoved(f: (_url: monaco.Uri) => void) {
        this.#onModelRemoved = f;
    }
    set onModelSelected(f: (_url: monaco.Uri) => void) {
        this.#onModelSelected = f;
    }

    protected async fetch_url_content(
        era: number,
        uri: monaco.Uri,
    ): Promise<string> {
        let model = monaco.editor.getModel(uri);
        if (model != null) {
            return model.getValue();
        }

        const response = await fetch(uri.toString());
        if (!response.ok) {
            return "Failed to access URL: " + response.statusText;
        }
        const doc = await response.text();

        model = monaco.editor.getModel(uri);
        if (model != null) {
            return model.getValue();
        }

        if (era == this.#edit_era) {
            createModel(doc, uri);
        }
        return doc;
    }

    replace_text(
        uri: string,
        range: TextRange,
        new_text: string,
        validate: (_old: string) => boolean,
    ): boolean {
        const model = monaco.editor.getModel(monaco.Uri.parse(uri));

        if (model !== null) {
            const old_text = model.getValueInRange(range);
            if (validate(old_text)) {
                model.pushEditOperations(
                    [],
                    [
                        {
                            forceMoveMarkers: true,
                            range: range,
                            text: new_text,
                        },
                    ],
                    (_) => {
                        return [];
                    },
                );
                return true;
            }
        }

        return false;
    }

    async read_from_url(url: string): Promise<string> {
        const uri = monaco.Uri.parse(url);
        return this.fetch_url_content(this.#edit_era, uri);
    }
}

export class EditorWidget extends Widget {
    #tab_bar: TabBar<Widget>;
    #editor: EditorPaneWidget;
    #tab_map: Map<monaco.Uri, Title<Widget>>;

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
        this.#tab_map = new Map();

        const layout = new BoxLayout({ spacing: 0 });

        this.#tab_bar = new TabBar<Widget>({ name: "Open Documents Tab Bar" });
        layout.addWidget(this.#tab_bar);

        this.#editor = new EditorPaneWidget();
        layout.addWidget(this.#editor);

        this.layout = layout;

        this.#editor.onModelsCleared = () => {
            this.#tab_bar.clearTabs();
            this.#tab_map.clear();
        };
        this.#editor.onModelAdded = (url: monaco.Uri) => {
            const title = this.#tab_bar.addTab({
                owner: this,
                label: tabTitleFromURL(url),
            });
            this.#tab_map.set(url, title);
        };
        this.#editor.onModelRemoved = (url: monaco.Uri) => {
            const title = this.#tab_map.get(url);
            if (title != null) {
                this.#tab_bar.removeTab(title);
                this.#tab_map.delete(url);
            }
        };
        this.#editor.onModelSelected = (url: monaco.Uri) => {
            const title = this.#tab_map.get(url);
            if (title != null && this.#tab_bar.currentTitle != title) {
                this.#tab_bar.currentTitle = title;
            }
        };
        this.#tab_bar.currentChanged.connect(
            (_: TabBar<Widget>, args: TabBar.ICurrentChangedArgs<Widget>) => {
                const title = args.currentTitle;

                for (const [url, value] of this.#tab_map.entries()) {
                    if (value === title) {
                        this.#editor.set_model(url);
                    }
                }
            },
        );

        const params = new URLSearchParams(window.location.search);
        const code = params.get("snippet");
        const load_url = params.get("load_url");

        if (code) {
            this.#editor.clear_models();
            createModel(code);
        } else if (load_url) {
            this.load_from_url(load_url);
        } else {
            this.#editor.clear_models();
            createModel(hello_world);
        }
    }

    get current_editor_content(): string {
        return this.#editor.current_editor_content;
    }

    get language_client(): MonacoLanguageClient | undefined {
        return this.#editor.language_client;
    }

    get current_text_document_uri(): string | undefined {
        return this.#editor.current_text_document_uri;
    }

    compile() {
        this.#editor.compile();
    }

    set auto_compile(value: boolean) {
        this.#editor.auto_compile = value;
    }

    get auto_compile() {
        return this.#editor.auto_compile;
    }

    set style(value: string) {
        this.#editor.style = value;
    }

    get style() {
        return this.#editor.style;
    }

    get editor_ready() {
        return this.#editor.editor_ready;
    }

    get supported_actions(): string[] | undefined {
        return this.#editor.supported_actions;
    }

    get supported_commands(): Thenable<string[]> {
        return this.#editor.supported_commands;
    }

    protected async load_from_url(url: string) {
        this.#editor.clear_models();
        await this.#editor.read_from_url(url);
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

    goto_position(uri: string, position: TextPosition | TextRange) {
        this.#editor?.goto_position(uri, position);
    }

    async set_demo(location: string) {
        if (location) {
            let tag = "master";
            {
                let found;
                if (
                    (found = window.location.pathname.match(
                        /releases\/([^/]*)\/editor/,
                    ))
                ) {
                    tag = "v" + found[1];
                }
            }
            await this.load_from_url(
                `https://raw.githubusercontent.com/slint-ui/slint/${tag}/${location}`,
            );
        } else {
            this.#editor.clear_models();
            createModel(hello_world);
        }
    }

    set onRenderRequest(
        request: (
            _style: string,
            _source: string,
            _url: string,
            _fetch: (_url: string) => Promise<string>,
        ) => Promise<monaco.editor.IMarkerData[]>,
    ) {
        this.#editor.onRenderRequest = request;
    }

    set onNewPropertyData(handler: PropertyDataNotifier) {
        this.#editor.onNewPropertyData = handler;
    }

    set onPositionChange(cb: PositionChangeCallback) {
        this.#editor.onPositionChangeCallback = cb;
    }

    replace_text(
        uri: string,
        range: TextRange,
        new_text: string,
        validator: (_old: string) => boolean,
    ): boolean {
        return this.#editor.replace_text(uri, range, new_text, validator);
    }

    get position(): DocumentAndTextPosition {
        return this.#editor.position;
    }
}

class ModelBindingTextProvider implements BindingTextProvider {
    #model: monaco.editor.ITextModel;
    constructor(model: monaco.editor.ITextModel) {
        this.#model = model;
    }

    binding_text(location: DefinitionPosition): string {
        const monaco_range = lsp_range_to_editor_range(
            this.#model,
            location.expression_range,
        );

        if (monaco_range == null) {
            return "";
        }

        return this.#model.getValueInRange(monaco_range);
    }
}
