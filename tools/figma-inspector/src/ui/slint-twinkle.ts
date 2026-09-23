// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import {
    ALNUM,
    DIGIT,
    HEX,
    LETTER,
    compile,
    create_language,
    enter,
    fallback,
    goto,
    keyword,
    leave,
    match,
    to_html,
    within,
    type TokenizeResult,
} from "@twinkleplop/core";

export type SourceTheme = "light-slint" | "dark-slint";

const macros = [
    "@tr",
    "@markdown",
    "@keys",
    "@linear-gradient",
    "@radial-gradient",
    "@conic-gradient",
    "@image-url",
];
const values = [
    match("//", "comment", enter("line-comment")),
    within("/*", "*/", "comment"),
    match('"', "string", enter("string")),
    match(macros, "macro", enter("macro")),
    match("#", "number", enter("hex")),
    match(["-", "+"], "sign"),
    match(DIGIT, "number", enter("number")),
    keyword(["true", "false"], {}, "keyword"),
    keyword(["root", "parent", "self"], {}, "keyword"),
];

const grammar = {
    name: "slint",
    states: {
        main: {
            rules: [
                ...values,
                keyword(["if"], {}, "conditional"),
                keyword(
                    [
                        "export",
                        "component",
                        "inherits",
                        "import",
                        "from",
                        "global",
                        "struct",
                        "enum",
                        "property",
                        "callback",
                        "pure",
                        "function",
                        "else",
                        "for",
                        "in-out",
                        "in",
                        "out",
                        "private",
                        "animate",
                        "states",
                        "transitions",
                    ],
                    {},
                    "keyword",
                ),
                match(":=", "punctuation"),
                match(":", "punctuation", goto("value")),
                match([LETTER, "_"], "plain", enter("identifier")),
                fallback(),
            ],
        },
        value: {
            rules: [
                ...values,
                match(";", "punctuation", goto("main")),
                match([LETTER, "_"], "plain", enter("identifier")),
                fallback(),
            ],
        },
        macro: {
            rules: [
                ...values,
                match(")", "macro", leave()),
                match([LETTER, "_"], "plain", enter("identifier")),
                fallback(),
            ],
        },
        string: {
            rules: [
                match("\\n", "escape"),
                match("\\\\", "escape"),
                match('"', "string", leave()),
                fallback({ token: "string" }),
            ],
        },
        "line-comment": {
            rules: [
                match("\n", "comment", leave()),
                fallback({ token: "comment" }),
            ],
        },
        number: {
            rules: [
                match([DIGIT, ".", "%", LETTER], "number"),
                fallback(leave()),
            ],
        },
        hex: { rules: [match(HEX, "number"), fallback(leave())] },
        identifier: {
            rules: [match([ALNUM, "_", "-"], "plain"), fallback(leave())],
        },
    },
};

const rawTokenize = create_language(compile(grammar))();

export function tokenizeSlint(source: string): TokenizeResult {
    const raw = rawTokenize(source);
    const tokenTypes = [...raw.token_types];
    const tokens = raw.tokens.slice();
    const plain = tokenTypes.indexOf("plain");
    const sign = tokenTypes.indexOf("sign");
    const number = tokenTypes.indexOf("number");
    const property = tokenTypes.push("property") - 1;
    const type = tokenTypes.push("type") - 1;

    for (let i = 0; i < tokens.length; i += 3) {
        const start = tokens[i + 1];
        const end = tokens[i + 2];
        if (tokens[i] === sign && /[0-9]/.test(source[end] ?? ""))
            tokens[i] = number;
        if (tokens[i] !== plain) continue;

        const before = source.slice(Math.max(0, start - 128), start);
        const after = source.slice(end, end + 16);
        const propertyBinding =
            /^\s*:/.test(after) &&
            !/^\s*:=/.test(after) &&
            !/:\s*\{[^}]*$/.test(before) &&
            !/\bif\s+[^;\n]*$/.test(before);
        const propertyDeclaration =
            /^\s*;/.test(after) && /\bproperty\s*<[^>]+>\s+$/.test(before);
        const elementName = /^\s*\{/.test(after);
        const declarationName =
            /\b(?:component|inherits|global|struct|enum)\s+$/.test(before);
        const propertyType = /<\s*$/.test(before) && /^\s*>/.test(after);
        const enumValue = /\benum\s+[\w-]+\s*\{[^}]*$/.test(before);
        if (propertyBinding || propertyDeclaration) tokens[i] = property;
        else if (elementName || declarationName || propertyType || enumValue)
            tokens[i] = type;
    }

    const split: number[] = [];
    for (let i = 0; i < tokens.length; i += 3) {
        const tokenType = tokens[i];
        const start = tokens[i + 1];
        const end = tokens[i + 2];
        const suffix =
            tokenType === plain ? /-\d+$/.exec(source.slice(start, end)) : null;
        if (suffix) {
            const cut = end - suffix[0].length;
            split.push(tokenType, start, cut, number, cut, end);
        } else split.push(tokenType, start, end);
    }

    return { ...raw, tokens: new Uint32Array(split), token_types: tokenTypes };
}

export function highlightSlint(source: string, theme: SourceTheme): string {
    const result = tokenizeSlint(source);
    const plain = result.token_types.indexOf("plain");
    const sign = result.token_types.indexOf("sign");
    const punctuation = result.token_types.indexOf("punctuation");
    const colored: number[] = [];
    for (let i = 0; i < result.tokens.length; i += 3) {
        if ([plain, sign, punctuation].includes(result.tokens[i])) continue;
        colored.push(
            result.tokens[i],
            result.tokens[i + 1],
            result.tokens[i + 2],
        );
    }
    return to_html(
        source,
        { ...result, tokens: new Uint32Array(colored) },
        { class_name: `twinkleplop ${theme}` },
    );
}
