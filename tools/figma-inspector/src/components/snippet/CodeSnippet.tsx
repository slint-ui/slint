// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
// cSpell: ignore shiki shikijs

import { useEffect, useState, type ReactNode } from "react";
import parse from "html-react-parser";
import darkSlint from "./dark-theme.json";
import lightSlint from "./light-theme.json";

// The default Shiki bundle is >9MB due to all the default themes and languages.
// The following setup if for a minimal bundle size of ~1MB.
import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine/oniguruma";
import type {
    LanguageRegistration,
    ThemeRegistration,
    HighlighterCore,
} from "@shikijs/types";
import OnigurumaEngine from "shiki/wasm";

import slintLang from "../../../../../editors/vscode/slint.tmLanguage.json";
import { getColorTheme, subscribeColorTheme } from "../../utils/bolt-utils.js";

let highlighter: HighlighterCore | null = null;
async function initHighlighter() {
    highlighter = await createHighlighterCore({
        themes: [
            darkSlint as ThemeRegistration,
            lightSlint as ThemeRegistration,
        ],
        langs: [slintLang as LanguageRegistration],
        engine: createOnigurumaEngine(OnigurumaEngine),
    });
}

export default function CodeSnippet({ code }: { code: string }) {
    const [highlightedCode, setHighlightedCode] = useState<ReactNode | null>(
        null,
    );
    const [lightOrDarkMode, setLightOrDarkMode] = useState(getColorTheme());
    useEffect(() => {
        subscribeColorTheme((mode) => {
            setLightOrDarkMode(mode);
        });
    }, []);

    useEffect(() => {
        let isMounted = true;

        const highlightCode = async () => {
            if (!highlighter) {
                await initHighlighter();
            }
            const html = highlighter!.codeToHtml(code, {
                lang: "slint",
                theme:
                    lightOrDarkMode === "dark" ? "dark-slint" : "light-slint",
            });

            if (isMounted) {
                setHighlightedCode(parse(html));
            }
        };

        highlightCode().catch(console.error);

        return () => {
            isMounted = false;
        };
    }, [code, lightOrDarkMode]);

    return (
        <div className="code-snippet" style={{ display: "flex" }}>
            {highlightedCode}
        </div>
    );
}
