import { useEffect, useRef, useState, type ReactNode } from 'react';
import parse from 'html-react-parser';
import nord from '@shikijs/themes/nord'

// The default Shiki bundle is >9MB due to all the default themes and languages.
// The following setup if for a minimal bundle size of ~1MB.
import { createHighlighterCore } from 'shiki/core'
import { createOnigurumaEngine } from 'shiki/engine/oniguruma'
import type { LanguageRegistration } from '@shikijs/types'
import OnigurumaEngine from 'shiki/wasm';

import slintLang from "./Slint-tmLanguage.json";


let highlighter: any;
const initHighlighter = async () => {
    highlighter = await createHighlighterCore({
        themes: [
            nord
        ],
        langs: [slintLang as LanguageRegistration],
        engine: createOnigurumaEngine(OnigurumaEngine)
    });
};


export default function CodeSnippet ({code}: {code: string}) {
    const [highlightedCode, setHighlightedCode] = useState<ReactNode | null>(null);

    useEffect(() => {
        let isMounted = true;

        const highlightCode = async () => {
            await initHighlighter();
            const html = await highlighter.codeToHtml(code, {
                lang: "slint",
                theme: "nord",
            });

            if (isMounted) {
                setHighlightedCode(parse(html));
            }
        };


        highlightCode().catch(console.error);


        return () => {
            isMounted = false;
        };
    }, [code]);

    return <div className="content">{highlightedCode}</div>;
};