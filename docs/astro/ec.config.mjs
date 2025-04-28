// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { definePlugin } from "@expressive-code/core";
import { h } from "@expressive-code/core/hast";
import fs from "node:fs";
import { pluginLineNumbers } from "@expressive-code/plugin-line-numbers";

function sideBorder() {
    return definePlugin({
        name: "Adds side border to slint code blocks",
        baseStyles: `

        .sideBar {
            position: absolute;
            top: calc(var(--button-spacing) - 6px);
            bottom: 0;
            left: 0;
            width: 100px;
            border-left-width: 2px;
            border-left-style: solid;
            border-color: #2479f4;
            border-top-left-radius: 0.4rem;
            border-bottom-left-radius: 0.4rem;
            pointer-events: none;
        }
        `,
        hooks: {
            postprocessRenderedBlock: async (context) => {
                if (
                    context.renderData.blockAst.children[1].properties
                        .dataLanguage !== "slint"
                ) {
                    return;
                }
                const side = h("div.sideBar");

                const ast = context.renderData.blockAst;
                ast.children.push(side);

                context.renderData.blockAst = ast;
            },
        },
    });
}

function remapLanguageIdentifiers(lang) {
    switch (lang) {
        case "cpp": {
            return "C++";
        }
        case "sh": {
            return "bash";
        }
        default: {
            return lang;
        }
    }
}

function languageLabel() {
    return definePlugin({
        name: "Adds language label to code blocks",
        baseStyles: `
        .language-label {
            display: flex;
            align-items: center;
            justify-content: center;
            position: absolute;
            inset-block-start: calc(var(--ec-brdWd) + var(--button-spacing));
            inset-inline-end: calc(var(--ec-brdWd) + var(--ec-uiPadInl) );
            direction: ltr;
            font-size: 0.8rem;
            color:rgb(169, 169, 169);
            opacity: 1;
            transition: opacity 0.3s;
        }
        div.expressive-code:hover .language-label,
        .expressive-code:hover .language-label {
            opacity: 0;
        }
        `,
        hooks: {
            postprocessRenderedBlock: async (context) => {
                const language =
                    context.renderData.blockAst.children[1].properties
                        .dataLanguage;

                const label = h("div.language-label", {}, [
                    remapLanguageIdentifiers(language),
                ]);

                const ast = context.renderData.blockAst;
                ast.children.push(label);

                context.renderData.blockAst = ast;
            },
        },
    });
}

function workersPlaygroundButton() {
    return definePlugin({
        name: "Adds 'Run in SlintPad' button to slint codeblocks",
        baseStyles: `
        .run {
            display: flex;
            align-items: center;
            justify-content: center;
            position: absolute;
            inset-block-start: calc(var(--ec-brdWd) + var(--button-spacing));
            inset-inline-end: calc(var(--ec-brdWd) + var(--ec-uiPadInl) * 3);
            direction: ltr;
            unicode-bidi: isolate;

            background-color: color-mix(in srgb, var(--sl-color-accent) 50%, transparent);
            color: var(--sl-color-white);
            text-decoration: none;
            width: 2rem;
            height: 2rem;
            border-radius: 50%;
            opacity: 0;
            font-size: 0;
            transition: opacity 0.3s, background-color 0.3s;

            &:hover {
                background-color: color-mix(in srgb, var(--sl-color-accent) 90%, transparent);
            }

            &::before {
                content: '';
                display: inline-block;
                margin-left: 0.25rem;
                border-style: solid;
                border-width: 0.5rem 0 0.5rem 0.75rem;
                border-color: transparent transparent transparent white;
            }
        }
        div.expressive-code:hover .run,
            .expressive-code:hover .run {
                opacity: 1;
            }
        `,
        hooks: {
            postprocessRenderedBlock: async (context) => {
                if (!context.codeBlock.meta.includes("playground")) {
                    return;
                }

                const content = context.codeBlock.code;
                const url = `https://slintpad.com?snippet=${encodeURIComponent(content)}`;

                const runButton = h(
                    "a.run",
                    {
                        href: url,
                        target: "__blank",
                        title: "Open in SlintPad",
                    },
                    [],
                );

                const ast = context.renderData.blockAst;
                ast.children.push(runButton);

                context.renderData.blockAst = ast;
            },
        },
    });
}

export default {
    plugins: [
        workersPlaygroundButton(),
        sideBorder(),
        languageLabel(),
        pluginLineNumbers(),
    ],
    defaultProps: {
        showLineNumbers: false,
    },
    themes: ["dark-plus", "light-plus"],
    styleOverrides: {
        borderRadius: "0.4rem",
        borderColor: "var(--slint-code-background)",
        frames: { shadowColor: "transparent" },
        codeBackground: "var(--slint-code-background)",
    },
    shiki: {
        langs: [
            JSON.parse(
                fs.readFileSync(
                    "../../editors/vscode/slint.tmLanguage.json",
                    "utf-8",
                ),
            ),
        ],
    },
    frames: {
        extractFileNameFromCode: false,
    },
};
