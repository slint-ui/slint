// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino inmemory mimetypes printerdemo

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

export class Lsp {
    #lsp_client: MonacoLanguageClient | null = null;
    #file_reader: FileReader | null = null;

    readonly #lsp_worker: Worker;
    readonly #lsp_reader: MessageReader;
    readonly #lsp_writer: MessageWriter;

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
}
