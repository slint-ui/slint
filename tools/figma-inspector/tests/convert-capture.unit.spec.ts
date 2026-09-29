// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { expect, test } from "vitest";
import {
    convertCapture,
    captureSnapshotJson,
    type CaptureConversionState,
} from "../src/preview/convert-capture";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import { TimingTraceBuilder } from "../src/performance/timing";
import { generateExport } from "../src/export/generate";
import { previewAssetSource } from "./preview-asset-source";

test("worker pipeline preserves source, snapshots, warnings and correlated timings", async () => {
    for (const file of [
        "basic",
        "png-first",
        "image-size-unavailable",
        "multiple-failures",
    ]) {
        const captureJson = await readFile(
            `fixtures/source/${file}.json`,
            "utf8",
        );
        const normalized = await normalizeSource(JSON.parse(captureJson));
        const trace = new TimingTraceBuilder(12, "node-change").toTrace();
        const state: CaptureConversionState = {};
        const result = await convertCapture(
            {
                type: "preview-capture",
                revision: 12,
                captureJson,
                trace,
            },
            state,
        );
        expect(result.revision).toBe(12);
        expect(result.trace?.traceId).toBe("12");
        if (!normalized.ok) {
            expect(result).toMatchObject({
                type: "preview-diagnostics",
                diagnostics: normalized.diagnostics,
            });
            continue;
        }
        if (normalized.empty) {
            expect(result.type).toBe("preview-clear");
            continue;
        }
        const expected = convertSnapshot(normalized.snapshot, {
            specialize: true,
        });
        expect(expected.ok).toBe(true);
        if (!expected.ok || result.type !== "preview-source")
            throw Error("Missing converted output");
        const decoded =
            typeof result.source === "string"
                ? result
                : { source: previewAssetSource(result.source) };
        expect(decoded.source).toBe(expected.source);
        expect(result.trace?.phases.jsonSerialization).toBeNull();
        expect(captureSnapshotJson(state)).toBe(
            JSON.stringify(normalized.snapshot),
        );
        expect(result.warnings).toEqual([
            ...normalized.warnings,
            ...expected.warnings,
        ]);
        expect(result.trace?.phases.normalization).toBeGreaterThanOrEqual(0);
        expect(result.trace?.phases.slintConversion).toBeGreaterThanOrEqual(0);
    }
});

test("worker pipeline reports invalid captured JSON as a recoverable conversion error", async () => {
    for (const captureJson of ["{", "null", '{"sourceVersion":99}'])
        expect(
            await convertCapture({
                type: "preview-capture",
                revision: 3,
                captureJson,
            }),
        ).toMatchObject({
            type: "preview-diagnostics",
            revision: 3,
            trace: { outcome: "conversion-error" },
        });
});

test("a failing unselected native variant rejects an otherwise valid specialized preview", async () => {
    const capture = JSON.parse(
        await readFile("fixtures/source/component-tokens.json", "utf8"),
    );
    capture.root.children = capture.root.children.filter(
        (node: { id: string }) => node.id !== "hug:Pressed",
    );
    capture.variables.variables.pressed.values.light = {
        type: "VARIABLE_ALIAS",
        id: "pressed",
    };
    const normalized = await normalizeSource(capture);
    if (!normalized.ok || normalized.empty) throw Error("Normalization failed");
    expect(convertSnapshot(normalized.snapshot, { specialize: true }).ok).toBe(
        true,
    );
    const result = await convertCapture({
        type: "preview-capture",
        revision: 6,
        captureJson: JSON.stringify(capture),
    });
    expect(result).toMatchObject({
        type: "preview-diagnostics",
        trace: { outcome: "conversion-error" },
        diagnostics: [
            expect.objectContaining({
                message: expect.stringContaining("Cyclic variable alias"),
            }),
        ],
    });
    expect(result).not.toHaveProperty("exportPackage");
});

test.each([
    "component-variants",
    "conditional-root",
    "conditional-inner-row",
    "component-raster-icon",
    "component-tokens",
])(
    "%s exposes reusable export warnings with the specialized preview",
    async (file) => {
        const captureJson = await readFile(
            `fixtures/source/${file}.json`,
            "utf8",
        );
        const native = await normalizeSource(JSON.parse(captureJson), "export");
        if (!native.ok || native.empty) throw Error(JSON.stringify(native));
        const expected = generateExport(native.snapshot, native.warnings);
        const result = await convertCapture({
            type: "preview-capture",
            revision: 1,
            captureJson,
        });
        if (result.type !== "preview-source")
            throw Error(JSON.stringify(result));
        expect(result.exportPackage).toEqual(expected.exportPackage);
        expect(result.warnings).toEqual(
            expect.arrayContaining([...expected.warnings]),
        );
        expect(
            new Set(result.warnings?.map((warning) => JSON.stringify(warning)))
                .size,
        ).toBe(result.warnings?.length);
    },
);
