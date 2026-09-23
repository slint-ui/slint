// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { highlightSlint, type SourceTheme } from "./slint-twinkle";

self.onmessage = (event: MessageEvent<unknown>) => {
    const request = event.data;
    if (
        !request ||
        typeof request !== "object" ||
        !("id" in request) ||
        !Number.isSafeInteger(request.id) ||
        !("source" in request) ||
        typeof request.source !== "string" ||
        !("theme" in request) ||
        !["light-slint", "dark-slint"].includes(String(request.theme))
    )
        return;
    try {
        const html = highlightSlint(
            request.source,
            String(request.theme) as SourceTheme,
        );
        self.postMessage({ id: request.id, html });
    } catch {
        self.postMessage({ id: request.id, error: true });
    }
};
