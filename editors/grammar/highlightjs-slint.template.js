// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

(function () {
    function slint() {
        const IDENT = "[a-zA-Z_][a-zA-Z0-9_-]*";
        const KEYWORDS = {
            $pattern: new RegExp(IDENT),
            keyword: "@DECLARATIONS@|@KEYWORDS@|@CONTROLS@".split("|"),
            literal: "@BOOLEANS@".split("|"),
            "variable.language": "@ELEMENTS@".split("|"),
            type: "@TYPES@".split("|"),
        };
        const reserved = [
            ...KEYWORDS.keyword,
            ...KEYWORDS.literal,
            ...KEYWORDS["variable.language"],
        ];
        const NON_KEYWORD = `(?<![\\w-])(?!(${reserved.join("|")})(?![\\w-]))${IDENT}`;
        const CONTAINS = [];
        const STRING = {
            scope: "string",
            begin: /"/,
            end: /"/,
            contains: [
                { scope: "char.escape", match: /\\(n|\\|"|u\{[0-9a-fA-F]+\})/ },
                {
                    scope: "subst",
                    begin: /\\\{/,
                    end: /\}/,
                    keywords: KEYWORDS,
                    contains: CONTAINS,
                },
            ],
        };
        CONTAINS.push(
            { scope: "comment", begin: /\/\//, end: /$/ },
            {
                scope: "comment",
                begin: /\/\*/,
                end: /\*\//,
                contains: ["self"],
            },
            STRING,
            { scope: "built_in", match: /@[a-z][a-z-]*/ },
            { scope: "literal", match: /\bColors\.(@COLORS@)\b/ },
            { scope: "number", match: /#[0-9a-fA-F]{3,8}\b/ },
            {
                scope: "number",
                match: /(?<![a-zA-Z_][\w-]*)[+-]?\b\d+(\.\d*)?(@UNITS@)?/,
            },
            {
                match: [
                    /\b(?:component|struct|enum|global)\s+/,
                    new RegExp(IDENT),
                ],
                scope: { 2: "title.class" },
            },
            {
                match: [/\binherits\s+/, new RegExp(IDENT)],
                scope: { 2: "title.class.inherited" },
            },
            {
                scope: "title.function",
                match: new RegExp(`${NON_KEYWORD}(?=[ \\t]*(\\(|=>))`),
            },
            { scope: "type", match: /\b[A-Z][a-zA-Z0-9_-]*(?=[ \t]*\{)/ },
            {
                scope: "property",
                match: new RegExp(
                    `(?<=(^|[{;,(>])[ \\t]*)${NON_KEYWORD}(?=[ \\t]*:(?!=))`,
                ),
            },
        );
        return {
            name: "Slint",
            // "60" and "sixtyfps" are used by old blog posts.
            aliases: ["60", "sixtyfps"],
            disableAutodetect: true,
            keywords: KEYWORDS,
            contains: CONTAINS,
        };
    }

    if (typeof module === "object" && module.exports) {
        module.exports = slint;
    } else if (typeof hljs === "object") {
        hljs.registerLanguage("slint", slint);
    }
})();
