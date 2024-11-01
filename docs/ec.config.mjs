import { ExpressiveCodeTheme } from "@astrojs/starlight/expressive-code";
import { definePlugin } from "@expressive-code/core";
import { h } from "@expressive-code/core/hast";
import fs from "node:fs";

const lightJsoncString = fs.readFileSync(
    new URL(`./src/misc/light-theme.jsonc`, import.meta.url),
    "utf-8",
);
const lightTheme = ExpressiveCodeTheme.fromJSONString(lightJsoncString);

const darkJsoncString = fs.readFileSync(
    new URL(`./src/misc/dark-theme.jsonc`, import.meta.url),
    "utf-8",
);
const darkTheme = ExpressiveCodeTheme.fromJSONString(darkJsoncString);


function workersPlaygroundButton() {
    return definePlugin({
        name: "Adds 'Run in Slintpad' button to slint codeblocks",
        baseStyles: `
        .run {
            display: flex;
            gap: 0.25rem;
            flex-direction: row;
            position: absolute;
            inset-block-start: calc(var(--ec-brdWd) + var(--button-spacing));
            inset-inline-end: calc(var(--ec-brdWd) + var(--ec-uiPadInl) * 3);
            direction: ltr;
            unicode-bidi: isolate;

            text-decoration-color: var(--sl-color-accent);
            span {
                color: var(--sl-color-white);
                font-family: var(--sl-font-system);
            }
        }
        `,
        hooks: {
            postprocessRenderedBlock: async (context) => {
                if (!context.codeBlock.meta.includes("playground")) return;

                const content = context.codeBlock.code;
                const url = `https://slintpad.com?snippet=${encodeURIComponent(content)}`;

                const runButton = h("a.run", { href: url, target: "__blank" }, [
                    h("span", "Run in Slintpad"),
                ]);

                const ast = context.renderData.blockAst;
                ast.children.push(runButton);

                context.renderData.blockAst = ast;
            },
        },
    });
}

export default {
    plugins: [workersPlaygroundButton()],
    themes: [darkTheme, lightTheme],
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
