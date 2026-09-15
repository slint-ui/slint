// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { buttonFamily } from "./button-family";
import { mountPreview, readFixture, canvasPixels } from "./browser-harness";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import type { SourceCapture } from "../src/plugin/source";

test("local 144-variant family preserves pixels in both emission modes", async () => {
    let referencePixels: Awaited<ReturnType<typeof canvasPixels>> | undefined;
    for (const specialize of [false, true]) {
        const capture = buttonFamily(
            JSON.parse(
                await readFixture("fixtures/source/conditional-root.json"),
            ) as SourceCapture,
        );
        const normalized = await normalizeSource(capture);
        if (!normalized.ok || normalized.empty)
            throw Error("Invalid button family");
        const converted = convertSnapshot(normalized.snapshot, { specialize });
        if (!converted.ok) throw Error(JSON.stringify(converted.diagnostics));
        const p = await mountPreview();
        p.send({
            type: "preview-source",
            revision: 1,
            source: converted.source,
            exportPackage: { source: converted.source, files: [] },
        });
        await p.ready(1);
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
    }
}, 120000);
