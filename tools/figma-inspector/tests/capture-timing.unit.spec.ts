// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import {
    CaptureTimingCollector,
    isCaptureTimings,
} from "../src/performance/capture-timing";
import { isCaptureMetrics } from "../src/performance/timing";

test("capture timings distinguish summed exports, active time, and overlapping categories", () => {
    let now = 0;
    const collector = new CaptureTimingCollector(() => now);
    const first = collector.start("pngExports");
    now = 3;
    const second = collector.start("pngExports");
    now = 4;
    const svg = collector.start("svgExports");
    now = 5;
    first();
    now = 7;
    second();
    now = 9;
    svg();
    now = 12;
    const timing = collector.snapshot();
    expect(timing).toMatchObject({
        durationMs: 12,
        trackedMs: 9,
        operations: {
            pngExports: { count: 2, totalMs: 9, busyMs: 7 },
            svgExports: { count: 1, totalMs: 5, busyMs: 5 },
        },
    });
    expect(isCaptureTimings(timing)).toBe(true);
    expect(
        isCaptureMetrics({
            durationMs: 0,
            requests: 0,
            exports: 0,
            cacheHits: 0,
            timings: timing,
        }),
    ).toBe(true);
    expect(isCaptureTimings({ ...timing, trackedMs: 13 })).toBe(false);
    expect(
        isCaptureTimings({
            ...timing,
            operations: {
                ...timing.operations,
                pngExports: { count: 2, busyMs: 10, totalMs: Number.NaN },
            },
        }),
    ).toBe(false);
});

test("failed reads and async exports close timing intervals and preserve snapshots", async () => {
    let now = 0;
    const collector = new CaptureTimingCollector(() => now);
    expect(() =>
        collector.measure("nodeReads", () => {
            now = 2;
            throw Error("read");
        }),
    ).toThrow("read");
    await expect(
        collector.measureAsync("bulkJson", async () => {
            now = 7;
            throw Error("export");
        }),
    ).rejects.toThrow("export");
    const first = collector.snapshot();
    now = 10;
    collector.measure("componentDiscovery", () => {
        now = 13;
    });
    expect(first).toMatchObject({ durationMs: 7, trackedMs: 7 });
    expect(collector.snapshot()).toMatchObject({
        durationMs: 13,
        trackedMs: 10,
        operations: {
            nodeReads: { count: 1, totalMs: 2, busyMs: 2 },
            bulkJson: { count: 1, totalMs: 5, busyMs: 5 },
        },
    });
});

test("async intervals report synchronous overlap without counting nested reads twice", () => {
    let now = 0;
    const collector = new CaptureTimingCollector(() => now);
    const finish = collector.start("componentLookup");
    now = 2;
    collector.measure("metadata", () => {
        now = 3;
        collector.measure("nodeReads", () => {
            now = 5;
        });
        collector.measure("encoding", () => {
            now = 8;
        });
        now = 9;
    });
    now = 12;
    finish();
    const timing = collector.snapshot();
    expect(timing.operations.componentLookup).toEqual({
        count: 1,
        totalMs: 12,
        busyMs: 12,
        synchronousOverlapMs: 7,
    });
    expect(timing.operations.nodeReads.totalMs).toBe(2);
    expect(timing.operations.encoding.totalMs).toBe(3);
    expect(isCaptureTimings(timing)).toBe(true);
    expect(
        isCaptureTimings({
            ...timing,
            operations: {
                ...timing.operations,
                componentLookup: {
                    ...timing.operations.componentLookup,
                    synchronousOverlapMs: 13,
                },
            },
        }),
    ).toBe(false);
});

test("live property counters preserve type boundaries and are immutable snapshots", () => {
    const collector = new CaptureTimingCollector(() => 0);
    collector.recordPropertyReads("TEXT", ["characters", "fontName"]);
    const first = collector.snapshot();
    collector.recordPropertyReads("TEXT", ["characters"]);
    collector.recordPropertyReads("VECTOR", ["cornerRadius"]);
    expect(first.propertyReads).toEqual({
        TEXT: { characters: 1, fontName: 1 },
    });
    const latest = collector.snapshot();
    expect(latest.propertyReads).toEqual({
        TEXT: { characters: 2, fontName: 1 },
        VECTOR: { cornerRadius: 1 },
    });
    expect(isCaptureTimings(latest)).toBe(true);
    expect(
        isCaptureTimings({
            ...latest,
            propertyReads: { TEXT: { characters: -1 } },
        }),
    ).toBe(false);
});
