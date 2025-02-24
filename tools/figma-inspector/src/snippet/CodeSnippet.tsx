// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { useEffect, useRef, useState, type ReactNode } from "react";
import parse from "html-react-parser";
import darkSlint from "./dark-theme.json";
import lightSlint from "./light-theme.json";

// The default Shiki bundle is >9MB due to all the default themes and languages.
// The following setup if for a minimal bundle size of ~1MB.
import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine/oniguruma";
import type { LanguageRegistration, ThemeRegistration } from "@shikijs/types";
import OnigurumaEngine from "shiki/wasm";

import slintLang from "./Slint-tmLanguage.json";

let highlighter: any;
const initHighlighter = async () => {
    highlighter = await createHighlighterCore({
        themes: [
            darkSlint as ThemeRegistration,
            lightSlint as ThemeRegistration,
        ],
        langs: [slintLang as LanguageRegistration],
        engine: createOnigurumaEngine(OnigurumaEngine),
    });
};

type HightlightTheme = "dark-slint" | "light-slint";

export default function CodeSnippet({
    code,
    theme,
}: { code: string; theme: HightlightTheme }) {
    const [highlightedCode, setHighlightedCode] = useState<ReactNode | null>(
        null,
    );

    useEffect(() => {
        let isMounted = true;

        const highlightCode = async () => {
            await initHighlighter();
            const html = await highlighter.codeToHtml(code, {
                lang: "slint",
                theme: theme,
            });
            console.log("ell0", html);

            if (isMounted) {
                setHighlightedCode(parse(html));
            }
        };

        highlightCode().catch(console.error);

        return () => {
            isMounted = false;
        };
    }, [code, theme]);

    return (
        <div className="content" style={{ display: "flex" }}>
            {highlightedCode}
        </div>
    );
}
