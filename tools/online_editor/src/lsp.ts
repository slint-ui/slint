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
    #lsp_worker: Worker | null;

    constructor() {
        this.#lsp_worker = new Worker(
            new URL("worker/lsp_worker.ts", import.meta.url),
            { type: "module" },
        );

        slint_init(); // Initialize Previewer!
    }

    async wait_for_lsp(): Promise<Lsp> {
        // eslint-disable-next-line @typescript-eslint/no-non-null-assertion
        const worker = this.#lsp_worker!;
        this.#lsp_worker = null;

        return new Promise<Lsp>((resolve) => {
            worker.onmessage = (m) => {
                // We cannot start sending messages to the client before we start listening which
                // the server only does in a future after the wasm is loaded.
                if (m.data === "OK") {
                    resolve(new Lsp(worker));
                }
            };
        });
    }
}

// TODO: Remove this again and hide this behind the LSP.
export class Previewer {
    #canvas_id: string | null = null;
    #instance: slint_preview.WrappedInstance | null = null;
    #onError: (error: string) => void = () => {
        return;
    };

    constructor() {}

    set canvas_id(id: string | null) {
        this.#canvas_id = id;
    }

    set on_error(callback: (error: string) => void) {
        this.#onError = callback;
    }

    get canvas_id() {
        return this.#canvas_id;
    }

    public async render(
        style: string,
        source: string,
        base_url: string,
        load_callback: (_url: string) => Promise<string>,
    ): Promise<monaco.editor.IMarkerData[]> {
        if (this.#canvas_id === null) {
            return Promise.resolve([]);
        }

        const { component, diagnostics, error_string } =
            await slint_preview.compile_from_string_with_style(
                source,
                base_url,
                style,
                load_callback,
            );

        this.#onError(error_string);

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
}

export class Lsp {
    #lsp_client: MonacoLanguageClient | null = null;
    #file_reader: FileReader | null = null;

    readonly #lsp_worker: Worker;
    readonly #lsp_reader: MessageReader;
    readonly #lsp_writer: MessageWriter;

    readonly #previewer: Previewer;

    constructor(worker: Worker) {
        this.#lsp_worker = worker;
        const reader = new FilterProxyReader(
            new BrowserMessageReader(this.#lsp_worker),
            (data: Message) => {
                if ((data as RequestMessage).method == "slint/load_file") {
                    const request = data as RequestMessage;
                    const url = (request.params as string[])[0];

                    this.read_url(url).then((text_contents) => {
                        writer.write({
                            jsonrpc: request.jsonrpc,
                            id: request.id,
                            result: text_contents,
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

        this.#previewer = new Previewer();
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
        return this.#file_reader?.(url) ?? Promise.reject();
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
