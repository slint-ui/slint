// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import {
    type Clock,
    type TimingTrace,
    TimingTraceBuilder,
    deriveUnattributedOverhead,
    finalizeSandboxTiming,
    isCaptureMetrics,
    isTimingTrace,
    transportDuration,
} from "../src/performance/timing";
import { isPluginToUiMessage } from "../src/protocol";

test.each(["rendered", "unchanged", "cleared", "compilation-error"] as const)(
    "finalized %s traces count dispatch only once in the 180ms regression",
    (outcome) => {
        let now = 0;
        const builder = new TimingTraceBuilder(7, "selection-change", {
            epochNow: () => now,
            monotonicNow: () => now,
        });
        builder.measure("figmaCapture", () => {
            now = 100;
        });
        const wire = builder.toTrace();
        builder.measure("messageDispatch", () => {
            now = 120;
        });
        const sandbox = {
            ...builder.toTrace(),
            pluginSentAtEpochMs: wire.pluginSentAtEpochMs,
        };
        const ui: TimingTrace = {
            ...wire,
            uiReceivedAtEpochMs: 130,
            completedAtEpochMs: 180,
            totalUiMs: 50,
            totalMs: 180,
            outcome,
            phases: {
                ...wire.phases,
                messageTransport: transportDuration(wire, 130),
                previewQueue: 50,
            },
        };
        const original = structuredClone(ui);
        const finalized = finalizeSandboxTiming(ui, sandbox);
        expect(finalized.phases.messageDispatch).toBe(20);
        expect(finalized.phases.messageTransport).toBe(10);
        expect(finalized.phases.unattributedOverhead).toBe(0);
        expect(
            Object.values(finalized.phases).reduce(
                (sum: number, value) => sum + (value ?? 0),
                0,
            ),
        ).toBe(180);
        expect(finalized).toMatchObject({
            traceId: "7",
            revision: 7,
            trigger: "selection-change",
            outcome,
            totalMs: 180,
            totalUiMs: 50,
            totalPluginMs: 120,
            pluginSentAtEpochMs: 100,
        });
        expect(ui).toEqual(original);
        expect(isTimingTrace(finalized)).toBe(true);
    },
);

test("finalization preserves unattributed time and unavailable measurements", () => {
    const sandbox = new TimingTraceBuilder(1, "initial").toTrace();
    const ui = { ...sandbox, phases: { ...sandbox.phases }, totalMs: 40 };
    expect(finalizeSandboxTiming(ui, sandbox).phases).toMatchObject({
        messageDispatch: null,
        messageTransport: null,
        unattributedOverhead: 40,
    });
    ui.phases.messageTransport = 30;
    expect(finalizeSandboxTiming(ui, sandbox).phases).toMatchObject({
        messageDispatch: null,
        messageTransport: 30,
        unattributedOverhead: 10,
    });
    sandbox.phases.messageDispatch = 20;
    expect(finalizeSandboxTiming(ui, sandbox).phases).toMatchObject({
        messageDispatch: 20,
        messageTransport: 10,
        unattributedOverhead: 10,
    });
    ui.phases.messageTransport = 19.5;
    expect(finalizeSandboxTiming(ui, sandbox).phases.messageTransport).toBe(0);
});

test("records deterministic non-negative phase durations and a correlated trace", () => {
    let now = 100;
    const clock: Clock = {
        monotonicNow: () => now,
        epochNow: () => 1_000 + now,
    };
    const trace = new TimingTraceBuilder(7, "selection-change", clock);
    now += 4;
    trace.measure("figmaCapture", () => {
        now += 3;
    });
    now += 5;
    const result = trace.toTrace("rendered");
    expect(result.traceId).toBe("7");
    expect(result.revision).toBe(7);
    expect(result.phases.figmaCapture).toBe(3);
    expect(result.phases.jsonSerialization).toBeNull();
    expect(result.totalPluginMs).toBe(12);
    expect(transportDuration(result, result.pluginSentAtEpochMs + 8)).toBe(8);
});

test("validates display density changes as a timing trigger", () => {
    const trace = new TimingTraceBuilder(13, "density-change").toTrace();
    expect(isTimingTrace(trace)).toBe(true);
    expect(
        isPluginToUiMessage({
            type: "preview-source",
            revision: trace.revision,
            source: "",
            trace,
        }),
    ).toBe(true);
});

test("clamps clock skew and failed operations to valid durations", async () => {
    let now = 50;
    const clock: Clock = {
        monotonicNow: () => now,
        epochNow: () => 50,
    };
    const trace = new TimingTraceBuilder(2, "node-change", clock);
    expect(() =>
        trace.measure("slintConversion", () => {
            now -= 100;
            throw new Error("conversion failed");
        }),
    ).toThrow("conversion failed");
    const result = trace.toTrace("conversion-error");
    expect(result.phases.slintConversion).toBe(0);
    expect(result.totalPluginMs).toBe(0);
});

test("accepts and preserves validated font conversion metrics", () => {
    const builder = new TimingTraceBuilder(8, "node-change");
    builder.setCaptureMetrics({
        durationMs: 8.3,
        requests: 10,
        exports: 1,
        cacheHits: 9,
        work: {
            capturedNodes: 12,
            componentFamilies: 2,
            componentVariants: 3,
            pngExports: 4,
            svgExports: 1,
            textCacheHits: 9,
        },
    });
    const trace = builder.toTrace("rendered");
    expect(trace.captureMetrics).toEqual({
        durationMs: 8.3,
        requests: 10,
        exports: 1,
        cacheHits: 9,
        work: {
            capturedNodes: 12,
            componentFamilies: 2,
            componentVariants: 3,
            pngExports: 4,
            svgExports: 1,
            textCacheHits: 9,
        },
    });
    expect(isCaptureMetrics(trace.captureMetrics)).toBe(true);
    expect(isTimingTrace(trace)).toBe(true);
    expect(isTimingTrace({ ...trace, captureMetrics: undefined })).toBe(true);
    expect(
        isTimingTrace({
            ...trace,
            captureMetrics: {
                durationMs: 8.3,
                requests: 10,
                exports: 2,
                cacheHits: 9,
            },
        }),
    ).toBe(false);
});

test("rejects invalid capture metrics at the builder boundary", () => {
    const builder = new TimingTraceBuilder(9, "node-change");
    for (const metrics of [
        { durationMs: Number.NaN, requests: 0, exports: 0, cacheHits: 0 },
        { durationMs: -1, requests: 0, exports: 0, cacheHits: 0 },
        { durationMs: 1, requests: -1, exports: 0, cacheHits: 0 },
        { durationMs: 1, requests: 1.5, exports: 1, cacheHits: 0 },
        { durationMs: 1, requests: 2, exports: 1, cacheHits: 0 },
        {
            durationMs: 1,
            requests: 0,
            exports: 0,
            cacheHits: 0,
            work: {
                capturedNodes: -1,
                componentFamilies: 0,
                componentVariants: 0,
                pngExports: 0,
                svgExports: 0,
                textCacheHits: 0,
            },
        },
    ]) {
        expect(() => builder.setCaptureMetrics(metrics)).toThrow(
            "Invalid capture metrics",
        );
    }
});

test("aggregates successful and failed font-to-image conversions", async () => {
    let now = 100;
    const clock: Clock = {
        monotonicNow: () => now,
        epochNow: () => 1_000 + now,
    };
    const trace = new TimingTraceBuilder(9, "selection-change", clock);

    await trace.measureFontToImageConversion(async () => {
        now += 4;
        return "first";
    });
    await expect(
        trace.measureFontToImageConversion(async () => {
            now += 3;
            throw new Error("SVG export failed");
        }),
    ).rejects.toThrow("SVG export failed");

    const result = trace.toTrace("capture-error");
    expect(result.breakdowns.fontToImageConversion).toEqual({
        totalMs: 7,
        count: 2,
    });
    expect(Object.isFrozen(result.breakdowns)).toBe(true);
    expect(Object.isFrozen(result.breakdowns.fontToImageConversion)).toBe(true);
});

test("records absent conversions as null and clamps invalid conversion clocks", async () => {
    let now = 100;
    const clock: Clock = {
        monotonicNow: () => now,
        epochNow: () => 1_000 + now,
    };
    const absent = new TimingTraceBuilder(10, "node-change", clock).toTrace();
    expect(absent.breakdowns.fontToImageConversion).toBeNull();

    const trace = new TimingTraceBuilder(11, "node-change", clock);
    await trace.measureFontToImageConversion(async () => {
        now = 90;
    });
    expect(trace.toTrace().breakdowns.fontToImageConversion).toEqual({
        totalMs: 0,
        count: 1,
    });
});

test("strictly validates the required breakdown and excludes it from totals", () => {
    const trace = new TimingTraceBuilder(12, "initial").toTrace();
    expect(isTimingTrace(trace)).toBe(true);

    const missingBreakdown = { ...trace } as Record<string, unknown>;
    missingBreakdown.breakdowns = undefined;
    expect(isTimingTrace(missingBreakdown)).toBe(false);
    expect(
        isPluginToUiMessage({
            type: "preview-source",
            revision: trace.revision,
            source: "",
            trace: missingBreakdown,
        }),
    ).toBe(false);

    expect(
        isTimingTrace({
            ...trace,
            breakdowns: {
                fontToImageConversion: { totalMs: -1, count: 1 },
            },
        }),
    ).toBe(false);
    expect(deriveUnattributedOverhead(trace.phases, 10)).toBe(10);
});
