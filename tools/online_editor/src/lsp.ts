// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore winit

import { FilterProxyReader } from "./proxy";
import {
    CloseAction,
    ErrorAction,
    Message,
    MessageTransports,
    MonacoLanguageClient,
    RequestMessage,
    ResponseMessage,
} from "monaco-languageclient";

import {
    BrowserMessageReader,
    BrowserMessageWriter,
    MessageReader,
    MessageWriter,
} from "vscode-languageserver-protocol/browser";

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";

import slint_init, * as slint_preview from "@preview/slint_wasm_interpreter.js";

let is_event_loop_running = false;

function createLanguageClient(
    transports: MessageTransports,
): MonacoLanguageClient {
    const client = new MonacoLanguageClient({
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
    client.registerProgressFeatures();
    return client;
}

export type FileReader = (_url: string) => Promise<string>;

export class LspWaiter {
    #previewer_port: MessagePort;
    #previewer_promise: Promise<slint_preview.InitOutput> | null;
    #lsp_promise: Promise<Worker> | null;

    constructor() {
        const lsp_previewer_channel = new MessageChannel();
        const lsp_side = lsp_previewer_channel.port1;
        this.#previewer_port = lsp_previewer_channel.port2;

        const worker = new Worker(
            new URL("worker/lsp_worker.ts", import.meta.url),
            { type: "module" },
        );
        this.#lsp_promise = new Promise<Worker>((resolve) => {
            worker.onmessage = (m) => {
                // We cannot start sending messages to the client before we start listening which
                // the server only does in a future after the wasm is loaded.
                if (m.data === "OK") {
                    resolve(worker);
                }
            };
        });
        worker.postMessage(lsp_side, [lsp_side]);

        this.#previewer_promise = slint_init();
    }

    async wait_for_lsp(): Promise<Lsp> {
        // eslint-disable-next-line @typescript-eslint/no-non-null-assertion
        const lp = this.#lsp_promise!;
        this.#lsp_promise = null;
        // eslint-disable-next-line @typescript-eslint/no-non-null-assertion
        const pp = this.#previewer_promise!;
        this.#previewer_promise = null;

        const [_, worker] = await Promise.all([pp, lp]);

        return Promise.resolve(new Lsp(worker, this.#previewer_port));
    }
}

type LoadUrlReply =
    | { type: "Content"; data: string }
    | { type: "Error"; data: string };
type RenderReply =
    | { type: "LoadUrl"; data: string }
    | { type: "Error"; data: string }
    | { type: "Result"; data: monaco.editor.IMarkerData[] };
type BackendChatter = { type: "ErrorReport"; data: string };

type HighlightInfo = { file: string; offset: number };

class PreviewerBackend {
    #client_port: MessagePort;
    #lsp_port: MessagePort;
    #canvas_id: string | null = null;
    #instance: slint_preview.WrappedInstance | null = null;
    #to_highlight: HighlightInfo = { file: "", offset: 0 };
    #is_rendering = false;

    constructor(client_port: MessagePort, lsp_port: MessagePort) {
        this.#lsp_port = lsp_port;
        this.#lsp_port.onmessage = (m) => {
            if (m.data.command === "highlight") {
                this.highlight(m.data.data.path, m.data.data.offset);
            }
        };

        this.#client_port = client_port;
        this.#client_port.onmessage = (m) => {
            try {
                if (m.data.command === "set_canvas_id") {
                    this.canvas_id = m.data.canvas_id;
                }
                if (m.data.command === "render") {
                    const port = m.ports[0];
                    if (this.#is_rendering) {
                        port.postMessage({
                            type: "Error",
                            data: "Already rendering",
                        });
                        port.close();
                        return;
                    }
                    this.#is_rendering = true;

                    this.render(
                        m.data.style,
                        m.data.source,
                        m.data.base_url,
                        (url: string) => {
                            return new Promise((resolve, reject) => {
                                const channel = new MessageChannel();
                                channel.port1.onmessage = (m) => {
                                    const reply = m.data as LoadUrlReply;
                                    if (reply.type == "Error") {
                                        channel.port1.close();
                                        reject(reply.data);
                                    } else if (reply.type == "Content") {
                                        channel.port1.close();
                                        resolve(reply.data);
                                    }
                                };
                                port.postMessage(
                                    { type: "LoadUrl", data: url },
                                    [channel.port2],
                                );
                            });
                        },
                    )
                        .then((diagnostics) => {
                            // Re-apply highlight:
                            this.highlight(
                                this.#to_highlight.file,
                                this.#to_highlight.offset,
                            );

                            port.postMessage({
                                type: "Result",
                                data: diagnostics,
                            });
                            port.close();
                        })
                        .catch((e) => {
                            port.postMessage({ type: "Error", data: e });
                            port.close();
                        });
                    this.#is_rendering = false;
                }
            } catch (e) {
                client_port.postMessage({ type: "Error", data: e });
            }
        };
    }

    set canvas_id(id: string | null) {
        this.#canvas_id = id;
    }

    get canvas_id() {
        return this.#canvas_id;
    }

    private async render(
        style: string,
        source: string,
        base_url: string,
        load_callback: (_url: string) => Promise<string>,
    ): Promise<monaco.editor.IMarkerData[]> {
        if (this.#canvas_id == null) {
            return Promise.resolve([]);
        }

        const { component, diagnostics, error_string } =
            await slint_preview.compile_from_string_with_style(
                source,
                base_url,
                style,
                load_callback,
            );

        this.#client_port.postMessage({
            type: "ErrorReport",
            data: error_string,
        });

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
            if (this.#instance == null) {
                this.#instance = component.create(this.canvas_id!); // eslint-disable-line
                this.#instance.show();
                try {
                    if (!is_event_loop_running) {
                        slint_preview.run_event_loop();
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

        return Promise.resolve(markers);
    }

    private highlight(file_path: string, offset: number) {
        this.#to_highlight = { file: file_path, offset: offset };
        this.#instance?.highlight(file_path, offset);
    }
}

// TODO: Remove this again and hide this behind the LSP.
export class Previewer {
    #channel: MessagePort;
    #canvas_id: string | null = null;
    #on_error: (_error: string) => void = () => {
        return;
    };

    constructor(channel: MessagePort) {
        this.#channel = channel;
        channel.onmessage = (m) => {
            const data = m.data as BackendChatter;
            if (data.type == "ErrorReport") {
                this.#on_error(data.data);
            }
        };
    }

    get canvas_id() {
        return this.#canvas_id;
    }

    set canvas_id(id: string | null) {
        this.#canvas_id = id;
        this.#channel.postMessage({ command: "set_canvas_id", canvas_id: id });
    }

    set on_error(callback: (_error: string) => void) {
        this.#on_error = callback;
    }

    public async render(
        style: string,
        source: string,
        base_url: string,
        load_callback: (_url: string) => Promise<string>,
    ): Promise<monaco.editor.IMarkerData[]> {
        return new Promise((resolve, reject) => {
            const channel = new MessageChannel();

            channel.port1.onmessage = (m) => {
                try {
                    const data = m.data as RenderReply;
                    switch (data.type) {
                        case "LoadUrl": {
                            const reply_port = m.ports[0];
                            load_callback(data.data)
                                .then((content) => {
                                    reply_port.postMessage({
                                        type: "Content",
                                        data: content,
                                    });
                                })
                                .catch((e) => {
                                    reply_port.postMessage({
                                        type: "Error",
                                        data: e,
                                    });
                                });
                            break;
                        }
                        case "Error":
                            channel.port1.close();
                            reject(data.data);
                            break;
                        case "Result":
                            channel.port1.close();
                            resolve(data.data as monaco.editor.IMarkerData[]);
                            break;
                    }
                } catch (e) {
                    reject(e);
                }
            };
            this.#channel.postMessage(
                {
                    command: "render",
                    style: style,
                    source: source,
                    base_url: base_url,
                },
                [channel.port2],
            );
        });
    }
}

export class Lsp {
    #lsp_client: MonacoLanguageClient | null = null;
    #file_reader: FileReader | null = null;

    readonly #lsp_worker: Worker;
    readonly #lsp_reader: MessageReader;
    readonly #lsp_writer: MessageWriter;

    readonly #previewer_backend: PreviewerBackend;
    readonly #previewer: Previewer;

    constructor(worker: Worker, lsp_previewer_port: MessagePort) {
        this.#lsp_worker = worker;
        const reader = new FilterProxyReader(
            new BrowserMessageReader(this.#lsp_worker),
            (data: Message) => {
                if ((data as RequestMessage).method == "slint/load_file") {
                    const request = data as RequestMessage;
                    const url = (request.params as string[])[0];

                    this.read_url(url)
                        .then((text_contents) => {
                            writer.write({
                                jsonrpc: request.jsonrpc,
                                id: request.id,
                                result: text_contents,
                                error: undefined,
                            } as ResponseMessage);
                        })
                        .catch((_) => {
                            // Some files will fail to load, so fake them as empty files
                            writer.write({
                                jsonrpc: request.jsonrpc,
                                id: request.id,
                                result: "",
                                error: undefined,
                            } as ResponseMessage);
                        });

                    return true;
                }
                return false;
            },
        );
        const writer = new BrowserMessageWriter(this.#lsp_worker);

        this.#lsp_reader = reader;
        this.#lsp_writer = writer;

        const channel = new MessageChannel();

        this.#previewer_backend = new PreviewerBackend(
            channel.port1,
            lsp_previewer_port,
        );
        this.#previewer = new Previewer(channel.port2);
    }

    get lsp_worker(): Worker {
        return this.#lsp_worker;
    }

    get lsp_reader(): MessageReader {
        return this.#lsp_reader;
    }

    get lsp_writer(): MessageWriter {
        return this.#lsp_writer;
    }

    set file_reader(fr: FileReader) {
        this.#file_reader = fr;
    }

    private read_url(url: string): Promise<string> {
        try {
            return this.#file_reader?.(url) ?? Promise.reject();
        } catch (e) {
            return Promise.reject("Failed to read file");
        }
    }

    get language_client(): MonacoLanguageClient {
        let lsp_client = this.#lsp_client;
        if (lsp_client === null) {
            const client = createLanguageClient({
                reader: this.#lsp_reader,
                writer: this.#lsp_writer,
            });
            this.#lsp_client = client;

            client.start();

            this.#lsp_reader.onClose(() => {
                client.stop();
            });

            lsp_client = client;
        }
        return lsp_client;
    }

    // TODO: This should not be necessary to expose!
    get previewer(): Previewer {
        return this.#previewer;
    }
}
