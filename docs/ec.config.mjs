// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { definePlugin } from "@expressive-code/core";
import { h } from "@expressive-code/core/hast";
import fs from "node:fs";

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
    plugins: [workersPlaygroundButton()],
    themes: ["dark-plus", "light-plus"],
    styleOverrides: { borderRadius: "0.2rem" },
    shiki: {
        langs: [
            JSON.parse(
                fs.readFileSync("src/misc/Slint-tmLanguage.json", "utf-8"),
            ),
        ],
    },
    frames: {
        extractFileNameFromCode: false,
    },
};
