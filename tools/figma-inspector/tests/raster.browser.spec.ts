// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { server } from "vitest/browser";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import {
    mountPreview,
    readFixture,
    canvasPixels,
    decodePng,
} from "./browser-harness";
import { compareScreenshotPixels } from "./screenshot-compare";

for (const name of ["asymmetric", "stroke", "mask-shadow"])
    test(`${name} matches independent authored Figma pixels`, async () => {
        const p = await mountPreview();
        const density = p.win.devicePixelRatio;
        const stem = `fixtures/authored/raster/${name}-${density}x`;
        const source = JSON.parse(await readFixture(`${stem}.json`));
        const normalized = await normalizeSource(source);
        if (!normalized.ok || normalized.empty)
            throw Error("Expected authored capture");
        const result = convertSnapshot(normalized.snapshot);
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        p.send({ type: "preview-source", revision: 1, source: result.source });
        await p.ready(1);
        const actual = await canvasPixels(p);
        const expected = await decodePng(
            await server.commands.readFile(`${stem}.png`, "base64"),
        );
        const diff = compareScreenshotPixels(expected, actual, 2);
        expect(diff.differingPixels).toBe(0);
        // The independent reference must contain visible content, not approve blank output.
        expect(expected.data.some((v, i) => i % 4 !== 3 && v < 200)).toBe(true);
    });
