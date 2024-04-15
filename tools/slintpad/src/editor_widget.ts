// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

// cSpell: ignore codingame lumino mimetypes printerdemo

import * as monaco from "monaco-editor";

import { slint_language } from "./highlighting";
import {
    LspPosition,
    LspRange,
    editor_position_to_lsp_position,
    lsp_range_to_editor_range,
} from "./lsp_integration";
import { Lsp } from "./lsp";
import { PositionChangeCallback, VersionedDocumentAndPosition } from "./text";
import * as github from "./github";

import { BoxLayout, TabBar, Title, Widget } from "@lumino/widgets";
import { Message as LuminoMessage } from "@lumino/messaging";

import { MonacoLanguageClient } from "monaco-languageclient";
import { createConfiguredEditor } from "vscode/monaco";

import { initialize as initializeMonacoServices } from "vscode/services";
import getConfigurationServiceOverride from "@codingame/monaco-vscode-configuration-service-override";
import getEditorServiceOverride, {
    IReference,
    IEditorOptions,
    IResolvedTextEditorModel,
} from "@codingame/monaco-vscode-editor-service-override";
import getLanguagesServiceOverride from "@codingame/monaco-vscode-languages-service-override";
import getModelServiceOverride from "@codingame/monaco-vscode-model-service-override";
import getSnippetServiceOverride from "@codingame/monaco-vscode-snippets-service-override";
import getStorageServiceOverride from "@codingame/monaco-vscode-storage-service-override";

function openEditor(
    _modelRef: IReference<IResolvedTextEditorModel>,
    _options: IEditorOptions | undefined,
    _sideBySide?: boolean,
): Promise<monaco.editor.IStandaloneCodeEditor | undefined> {
    // We only have one editor and do not want to open more.
    return Promise.resolve(undefined);
}

export function initialize(): Promise<void> {
    return new Promise((resolve, reject) => {
        try {
            initializeMonacoServices({
                ...getConfigurationServiceOverride(monaco.Uri.file("/tmp")),
                ...getEditorServiceOverride(openEditor),
                ...getLanguagesServiceOverride(),
                ...getModelServiceOverride(),
                ...getSnippetServiceOverride(),
                ...getStorageServiceOverride(),
            }).then(() => {
                resolve();
            });
        } catch (e) {
            reject(e);
        }
    });
}

const hello_world = `import { AboutSlint, Button, VerticalBox } from "std-widgets.slint";
export component Demo {
    VerticalBox {
        alignment: start;
        Text {
            text: "Hello World!";
            font-size: 24px;
            horizontal-alignment: center;
        }
        AboutSlint {
            preferred-height: 150px;
        }
        HorizontalLayout { alignment: center; Button { text: "OK!"; } }
    }
}
`;

function internal_file_uri(uuid: string, file_name: string): monaco.Uri {
    console.assert(file_name.startsWith("/"));
    return monaco.Uri.from({
        scheme: "user",
        authority: uuid + ".slint.rs",
        path: file_name,
    });
}

function is_internal_uri(uuid: string, uri: monaco.Uri): boolean {
    return uri.scheme === "user" && uri.authority === uuid + ".slint.rs";
}

function file_from_internal_uri(uuid: string, uri: monaco.Uri): string {
    console.assert(is_internal_uri(uuid, uri));
    return uri.path;
}

export interface UrlMapper {
    from_internal(_uri: monaco.Uri): monaco.Uri | null;
}

export class KnownUrlMapper implements UrlMapper {
    #map: { [path: string]: string };
    #uuid: string;

    constructor(uuid: string, map: { [path: string]: string }) {
        this.#uuid = uuid;
        this.#map = map;
        console.assert(Object.keys(map).length > 0);
        Object.keys(map).forEach((k) => console.assert(k.startsWith("/")));
    }

    from_internal(uri: monaco.Uri): monaco.Uri | null {
        if (!is_internal_uri(this.#uuid, uri)) {
            return uri;
        }

        const file_path = file_from_internal_uri(this.#uuid, uri);

        const mapped_url = this.#map[file_path] || null;
        if (mapped_url) {
            return (
                monaco.Uri.parse(mapped_url) ??
                monaco.Uri.parse("file:///broken_url")
            );
        } else {
            return uri;
        }
    }
}

export class RelativeUrlMapper implements UrlMapper {
    #base_uri: monaco.Uri;
    #uuid: string;

    constructor(uuid: string, uri: monaco.Uri) {
        this.#uuid = uuid;
        this.#base_uri = uri;
    }

    from_internal(uri: monaco.Uri): monaco.Uri | null {
        if (!is_internal_uri(this.#uuid, uri)) {
            return uri;
        }

        return monaco.Uri.from({
            scheme: this.#base_uri.scheme,
            authority: this.#base_uri.authority,
            path: file_from_internal_uri(this.#uuid, uri),
        });
    }
}

async function createModel(
    uuid: string,
    source: string,
    uri?: monaco.Uri,
): Promise<monaco.editor.ITextModel | null> {
    const url = uri ?? internal_file_uri(uuid, "/main.slint");
    console.assert(is_internal_uri(uuid, url));

    const model = monaco.editor.getModel(url);
    if (model !== null) {
        return Promise.resolve(model);
    }

    return monaco.editor.createModel(source, "slint", url);
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
    #main_uri: monaco.Uri | null = null;
    #editor_view_states: Map<
        monaco.Uri,
        monaco.editor.ICodeEditorViewState | null | undefined
    >;
    #editor: monaco.editor.IStandaloneCodeEditor | null = null;
    #client: MonacoLanguageClient | null = null;
    #url_mapper: UrlMapper | null = null;
    #edit_era: number;
    #disposables: monaco.IDisposable[] = [];
    #internal_uuid = self.crypto.randomUUID();

    #extra_file_urls: { [key: string]: string } = {};

    onPositionChangeCallback: PositionChangeCallback = (
        _pos: VersionedDocumentAndPosition,
    ) => {
        return;
    };

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

        monaco.editor.onDidCreateModel((model: monaco.editor.ITextModel) =>
            this.add_model_listener(model),
        );
    }

    async map_url(url_: string): Promise<string | undefined> {
        const js_url = new URL(url_);

        const absolute_uri = monaco.Uri.parse(js_url.toString());
        const mapped_uri =
            this.#url_mapper?.from_internal(absolute_uri) ?? absolute_uri;
        const mapped_string = mapped_uri.toString();

        if (is_internal_uri(this.#internal_uuid, mapped_uri)) {
            const file = file_from_internal_uri(
                this.#internal_uuid,
                mapped_uri,
            );
            this.#extra_file_urls[file] = mapped_string;
        }

        return mapped_string;
    }

    get editor(): monaco.editor.IStandaloneCodeEditor | undefined {
        if (this.#editor === null) {
            return undefined;
        }
        return this.#editor;
    }

    dispose() {
        this.#disposables.forEach((d: monaco.IDisposable) => d.dispose());
        this.#disposables = [];
        this.#editor?.dispose();
        this.#editor = null;
        this.dispose();
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
        if (!monaco.editor.getModel(uri_) || !this.set_model(uri_)) {
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

    get open_document_urls(): string[] {
        const main_file = this.#main_uri?.toString();

        const result = [];

        if (main_file != null) {
            result.push(main_file);
        }

        monaco.editor.getModels().forEach((m: monaco.editor.ITextModel) => {
            const u = m?.uri.toString();

            if (u != null && u != main_file) {
                result.push(u);
            }
        });

        return result;
    }

    document_contents(url: string): string | undefined {
        const uri = monaco.Uri.parse(url);
        return monaco.editor.getModel(uri)?.getValue();
    }

    get extra_files(): { [key: string]: string } {
        return this.#extra_file_urls;
    }

    public clear_models() {
        this.#edit_era += 1;
        this.#url_mapper = null;
        this.#editor_view_states.clear();
        this.#extra_file_urls = {};
        monaco.editor
            .getModels()
            .forEach((model: monaco.editor.ITextModel) => model.dispose());
        this.#onModelsCleared?.();
    }

    private resize_editor() {
        if (this.#editor != null) {
            // This has a 1px wide border all around, so subtract 2px...
            const width = this.contentNode.offsetWidth - 2;
            const height = this.contentNode.offsetHeight - 2;
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
        this.#editor_view_states.set(uri, null);
        this.#onModelAdded?.(uri);
        if (monaco.editor.getModels().length === 1) {
            this.#main_uri = uri;
            this.set_model(uri);
            this.language_client?.sendRequest("workspace/executeCommand", {
                command: "slint/showPreview",
                arguments: [this.#main_uri?.toString() ?? "", ""],
            });
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
        this.#editor?.setModel(monaco.editor.getModel(uri));
        this.#editor?.focus();
        return true;
    }

    protected onResize(_msg: LuminoMessage): void {
        if (this.isAttached) {
            this.resize_editor();
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

        const editor = createConfiguredEditor(container, {
            language: "slint",
            glyphMargin: true,
            lightbulb: {
                enabled: true,
            },
        });

        monaco.editor.registerEditorOpener({
            openCodeEditor: (
                _source,
                resource: monaco.Uri,
                selectionOrPosition?: monaco.IPosition | monaco.IRange,
            ) => {
                editor.setModel(monaco.editor.getModel(resource));
                if (monaco.Position.isIPosition(selectionOrPosition)) {
                    const pos = selectionOrPosition as monaco.IPosition;
                    editor.setSelection({
                        startLineNumber: pos.lineNumber,
                        startColumn: pos.column,
                        endLineNumber: pos.lineNumber,
                        endColumn: pos.column,
                    });
                    editor.revealPosition(pos);
                } else {
                    const range = selectionOrPosition as monaco.IRange;
                    editor.setSelection(range);
                    editor.revealRange(range);
                }
                return true;
            },
        } as monaco.editor.ICodeEditorOpener);

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

        this.#disposables.push(editor);

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

        return (
            await this.safely_open_editor_with_url_content(
                era,
                uri,
                internal_uri,
                false,
            )
        )[1];
    }

    private async safely_open_editor_with_url_content(
        era: number,
        uri: monaco.Uri,
        internal_uri: monaco.Uri,
        raise_alert: boolean,
    ): Promise<[monaco.Uri | null, string]> {
        let model = monaco.editor.getModel(internal_uri);
        if (model != null) {
            return [model.uri, model.getValue()];
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
                return [null, ""];
            }
            doc = await response.text();
        } catch (e) {
            if (raise_alert) {
                alert("Failed to download data from " + uri + ".");
            }
            return [null, ""];
        }

        model = monaco.editor.getModel(internal_uri);
        if (model != null) {
            return [model.uri, model.getValue()];
        }

        let result_uri = null;
        if (era == this.#edit_era) {
            model = await createModel(this.internal_uuid, doc, internal_uri);
            if (model) {
                result_uri = model.uri;
            }
        }
        return [result_uri, doc];
    }

    async open_tab_from_url(
        input_url: monaco.Uri,
    ): Promise<[monaco.Uri | null, string]> {
        const [url, file_name, mapper] = await github.open_url(
            this.#internal_uuid,
            input_url.toString(),
        );

        const output_url = monaco.Uri.parse(url ?? input_url.toString());
        this.#url_mapper =
            mapper ?? new RelativeUrlMapper(this.#internal_uuid, output_url);

        return this.safely_open_editor_with_url_content(
            this.#edit_era,
            output_url,
            internal_file_uri(
                this.#internal_uuid,
                file_name ?? output_url.path,
            ),
            true,
        );
    }

    add_empty_file(name: string): boolean {
        let abs_name = name;
        if (!abs_name.startsWith("/")) {
            abs_name = "/" + abs_name;
        }

        const uri = internal_file_uri(this.#internal_uuid, abs_name);

        if (monaco.editor.getModel(uri)) {
            return false;
        }

        createModel(this.internal_uuid, "", uri);

        return true;
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

        super.layout = layout;

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

    async map_url(url: string): Promise<string | undefined> {
        return this.#editor.map_url(url);
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

    async project_from_url(url: string | null): Promise<monaco.Uri | null> {
        if (url == null) {
            return null;
        }

        this.#editor.clear_models();
        const uri = monaco.Uri.parse(url);
        return (await this.#editor.open_tab_from_url(uri))[0];
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

    add_empty_file_to_project(name: string) {
        this.#editor.add_empty_file(name);
    }

    async set_demo(location: string) {
        if (location) {
            const default_tag = "XXXX_DEFAULT_TAG_XXXX";
            let tag = default_tag.startsWith("XXXX_DEFAULT_TAG_")
                ? "master"
                : default_tag;
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
            await createModel(this.#editor.internal_uuid, hello_world);
        }
    }

    set onPositionChange(cb: PositionChangeCallback) {
        this.#editor.onPositionChangeCallback = cb;
    }

    get position(): VersionedDocumentAndPosition {
        return this.#editor.position;
    }

    get open_document_urls(): string[] {
        return this.#editor.open_document_urls;
    }

    get extra_files(): { [key: string]: string } {
        return this.#editor.extra_files;
    }

    document_contents(url: string): string | undefined {
        return this.#editor.document_contents(url);
    }

    get internal_url_prefix(): string {
        return this.#editor.internal_url_prefix;
    }
}
