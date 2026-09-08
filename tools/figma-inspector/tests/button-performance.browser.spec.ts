// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { page, server } from "vitest/browser";
import { expect, test } from "vitest";
import { buttonFamily } from "./button-family";
import { mountPreview, readFixture, canvasPixels } from "./browser-harness";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import type { SourceCapture } from "../src/plugin/source";

test("local 144-variant family preserves pixels and measures both emission modes", async () => {
    let referencePixels: Awaited<ReturnType<typeof canvasPixels>> | undefined;
    for (const specialize of [false, true]) {
        const capture = buttonFamily(
            JSON.parse(
                await readFixture("fixtures/source/conditional-root.json"),
            ) as SourceCapture,
        );
        const start = performance.now();
        const normalized = await normalizeSource(capture);
        if (!normalized.ok || normalized.empty)
            throw Error("Invalid benchmark family");
        const normalizedAt = performance.now();
        const converted = convertSnapshot(normalized.snapshot, { specialize });
        if (!converted.ok) throw Error(JSON.stringify(converted.diagnostics));
        const generatedAt = performance.now();
        const p = await mountPreview();
        const samples: number[] = [];
        for (let revision = 1; revision <= (specialize ? 21 : 4); revision++) {
            const started = performance.now();
            p.send({
                type: "preview-source",
                revision,
                source: `${converted.source}\n// replacement ${revision}`,
            });
            await p.ready(revision);
            samples.push(performance.now() - started);
        }
        const pixels = await canvasPixels(p);
        expect(pixels.data.some((value, i) => i % 4 === 2 && value < 240)).toBe(
            true,
        );
        if (referencePixels) {
            expect(pixels.width).toBe(referencePixels.width);
            expect(pixels.height).toBe(referencePixels.height);
            let differences = 0;
            for (let i = 0; i < pixels.data.length; i++)
                if (pixels.data[i] !== referencePixels.data[i]) differences++;
            expect(differences).toBe(0);
        } else referencePixels = pixels;
        await server.commands.writeFile(
            `test-results/button-family-${specialize}.png`,
            await page.elementLocator(p.iframe).screenshot({ save: false }),
            "base64",
        );
        await server.commands.writeFile(
            `test-results/button-benchmark-${specialize}.json`,
            JSON.stringify({
                normalizationMs: normalizedAt - start,
                generationMs: generatedAt - normalizedAt,
                sourceBytes: converted.source.length,
                conditions: (converted.source.match(/\bif\b/g) ?? []).length,
                coldMs: samples[0],
                warmMs: samples.slice(1),
                warmP95Ms: [...samples.slice(1)].sort((a, b) => a - b)[
                    Math.ceil((samples.length - 1) * 0.95) - 1
                ],
                traces: p.messages.filter(
                    (message) => message.type === "preview-complete",
                ),
            }),
        );
    }
}, 120000);
