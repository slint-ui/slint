// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino inmemory mimetypes printerdemo

import { slint_language } from "./highlighting";
import {
    LspPosition,
    LspRange,
    editor_position_to_lsp_position,
    lsp_range_to_editor_range,
} from "./lsp_integration";
import { Lsp } from "./lsp";
import { PositionChangeCallback, VersionedDocumentAndPosition } from "./text";

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

import { MonacoLanguageClient, MonacoServices } from "monaco-languageclient";

import { commands } from "vscode";
import { StandaloneServices, ICodeEditorService } from "vscode/services";

const hello_world = `import { Button, VerticalBox } from "std-widgets.slint";
export component Demo {
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

function internal_file_uri(uuid: string, file_name: string): monaco.Uri {
    console.assert(file_name.startsWith("/"));
    return monaco.Uri.from({
        scheme: "https",
        authority: uuid + ".slint.rs",
        path: file_name,
    });
}

function is_internal_uri(uuid: string, uri: monaco.Uri): boolean {
    return uri.scheme === "https" && uri.authority === uuid + ".slint.rs";
}

function file_from_internal_uri(uuid: string, uri: monaco.Uri): string {
    console.assert(is_internal_uri(uuid, uri));
    return uri.path;
}

interface UrlMapper {
    from_internal(_uri: monaco.Uri): monaco.Uri | null;
}

class KnownUrlMapper implements UrlMapper {
    #map: { [path: string]: monaco.Uri };
    #uuid: string;

    constructor(uuid: string, map: { [path: string]: monaco.Uri }) {
        this.#uuid = uuid;
        this.#map = map;
        console.assert(Object.keys(map).length > 0);
        Object.keys(map).forEach((k) => console.assert(k.startsWith("/")));
    }

    from_internal(uri: monaco.Uri): monaco.Uri | null {
        const file_path = file_from_internal_uri(this.#uuid, uri);

        return this.#map[file_path] || null;
    }
}

class RelativeUrlMapper implements UrlMapper {
    #base_uri: monaco.Uri;
    #base_path: string;
    #uuid: string;

    constructor(uuid: string, uri: monaco.Uri) {
        const path = uri.path;

        this.#uuid = uuid;
        this.#base_path = path.slice(0, path.lastIndexOf("/") ?? 0);
        console.assert(this.#base_path.endsWith("/"));
        this.#base_uri = uri;
    }

    from_internal(uri: monaco.Uri): monaco.Uri | null {
        return monaco.Uri.from({
            scheme: this.#base_uri.scheme,
            authority: this.#base_uri.authority,
            path: this.#base_path + file_from_internal_uri(this.#uuid, uri),
        });
    }
}

function createModel(
    uuid: string,
    source: string,
    uri?: monaco.Uri,
): monaco.editor.ITextModel {
    const url = uri ?? internal_file_uri(uuid, "/main.slint");
    console.assert(is_internal_uri(uuid, url));

    const model = monaco.editor.getModel(url);
    return model ?? monaco.editor.createModel(source, "slint", url);
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
    try {
        const path = url.path;
        return path.substring(path.lastIndexOf("/") + 1);
    } catch (e) {
        return url.toString();
    }
}

class EditorPaneWidget extends Widget {
    auto_compile = true;
    #style = "fluent-light";
    #main_uri: monaco.Uri | null = null;
    #editor_view_states: Map<
        monaco.Uri,
        monaco.editor.ICodeEditorViewState | null | undefined
    >;
    #editor: monaco.editor.IStandaloneCodeEditor | null = null;
    #client: MonacoLanguageClient | null = null;
    #keystroke_timeout_handle?: number;
    #url_mapper: UrlMapper | null = null;
    #edit_era: number;
    #disposables: monaco.IDisposable[] = [];
    #internal_uuid = self.crypto.randomUUID();

    #service_worker_port: MessagePort;

    onPositionChangeCallback: PositionChangeCallback = (
        _pos: VersionedDocumentAndPosition,
    ) => {
        return;
    };

    #onRenderRequest?: (
        _style: string,
        _source: string,
        _url: string,
        _fetch: (_url: string) => Promise<string>,
    ) => Promise<monaco.editor.IMarkerData[]>;

    #onModelRemoved?: (_url: monaco.Uri) => void;
    #onModelAdded?: (_url: monaco.Uri) => void;
    #onModelSelected?: (_url: monaco.Uri | null) => void;
    #onModelsCleared?: () => void;

    static createNode(): HTMLElement {
        const node = document.createElement("div");
        const content = document.createElement("div");
        node.appendChild(content);

        return node;
    }

    constructor(lsp: Lsp) {
        const node = EditorPaneWidget.createNode();

        super({ node: node });

        this.#editor_view_states = new Map();
        this.setFlag(Widget.Flag.DisallowLayout);
        this.addClass("content");
        this.addClass("editor");
        this.title.label = "Editor";
        this.title.closable = false;
        this.title.caption = `Slint Code Editor`;

        this.#edit_era = 0;

        this.#client = this.setup_editor(this.contentNode, lsp);

        lsp.file_reader = (url) => {
            return this.handle_lsp_url_request(this.#edit_era, url);
        };

        monaco.editor.onDidCreateModel((model) =>
            this.add_model_listener(model),
        );

        const sw_channel = new MessageChannel();
        sw_channel.port1.onmessage = (m) => {
            if (m.data.type === "MapUrl") {
                const reply_port = m.ports[0];
                const internal_uri = monaco.Uri.parse(m.data.url);
                const mapped_url =
                    this.#url_mapper?.from_internal(internal_uri)?.toString() ??
                    "";
                const file = file_from_internal_uri(
                    this.#internal_uuid,
                    internal_uri,
                );
                reply_port.postMessage(mapped_url);
            } else {
                console.error(
                    "Unknown message received from service worker:",
                    m.data,
                );
            }
        };
        if (navigator.serviceWorker.controller == null) {
            console.error("No active service worker!");
        } else {
            navigator.serviceWorker.controller.postMessage(
                { type: "EditorOpened", url_prefix: this.internal_url_prefix },
                [sw_channel.port2],
            );
        }
        this.#service_worker_port = sw_channel.port1;
    }

    dispose() {
        this.#service_worker_port.close();
        this.#disposables.forEach((d: monaco.IDisposable) => d.dispose());
        this.#disposables = [];
        this.#editor?.dispose();
        this.#editor = null;
        super.dispose();
    }

    get internal_uuid(): string {
        return this.#internal_uuid;
    }

    get internal_url_prefix(): string {
        return internal_file_uri(this.#internal_uuid, "/").toString();
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

    get language_client(): MonacoLanguageClient | null {
        return this.#client;
    }

    get current_text_document_uri(): string | undefined {
        return this.#editor?.getModel()?.uri.toString();
    }

    get current_text_document_version(): number | undefined {
        return this.#editor?.getModel()?.getVersionId();
    }

    goto_position(uri: string, position: LspPosition | LspRange) {
        const uri_ = monaco.Uri.parse(uri);
        if (!this.set_model(uri_)) {
            return;
        }

        const model = this.#editor?.getModel();

        let start: LspPosition;
        let end: LspPosition;

        if ("start" in position) {
            start = position.start;
            end = position.end;
        } else {
            start = position;
            end = position;
        }

        const selection = lsp_range_to_editor_range(model, {
            start: start,
            end: end,
        });

        if (selection != null) {
            this.#editor?.setSelection(selection);
            this.#editor?.revealLine(selection.startLineNumber);
        }
    }

    compile() {
        this.update_preview();
    }

    set style(value: string) {
        this.#style = value;
        this.update_preview();
    }

    get style() {
        return this.#style;
    }

    public clear_models() {
        this.#edit_era += 1;
        this.#url_mapper = null;
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

    get position(): VersionedDocumentAndPosition {
        const model = this.#editor?.getModel();
        const version = model?.getVersionId() ?? -1;
        const uri = model?.uri.toString() ?? "";
        const position = editor_position_to_lsp_position(
            model,
            this.#editor?.getPosition(),
        ) ?? {
            line: 0,
            character: 0,
        };

        return { uri: uri, position: position, version: version };
    }

    private add_model_listener(model: monaco.editor.ITextModel) {
        const uri = model.uri;
        model.onDidChangeContent(() => {
            this.maybe_update_preview_automatically();
        });
        this.#editor_view_states.set(uri, null);
        this.#onModelAdded?.(uri);
        if (monaco.editor.getModels().length === 1) {
            this.#main_uri = uri;
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
        const model = monaco.editor.getModel(
            this.#main_uri ?? new monaco.Uri(),
        );
        if (model != null) {
            const source = model.getValue();
            const era = this.#edit_era;

            setTimeout(() => {
                if (this.#onRenderRequest != null) {
                    this.#onRenderRequest(
                        this.#style,
                        source,
                        this.#main_uri?.toString() ?? "",
                        (url: string) => {
                            return this.handle_lsp_url_request(era, url);
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

    private setup_editor(
        container: HTMLDivElement,
        lsp: Lsp,
    ): MonacoLanguageClient {
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

        const original_set_model = editor.setModel;
        editor.setModel = (model: monaco.editor.ITextModel) => {
            const current_model = editor.getModel();
            if (current_model != null) {
                this.#editor_view_states.set(
                    current_model.uri,
                    editor.saveViewState(),
                );
            }

            const state = this.#editor_view_states.get(model?.uri);
            original_set_model.apply(editor, [model]);
            if (state != null) {
                editor.restoreViewState(state);
            }
        };

        this.#editor = editor;

        this.#disposables.push(
            editor.onDidChangeCursorPosition((_) =>
                this.onPositionChangeCallback(this.position),
            ),
        );
        this.#disposables.push(
            editor.onDidChangeModel((event) => {
                this.onPositionChangeCallback(this.position);
                this.#onModelSelected?.(event.newModelUrl);
            }),
        );
        this.#disposables.push(
            editor.onDidChangeModelContent((_) =>
                this.onPositionChangeCallback(this.position),
            ),
        );

        return lsp.language_client;
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
    set onModelSelected(f: (_url: monaco.Uri | null) => void) {
        this.#onModelSelected = f;
    }

    protected async handle_lsp_url_request(
        era: number,
        url: string,
    ): Promise<string> {
        if (this.#url_mapper === null) {
            return Promise.resolve("Error: Can not resolve URL.");
        }

        const internal_uri = monaco.Uri.parse(url);
        const uri = this.#url_mapper.from_internal(internal_uri);

        if (uri === null) {
            return Promise.resolve("Error: Can not map URL.");
        }

        return this.safely_open_editor_with_url_content(
            era,
            uri,
            internal_uri,
            false,
        );
    }

    private async safely_open_editor_with_url_content(
        era: number,
        uri: monaco.Uri,
        internal_uri: monaco.Uri,
        raise_alert: boolean,
    ): Promise<string> {
        let model = monaco.editor.getModel(uri);
        if (model != null) {
            return model.getValue();
        }

        let doc = "";
        try {
            const response = await fetch(uri.toString());
            if (!response.ok) {
                if (raise_alert) {
                    alert(
                        "Failed to download data from " +
                            uri +
                            ":\n" +
                            response.status +
                            " " +
                            response.statusText,
                    );
                }
                return "";
            }
            doc = await response.text();
        } catch (e) {
            if (raise_alert) {
                alert("Failed to download data from " + uri + ".");
            }
            return "";
        }

        model = monaco.editor.getModel(uri);
        if (model != null) {
            return model.getValue();
        }

        if (era == this.#edit_era) {
            createModel(this.internal_uuid, doc, internal_uri);
        }
        return doc;
    }

    async open_tab_from_url(url: monaco.Uri): Promise<string> {
        let mapper: UrlMapper | null = null;
        const file_name_start = url.path.lastIndexOf("/") ?? 0;
        let file_name = url.path.slice(file_name_start);

        if (url.authority == "github.com") {
            const orig = url;
            const path = orig.path.split("/");

            if (path[3] === "blob") {
                path.splice(3, 1);

                url = monaco.Uri.from({
                    scheme: orig.scheme,
                    authority: "raw.githubusercontent.com",
                    path: path.join("/"),
                    query: orig.query,
                    fragment: orig.fragment,
                });
            }
        }

        this.#url_mapper =
            mapper ?? new RelativeUrlMapper(this.#internal_uuid, url);
        return this.safely_open_editor_with_url_content(
            this.#edit_era,
            url,
            internal_file_uri(this.#internal_uuid, file_name),
            true,
        );
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

    constructor(lsp: Lsp) {
        super({ node: EditorWidget.createNode() });
        this.title.label = "Editor";
        this.title.closable = false;
        this.title.caption = `Slint code editor`;
        this.#tab_map = new Map();

        const layout = new BoxLayout({ spacing: 0 });

        this.#tab_bar = new TabBar<Widget>({ name: "Open Documents Tab Bar" });
        layout.addWidget(this.#tab_bar);

        this.#editor = new EditorPaneWidget(lsp);
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
        this.#editor.onModelSelected = (url: monaco.Uri | null) => {
            let title = null;
            if (url !== null) {
                title = this.#tab_map.get(url) ?? null;
            }
            if (this.#tab_bar.currentTitle != title) {
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
        const load_demo = params.get("load_demo");

        if (code) {
            this.#editor.clear_models();
            createModel(this.#editor.internal_uuid, code);
        } else if (load_url) {
            this.project_from_url(load_url);
        } else {
            this.set_demo(load_demo ?? "");
        }
    }

    get current_editor_content(): string {
        return this.#editor.current_editor_content;
    }

    get language_client(): MonacoLanguageClient | null {
        return this.#editor.language_client;
    }

    get current_text_document_uri(): string | undefined {
        return this.#editor.current_text_document_uri;
    }

    get current_text_document_version(): number | undefined {
        return this.#editor.current_text_document_version;
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

    get supported_actions(): string[] | undefined {
        return this.#editor.supported_actions;
    }

    get supported_commands(): Thenable<string[]> {
        return this.#editor.supported_commands;
    }

    async project_from_url(url: string | null) {
        if (url == null) {
            return;
        }

        this.#editor.clear_models();
        const uri = monaco.Uri.parse(url);
        await this.#editor.open_tab_from_url(uri);
    }

    known_demos(): [string, string][] {
        return [
            ["", "Hello World!"],
            ["examples/gallery/gallery.slint", "Gallery"],
            ["examples/printerdemo/ui/printerdemo.slint", "Printer Demo"],
            [
                "examples/energy-monitor/ui/desktop_window.slint",
                "Energy Monitor",
            ],
            ["examples/todo/ui/todo.slint", "Todo Demo"],
            ["examples/iot-dashboard/main.slint", "IOT Dashboard"],
        ];
    }

    goto_position(uri: string, position: LspPosition | LspRange) {
        this.#editor.goto_position(uri, position);
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
            await this.project_from_url(
                `https://raw.githubusercontent.com/slint-ui/slint/${tag}/${location}`,
            );
        } else {
            this.#editor.clear_models();
            createModel(this.#editor.internal_uuid, hello_world);
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

    set onPositionChange(cb: PositionChangeCallback) {
        this.#editor.onPositionChangeCallback = cb;
    }

    get position(): VersionedDocumentAndPosition {
        return this.#editor.position;
    }

    get internal_url_prefix(): string {
        return this.#editor.internal_url_prefix;
    }
}
