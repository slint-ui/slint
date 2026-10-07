// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine/oniguruma";
import wasm from "shiki/wasm";
import darkTheme from "../../../tools/figma-inspector/src/ui/syntax-assets/dark-theme.json";
import lightTheme from "../../../tools/figma-inspector/src/ui/syntax-assets/light-theme.json";
import language from "../../../docs/common/src/utils/slint.tmLanguage.json";

let highlighter;
export async function highlight(source, theme) {
  highlighter ??= createHighlighterCore({
    themes: [darkTheme, lightTheme],
    langs: [language],
    engine: createOnigurumaEngine(wasm),
  });
  return (await highlighter).codeToHtml(source, { lang: "slint", theme });
}
