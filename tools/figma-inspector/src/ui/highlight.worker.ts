// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { LanguageRegistration, ThemeRegistration } from "@shikijs/types";
import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine/oniguruma";
import OnigurumaEngine from "shiki/wasm";
import darkSlint from "./syntax-assets/dark-theme.json";
import lightSlint from "./syntax-assets/light-theme.json";
import slintLang from "./syntax-assets/slint.tmLanguage.json";

const highlighter = createHighlighterCore({
    themes: [darkSlint as ThemeRegistration, lightSlint as ThemeRegistration],
    langs: [slintLang as LanguageRegistration],
    engine: createOnigurumaEngine(OnigurumaEngine),
});
// Install the rejection handler immediately, including before the first request.
void highlighter.catch(() => undefined);
self.onmessage = async (event: MessageEvent<unknown>) => {
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
        const html = (await highlighter).codeToHtml(request.source, {
            lang: "slint",
            theme: String(request.theme),
        });
        self.postMessage({ id: request.id, html });
    } catch {
        self.postMessage({ id: request.id, error: true });
    }
};
