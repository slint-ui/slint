// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

import {
    MessageReader,
    DataCallback,
    Message,
    Disposable,
    Event,
    PartialMessageInfo,
} from "vscode-languageclient";

export class FilterProxyReader implements MessageReader {
    constructor(reader: MessageReader, callback: (_: Message) => boolean) {
        this.#reader = reader;
        this.#callback = callback;
        this.onError = this.#reader.onError;
        this.onClose = this.#reader.onClose;
        this.onPartialMessage = this.#reader.onPartialMessage;
    }

    readonly #reader: MessageReader;
    readonly #callback: (_: Message) => boolean;
    onError: Event<Error>;
    onClose: Event<void>;
    onPartialMessage: Event<PartialMessageInfo>;

    dispose() {
        this.#reader.dispose();
    }

    listen(callback: DataCallback): Disposable {
        return this.#reader.listen((data: Message) => {
            this.#callback(data) || callback(data);
        });
    }
}
