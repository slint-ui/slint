// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore winit

import { FilterProxyReader } from "./proxy";
import {
    CloseAction,
    ErrorAction,
    Message,
    MessageTransports,
    NotificationMessage,
    RequestMessage,
    ResponseMessage,
} from "vscode-languageclient";
import { MonacoLanguageClient } from "monaco-languageclient";

import {
    BrowserMessageReader,
    BrowserMessageWriter,
    MessageReader,
    MessageWriter,
} from "vscode-languageserver-protocol/browser";

import * as monaco from "monaco-editor";

import slint_init, * as slint_preview from "@lsp/slint_lsp_wasm.js";
import { HighlightRequestCallback } from "./text";

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

        const [_1, worker] = await Promise.all([pp, lp]);

        return Promise.resolve(new Lsp(worker));
    }
}

type LoadUrlReply =
    | { type: "Content"; data: string }
    | { type: "Error"; data: string };
type RenderReply =
    | { type: "LoadUrl"; data: string }
    | { type: "Error"; data: string }
    | { type: "Result"; data: monaco.editor.IMarkerData[] };
type BackendChatter =
    | { type: "ErrorReport"; data: string }
    | {
          type: "HighlightRequest";
          url: string;
          start: { line: number; column: number };
          end: { line: number; column: number };
      };

type HighlightInfo = { file: string; offset: number };
type InstanceCallback<R> = (_instance: slint_preview.WrappedInstance) => R;

// TODO: Remove this again and hide this behind the LSP.
export class Previewer {
    #preview_connector: slint_preview.PreviewConnector;

    constructor(connector: slint_preview.PreviewConnector) {
        console.log("LSP/Previewer: Constructor");
        this.#preview_connector = connector;
    }

    show_ui(): Promise<void> {
        return this.#preview_connector.show_ui();
    }
}

export class Lsp {
    #lsp_client: MonacoLanguageClient | null = null;
    #file_reader: FileReader | null = null;

    readonly #lsp_worker: Worker;
    readonly #lsp_reader: MessageReader;
    readonly #lsp_writer: MessageWriter;

    #preview_connector: slint_preview.PreviewConnector | null = null;

    constructor(worker: Worker) {
        this.#lsp_worker = worker;
        const reader = new FilterProxyReader(
            new BrowserMessageReader(this.#lsp_worker),
            (data: Message) => {
                if (
                    (data as NotificationMessage).method ==
                    "slint/lsp_to_preview"
                ) {
                    const notification = data as NotificationMessage;
                    const params = notification.params;

                    console.log("Got lsp_to_preview communication:", params);
                    this.#preview_connector?.process_lsp_to_preview_message(
                        params,
                    );

                    return true;
                }
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

    async previewer(): Promise<Previewer> {
        console.log("LSP: Grabbing Previewer!");
        if (this.#preview_connector === null) {
            console.log("LSP: Running event loop!");
            try {
                slint_preview.run_event_loop();
            } catch (e) {
                // this is not an error!
            }
            console.log("LSP: Creating Preview connector");
            this.#preview_connector =
                await slint_preview.PreviewConnector.create();
        }
        console.log("LSP: Got preview connector...", this.#preview_connector);
        return new Previewer(this.#preview_connector);
    }
}
