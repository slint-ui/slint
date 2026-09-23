// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { expect, test } from "vitest";
import expectedColors from "../fixtures/highlight-colors.json";
import { normalizeSource } from "../src/plugin/normalize";
import type { SourceBytes, SourceCapture } from "../src/plugin/source";
import { convertSnapshot } from "../src/preview/converter";
import {
    highlightSlint,
    tokenizeSlint,
    type SourceTheme,
} from "../src/ui/slint-twinkle";

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

function sha256(value: string): string {
    return createHash("sha256").update(value).digest("hex");
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

function visibleColorHash(source: string, mapped: string[]): string {
    return sha256(
        mapped.filter((_, index) => !/\s/u.test(source[index])).join("\n"),
    );
}

test("generated Slint syntax categories keep their expected colors", async () => {
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
        const actual = twinkleColors(source, theme);
        for (const [sample, category] of categories) {
            const offset = source.indexOf(sample);
            expect(offset, sample).toBeGreaterThanOrEqual(0);
            expect(actual[offset], `${theme}: ${sample}`).toBe(
                colors[theme][category],
            );
        }
    }
});

test("Twinkleplop keeps the frozen colors across Figma conversions", async () => {
    const sources = [
        ...(await fixtureSources()),
        ...(await generatedSources()),
    ].sort((a, b) => a.name.localeCompare(b.name));
    expect(sources.map(({ name }) => name)).toEqual(
        expectedColors.map(({ name }) => name),
    );
    for (const [index, { name, source }] of sources.entries()) {
        const expected = expectedColors[index];
        expect(sha256(source), name).toBe(expected.sourceSha256);
        for (const [theme, hash] of [
            ["light-slint", expected.lightSha256],
            ["dark-slint", expected.darkSha256],
        ] as const) {
            expect(
                visibleColorHash(source, twinkleColors(source, theme)),
                `${name} (${theme})`,
            ).toBe(hash);
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
