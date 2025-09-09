// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore codingame lumino mimetypes printerdemo

import * as monaco from "monaco-editor";

import { slint_language } from "./highlighting";
import type { Lsp } from "./lsp";
import * as github from "./github";

import { BoxLayout, TabPanel, Widget } from "@lumino/widgets";
import type { Message as LuminoMessage } from "@lumino/messaging";

import type { MonacoLanguageClient } from "monaco-languageclient";
import type { IReference } from "vscode/monaco";

import { initialize as initializeMonacoServices } from "vscode/services";
import getConfigurationServiceOverride from "@codingame/monaco-vscode-configuration-service-override";
import getEditorServiceOverride from "@codingame/monaco-vscode-editor-service-override";
import {
    RegisteredFileSystemProvider,
    RegisteredMemoryFile,
    registerCustomProvider,
} from "@codingame/monaco-vscode-files-service-override";
import getKeybindingsServiceOverride from "@codingame/monaco-vscode-keybindings-service-override";
import getLanguageServiceOverride from "@codingame/monaco-vscode-languages-service-override";
import getModelServiceOverride from "@codingame/monaco-vscode-model-service-override";
import getStorageServiceOverride from "@codingame/monaco-vscode-storage-service-override";

import "vscode/localExtensionHost";

import type { IStandaloneCodeEditor } from "vscode/vscode/vs/editor/standalone/browser/standaloneCodeEditor";
import type { ITextEditorModel } from "vscode/vscode/vs/editor/common/services/resolverService";

let EDITOR_WIDGET: EditorWidget | null = null;

const FILESYSTEM_PROVIDER: RegisteredFileSystemProvider =
    new RegisteredFileSystemProvider(false);

export function initialize(): Promise<void> {
    return new Promise((resolve, reject) => {
        try {
            registerCustomProvider("slintpad", FILESYSTEM_PROVIDER);

            return initializeMonacoServices(
                {
                    ...getConfigurationServiceOverride(),
                    ...getEditorServiceOverride(
                        (model, _options, _side_by_side) => {
                            return EDITOR_WIDGET!.open_model_ref(model);
                        },
                    ),
                    ...getKeybindingsServiceOverride(),
                    ...getLanguageServiceOverride(),
                    ...getModelServiceOverride(),
                    ...getStorageServiceOverride(),
                },
                undefined,
                {
                    workspaceProvider: {
                        trusted: true,
                        workspace: {
                            folderUri: monaco.Uri.parse("slintpad:///"),
                        },
                        open: (_) => Promise.resolve(false),
                    },
                },
            )
                .then(() => {
                    monaco.languages.register({
                        id: "slint",
                        extensions: [".slint"],
                        aliases: ["Slint", "slint"],
                        mimetypes: ["application/slint"],
                    });
                    monaco.languages.setLanguageConfiguration("slint", {
                        comments: {
                            lineComment: "//",
                            blockComment: ["/*", "*/"],
                        },
                        brackets: [
                            ["{", "}"],
                            ["[", "]"],
                            ["(", ")"],
                        ],
                        autoClosingPairs: [
                            {
                                open: "{",
                                close: "}",
                            },
                            {
                                open: "[",
                                close: "]",
                            },
                            {
                                open: "(",
                                close: ")",
                            },
                            {
                                open: "'",
                                close: "'",
                                notIn: ["string", "comment"],
                            },
                            {
                                open: '"',
                                close: '"',
                                notIn: ["string"],
                            },
                            {
                                open: "`",
                                close: "`",
                                notIn: ["string", "comment"],
                            },
                            {
                                open: "/**",
                                close: " */",
                                notIn: ["string"],
                            },
                        ],
                        autoCloseBefore: ";:.,=}])>` \n\t",
                        surroundingPairs: [
                            {
                                open: "{",
                                close: "}",
                            },
                            {
                                open: "[",
                                close: "]",
                            },
                            {
                                open: "(",
                                close: ")",
                            },
                            {
                                open: "'",
                                close: "'",
                            },
                            {
                                open: '"',
                                close: '"',
                            },
                            {
                                open: "`",
                                close: "`",
                            },
                            {
                                open: "/**",
                                close: " */",
                            },
                        ],
                        folding: {
                            markers: {
                                start: new RegExp("^\\s*//\\s*#?region\\b"),
                                end: new RegExp("^\\s*//\\s*#?endregion\\b"),
                            },
                        },
                        wordPattern: new RegExp(
                            "(-?\\d*\\.\\d\\w*)|([^\\`\\~\\!\\@\\#\\%\\^\\&\\*\\(\\)\\=\\+\\[\\{\\]\\}\\\\\\|\\;\\:\\'\\\"\\,\\.\\<\\>\\/\\?\\s]+)",
                        ),
                        indentationRules: {
                            increaseIndentPattern: new RegExp(
                                "^((?!\\/\\/).)*(\\{[^}\"'`]*|\\([^)\"'`]*|\\[[^\\]\"'`]*)$",
                            ),
                            decreaseIndentPattern: new RegExp(
                                "^((?!.*?\\/\\*).*\\*/)?\\s*[\\}\\]].*$",
                            ),
                        },
                    });
                    monaco.languages.onLanguage("slint", () => {
                        monaco.languages.setMonarchTokensProvider(
                            "slint",
                            slint_language,
                        );
                    });

                    resolve();
                })
                .catch(reject);
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

function internal_file_uri(file_name: string): monaco.Uri {
    console.assert(file_name.startsWith("/"));
    return monaco.Uri.from({
        scheme: "slintpad",
        path: file_name,
    });
}

function is_internal_uri(uri: monaco.Uri): boolean {
    return uri.scheme === "slintpad";
}

function file_from_internal_uri(uri: monaco.Uri): string {
    console.assert(is_internal_uri(uri));
    return uri.path;
}

export interface UrlMapper {
    from_internal(_uri: monaco.Uri): monaco.Uri | null;
}

export class KnownUrlMapper implements UrlMapper {
    #map: { [path: string]: string };

    constructor(map: { [path: string]: string }) {
        this.#map = map;
        console.assert(Object.keys(map).length > 0);
        Object.keys(map).forEach((k) => console.assert(k.startsWith("/")));
    }

    from_internal(uri: monaco.Uri): monaco.Uri | null {
        if (!is_internal_uri(uri)) {
            return uri;
        }

        const file_path = file_from_internal_uri(uri);

        const mapped_url = this.#map[file_path] || null;
        if (mapped_url) {
            return (
                monaco.Uri.parse(mapped_url) ??
                monaco.Uri.parse("file:///broken_url")
            );
        }
        return uri;
    }
}

export class RelativeUrlMapper implements UrlMapper {
    #base_uri: monaco.Uri;

    constructor(uri: monaco.Uri) {
        this.#base_uri = uri;
    }

    from_internal(uri: monaco.Uri): monaco.Uri | null {
        if (!is_internal_uri(uri)) {
            return uri;
        }

        return monaco.Uri.from({
            scheme: this.#base_uri.scheme,
            authority: this.#base_uri.authority,
            path: file_from_internal_uri(uri),
        });
    }
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

function tabTitleFromURL(url: monaco.Uri | undefined): string {
    try {
        const path = url?.path ?? "";
        return path.substring(path.lastIndexOf("/") + 1);
    } catch (e) {
        return url?.toString() ?? "";
    }
}

class EditorPaneWidget extends Widget {
    #editor: monaco.editor.IStandaloneCodeEditor;
    #model_ref: IReference<ITextEditorModel>;

    static createNode(): HTMLElement {
        const node = document.createElement("div");
        const content = document.createElement("div");
        node.appendChild(content);

        return node;
    }

    constructor(model_ref: IReference<ITextEditorModel>) {
        const node = EditorPaneWidget.createNode();

        super({ node: node });

        this.#model_ref = model_ref;

        this.id = model_ref.object.textEditorModel?.uri.toString() ?? "";

        this.#editor = monaco.editor.create(this.contentNode, {
            model: model_ref.object.textEditorModel,
        });

        this.#editor.onDidFocusEditorText((_) => {
            EDITOR_WIDGET!.switch_to_pane(this);
        });

        this.setFlag(Widget.Flag.DisallowLayout);
        this.addClass("content");
        this.addClass("editor");
        this.title.label = tabTitleFromURL(
            model_ref.object.textEditorModel?.uri,
        );
        this.title.closable = false;
        this.title.caption = "Slint Code Editor";
    }

    get editor(): monaco.editor.IStandaloneCodeEditor {
        return this.#editor;
    }

    dispose() {
        this.#editor.dispose();
        this.#model_ref.dispose();
        super.dispose();
    }

    protected get contentNode(): HTMLDivElement {
        return this.node.getElementsByTagName("div")[0] as HTMLDivElement;
    }

    private resize_editor() {
        if (this.#editor != null) {
            // This has a 1px wide border all around, so subtract 2px...
            const width = this.contentNode.offsetWidth - 2;
            const height = this.contentNode.offsetHeight - 2;
            this.#editor.layout({ width, height });
        }
    }

    protected onResize(_msg: LuminoMessage): void {
        if (this.isAttached) {
            this.resize_editor();
        }
    }
}

export class EditorWidget extends Widget {
    #layout: BoxLayout;
    #tab_map: Map<string, EditorPaneWidget> = new Map();
    #tab_panel: TabPanel | null = null;
    #open_files: monaco.IDisposable[] = [];

    #client: MonacoLanguageClient | null = null;

    #edit_era: number;

    #url_mapper: UrlMapper | null = null;
    #extra_file_urls: { [key: string]: string } = {};

    constructor(lsp: Lsp) {
        super({});

        this.#edit_era = 0;

        this.title.label = "Editor";
        this.title.closable = false;
        this.title.caption = "Slint code editor";

        this.#layout = new BoxLayout({ spacing: 0 });
        super.layout = this.#layout;

        this.#client = lsp.language_client;

        EDITOR_WIDGET = this;

        lsp.file_reader = (url) => {
            return this.handle_lsp_url_request(url);
        };

        this.clear_editors();

        void this.open_default_content();
    }

    switch_to_pane(pane: EditorPaneWidget) {
        this.#tab_panel!.currentWidget = pane;
    }

    private async open_default_content() {
        const params = new URLSearchParams(window.location.search);
        const compressed = params.get("gz");
        let code = params.get("snippet");
        if (compressed) {
            code = await decompress(compressed);
        }
        const load_url = params.get("load_url");
        const load_demo = params.get("load_demo");

        if (code) {
            this.clear_editors();
            return Promise.resolve(
                this.open_file_with_content(
                    internal_file_uri("/main.slint"),
                    code,
                ),
            );
        }
        if (load_url) {
            void this.project_from_url(load_url);
        } else {
            void this.set_demo(load_demo ?? "");
        }
    }

    private clear_editors() {
        this.#edit_era += 1;
        this.#url_mapper = null;

        if (this.#tab_panel !== null) {
            this.#tab_panel.dispose();
        }
        this.#tab_panel = new TabPanel({ addButtonEnabled: false });
        this.#layout.addWidget(this.#tab_panel);

        this.#tab_map.clear();
        this.#extra_file_urls = {};

        for (const d of this.#open_files) {
            d.dispose();
        }
        this.#open_files = [];
    }

    private open_hello_world(): monaco.Uri {
        this.clear_editors();

        const uri = internal_file_uri("/main.slint");

        this.open_file_with_content(uri, hello_world);
        return uri;
    }

    private open_file_with_content(uri: monaco.Uri, content: string) {
        this.#open_files.push(
            FILESYSTEM_PROVIDER.registerFile(
                new RegisteredMemoryFile(uri, content),
            ),
        );

        monaco.editor
            .createModelReference(uri)
            .then((model_ref) => this.open_model_ref(model_ref));
    }

    public async open_model_ref(
        model_ref: IReference<ITextEditorModel>,
    ): Promise<IStandaloneCodeEditor> {
        const uri =
            model_ref.object.textEditorModel?.uri ??
            internal_file_uri("unknown.slint");

        const pane = new EditorPaneWidget(model_ref);

        this.#tab_map.set(uri.toString(), pane);
        this.#tab_panel!.addWidget(pane);

        if (this.#tab_map.size === 1) {
            await this.#client?.sendRequest("workspace/executeCommand", {
                command: "slint/showPreview",
                arguments: [
                    model_ref.object.textEditorModel?.uri.toString() ?? "",
                    "",
                ],
            });
        }

        return Promise.resolve(pane.editor);
    }

    public map_url(url_: string): Promise<string | undefined> {
        const js_url = new URL(url_);

        const absolute_uri = monaco.Uri.parse(js_url.toString());
        const mapped_uri =
            this.#url_mapper?.from_internal(absolute_uri) ?? absolute_uri;
        const mapped_string = mapped_uri.toString();

        if (is_internal_uri(mapped_uri)) {
            const file = file_from_internal_uri(mapped_uri);
            this.#extra_file_urls[file] = mapped_string;
        }

        return Promise.resolve(mapped_string);
    }

    private get current_editor_pane(): EditorPaneWidget {
        const uri =
            monaco.Uri.parse(this.current_text_document_uri ?? "") ??
            internal_file_uri("broken.slint");
        return (
            this.#tab_map.get(uri.toString()) ??
            this.#tab_map.entries().next().value![1]
        );
    }

    private get current_editor(): IStandaloneCodeEditor {
        return this.current_editor_pane.editor;
    }

    get current_editor_content(): string {
        return this.current_editor.getModel()?.getValue() ?? "";
    }

    private get current_text_document_uri(): string | undefined {
        return this.#tab_panel!.currentWidget?.id;
    }

    public async project_from_url(
        uri: string | null,
    ): Promise<monaco.Uri | null> {
        if (uri == null) {
            return null;
        }

        this.clear_editors();

        return (await this.open_tab_from_url(monaco.Uri.parse(uri)))[0];
    }

    private async open_tab_from_url(
        input_url: monaco.Uri,
    ): Promise<[monaco.Uri | null, string]> {
        const [url, file_name, mapper] = await github.open_url(
            input_url.toString(),
        );

        const output_url = monaco.Uri.parse(url ?? input_url.toString());
        this.#url_mapper = mapper ?? new RelativeUrlMapper(output_url);

        return this.safely_open_editor_with_url_content(
            output_url,
            internal_file_uri(file_name ?? output_url.path),
            true,
        );
    }

    public add_empty_file_to_project(name: string) {
        let abs_name = name;
        if (!abs_name.startsWith("/")) {
            abs_name = "/" + abs_name;
        }

        const uri = internal_file_uri(abs_name);

        if (monaco.editor.getModel(uri)) {
            return false;
        }

        this.open_file_with_content(uri, "");

        return true;
    }

    public set_demo(location: string): Promise<monaco.Uri | null> {
        if (location) {
            const default_tag = "XXXX_DEFAULT_TAG_XXXX";
            let tag = default_tag.startsWith("XXXX_DEFAULT_TAG_")
                ? "master"
                : default_tag;
            {
                let found: RegExpMatchArray | null;
                if (
                    (found = window.location.pathname.match(
                        /releases\/([^/]*)\/editor/,
                    ))
                ) {
                    tag = "v" + found[1];
                }
            }
            return this.project_from_url(
                `https://raw.githubusercontent.com/slint-ui/slint/${tag}/${location}`,
            );
        }
        return Promise.resolve(this.open_hello_world());
    }

    public get open_document_urls(): string[] {
        return [...this.#tab_map.keys()];
    }

    public document_contents(url: string): string | undefined {
        const pane = this.#tab_map.get(url);
        return pane?.editor.getModel()?.getValue();
    }

    public get extra_files(): { [key: string]: string } {
        return this.#extra_file_urls;
    }

    protected async handle_lsp_url_request(url: string): Promise<string> {
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
                uri,
                internal_uri,
                false,
            )
        )[1];
    }

    private async safely_open_editor_with_url_content(
        uri: monaco.Uri,
        internal_uri: monaco.Uri,
        raise_alert: boolean,
    ): Promise<[monaco.Uri | null, string]> {
        try {
            const content = await FILESYSTEM_PROVIDER.readFile(internal_uri);
            return [internal_uri, new TextDecoder().decode(content) ?? ""];
        } catch (e) {}

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

        this.open_file_with_content(internal_uri, doc);

        return [internal_uri, doc];
    }

    public async copy_permalink_to_clipboard() {
        const params = new URLSearchParams();
        params.set("gz", await compress(this.current_editor_content));
        const url = new URL(window.location.href);
        url.search = params.toString();
        navigator.clipboard.writeText(url.toString());
    }
}

// Return an URL-compatible base64 encoded string
async function compress(text: string): Promise<string> {
    const input = new TextEncoder().encode(text);
    const compressedStream = new Blob([input])
        .stream()
        .pipeThrough(new CompressionStream("gzip"));

    const compressedBuffer = await new Response(compressedStream).arrayBuffer();
    const binary = String.fromCharCode(...new Uint8Array(compressedBuffer));
    const b64 = btoa(binary);
    return b64.replace(/\+/g, "-").replace(/\//g, "_");
}

async function decompress(b64: string): Promise<string> {
    const base64 = b64.replace(/-/g, "+").replace(/_/g, "/");
    const binary = atob(base64);
    const compressed = Uint8Array.from(binary, (c) => c.charCodeAt(0));

    const decompressedStream = new Blob([compressed])
        .stream()
        .pipeThrough(new DecompressionStream("gzip"));

    const decompressedBuffer = await new Response(
        decompressedStream,
    ).arrayBuffer();
    return new TextDecoder().decode(decompressedBuffer);
}
