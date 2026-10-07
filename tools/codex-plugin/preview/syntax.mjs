import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine/oniguruma";
import wasm from "shiki/wasm";
import darkTheme from "../../figma-inspector/src/ui/syntax-assets/dark-theme.json";
import lightTheme from "../../figma-inspector/src/ui/syntax-assets/light-theme.json";
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
