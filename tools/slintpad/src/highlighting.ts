// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore abfnrtv

import type * as monaco from "monaco-editor";
import words from "../../../editors/grammar/words.json";

export const slint_language = <monaco.languages.IMonarchLanguage>{
    defaultToken: "invalid",

    root_keywords: words.declarations,
    inner_keywords: [...words.keywords, ...words.controls],
    lang_keywords: [...words.elements, ...words.booleans, ...words.controls],
    type_keywords: words.types,
    escapes:
        /\\(?:[abfnrtv\\"']|x[0-9A-Fa-f]{1,4}|u[0-9A-Fa-f]{4}|U[0-9A-Fa-f]{8})/,

    symbols: /[#!%&*+\-./:<=>@^|_?,()]+|;/,

    tokenizer: {
        root: [
            [
                /[a-zA-Z_][a-zA-Z0-9_]*/,
                {
                    cases: {
                        "@root_keywords": { token: "keyword" },
                        "@default": "identifier",
                    },
                },
            ],
            { include: "@whitespace" },
            { include: "@numbers" },
            [/"/, "string", "@string"],
            [/\{/, "", "@inner"],
            [/@symbols/, ""],
        ],
        inner: [
            [/[a-zA-Z_][a-zA-Z0-9_-]*\s*:=/, "variable.parameter"],
            [
                /[a-zA-Z_][a-zA-Z0-9_-]*\s*:\s*\{/,
                "variable.parameter",
                "@binding_1",
            ],
            [/[a-zA-Z_][a-zA-Z0-9_-]*\s*:/, "variable.parameter", "@binding_0"],
            [
                /[a-zA-Z_][a-zA-Z0-9_-]*/,
                {
                    cases: {
                        "@inner_keywords": { token: "keyword" },
                        "@default": "identifier",
                    },
                },
            ],
            { include: "@whitespace" },
            { include: "@numbers" },
            [/"/, "string", "@string"],
            [/\{/, "", "@push"],
            [/\}/, "", "@pop"],
            [/:=/, ""],
            [/<=>/, "", "@binding_0"],
            [/=>\s*{/, "", "@binding_1"],
            [/</, "", "@type"],
            [/\[/, "", "binding_1"],
            [/@symbols/, ""],
        ],

        type: [
            [
                /[a-zA-Z_][a-zA-Z0-9_-]*/,
                {
                    cases: {
                        "@type_keywords": { token: "keyword.type" },
                        "@default": "identifier",
                    },
                },
            ],
            { include: "@whitespace" },
            { include: "@numbers" },
            [/"/, "string", "@string"],
            [/</, "", "@push"],
            [/>/, "", "@pop"],
            [/@symbols/, ""],
        ],

        binding_0: [
            { include: "@whitespace" },
            [/\{/, "", "@binding_1"],
            [/;/, "", "@pop"],
            [/\}/, { token: "", next: "@pop", goBack: 1 }],
            [
                /[a-zA-Z_][a-zA-Z0-9_]*/,
                {
                    cases: {
                        "@lang_keywords": { token: "keyword.type" },
                        "@default": "identifier",
                    },
                },
            ],
            { include: "@numbers" },
            [/"/, "string", "@string"],
            [/@symbols/, ""],
        ],

        // inside a '{'
        binding_1: [
            [
                /[a-zA-Z_][a-zA-Z0-9_]*/,
                {
                    cases: {
                        "@lang_keywords": { token: "keyword.type" },
                        "@default": "identifier",
                    },
                },
            ],
            { include: "@whitespace" },
            { include: "@numbers" },
            [/"/, "string", "@string"],
            [/\{/, "", "@push"],
            [/\}/, "", "@pop"],
            [/\[/, "", "@push"],
            [/\]/, "", "@pop"],
            [/@symbols/, ""],
        ],

        whitespace: [
            [/[ \t\r\n]+/, "white"],
            [/\/\*/, "comment", "@comment"],
            [/\/\/.*$/, "comment"],
        ],
        string: [
            [/[^\\"]+/, "string"],
            [/@escapes/, "string.escape"],
            [/\\./, "string.escape.invalid"],
            [/"/, "string", "@pop"],
        ],
        comment: [
            [/[^/*]+/, "comment"],
            [/\/\*/, "comment", "@push"],
            ["\\*/", "comment", "@pop"],
            [/[/*]/, "comment"],
        ],

        numbers: [
            [/\d+(\.\d+)?\w*/, { token: "number" }],
            [/#[0-9a-fA-F]+/, { token: "number" }],
        ],
    },
};
