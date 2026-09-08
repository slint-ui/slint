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
import { unpackPreviewAssets } from "../src/asset-transport";
import { TimingTraceBuilder } from "../src/performance/timing";

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
            undefined,
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
        const expected = convertSnapshot(normalized.snapshot);
        expect(expected.ok).toBe(true);
        if (!expected.ok || result.type !== "preview-source")
            throw Error("Missing converted output");
        const decoded =
            typeof result.source === "string"
                ? result
                : unpackPreviewAssets(result.source);
        expect(decoded.source).toBe(expected.source);
        expect(decoded.snapshotJson ?? "").toBe("");
        expect(state.snapshotJson).toBeUndefined();
        expect(result.trace?.phases.jsonSerialization).toBeNull();
        expect(captureSnapshotJson(state)).toBe(
            JSON.stringify(normalized.snapshot),
        );
        expect(captureSnapshotJson(state)).toBe(state.snapshotJson);
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
