// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile, readdir } from "node:fs/promises";
import type { LanguageRegistration, ThemeRegistration } from "@shikijs/types";
import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine/oniguruma";
import OnigurumaEngine from "shiki/wasm";
import { expect, test } from "vitest";
import slintLanguage from "../../../docs/common/src/utils/slint.tmLanguage.json";
import { normalizeSource } from "../src/plugin/normalize";
import type { SourceBytes, SourceCapture } from "../src/plugin/source";
import { convertSnapshot } from "../src/preview/converter";
import darkTheme from "../src/ui/syntax-assets/dark-theme.json";
import lightTheme from "../src/ui/syntax-assets/light-theme.json";
import {
    highlightSlint,
    tokenizeSlint,
    type SourceTheme,
} from "../src/ui/slint-twinkle";

const shiki = createHighlighterCore({
    themes: [lightTheme as ThemeRegistration, darkTheme as ThemeRegistration],
    langs: [slintLanguage as LanguageRegistration],
    engine: createOnigurumaEngine(OnigurumaEngine),
});

const colors: Record<SourceTheme, Record<string, string>> = {
    "light-slint": {
        default: "#aaa",
        comment: "#6a9955",
        string: "#ce9178",
        escape: "#d7ba7d",
        number: "#ea0fac",
        keyword: "#191919",
        conditional: "#c586c0",
        property: "#191919",
        type: "#027be5",
        macro: "#dcdcaa",
    },
    "dark-slint": {
        default: "#a4d4d4",
        comment: "#6a9955",
        string: "#ce9178",
        escape: "#d7ba7d",
        number: "#fc9bdf",
        keyword: "white",
        conditional: "#c586c0",
        property: "white",
        type: "#7bc4f8",
        macro: "#dcdcaa",
    },
};

async function generatedSources(): Promise<{ name: string; source: string }[]> {
    const sources: { name: string; source: string }[] = [];
    for (const name of await readdir("fixtures/source")) {
        if (!name.endsWith(".json")) continue;
        const capture = JSON.parse(
            await readFile(`fixtures/source/${name}`, "utf8"),
        ) as SourceCapture<SourceBytes>;
        const normalized = await normalizeSource(capture);
        if (!normalized.ok || normalized.empty) continue;
        const converted = convertSnapshot(normalized.snapshot);
        if (converted.ok) sources.push({ name, source: converted.source });
    }
    return sources;
}

async function fixtureSources(): Promise<{ name: string; source: string }[]> {
    const names = (await readdir("fixtures")).filter((name) =>
        name.endsWith(".slint"),
    );
    return Promise.all(
        names.map(async (name) => ({
            name,
            source: await readFile(`fixtures/${name}`, "utf8"),
        })),
    );
}

async function shikiColors(
    source: string,
    theme: SourceTheme,
): Promise<string[]> {
    const result = (await shiki).codeToTokens(source, {
        lang: "slint",
        theme,
    });
    const mapped = new Array<string>(source.length).fill(colors[theme].default);
    for (const row of result.tokens) {
        for (const token of row) {
            expect(token.fontStyle).toBe(0);
            for (
                let offset = token.offset;
                offset < token.offset + token.content.length;
                offset++
            )
                mapped[offset] = (
                    token.color ?? colors[theme].default
                ).toLowerCase();
        }
    }
    return mapped;
}

function twinkleColors(source: string, theme: SourceTheme): string[] {
    const result = tokenizeSlint(source);
    const mapped = new Array<string>(source.length).fill(colors[theme].default);
    for (let i = 0; i < result.tokens.length; i += 3) {
        const type = result.token_types[result.tokens[i]];
        const color = colors[theme][type] ?? colors[theme].default;
        for (
            let offset = result.tokens[i + 1];
            offset < result.tokens[i + 2];
            offset++
        )
            mapped[offset] = color;
    }
    return mapped;
}

test("Shiki colors the generated Slint syntax categories", async () => {
    const source = await readFile("fixtures/highlight-coverage.slint", "utf8");
    const categories = [
        ["// Copyright", "comment"],
        ["export", "keyword"],
        ["PreviewState", "type"],
        ["enabled", "type"],
        ["accent", "property"],
        ["#3399FF", "number"],
        ["HighlightCoverage", "type"],
        ["Rectangle", "type"],
        ["clip", "property"],
        ["true", "keyword"],
        ["320px", "number"],
        ['"Hello', "string"],
        ["\\n", "escape"],
        ["@linear-gradient", "macro"],
        ["@image-url", "macro"],
        ["@markdown", "macro"],
        ["if root", "conditional"],
    ] as const;
    for (const theme of ["light-slint", "dark-slint"] as const) {
        const actual = await shikiColors(source, theme);
        for (const [sample, category] of categories) {
            const offset = source.indexOf(sample);
            expect(offset, sample).toBeGreaterThanOrEqual(0);
            expect(actual[offset], `${theme}: ${sample}`).toBe(
                colors[theme][category],
            );
        }
    }
});

test("Twinkleplop matches Shiki colors across Figma conversions", async () => {
    const sources = [
        ...(await fixtureSources()),
        ...(await generatedSources()),
    ];
    expect(sources.length).toBeGreaterThanOrEqual(40);
    for (const { name, source } of sources) {
        for (const theme of ["light-slint", "dark-slint"] as const) {
            const expected = await shikiColors(source, theme);
            const actual = twinkleColors(source, theme);
            const mismatches: string[] = [];
            for (let i = 0; i < source.length; i++) {
                if (/\s/u.test(source[i]) || actual[i] === expected[i])
                    continue;
                const line = source.slice(0, i).split("\n").length;
                mismatches.push(
                    `${line}: ${JSON.stringify(source.slice(i, i + 25))} ${expected[i]} != ${actual[i]}`,
                );
                if (mismatches.length === 5) break;
            }
            expect(mismatches, `${name} (${theme})`).toEqual([]);
        }
    }
});

test("highlighted source escapes markup before the plugin inserts it", () => {
    const html = highlightSlint(
        'Text { text: "<img src=x onerror=alert(1)>"; }',
        "light-slint",
    );
    expect(html).toContain("&lt;img src=x onerror=alert(1)&gt;");
    expect(html).not.toContain("<img");
});
