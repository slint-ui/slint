// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { hook } from "capture-console";

export function captureAsyncStderr() {
    const chunks: string[] = [];

    const streams = new Set<NodeJS.WritableStream>();
    streams.add(process.stderr);

    const consoleStderr = (globalThis.console as any)?._stderr;
    if (consoleStderr && consoleStderr !== process.stderr) {
        streams.add(consoleStderr);
    }

    const unhooks = Array.from(streams).map((stream) =>
        hook(stream, { quiet: true }, (chunk) => {
            chunks.push(chunk);
        }),
    );

    return {
        output() {
            return chunks.join("");
        },
        restore() {
            while (unhooks.length) {
                const unhook = unhooks.pop();
                unhook && unhook();
            }
        },
    };
}
