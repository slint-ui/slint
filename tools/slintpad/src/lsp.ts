// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { FilterProxyReader } from "./proxy";
import {
    CloseAction,
    ErrorAction,
    type Message,
    type MessageTransports,
    type NotificationMessage,
    type RequestMessage,
    type ResponseMessage,
} from "vscode-languageclient";
import { MonacoLanguageClient } from "monaco-languageclient";

import {
    BrowserMessageReader,
    BrowserMessageWriter,
    type MessageReader,
    type MessageWriter,
} from "vscode-languageserver-protocol/browser";
export { Position as LspPosition } from "vscode-languageserver-types";
import type { Position as LspPosition } from "vscode-languageserver-types";

import slint_init, * as slint_preview from "@lsp/slint_lsp_wasm.js";

import {
    type ResourceUrlMapperFunction,
    type InvokeSlintpadCallback,
    SlintPadCallbackFunction,
} from "@lsp/slint_lsp_wasm.js";
export {
    ResourceUrlMapperFunction,
    InvokeSlintpadCallback,
    SlintPadCallbackFunction,
};

export type ShowDocumentCallback = (
    _uri: string,
    _position: LspPosition,
) => boolean;

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
    #previewer_promise: Promise<slint_preview.InitOutput> | null;
    #lsp_promise: Promise<Worker> | null;

    constructor() {
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

        this.#previewer_promise = slint_init({});
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

export class Previewer {
    #preview_connector: slint_preview.PreviewConnector;

    constructor(connector: slint_preview.PreviewConnector) {
        this.#preview_connector = connector;
    }

    show_ui(): Promise<void> {
        return this.#preview_connector.show_ui();
    }

    current_style(): string {
        return this.#preview_connector.current_style();
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
                    (data as NotificationMessage).method ===
                    "slint/lsp_to_preview"
                ) {
                    const notification = data as NotificationMessage;
                    const params = notification.params;

                    this.#preview_connector?.process_lsp_to_preview_message(
                        params,
                    );

                    return true;
                }
                if ((data as RequestMessage).method === "slint/load_file") {
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

    async previewer(
        resource_url_mapper: ResourceUrlMapperFunction,
        style: string,
        slintpad_callback: InvokeSlintpadCallback,
    ): Promise<Previewer> {
        if (this.#preview_connector === null) {
            slint_preview.run_event_loop();

            const params = new URLSearchParams(window.location.search);
            const experimental = params.get("SLINT_EXPERIMENTAL_FEATURES");

            this.#preview_connector =
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                await slint_preview.PreviewConnector.create(
                    (data) => {
                        this.language_client.sendNotification(
                            "slint/preview_to_lsp",
                            data,
                        );
                    },
                    resource_url_mapper,
                    style,
                    experimental === "1",
                    slintpad_callback,
                );
        }
        return new Previewer(this.#preview_connector);
    }
}
