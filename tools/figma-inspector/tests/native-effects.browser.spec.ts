// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { server } from "vitest/browser";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import {
    mountCanvasPreview,
    readFixture,
    decodePng,
    expectCanvasPixels,
} from "./browser-harness";

test("8px native inner shadow and gradient match Figma at 1×", async () => {
    const source = JSON.parse(
        await readFixture("fixtures/source/native-inner-shadow.json"),
    );
    // Exported from Figma, independently of the Slint renderer.
    const expected = await decodePng(
        await server.commands.readFile(
            "fixtures/authored/raster/native-inner-shadow.png",
            "base64",
        ),
    );
    const preview = await mountCanvasPreview();
    expect(preview.win.devicePixelRatio).toBe(1);
    let revision = 0;
    for (const target of ["preview", "export"] as const) {
        const normalized = await normalizeSource(source, target);
        if (!normalized.ok || normalized.empty)
            throw Error(JSON.stringify(normalized));
        const converted = convertSnapshot(normalized.snapshot);
        if (!converted.ok) throw Error(JSON.stringify(converted));
        expect(converted.source).toContain("inner-shadow-blur: 2px;");
        preview.send({
            type: "preview-source",
            revision: ++revision,
            source: converted.source,
            exportPackage: { source: converted.source, files: [] },
        });
        await preview.ready(revision);
        await expectCanvasPixels(preview, (actual) => {
            expect([actual.width, actual.height]).toEqual([8, 8]);
            const meanError =
                actual.data.reduce(
                    (total, value, index) =>
                        total + Math.abs(value - expected.data[index]),
                    0,
                ) / actual.data.length;
            expect(meanError).toBeLessThan(3);
        });
    }
});
